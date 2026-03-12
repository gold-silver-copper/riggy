use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, is_raw_mode_enabled};
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
use crate::simulation::{Interactable, UiMode, UiSnapshot};

const SPINNER_FRAMES: &[&str] = &["|", "/", "-", "\\"];
const NOTICE_HISTORY_LIMIT: usize = 48;

pub async fn run<B: LlmBackend + Clone + 'static>(game: GameService<B>) -> Result<()> {
    let raw_mode_was_enabled = is_raw_mode_enabled().unwrap_or(false);
    if !raw_mode_was_enabled {
        enable_raw_mode()?;
    }
    let mut terminal = ratatui::init();
    let _ = execute!(
        std::io::stdout(),
        EnableBracketedPaste,
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
        )
    );
    let mut app = App::new();
    let local = LocalSet::new();
    let result = local.run_until(app.run(&mut terminal, game)).await;
    let _ = execute!(
        std::io::stdout(),
        DisableBracketedPaste,
        PopKeyboardEnhancementFlags
    );
    ratatui::restore();
    if !raw_mode_was_enabled {
        let _ = disable_raw_mode();
    }
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListMenuKind {
    Travel,
    Interact,
}

impl ListMenuKind {
    fn title(self) -> &'static str {
        match self {
            Self::Travel => "Travel",
            Self::Interact => "Interact",
        }
    }

    fn hint(self) -> Line<'static> {
        match self {
            Self::Travel => Line::from("Up/Down route  Enter travel  Esc back  Ctrl+C quit"),
            Self::Interact => Line::from("Up/Down target  Enter interact  Esc back  Ctrl+C quit"),
        }
    }

    fn len(self, snapshot: &UiSnapshot) -> usize {
        match self {
            Self::Travel => snapshot.routes.len(),
            Self::Interact => snapshot.interactables.len(),
        }
    }

    fn items(self, snapshot: &UiSnapshot) -> Vec<ListItem<'static>> {
        match self {
            Self::Travel => list_items(
                snapshot
                    .routes
                    .iter()
                    .map(|route| render_route_label(snapshot.world_seed, route)),
            ),
            Self::Interact => list_items(
                snapshot
                    .interactables
                    .iter()
                    .map(|option| render_interactable_label(snapshot.world_seed, option)),
            ),
        }
    }

    fn action(self, snapshot: &UiSnapshot, index: usize) -> Option<GameCommand> {
        match self {
            Self::Travel => snapshot
                .routes
                .get(index)
                .filter(|route| route.travel_time.is_some())
                .map(|route| GameCommand::TravelTo(route.destination.id)),
            Self::Interact => {
                snapshot
                    .interactables
                    .get(index)
                    .map(|interactable| match interactable {
                        Interactable::Talk(actor) => GameCommand::OpenDialogue(actor.id),
                        Interactable::EnterVehicle(entity) => GameCommand::EnterVehicle(entity.id),
                        Interactable::ExitVehicle(_) => GameCommand::ExitVehicle,
                        Interactable::Inspect(entity) => GameCommand::InspectEntity(entity.id),
                    })
            }
        }
    }
}

fn list_items(items: impl Iterator<Item = String>) -> Vec<ListItem<'static>> {
    let items = items.map(ListItem::new).collect::<Vec<_>>();
    if items.is_empty() {
        vec![ListItem::new("Nothing available.")]
    } else {
        items
    }
}

#[derive(Default)]
struct DialogueInputState {
    input: String,
    cursor: usize,
}

impl DialogueInputState {
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

    fn take_trimmed(&mut self) -> Option<String> {
        let submitted = std::mem::take(&mut self.input);
        self.cursor = 0;
        let trimmed = submitted.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .map(|(index, _)| index)
            .nth(self.cursor)
            .unwrap_or(self.input.len())
    }
}

struct ListMenuState {
    kind: ListMenuKind,
    selected: usize,
}

