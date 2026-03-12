use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};
use tokio::sync::{Mutex, oneshot};
use tokio::task::{LocalSet, spawn_local};

use crate::app::service::GameService;
use crate::domain::commands::GameCommand;
use crate::domain::events::CommandResult;
use crate::domain::time::TimeDelta;
use crate::llm::LlmBackend;
use crate::presenter::{
    build_world_text, build_world_title, format_duration, render_event_notice,
    render_interactable_label, render_route_label,
};
use crate::simulation::{
    InteractableOption, InteractionTarget, InteractionVerb, RouteView, UiMode, UiSnapshot,
};

const SPINNER_FRAMES: &[&str] = &["|", "/", "-", "\\"];
const NOTICE_HISTORY_LIMIT: usize = 48;

pub async fn run<B: LlmBackend + Clone + 'static>(game: GameService<B>) -> Result<()> {
    let mut terminal = ratatui::init();
    let mut app = App::new();
    let local = LocalSet::new();
    let result = local.run_until(app.run(&mut terminal, game)).await;
    ratatui::restore();
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Normal,
    Input,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Menu {
    None,
    Travel,
    Interact,
    Wait,
}

struct App {
    mode: Mode,
    menu: Menu,
    input: String,
    cursor: usize,
    menu_state: ListState,
    notices: Vec<String>,
    wait_duration: TimeDelta,
    spinner_frame: usize,
    pending: Option<PendingCommand>,
}

struct PendingCommand {
    enter_input_on_success: bool,
    rx: oneshot::Receiver<anyhow::Result<CommandResult>>,
}

enum ActiveListMenu<'a> {
    Travel {
        world_seed: crate::domain::seed::WorldSeed,
        routes: &'a [RouteView],
    },
    Interact {
        world_seed: crate::domain::seed::WorldSeed,
        options: &'a [InteractableOption],
    },
}

impl<'a> ActiveListMenu<'a> {
    fn from(snapshot: &'a UiSnapshot, menu: Menu) -> Option<Self> {
        match menu {
            Menu::Travel => Some(Self::Travel {
                world_seed: snapshot.world_seed,
                routes: &snapshot.routes,
            }),
            Menu::Interact => Some(Self::Interact {
                world_seed: snapshot.world_seed,
                options: &snapshot.interactables,
            }),
            Menu::None | Menu::Wait => None,
        }
    }

    fn title(&self) -> &'static str {
        match self {
            Self::Travel { .. } => "Travel",
            Self::Interact { .. } => "Interact",
        }
    }

    fn hint(&self) -> Line<'static> {
        match self {
            Self::Travel { .. } => Line::from("Up/Down route  Enter travel  Esc back  Ctrl+C quit"),
            Self::Interact { .. } => {
                Line::from("Up/Down target  Enter interact  Esc back  Ctrl+C quit")
            }
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::Travel { routes, .. } => routes.len(),
            Self::Interact { options, .. } => options.len(),
        }
    }

    fn items(&self) -> Vec<ListItem<'static>> {
        match self {
            Self::Travel { world_seed, routes } => {
                if routes.is_empty() {
                    vec![ListItem::new("Nothing available.")]
                } else {
                    routes
                        .iter()
                        .map(|route| ListItem::new(render_route_label(*world_seed, route)))
                        .collect()
                }
            }
            Self::Interact {
                world_seed,
                options,
            } => {
                if options.is_empty() {
                    vec![ListItem::new("Nothing available.")]
                } else {
                    options
                        .iter()
                        .map(|option| ListItem::new(render_interactable_label(*world_seed, option)))
                        .collect()
                }
            }
        }
    }

    fn action(&self, index: usize) -> Option<GameCommand> {
        match self {
            Self::Travel { routes, .. } => routes
                .get(index)
                .filter(|route| route.travel_time.is_some())
                .map(|route| GameCommand::TravelTo(route.destination.id)),
            Self::Interact { options, .. } => options.get(index).map(|option| match option.verb {
                InteractionVerb::Talk => match option.target {
                    InteractionTarget::Npc(npc_id) => GameCommand::OpenDialogue(npc_id),
                    InteractionTarget::Entity(_) => unreachable!("talk verb only targets npcs"),
                },
                InteractionVerb::EnterVehicle => match option.target {
                    InteractionTarget::Entity(entity_id) => GameCommand::EnterVehicle(entity_id),
                    InteractionTarget::Npc(_) => {
                        unreachable!("enter vehicle only targets entities")
                    }
                },
                InteractionVerb::ExitVehicle => GameCommand::ExitVehicle,
                InteractionVerb::Inspect => match option.target {
                    InteractionTarget::Entity(entity_id) => GameCommand::InspectEntity(entity_id),
                    InteractionTarget::Npc(_) => unreachable!("inspect only targets entities"),
                },
            }),
        }
    }
}

