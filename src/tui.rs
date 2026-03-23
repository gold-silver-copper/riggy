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
use tracing::{debug, error, info};

use crate::app::service::GameService;
use crate::domain::commands::{ActionKind, ActionRequest, AvailableAction};
use crate::domain::events::ActionResult;
use crate::domain::time::TimeDelta;
use crate::llm::LlmBackend;
use crate::presenter::{
    build_world_text, build_world_title, format_duration, render_event_notice,
    render_interactable_label, render_route_label,
};
use crate::simulation::{ActorView, AgentDebugSnapshot, Interactable, UiSnapshot};

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
            Self::Travel => Line::from("Up/Down route  Enter travel  Esc back  F2 debug  Ctrl+C quit"),
            Self::Interact => Line::from("Up/Down target  Enter select  Esc back  F2 debug  Ctrl+C quit"),
        }
    }

    fn len(self, snapshot: &UiSnapshot) -> usize {
        self.actions(snapshot).len()
    }

    fn items(self, snapshot: &UiSnapshot) -> Vec<ListItem<'static>> {
        list_items(
            self.actions(snapshot)
                .into_iter()
                .map(|action| render_available_action_label(snapshot, action)),
        )
    }

    fn actions(self, snapshot: &UiSnapshot) -> Vec<AvailableAction> {
        snapshot
            .available_actions
            .iter()
            .copied()
            .filter(|action| match (self, action) {
                (Self::Travel, AvailableAction::MoveTo { .. }) => true,
                (Self::Interact, AvailableAction::SpeakTo { .. }) => true,
                (Self::Interact, AvailableAction::InspectEntity { .. }) => true,
                _ => false,
            })
            .collect()
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
struct TextInputState {
    input: String,
    cursor: usize,
}

impl TextInputState {
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

struct ConversationState {
    target: ActorView,
    input: TextInputState,
}

impl ConversationState {
    fn new(target: ActorView) -> Self {
        Self {
            target,
            input: TextInputState::default(),
        }
    }
}

struct ListMenuState {
    kind: ListMenuKind,
    selected: usize,
}

struct PendingState {
    resume_conversation: Option<ActorView>,
    rx: oneshot::Receiver<anyhow::Result<ActionResult>>,
}

enum UiState {
    Idle,
    ListMenu(ListMenuState),
    WaitMenu,
    Conversation(ConversationState),
    Pending(PendingState),
}

struct App {
    state: UiState,
    notices: Vec<String>,
    wait_duration: TimeDelta,
    spinner_frame: usize,
    debug_panel_open: bool,
    last_snapshot: Option<UiSnapshot>,
    last_backend_label: String,
}

impl App {
    fn new() -> Self {
        Self {
            state: UiState::Idle,
            notices: Vec::new(),
            wait_duration: TimeDelta::from_minutes(1),
            spinner_frame: 0,
            debug_panel_open: false,
            last_snapshot: None,
            last_backend_label: String::new(),
        }
    }

    async fn run<B: LlmBackend + Clone + 'static>(
        &mut self,
        terminal: &mut DefaultTerminal,
        game: GameService<B>,
    ) -> Result<()> {
        let game = Arc::new(Mutex::new(game));
        loop {
            tokio::task::yield_now().await;

            if let Some(should_quit) = self.poll_pending(&game).await? {
                if should_quit {
                    break;
                }
            }

            let (snapshot, backend_label) = if matches!(self.state, UiState::Pending(_)) {
                let snapshot = self
                    .last_snapshot
                    .clone()
                    .expect("pending state should keep the previous snapshot");
                (snapshot, self.last_backend_label.clone())
            } else {
                self.refresh_cached_view(&game).await?
            };
            self.sync_state(&snapshot);
            terminal.draw(|frame| self.render(frame, &snapshot, &backend_label))?;
            self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if !is_actionable_key_event(key) {
                        continue;
                    }
                    if self.handle_key(key, &game, &snapshot)? {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    async fn refresh_cached_view<B: LlmBackend + Clone + 'static>(
        &mut self,
        game: &Arc<Mutex<GameService<B>>>,
    ) -> Result<(UiSnapshot, String)> {
        let (snapshot, backend_label) = {
            let game = game.lock().await;
            (game.snapshot(), game.backend_label())
        };
        self.last_snapshot = Some(snapshot.clone());
        self.last_backend_label = backend_label.clone();
        Ok((snapshot, backend_label))
    }

    fn handle_key<B: LlmBackend + Clone + 'static>(
        &mut self,
        key: KeyEvent,
        game: &Arc<Mutex<GameService<B>>>,
        snapshot: &UiSnapshot,
    ) -> Result<bool> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(true);
        }
        if key.code == KeyCode::F(2) {
            self.debug_panel_open = !self.debug_panel_open;
            return Ok(false);
        }

        match &self.state {
            UiState::Pending(_) => Ok(false),
            UiState::Idle => self.handle_idle_key(key),
            UiState::ListMenu(_) => self.handle_list_menu_key(key, game, snapshot),
            UiState::WaitMenu => self.handle_wait_key(key, game, snapshot),
            UiState::Conversation(_) => self.handle_conversation_key(key, game, snapshot),
        }
    }

    fn handle_idle_key(&mut self, key: KeyEvent) -> Result<bool> {
        if let KeyCode::Char(ch) = key.code {
            let _ = self.try_open_explore_overlay(ch);
        }
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
            }
            KeyCode::Down => {
                cycle_selection(&mut menu.selected, menu.kind.len(snapshot), true);
            }
            KeyCode::Up => {
                cycle_selection(&mut menu.selected, menu.kind.len(snapshot), false);
            }
            KeyCode::Enter => match menu.kind {
                ListMenuKind::Travel => {
                    if let Some(AvailableAction::MoveTo { destination }) =
                        menu.kind.actions(snapshot).get(menu.selected).copied()
                    {
                        return self.execute_action(
                            game,
                            ActionRequest {
                                actor_id: snapshot.focused_actor_id,
                                action: ActionKind::MoveTo { destination },
                            },
                            None,
                        );
                    }
                }
                ListMenuKind::Interact => {
                    if let Some(action) = menu.kind.actions(snapshot).get(menu.selected).copied() {
                        match action {
                            AvailableAction::SpeakTo { target } => {
                                if let Some(actor) = actor_view_from_interactables(snapshot, target) {
                                    self.state =
                                        UiState::Conversation(ConversationState::new(actor));
                                }
                            }
                            AvailableAction::InspectEntity { entity_id } => {
                                return self.execute_action(
                                    game,
                                    ActionRequest {
                                        actor_id: snapshot.focused_actor_id,
                                        action: ActionKind::InspectEntity { entity_id },
                                    },
                                    None,
                                );
                            }
                            AvailableAction::MoveTo { .. } | AvailableAction::Wait => {}
                        }
                    }
                }
            },
            _ => {}
        }
        Ok(false)
    }

    fn handle_wait_key<B: LlmBackend + Clone + 'static>(
        &mut self,
        key: KeyEvent,
        game: &Arc<Mutex<GameService<B>>>,
        snapshot: &UiSnapshot,
    ) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.state = UiState::Idle;
            }
            KeyCode::Left => self.adjust_wait(-1),
            KeyCode::Right => self.adjust_wait(1),
            KeyCode::Down => self.adjust_wait(-60),
            KeyCode::Up => self.adjust_wait(60),
            KeyCode::Enter => {
                return self.execute_action(
                    game,
                    ActionRequest {
                        actor_id: snapshot.focused_actor_id,
                        action: ActionKind::Wait {
                            duration: self.wait_duration,
                        },
                    },
                    None,
                );
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_conversation_key<B: LlmBackend + Clone + 'static>(
        &mut self,
        key: KeyEvent,
        game: &Arc<Mutex<GameService<B>>>,
        snapshot: &UiSnapshot,
    ) -> Result<bool> {
        let UiState::Conversation(conversation) = &mut self.state else {
            return Ok(false);
        };

        match key.code {
            KeyCode::Esc => {
                self.state = UiState::Idle;
            }
            KeyCode::Enter => {
                if let Some(submitted) = conversation.input.take_trimmed() {
                    let target = conversation.target;
                    return self.execute_action(
                        game,
                        ActionRequest {
                            actor_id: snapshot.focused_actor_id,
                            action: ActionKind::Speak {
                                target: target.id,
                                text: submitted,
                            },
                        },
                        Some(target),
                    );
                }
            }
            KeyCode::Backspace => conversation.input.delete_char(),
            KeyCode::Left => conversation.input.move_cursor_left(),
            KeyCode::Right => conversation.input.move_cursor_right(),
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                conversation.input.insert_char(ch);
            }
            _ => {}
        }
        Ok(false)
    }

    fn execute_action<B: LlmBackend + Clone + 'static>(
        &mut self,
        game: &Arc<Mutex<GameService<B>>>,
        action: ActionRequest,
        resume_conversation: Option<ActorView>,
    ) -> Result<bool> {
        info!(
            actor_id = action.actor_id.index(),
            action = ?action.action,
            resume_conversation = resume_conversation.as_ref().map(|actor| actor.id.index()),
            "submitting action from tui"
        );
        let (tx, rx) = oneshot::channel();
        let game = Arc::clone(game);
        self.state = UiState::Pending(PendingState {
            resume_conversation,
            rx,
        });
        spawn_local(async move {
            let result = {
                let mut game = game.lock().await;
                game.apply_action(action).await
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
                    Ok(ActionResult {
                        events,
                        should_quit,
                    }) => {
                        debug!(event_count = events.len(), should_quit, "pending action completed");
                        let snapshot = self.refresh_cached_view(game).await?.0;
                        for event in &events {
                            if let Some(text) =
                                render_event_notice(
                                    snapshot.world_seed,
                                    snapshot.focused_actor_id,
                                    event,
                                )
                            {
                                self.push_notice(text);
                            }
                        }
                        should_quit
                    }
                    Err(error) => {
                        error!(error = %error, "pending action failed");
                        self.push_notice(format!("Action failed: {error:#}"));
                        false
                    }
                };
                let snapshot = self.refresh_cached_view(game).await?.0;
                self.state = if let Some(target) = pending.resume_conversation {
                    if snapshot
                        .interactables
                        .iter()
                        .any(|interactable| matches!(interactable, Interactable::Talk(actor) if actor.id == target.id))
                    {
                        UiState::Conversation(ConversationState::new(target))
                    } else {
                        UiState::Idle
                    }
                } else {
                    UiState::Idle
                };
                Ok(Some(should_quit))
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                self.state = UiState::Pending(pending);
                Ok(Some(false))
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                self.state = UiState::Idle;
                self.push_notice("The last action did not finish cleanly.".to_string());
                Ok(Some(false))
            }
        }
    }

    fn sync_state(&mut self, snapshot: &UiSnapshot) {
        let reset_to_idle = match &mut self.state {
            UiState::Idle | UiState::Pending(_) => false,
            UiState::WaitMenu => false,
            UiState::Conversation(conversation) => !snapshot
                .interactables
                .iter()
                .any(|interactable| matches!(interactable, Interactable::Talk(actor) if actor.id == conversation.target.id)),
            UiState::ListMenu(menu) => {
                let len = menu.kind.len(snapshot);
                menu.selected = if len == 0 {
                    0
                } else {
                    menu.selected.min(len - 1)
                };
                false
            }
        };

        if reset_to_idle {
            self.state = UiState::Idle;
        }
    }

    fn render(&mut self, frame: &mut Frame, snapshot: &UiSnapshot, backend_label: &str) {
        let layout = Layout::vertical([Constraint::Min(12), Constraint::Length(1)]);
        let [body_area, command_area] = frame.area().layout(&layout);
        let (world_area, debug_area) = if self.debug_panel_open {
            let horizontal = Layout::horizontal([Constraint::Min(32), Constraint::Length(58)]);
            let [world_area, debug_area] = body_area.layout(&horizontal);
            (world_area, Some(debug_area))
        } else {
            (body_area, None)
        };

        frame.render_widget(self.world_paragraph(snapshot), world_area);
        self.render_overlay(frame, snapshot, world_area);
        if let Some(debug_area) = debug_area {
            frame.render_widget(self.debug_widget(snapshot), debug_area);
        }
        frame.render_widget(self.command_bar(backend_label), command_area);

        if let UiState::Conversation(conversation) = &self.state {
            let area = self.overlay_area(world_area, 72, 6);
            frame.set_cursor_position(Position::new(
                area.x + conversation.input.cursor as u16 + 1,
                area.y + 2,
            ));
        }
    }

    fn world_paragraph(&self, snapshot: &UiSnapshot) -> Paragraph<'static> {
        let mode_label = match self.state {
            UiState::Conversation(_) => "Conversation",
            UiState::ListMenu(ListMenuState {
                kind: ListMenuKind::Travel,
                ..
            }) => "Travel",
            UiState::ListMenu(ListMenuState {
                kind: ListMenuKind::Interact,
                ..
            }) => "Interact",
            UiState::WaitMenu => "Wait",
            UiState::Pending(_) => "Working",
            UiState::Idle => "Explore",
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
            UiState::Conversation(conversation) => {
                let area = self.overlay_area(world_area, 72, 6);
                frame.render_widget(Clear, area);
                frame.render_widget(self.input_widget(snapshot, conversation), area);
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

    fn input_widget<'a>(
        &self,
        snapshot: &'a UiSnapshot,
        conversation: &'a ConversationState,
    ) -> Paragraph<'a> {
        Paragraph::new(vec![
            Line::from(format!(
                "Talking to {}. Press Enter to speak or Esc to close.",
                conversation.target.id.name(snapshot.world_seed)
            )),
            Line::from(conversation.input.input.as_str()),
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
                "World actions run in the background. The UI will stay responsive while this finishes.",
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

    fn command_bar(&self, backend_name: &str) -> Line<'static> {
        match &self.state {
            UiState::Pending(_) => Line::from(vec![
                "Ctrl+C".bold(),
                " quit  F2 debug  ".into(),
                Span::raw(format!("Waiting on {}...", backend_name)),
            ]),
            UiState::Conversation(_) => {
                Line::from("Enter speak  Esc close panel  Left/Right move  F2 debug  Ctrl+C quit")
            }
            UiState::ListMenu(menu) => menu.kind.hint(),
            UiState::WaitMenu => {
                Line::from("Left/Right +/-1s  Up/Down +/-1m  Enter wait  Esc back  F2 debug  Ctrl+C quit")
            }
            UiState::Idle => Line::from("g travel  e interact  w wait  F2 debug  Ctrl+C quit"),
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

    fn debug_widget(&self, snapshot: &UiSnapshot) -> Paragraph<'static> {
        Paragraph::new(build_agent_debug_lines(snapshot))
            .block(Block::bordered().title("Agent Debug"))
            .wrap(Wrap { trim: false })
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

fn render_available_action_label(snapshot: &UiSnapshot, action: AvailableAction) -> String {
    match action {
        AvailableAction::MoveTo { destination } => snapshot
            .routes
            .iter()
            .find(|route| route.destination.id == destination)
            .map(|route| render_route_label(snapshot.world_seed, route))
            .unwrap_or_else(|| format!("Travel to {:?}", destination)),
        AvailableAction::SpeakTo { target } => actor_view_from_interactables(snapshot, target)
            .map(|actor| render_interactable_label(snapshot, &Interactable::Talk(actor)))
            .unwrap_or_else(|| format!("Talk to {:?}", target)),
        AvailableAction::InspectEntity { entity_id } => entity_interactable(snapshot, entity_id)
            .map(|entity| render_interactable_label(snapshot, &Interactable::Inspect(entity)))
            .unwrap_or_else(|| format!("Inspect {:?}", entity_id)),
        AvailableAction::Wait => "Wait".to_string(),
    }
}

fn actor_view_from_interactables(snapshot: &UiSnapshot, target: crate::world::ActorId) -> Option<ActorView> {
    snapshot
        .interactables
        .iter()
        .find_map(|interactable| match interactable {
            Interactable::Talk(actor) if actor.id == target => Some(*actor),
            _ => None,
        })
}

fn entity_interactable(
    snapshot: &UiSnapshot,
    entity_id: crate::world::EntityId,
) -> Option<crate::domain::events::EntitySummary> {
    snapshot
        .interactables
        .iter()
        .find_map(|interactable| match interactable {
            Interactable::Inspect(entity) if entity.id == entity_id => Some(*entity),
            _ => None,
        })
}

fn build_agent_debug_lines(snapshot: &UiSnapshot) -> Vec<Line<'static>> {
    if snapshot.agent_debug.is_empty() {
        return vec![
            Line::from("F2 toggles this panel."),
            Line::from(""),
            Line::from("No autonomous NPC agents are currently in this place."),
        ];
    }

    let mut lines = vec![Line::from("F2 toggles this panel."), Line::from("")];
    for (index, agent) in snapshot.agent_debug.iter().enumerate() {
        if index > 0 {
            lines.push(Line::from(""));
        }
        lines.extend(render_agent_debug_snapshot(snapshot, agent));
    }
    lines
}

fn render_agent_debug_snapshot(
    snapshot: &UiSnapshot,
    agent: &AgentDebugSnapshot,
) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(format!(
        "{} | {}, {}",
        agent.actor.id.name(snapshot.world_seed),
        agent.actor.occupation.label(),
        agent.actor.archetype.label()
    ))];

    let Some(trace) = &agent.trace else {
        lines.push(Line::from("  No agent decision trace yet."));
        return lines;
    };

    lines.push(Line::from(format!("  Backend: {}", trace.backend_name)));
    lines.push(Line::from(format!(
        "  Selected: {}",
        trace.selected_action
            .as_ref()
            .map(|action| render_action_kind(snapshot, action))
            .unwrap_or_else(|| "none".to_string())
    )));
    if let Some(error) = &trace.error {
        lines.push(Line::from(format!("  Error: {}", clean_debug_text(error))));
    }
    lines.push(Line::from(format!(
        "  Tools: {}",
        if trace.toolset.is_empty() {
            "none".to_string()
        } else {
            trace.toolset.join(", ")
        }
    )));
    lines.push(Line::from(format!(
        "  Available: {}",
        if trace.available_actions.is_empty() {
            "none".to_string()
        } else {
            trace.available_actions
                .iter()
                .map(|action| render_agent_available_action(snapshot, action))
                .collect::<Vec<_>>()
                .join(" | ")
        }
    )));

    if trace.recent_speech.is_empty() {
        lines.push(Line::from("  Recent speech: none"));
    } else {
        lines.push(Line::from("  Recent speech:"));
        for line in trace.recent_speech.iter().rev().take(4).rev() {
            let speaker = match line.speaker {
                crate::domain::events::DialogueSpeaker::Actor(actor_id)
                    if actor_id == snapshot.focused_actor_id =>
                {
                    "You".to_string()
                }
                crate::domain::events::DialogueSpeaker::Actor(actor_id) => {
                    actor_id.name(snapshot.world_seed)
                }
                crate::domain::events::DialogueSpeaker::System => "System".to_string(),
            };
            lines.push(Line::from(format!(
                "    [{}] {}: {}",
                line.timestamp.format(),
                speaker,
                clean_debug_text(&line.text)
            )));
        }
    }

    lines.push(Line::from("  Decision prompt:"));
    for prompt_line in trace.prompt.lines().take(8) {
        lines.push(Line::from(format!("    {}", clean_debug_text(prompt_line))));
    }
    if trace.prompt.lines().count() > 8 {
        lines.push(Line::from("    ..."));
    }

    if let Some(model_output) = &trace.model_output {
        lines.push(Line::from("  Model output:"));
        for line in model_output.lines().take(6) {
            lines.push(Line::from(format!("    {}", clean_debug_text(line))));
        }
        if model_output.lines().count() > 6 {
            lines.push(Line::from("    ..."));
        }
    }

    if trace.tool_calls.is_empty() {
        lines.push(Line::from("  Tool calls: none"));
    } else {
        lines.push(Line::from("  Tool calls:"));
        for call in &trace.tool_calls {
            lines.push(Line::from(format!("    {}", call.tool_name)));
            lines.push(Line::from(format!(
                "      args {}",
                compact_debug_block(&call.arguments)
            )));
            if let Some(result) = &call.result {
                lines.push(Line::from(format!(
                    "      => {}",
                    compact_debug_block(result)
                )));
            }
            if let Some(error) = &call.error {
                lines.push(Line::from(format!(
                    "      !! {}",
                    clean_debug_text(error)
                )));
            }
        }
    }

    lines
}

fn render_action_kind(snapshot: &UiSnapshot, action: &ActionKind) -> String {
    match action {
        ActionKind::MoveTo { destination } => snapshot
            .routes
            .iter()
            .find(|route| route.destination.id == *destination)
            .map(|route| format!("move_to {}", render_route_label(snapshot.world_seed, route)))
            .unwrap_or_else(|| format!("move_to place#{}", destination.index())),
        ActionKind::Speak { target, text } => {
            format!(
                "speak to {}: {}",
                render_actor_debug_name(snapshot.world_seed, *target),
                clean_debug_text(text)
            )
        }
        ActionKind::InspectEntity { entity_id } => format!("inspect entity#{}", entity_id.index()),
        ActionKind::Wait { duration } => format!("wait {}", format_duration(*duration)),
        ActionKind::DoNothing => "do_nothing".to_string(),
    }
}

fn render_agent_available_action(
    snapshot: &UiSnapshot,
    action: &crate::domain::commands::AgentAvailableAction,
) -> String {
    match action {
        crate::domain::commands::AgentAvailableAction::MoveTo { destination } => {
            format!("move_to place#{}", destination.index())
        }
        crate::domain::commands::AgentAvailableAction::SpeakTo { target } => {
            format!("speak {}", render_actor_debug_name(snapshot.world_seed, *target))
        }
        crate::domain::commands::AgentAvailableAction::InspectEntity { entity_id } => {
            format!("inspect entity#{}", entity_id.index())
        }
        crate::domain::commands::AgentAvailableAction::DoNothing => "do_nothing".to_string(),
    }
}

fn clean_debug_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn compact_debug_block(text: &str) -> String {
    let cleaned = clean_debug_text(text);
    if cleaned.len() > 160 {
        format!("{}...", &cleaned[..160])
    } else {
        cleaned
    }
}

fn render_actor_debug_name(
    world_seed: crate::domain::seed::WorldSeed,
    actor_id: crate::world::ActorId,
) -> String {
    format!("{} (#{} )", actor_id.name(world_seed), actor_id.index())
        .replace("(#", "(#")
        .replace(" )", ")")
}