struct PendingState {
    resume_input_on_success: bool,
    rx: oneshot::Receiver<anyhow::Result<CommandResult>>,
}

enum UiState {
    Idle,
    ListMenu(ListMenuState),
    WaitMenu,
    DialogueInput(DialogueInputState),
    Pending(PendingState),
}

struct App {
    state: UiState,
    notices: Vec<String>,
    wait_duration: TimeDelta,
    spinner_frame: usize,
    input_debug: InputDebugLogger,
}

impl App {
    fn new() -> Self {
        Self {
            state: UiState::Idle,
            notices: Vec::new(),
            wait_duration: TimeDelta::from_minutes(1),
            spinner_frame: 0,
            input_debug: InputDebugLogger::from_env(),
        }
    }

    async fn run<B: LlmBackend + Clone + 'static>(
        &mut self,
        terminal: &mut DefaultTerminal,
        game: GameService<B>,
    ) -> Result<()> {
        self.input_debug.log(format!(
            "tui start raw_mode_enabled={} pid={} term={:?}",
            is_raw_mode_enabled().unwrap_or(false),
            std::process::id(),
            std::env::var("TERM").ok()
        ));
        let game = Arc::new(Mutex::new(game));
        loop {
            if let Some(should_quit) = self.poll_pending(&game).await? {
                if should_quit {
                    self.input_debug.log("poll_pending requested quit");
                    break;
                }
            }

            let (snapshot, backend_name) = {
                let game = game.lock().await;
                (game.snapshot(), game.backend_name())
            };
            self.sync_state(&snapshot);
            self.input_debug.log(format!(
                "draw mode={:?} state={} routes={} interactables={} notices={}",
                snapshot.mode,
                self.state_label(),
                snapshot.routes.len(),
                snapshot.interactables.len(),
                self.notices.len()
            ));
            terminal.draw(|frame| self.render(frame, &snapshot, backend_name))?;
            self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();

            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Key(key) => {
                        self.input_debug.log(format!(
                            "event key={key:?} state={} mode={:?}",
                            self.state_label(),
                            snapshot.mode
                        ));
                        if !is_actionable_key_event(key) {
                            self.input_debug.log("ignored non-actionable key event");
                            continue;
                        }
                        if self.handle_key(key, &game, &snapshot)? {
                            self.input_debug.log("handle_key requested quit");
                            break;
                        }
                    }
                    Event::Paste(text) => {
                        self.input_debug.log(format!(
                            "event paste={text:?} state={} mode={:?}",
                            self.state_label(),
                            snapshot.mode
                        ));
                        if self.handle_paste(text, &game, &snapshot)? {
                            self.input_debug.log("handle_paste requested quit");
                            break;
                        }
                    }
                    other => self.input_debug.log(format!(
                        "event other={other:?} state={}",
                        self.state_label()
                    )),
                }
            }
        }

        self.input_debug.log("tui stop");
        Ok(())
    }

    fn handle_key<B: LlmBackend + Clone + 'static>(
        &mut self,
        key: KeyEvent,
        game: &Arc<Mutex<GameService<B>>>,
        snapshot: &UiSnapshot,
    ) -> Result<bool> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.input_debug.log("ctrl-c quit");
            return Ok(true);
        }

        match &self.state {
            UiState::Pending(_) => Ok(false),
            UiState::Idle => self.handle_idle_key(key, game, snapshot),
            UiState::ListMenu(_) => self.handle_list_menu_key(key, game, snapshot),
            UiState::WaitMenu => self.handle_wait_key(key, game),
            UiState::DialogueInput(_) => self.handle_dialogue_input_key(key, game),
        }
    }

    fn handle_idle_key<B: LlmBackend + Clone + 'static>(
        &mut self,
        key: KeyEvent,
        game: &Arc<Mutex<GameService<B>>>,
        snapshot: &UiSnapshot,
    ) -> Result<bool> {
        match key.code {
            KeyCode::Char(ch) if snapshot.mode == UiMode::Explore => {
                if self.try_open_explore_overlay(ch) {
                    self.input_debug.log(format!(
                        "idle explore shortcut opened overlay key={ch:?} state={}",
                        self.state_label()
                    ));
                    return Ok(false);
                }
                self.input_debug
                    .log(format!("idle explore char ignored key={ch:?}"));
            }
            KeyCode::Char(ch)
                if snapshot.mode == UiMode::Dialogue
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                let mut input = DialogueInputState::default();
                input.insert_char(ch);
                self.state = UiState::DialogueInput(input);
                self.input_debug
                    .log(format!("dialogue input opened with char key={ch:?}"));
            }
            KeyCode::Enter if snapshot.mode == UiMode::Dialogue => {
                self.state = UiState::DialogueInput(DialogueInputState::default());
                self.input_debug.log("dialogue input opened with enter");
            }
            KeyCode::Esc if snapshot.mode == UiMode::Dialogue => {
                self.input_debug.log("idle dialogue esc leave");
                return self.execute_action(game, GameCommand::LeaveDialogue, false);
            }
            _ => self
                .input_debug
                .log(format!("idle key ignored key={:?}", key.code)),
        }
        Ok(false)
    }

    fn handle_paste<B: LlmBackend + Clone + 'static>(
        &mut self,
        text: String,
        game: &Arc<Mutex<GameService<B>>>,
        snapshot: &UiSnapshot,
    ) -> Result<bool> {
        if text.is_empty() {
            self.input_debug.log("paste ignored empty");
            return Ok(false);
        }

        match &mut self.state {
            UiState::Idle if snapshot.mode == UiMode::Explore => {
                if text.chars().count() == 1
                    && self.try_open_explore_overlay(text.chars().next().unwrap())
                {
                    self.input_debug
                        .log(format!("paste opened explore overlay text={text:?}"));
                    return Ok(false);
                }
            }
            UiState::DialogueInput(input) => {
                for ch in text.chars() {
                    input.insert_char(ch);
                }
                self.input_debug
                    .log(format!("paste appended to dialogue input text={text:?}"));
                return Ok(false);
            }
            UiState::Idle if snapshot.mode == UiMode::Dialogue => {
                let mut input = DialogueInputState::default();
                for ch in text.chars() {
                    input.insert_char(ch);
                }
                self.state = UiState::DialogueInput(input);
                self.input_debug
                    .log(format!("paste opened dialogue input text={text:?}"));
                return Ok(false);
            }
            _ => {}
        }

        let _ = game;
        self.input_debug.log(format!(
            "paste ignored text={text:?} state={}",
            self.state_label()
        ));
        Ok(false)
    }

    fn handle_list_menu_key<B: LlmBackend + Clone + 'static>(
        &mut self,
        key: KeyEvent,
        game: &Arc<Mutex<GameService<B>>>,
        snapshot: &UiSnapshot,
    ) -> Result<bool> {
        let UiState::ListMenu(menu) = &mut self.state else {
            return Ok(false);
        };
        match key.code {
            KeyCode::Esc => {
                self.state = UiState::Idle;
                self.input_debug.log("list menu closed");
            }
            KeyCode::Down => {
                cycle_selection(&mut menu.selected, menu.kind.len(snapshot), true);
                self.input_debug
                    .log(format!("list menu moved down selected={}", menu.selected));
            }
            KeyCode::Up => {
                cycle_selection(&mut menu.selected, menu.kind.len(snapshot), false);
                self.input_debug
                    .log(format!("list menu moved up selected={}", menu.selected));
            }
            KeyCode::Enter => {
                if let Some(action) = menu.kind.action(snapshot, menu.selected) {
                    let resume_input = matches!(action, GameCommand::OpenDialogue(_));
                    self.input_debug.log(format!(
                        "list menu execute selected={} action={action:?}",
                        menu.selected
                    ));
                    return self.execute_action(game, action, resume_input);
                }
                self.input_debug.log("list menu enter had no action");
            }
            _ => self
                .input_debug
                .log(format!("list menu key ignored key={:?}", key.code)),
        }
        Ok(false)
    }

    fn handle_wait_key<B: LlmBackend + Clone + 'static>(
        &mut self,
        key: KeyEvent,
        game: &Arc<Mutex<GameService<B>>>,
    ) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.state = UiState::Idle;
                self.input_debug.log("wait menu closed");
            }
            KeyCode::Left => {
                self.adjust_wait(-1);
                self.input_debug.log(format!(
                    "wait menu left duration={}",
                    format_duration(self.wait_duration)
                ));
            }
            KeyCode::Right => {
                self.adjust_wait(1);
                self.input_debug.log(format!(
                    "wait menu right duration={}",
                    format_duration(self.wait_duration)
                ));
            }
            KeyCode::Down => {
                self.adjust_wait(-60);
                self.input_debug.log(format!(
                    "wait menu down duration={}",
                    format_duration(self.wait_duration)
                ));
            }
            KeyCode::Up => {
                self.adjust_wait(60);
                self.input_debug.log(format!(
                    "wait menu up duration={}",
                    format_duration(self.wait_duration)
                ));
            }
            KeyCode::Enter => {
                self.input_debug.log(format!(
                    "wait menu execute duration={}",
                    format_duration(self.wait_duration)
                ));
                return self.execute_action(game, GameCommand::Wait(self.wait_duration), false);
            }
            _ => self
                .input_debug
                .log(format!("wait menu key ignored key={:?}", key.code)),
        }
        Ok(false)
    }

    fn handle_dialogue_input_key<B: LlmBackend + Clone + 'static>(
        &mut self,
        key: KeyEvent,
        game: &Arc<Mutex<GameService<B>>>,
    ) -> Result<bool> {
        let UiState::DialogueInput(input) = &mut self.state else {
            return Ok(false);
        };
        match key.code {
            KeyCode::Esc => {
                self.input_debug.log("dialogue input esc leave");
                return self.execute_action(game, GameCommand::LeaveDialogue, false);
            }
            KeyCode::Enter => {
                if let Some(submitted) = input.take_trimmed() {
                    self.input_debug
                        .log(format!("dialogue submit text={submitted:?}"));
                    return self.execute_action(
                        game,
                        GameCommand::SubmitDialogueLine(submitted),
                        true,
                    );
                }
                self.state = UiState::Idle;
                self.input_debug.log("dialogue input enter empty -> idle");
            }
            KeyCode::Backspace => {
                input.delete_char();
                self.input_debug
                    .log(format!("dialogue backspace text={:?}", input.input));
            }
            KeyCode::Left => {
                input.move_cursor_left();
                self.input_debug
                    .log(format!("dialogue cursor left cursor={}", input.cursor));
            }
            KeyCode::Right => {
                input.move_cursor_right();
                self.input_debug
                    .log(format!("dialogue cursor right cursor={}", input.cursor));
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                input.insert_char(ch);
                self.input_debug
                    .log(format!("dialogue char key={ch:?} text={:?}", input.input));
            }
            _ => self
                .input_debug
                .log(format!("dialogue key ignored key={:?}", key.code)),
        }
        Ok(false)
    }

    fn execute_action<B: LlmBackend + Clone + 'static>(
        &mut self,
        game: &Arc<Mutex<GameService<B>>>,
        action: GameCommand,
        resume_input_on_success: bool,
    ) -> Result<bool> {
        let (tx, rx) = oneshot::channel();
        let game = Arc::clone(game);
        self.input_debug.log(format!(
            "execute action={action:?} resume_input_on_success={resume_input_on_success}"
        ));
        self.state = UiState::Pending(PendingState {
            resume_input_on_success,
            rx,
        });
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
        if !matches!(self.state, UiState::Pending(_)) {
            return Ok(None);
        }
        let UiState::Pending(mut pending) = std::mem::replace(&mut self.state, UiState::Idle)
        else {
            unreachable!("pending state should have been checked above");
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
                        self.input_debug.log(format!(
                            "pending finished ok events={} should_quit={should_quit}",
                            events.len()
                        ));
                        should_quit
                    }
                    Err(error) => {
                        self.input_debug
                            .log(format!("pending finished error={error:#}"));
                        self.push_notice(format!("Action failed: {error:#}"));
                        false
                    }
                };
                let in_dialogue = {
                    let game = game.lock().await;
                    game.snapshot().mode == UiMode::Dialogue
                };
                self.state = if pending.resume_input_on_success && in_dialogue {
                    UiState::DialogueInput(DialogueInputState::default())
                } else {
                    UiState::Idle
                };
                self.input_debug.log(format!(
                    "pending transitioned to state={}",
                    self.state_label()
                ));
                Ok(Some(should_quit))
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                self.state = UiState::Pending(pending);
                Ok(Some(false))
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                self.input_debug.log("pending receiver closed");
                self.state = UiState::Idle;
                self.push_notice("The last action did not finish cleanly.".to_string());
                Ok(Some(false))
            }
        }
    }

    fn sync_state(&mut self, snapshot: &UiSnapshot) {
        let reset_to_idle = match &mut self.state {
            UiState::Idle | UiState::Pending(_) => false,
            UiState::WaitMenu => snapshot.mode != UiMode::Explore,
            UiState::DialogueInput(_) => snapshot.mode != UiMode::Dialogue,
            UiState::ListMenu(menu) => {
                if snapshot.mode != UiMode::Explore {
                    true
                } else {
                    let len = menu.kind.len(snapshot);
                    menu.selected = if len == 0 {
                        0
                    } else {
                        menu.selected.min(len - 1)
                    };
                    false
                }
            }
        };

        if reset_to_idle {
            self.state = UiState::Idle;
            self.input_debug.log("sync_state reset ui state to idle");
        }
    }

    fn render(&mut self, frame: &mut Frame, snapshot: &UiSnapshot, backend_name: &str) {
        let layout = Layout::vertical([Constraint::Min(12), Constraint::Length(1)]);
        let [world_area, command_area] = frame.area().layout(&layout);

        frame.render_widget(self.world_paragraph(snapshot), world_area);
        self.render_overlay(frame, snapshot, world_area);
        frame.render_widget(self.command_bar(snapshot, backend_name), command_area);

        if let UiState::DialogueInput(input) = &self.state {
            let area = self.overlay_area(world_area, 72, 6);
            frame.set_cursor_position(Position::new(area.x + input.cursor as u16 + 1, area.y + 2));
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
        match &self.state {
            UiState::Idle => {}
            UiState::Pending(_) => {
                let area = self.overlay_area(world_area, 54, 5);
                frame.render_widget(Clear, area);
                frame.render_widget(self.pending_widget(), area);
            }
            UiState::DialogueInput(input) => {
                let area = self.overlay_area(world_area, 72, 6);
                frame.render_widget(Clear, area);
                frame.render_widget(self.input_widget(&input.input), area);
            }
            UiState::WaitMenu => {
                let area = self.overlay_area(world_area, 38, 5);
                frame.render_widget(Clear, area);
                frame.render_widget(self.wait_widget(), area);
            }
            UiState::ListMenu(menu) => {
                let items = menu.kind.items(snapshot);
                let height = items.len().min(8) as u16 + 2;
                let area = self.overlay_area(world_area, 76, height.max(4));
                let list = List::new(items)
                    .block(Block::bordered().title(menu.kind.title()))
                    .highlight_style(selected_style())
                    .highlight_symbol("› ");
                let mut state = ListState::default();
                state.select(Some(menu.selected));
                frame.render_widget(Clear, area);
                frame.render_stateful_widget(list, area, &mut state);
            }
        }
    }

    fn input_widget<'a>(&self, input: &'a str) -> Paragraph<'a> {
        Paragraph::new(vec![
            Line::from("Type your reply. Press Enter to send or Esc to leave."),
            Line::from(input),
        ])
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Left)
        .block(Block::bordered().title("Conversation"))
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
        .block(Block::bordered().title("Working"))
        .wrap(Wrap { trim: false })
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
        .block(Block::bordered().title("Wait"))
    }

    fn command_bar(&self, snapshot: &UiSnapshot, backend_name: &str) -> Line<'static> {
        match &self.state {
            UiState::Pending(_) => Line::from(vec![
                "Ctrl+C".bold(),
                " quit  ".into(),
                Span::raw(format!("Waiting on {}...", backend_name)),
            ]),
            UiState::DialogueInput(_) => {
                Line::from("Enter send  Esc leave dialogue  Left/Right move  Ctrl+C quit")
            }
            UiState::ListMenu(menu) => menu.kind.hint(),
            UiState::WaitMenu => {
                Line::from("Left/Right +/-1s  Up/Down +/-1m  Enter wait  Esc back  Ctrl+C quit")
            }
            UiState::Idle => match snapshot.mode {
                UiMode::Explore => Line::from("g travel  e interact  w wait  Ctrl+C quit"),
                UiMode::Dialogue => Line::from("Type reply  Esc leave dialogue  Ctrl+C quit"),
            },
        }
    }

    fn push_notice(&mut self, text: String) {
        self.notices.push(text);
        if self.notices.len() > NOTICE_HISTORY_LIMIT {
            let extra = self.notices.len() - NOTICE_HISTORY_LIMIT;
            self.notices.drain(0..extra);
        }
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

    fn state_label(&self) -> &'static str {
        match &self.state {
            UiState::Idle => "idle",
            UiState::ListMenu(_) => "list_menu",
            UiState::WaitMenu => "wait_menu",
            UiState::DialogueInput(_) => "dialogue_input",
            UiState::Pending(_) => "pending",
        }
    }

    fn try_open_explore_overlay(&mut self, ch: char) -> bool {
        if ch.eq_ignore_ascii_case(&'g') {
            self.state = UiState::ListMenu(ListMenuState {
                kind: ListMenuKind::Travel,
                selected: 0,
            });
            true
        } else if ch.eq_ignore_ascii_case(&'e') {
            self.state = UiState::ListMenu(ListMenuState {
                kind: ListMenuKind::Interact,
                selected: 0,
            });
            true
        } else if ch.eq_ignore_ascii_case(&'w') {
            self.state = UiState::WaitMenu;
            true
        } else {
            false
        }
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

fn is_actionable_key_event(key: KeyEvent) -> bool {
    !matches!(key.kind, KeyEventKind::Release)
}

fn cycle_selection(selected: &mut usize, len: usize, forward: bool) {
    if len == 0 {
        *selected = 0;
    } else if forward {
        *selected = if *selected + 1 < len {
            *selected + 1
        } else {
            0
        };
    } else {
        *selected = if *selected == 0 {
            len - 1
        } else {
            *selected - 1
        };
    }
}

fn selected_style() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

struct InputDebugLogger {
    sink: Option<File>,
    path: Option<PathBuf>,
}

impl InputDebugLogger {
    fn from_env() -> Self {
        let env_enabled = std::env::var("RIGGY_INPUT_DEBUG")
            .map(|value| value != "0" && !value.eq_ignore_ascii_case("false"))
            .unwrap_or(false);
        let enabled = (cfg!(debug_assertions) && !cfg!(test)) || env_enabled;
        if !enabled {
            return Self {
                sink: None,
                path: None,
            };
        }

        let path = std::env::var_os("RIGGY_INPUT_LOG")
            .map(PathBuf::from)
            .unwrap_or_else(default_input_log_path);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let sink = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok();

        let mut logger = Self {
            sink,
            path: Some(path),
        };
        if let Some(path) = logger.path.as_ref() {
            logger.log(format!("logging enabled path={}", path.display()));
        }
        logger.log("==== new riggy input log session ====");
        logger
    }

    fn log(&mut self, message: impl AsRef<str>) {
        let Some(sink) = &mut self.sink else {
            return;
        };
        let _ = writeln!(sink, "{}", message.as_ref());
        let _ = sink.flush();
    }
}

fn default_input_log_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("riggy-input.log")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crossterm::event::{KeyEventState, KeyModifiers};
    use tokio::sync::Mutex;

    use crate::app::service::GameService;
    use crate::llm::MockBackend;

    use super::{
        App, KeyCode, KeyEvent, KeyEventKind, ListMenuKind, UiState, default_input_log_path,
        is_actionable_key_event,
    };

    #[test]
    fn actionable_key_events_ignore_release_only() {
        let press = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        let repeat = KeyEvent {
            code: KeyCode::Char('g'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Repeat,
            state: KeyEventState::NONE,
        };
        let release = KeyEvent {
            code: KeyCode::Char('g'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        };

        assert!(is_actionable_key_event(press));
        assert!(is_actionable_key_event(repeat));
        assert!(!is_actionable_key_event(release));
    }

    #[tokio::test]
    async fn explore_keys_open_the_expected_overlays() {
        let game = Arc::new(Mutex::new(GameService::new(MockBackend).unwrap()));
        let snapshot = {
            let game = game.lock().await;
            game.snapshot()
        };
        let mut app = App::new();

        app.handle_key(
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
            &game,
            &snapshot,
        )
        .unwrap();
        assert!(matches!(
            app.state,
            UiState::ListMenu(ref menu) if menu.kind == ListMenuKind::Travel
        ));

        app.state = UiState::Idle;
        app.handle_key(
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
            &game,
            &snapshot,
        )
        .unwrap();
        assert!(matches!(
            app.state,
            UiState::ListMenu(ref menu) if menu.kind == ListMenuKind::Interact
        ));

        app.state = UiState::Idle;
        app.handle_key(
            KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE),
            &game,
            &snapshot,
        )
        .unwrap();
        assert!(matches!(app.state, UiState::WaitMenu));
    }

    #[tokio::test]
    async fn uppercase_explore_keys_also_open_overlays() {
        let game = Arc::new(Mutex::new(GameService::new(MockBackend).unwrap()));
        let snapshot = {
            let game = game.lock().await;
            game.snapshot()
        };
        let mut app = App::new();

        app.handle_key(
            KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT),
            &game,
            &snapshot,
        )
        .unwrap();

        assert!(matches!(
            app.state,
            UiState::ListMenu(ref menu) if menu.kind == ListMenuKind::Travel
        ));
    }

    #[tokio::test]
    async fn pasted_explore_shortcuts_open_overlays() {
        let game = Arc::new(Mutex::new(GameService::new(MockBackend).unwrap()));
        let snapshot = {
            let game = game.lock().await;
            game.snapshot()
        };
        let mut app = App::new();

        app.handle_paste("e".to_string(), &game, &snapshot).unwrap();

        assert!(matches!(
            app.state,
            UiState::ListMenu(ref menu) if menu.kind == ListMenuKind::Interact
        ));
    }

    #[test]
    fn debug_log_defaults_into_repo_target_dir() {
        let path = default_input_log_path();
        assert!(path.ends_with("target/riggy-input.log"));
    }

    #[tokio::test]
    async fn poll_pending_preserves_non_pending_state() {
        let game = Arc::new(Mutex::new(GameService::new(MockBackend).unwrap()));
        let mut app = App::new();
        app.state = UiState::WaitMenu;

        let result = app.poll_pending(&game).await.unwrap();

        assert_eq!(result, None);
        assert!(matches!(app.state, UiState::WaitMenu));
    }
}