impl App {
    fn new() -> Self {
        let mut menu_state = ListState::default();
        menu_state.select(Some(0));
        Self {
            mode: Mode::Normal,
            menu: Menu::None,
            input: String::new(),
            cursor: 0,
            menu_state,
            notices: Vec::new(),
            wait_duration: TimeDelta::from_minutes(1),
            spinner_frame: 0,
            pending: None,
        }
    }

    async fn run<B: LlmBackend + Clone + 'static>(
        &mut self,
        terminal: &mut DefaultTerminal,
        game: GameService<B>,
    ) -> Result<()> {
        let game = Arc::new(Mutex::new(game));
        loop {
            if let Some(should_quit) = self.poll_pending(&game).await? {
                if should_quit {
                    break;
                }
            }

            let (snapshot, backend_name) = {
                let game = game.lock().await;
                (game.snapshot(), game.backend_name())
            };
            self.sync_menu(&snapshot);
            terminal.draw(|frame| self.render(frame, &snapshot, backend_name))?;
            self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();

            if event::poll(Duration::from_millis(50))? {
                let Event::Key(key) = event::read()? else {
                    continue;
                };
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                let should_quit = if self.pending.is_some() {
                    self.handle_pending_key(key)?
                } else {
                    match self.mode {
                        Mode::Normal => self.handle_normal_key(key, &game, &snapshot).await?,
                        Mode::Input => self.handle_input_key(key, &game, &snapshot).await?,
                    }
                };

                if should_quit {
                    break;
                }
            }
        }

        Ok(())
    }

    fn handle_pending_key(&mut self, key: KeyEvent) -> Result<bool> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(true);
        }
        Ok(false)
    }

    async fn handle_normal_key<B: LlmBackend + Clone + 'static>(
        &mut self,
        key: KeyEvent,
        game: &Arc<Mutex<GameService<B>>>,
        snapshot: &UiSnapshot,
    ) -> Result<bool> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(true);
        }

        match key.code {
            KeyCode::Char('g') if snapshot.mode == UiMode::Explore => self.open_menu(Menu::Travel),
            KeyCode::Char('e') if snapshot.mode == UiMode::Explore => {
                self.open_menu(Menu::Interact)
            }
            KeyCode::Char('w') if snapshot.mode == UiMode::Explore => self.menu = Menu::Wait,
            KeyCode::Char(ch)
                if snapshot.mode == UiMode::Dialogue
                    && self.menu == Menu::None
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.mode = Mode::Input;
                self.insert_char(ch);
            }
            KeyCode::Esc => {
                if self.menu != Menu::None {
                    self.menu = Menu::None;
                } else if snapshot.mode == UiMode::Dialogue {
                    return self.execute_action(game, GameCommand::LeaveDialogue, false);
                }
            }
            KeyCode::Left if self.menu == Menu::Wait => self.adjust_wait(-1),
            KeyCode::Right if self.menu == Menu::Wait => self.adjust_wait(1),
            KeyCode::Down if self.menu == Menu::Wait => self.adjust_wait(-60),
            KeyCode::Up if self.menu == Menu::Wait => self.adjust_wait(60),
            KeyCode::Down if ActiveListMenu::from(snapshot, self.menu).is_some() => {
                self.next_item(snapshot)
            }
            KeyCode::Up if ActiveListMenu::from(snapshot, self.menu).is_some() => {
                self.previous_item(snapshot)
            }
            KeyCode::Enter => {
                if let Some(action) = self.selected_menu_action(snapshot) {
                    let enter_input = matches!(action, GameCommand::OpenDialogue(_));
                    return self.execute_action(game, action, enter_input);
                } else if snapshot.mode == UiMode::Dialogue {
                    self.mode = Mode::Input;
                }
            }
            _ => {}
        }
        Ok(false)
    }

    async fn handle_input_key<B: LlmBackend + Clone + 'static>(
        &mut self,
        key: KeyEvent,
        game: &Arc<Mutex<GameService<B>>>,
        snapshot: &UiSnapshot,
    ) -> Result<bool> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(true);
        }

        match key.code {
            KeyCode::Esc => {
                if snapshot.mode == UiMode::Dialogue {
                    self.input.clear();
                    self.cursor = 0;
                    return self.execute_action(game, GameCommand::LeaveDialogue, false);
                }
                self.input.clear();
                self.cursor = 0;
                self.mode = Mode::Normal;
            }
            KeyCode::Enter => {
                let submitted = std::mem::take(&mut self.input);
                self.cursor = 0;
                if submitted.trim().is_empty() {
                    self.mode = Mode::Normal;
                } else {
                    return self.execute_action(
                        game,
                        GameCommand::SubmitDialogueLine(submitted.trim().to_string()),
                        true,
                    );
                }
            }
            KeyCode::Backspace => self.delete_char(),
            KeyCode::Left => self.move_cursor_left(),
            KeyCode::Right => self.move_cursor_right(),
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.insert_char(ch)
            }
            _ => {}
        }
        Ok(false)
    }

    fn execute_action<B: LlmBackend + Clone + 'static>(
        &mut self,
        game: &Arc<Mutex<GameService<B>>>,
        action: GameCommand,
        enter_input_on_success: bool,
    ) -> Result<bool> {
        let (tx, rx) = oneshot::channel();
        let game = Arc::clone(game);
        self.pending = Some(PendingCommand {
            enter_input_on_success,
            rx,
        });
        self.mode = Mode::Normal;
        spawn_local(async move {
            let result = {
                let mut game = game.lock().await;
                game.apply_command(action).await
            };
            let _ = tx.send(result);
        });
        Ok(false)
    }

    async fn poll_pending<B: LlmBackend + Clone + 'static>(
        &mut self,
        game: &Arc<Mutex<GameService<B>>>,
    ) -> Result<Option<bool>> {
        let Some(mut pending) = self.pending.take() else {
            return Ok(None);
        };
        match pending.rx.try_recv() {
            Ok(result) => {
                let should_quit = match result {
                    Ok(CommandResult {
                        events,
                        should_quit,
                    }) => {
                        let world_seed = {
                            let game = game.lock().await;
                            game.snapshot().world_seed
                        };
                        for event in &events {
                            if let Some(text) = render_event_notice(world_seed, event) {
                                self.push_notice(text);
                            }
                        }
                        should_quit
                    }
                    Err(error) => {
                        self.push_notice(format!("Action failed: {error:#}"));
                        false
                    }
                };
                let in_dialogue = {
                    let game = game.lock().await;
                    game.snapshot().mode == UiMode::Dialogue
                };
                self.mode = if pending.enter_input_on_success && in_dialogue {
                    Mode::Input
                } else {
                    Mode::Normal
                };
                if !in_dialogue {
                    self.menu = Menu::None;
                }
                Ok(Some(should_quit))
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                self.pending = Some(pending);
                Ok(Some(false))
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                self.push_notice("The last action did not finish cleanly.".to_string());
                Ok(Some(false))
            }
        }
    }

    fn render(&mut self, frame: &mut Frame, snapshot: &UiSnapshot, backend_name: &str) {
        let layout = Layout::vertical([Constraint::Min(12), Constraint::Length(1)]);
        let [world_area, command_area] = frame.area().layout(&layout);

        frame.render_widget(self.world_paragraph(snapshot), world_area);
        self.render_overlay(frame, snapshot, world_area);
        frame.render_widget(self.command_bar(snapshot, backend_name), command_area);

        if self.mode == Mode::Input && self.pending.is_none() {
            let area = self.overlay_area(world_area, 72, 6);
            frame.set_cursor_position(Position::new(area.x + self.cursor as u16 + 1, area.y + 2));
        }
    }

    fn world_paragraph(&self, snapshot: &UiSnapshot) -> Paragraph<'static> {
        let mode_label = match snapshot.mode {
            UiMode::Explore => "Explore",
            UiMode::Dialogue => "Dialogue",
        };
        let mut title = build_world_title(snapshot).spans;
        title.push(Span::raw("  "));
        title.push(Span::styled(
            format!("[{}]", mode_label),
            Style::default().fg(Color::Cyan),
        ));
        Paragraph::new(build_world_text(snapshot, &self.notices))
            .block(Block::bordered().title(Line::from(title)))
            .wrap(Wrap { trim: false })
    }

    fn render_overlay(&mut self, frame: &mut Frame, snapshot: &UiSnapshot, world_area: Rect) {
        if self.pending.is_some() {
            let area = self.overlay_area(world_area, 54, 5);
            frame.render_widget(Clear, area);
            frame.render_widget(self.pending_widget(), area);
            return;
        }

        match self.mode {
            Mode::Input => {
                let area = self.overlay_area(world_area, 72, 6);
                frame.render_widget(Clear, area);
                frame.render_widget(self.input_widget(snapshot.mode == UiMode::Dialogue), area);
            }
            Mode::Normal => match self.menu {
                Menu::None => {}
                Menu::Wait => {
                    let area = self.overlay_area(world_area, 38, 5);
                    frame.render_widget(Clear, area);
                    frame.render_widget(self.wait_widget(), area);
                }
                Menu::Travel | Menu::Interact => {
                    let Some(menu) = ActiveListMenu::from(snapshot, self.menu) else {
                        return;
                    };
                    let items = menu.items();
                    let height = items.len().min(8) as u16 + 2;
                    let area = self.overlay_area(world_area, 76, height.max(4));
                    let list = List::new(items)
                        .block(Block::bordered().title(menu.title()))
                        .highlight_style(selected_style())
                        .highlight_symbol("› ");
                    frame.render_widget(Clear, area);
                    frame.render_stateful_widget(list, area, &mut self.menu_state);
                }
            },
        }
    }

    fn input_widget(&self, in_dialogue: bool) -> Paragraph<'_> {
        let title = if in_dialogue { "Conversation" } else { "Input" };
        let lines = vec![
            Line::from("Type your reply. Press Enter to send or Esc to close."),
            Line::from(self.input.as_str()),
        ];
        Paragraph::new(lines)
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Left)
            .block(self.context_block().title(title))
    }

    fn pending_widget(&self) -> Paragraph<'static> {
        let spinner = SPINNER_FRAMES[self.spinner_frame % SPINNER_FRAMES.len()];
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    spinner,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Waiting for the world to respond..."),
            ]),
            Line::from(""),
            Line::from(
                "Dialogue and world commands run in the background. The UI will stay responsive while this finishes.",
            ),
        ])
        .block(self.context_block().title("Working"))
        .wrap(Wrap { trim: false })
    }

    fn command_bar(&self, snapshot: &UiSnapshot, backend_name: &str) -> Line<'static> {
        if self.pending.is_some() {
            return Line::from(vec![
                "Ctrl+C".bold(),
                " quit  ".into(),
                Span::raw(format!("Waiting on {}...", backend_name)),
            ]);
        }

        match self.mode {
            Mode::Input => match snapshot.mode {
                UiMode::Dialogue => {
                    Line::from("Enter send  Esc leave dialogue  Left/Right move  Ctrl+C quit")
                }
                UiMode::Explore => {
                    Line::from("Enter send  Esc close  Left/Right move  Ctrl+C quit")
                }
            },
            Mode::Normal => match self.menu {
                Menu::Travel | Menu::Interact => ActiveListMenu::from(snapshot, self.menu)
                    .expect("list menu should exist")
                    .hint(),
                Menu::Wait => {
                    Line::from("Left/Right +/-1s  Up/Down +/-1m  Enter wait  Esc back  Ctrl+C quit")
                }
                Menu::None => match snapshot.mode {
                    UiMode::Explore => Line::from("g travel  e interact  w wait  Ctrl+C quit"),
                    UiMode::Dialogue => Line::from("Type reply  Esc leave dialogue  Ctrl+C quit"),
                },
            },
        }
    }

    fn selected_menu_action(&self, snapshot: &UiSnapshot) -> Option<GameCommand> {
        let index = self.menu_state.selected().unwrap_or(0);
        match self.menu {
            Menu::Travel | Menu::Interact => {
                ActiveListMenu::from(snapshot, self.menu).and_then(|menu| menu.action(index))
            }
            Menu::Wait => Some(GameCommand::Wait(self.wait_duration)),
            Menu::None => None,
        }
    }

    fn sync_menu(&mut self, snapshot: &UiSnapshot) {
        let len = ActiveListMenu::from(snapshot, self.menu)
            .map(|menu| menu.len())
            .unwrap_or(0);
        if len == 0 {
            self.menu_state.select(Some(0));
        } else {
            let current = self.menu_state.selected().unwrap_or(0).min(len - 1);
            self.menu_state.select(Some(current));
        }
    }

    fn next_item(&mut self, snapshot: &UiSnapshot) {
        let len = ActiveListMenu::from(snapshot, self.menu)
            .map(|menu| menu.len())
            .unwrap_or(0);
        if len == 0 {
            return;
        }
        let next = match self.menu_state.selected() {
            Some(index) if index + 1 < len => index + 1,
            _ => 0,
        };
        self.menu_state.select(Some(next));
    }

    fn previous_item(&mut self, snapshot: &UiSnapshot) {
        let len = ActiveListMenu::from(snapshot, self.menu)
            .map(|menu| menu.len())
            .unwrap_or(0);
        if len == 0 {
            return;
        }
        let previous = match self.menu_state.selected() {
            Some(0) | None => len - 1,
            Some(index) => index - 1,
        };
        self.menu_state.select(Some(previous));
    }

    fn open_menu(&mut self, menu: Menu) {
        self.menu = menu;
        self.menu_state.select(Some(0));
    }

    fn push_notice(&mut self, text: String) {
        self.notices.push(text);
        if self.notices.len() > NOTICE_HISTORY_LIMIT {
            let extra = self.notices.len() - NOTICE_HISTORY_LIMIT;
            self.notices.drain(0..extra);
        }
    }

    fn insert_char(&mut self, ch: char) {
        let byte_index = self.byte_index();
        self.input.insert(byte_index, ch);
        self.move_cursor_right();
    }

    fn delete_char(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let current = self.cursor;
        let left = current - 1;
        self.input = self
            .input
            .chars()
            .take(left)
            .chain(self.input.chars().skip(current))
            .collect();
        self.move_cursor_left();
    }

    fn move_cursor_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    fn move_cursor_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.input.chars().count());
    }

    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.cursor)
            .unwrap_or(self.input.len())
    }

    fn context_block(&self) -> Block<'static> {
        let title = match self.mode {
            Mode::Input => "Conversation",
            Mode::Normal => match self.menu {
                Menu::Travel => "Travel",
                Menu::Interact => "Interact",
                Menu::Wait => "Wait",
                Menu::None => "Menu",
            },
        };
        Block::bordered().title(title)
    }

    fn wait_widget(&self) -> Paragraph<'static> {
        Paragraph::new(vec![
            Line::from("Choose how long to wait."),
            Line::from(""),
            Line::from(vec![
                Span::styled("Wait for ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    format_duration(self.wait_duration),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
        ])
        .block(self.context_block())
    }

    fn adjust_wait(&mut self, delta: i64) {
        let next = if delta.is_negative() {
            self.wait_duration
                .saturating_sub(TimeDelta::from_seconds(delta.unsigned_abs() as u32))
        } else {
            self.wait_duration
                .saturating_add(TimeDelta::from_seconds(delta as u32))
        };
        self.wait_duration = next.clamp(TimeDelta::ONE_SECOND, TimeDelta::from_hours(12));
    }

    fn overlay_area(&self, area: Rect, width: u16, height: u16) -> Rect {
        let vertical = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(height.min(area.height.saturating_sub(2))),
            Constraint::Fill(1),
        ]);
        let [_, mid, _] = area.layout(&vertical);
        let horizontal = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(width.min(area.width.saturating_sub(2))),
            Constraint::Fill(1),
        ]);
        let [_, center, _] = mid.layout(&horizontal);
        center
    }
}

fn selected_style() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}
