use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};
use tracing::info;

use crate::app::service::GameService;
use crate::domain::commands::{ActionKind, ActionRequest};
use crate::domain::events::{ActionResult, ContextEntry, DialogueSpeaker, GameEvent};
use crate::domain::time::TimeDelta;
use crate::llm::{AnyBackend, LlmBackend, MockBackend};
use crate::presenter::{render_event_notice, render_route_label};
use crate::simulation::{ActorView, Interactable, UiSnapshot};
use crate::world::{ActorId, EntityId, PlaceId, entity_name_from_parts, place_name_from_parts};

#[derive(Debug, Clone, PartialEq, Eq)]
enum HeadlessCommand {
    Help,
    Look,
    Actions,
    People,
    Routes,
    Entities,
    Context,
    Debug,
    FocusManual,
    FocusActor { actor_id: ActorId },
    Travel { selector: usize },
    Say { selector: usize, text: String },
    Inspect { selector: usize },
    Wait { duration: TimeDelta },
    AgentTurn { actor_id: ActorId },
    Save { path: PathBuf },
    Load { path: PathBuf },
    Source { path: PathBuf },
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub text: String,
    pub should_quit: bool,
}

pub struct HeadlessSession<B> {
    game: GameService<B>,
    focused_actor_id: ActorId,
}

impl<B: LlmBackend> HeadlessSession<B> {
    pub fn new(game: GameService<B>) -> Self {
        let focused_actor_id = game.manual_actor_id();
        Self {
            game,
            focused_actor_id,
        }
    }

    pub fn snapshot(&self) -> UiSnapshot {
        self.game.snapshot_for(self.focused_actor_id)
    }

    pub async fn execute_line(&mut self, line: &str) -> Result<Option<CommandOutput>> {
        let Some(command) = parse_command(line, self.game.manual_actor_id())? else {
            return Ok(None);
        };
        self.execute_parsed_command(command).await.map(Some)
    }

    async fn execute_parsed_command(&mut self, command: HeadlessCommand) -> Result<CommandOutput> {
        if let HeadlessCommand::Source { path } = command {
            return Ok(CommandOutput {
                text: self.execute_script(&path).await?,
                should_quit: false,
            });
        }
        self.execute_non_source_command(command).await
    }

    async fn execute_non_source_command(
        &mut self,
        command: HeadlessCommand,
    ) -> Result<CommandOutput> {
        match command {
            HeadlessCommand::Help => Ok(CommandOutput {
                text: help_text(),
                should_quit: false,
            }),
            HeadlessCommand::Look => Ok(CommandOutput {
                text: self.render_snapshot(),
                should_quit: false,
            }),
            HeadlessCommand::Actions => Ok(CommandOutput {
                text: self.render_actions(),
                should_quit: false,
            }),
            HeadlessCommand::People => Ok(CommandOutput {
                text: self.render_people(),
                should_quit: false,
            }),
            HeadlessCommand::Routes => Ok(CommandOutput {
                text: self.render_routes(),
                should_quit: false,
            }),
            HeadlessCommand::Entities => Ok(CommandOutput {
                text: self.render_entities(),
                should_quit: false,
            }),
            HeadlessCommand::Context => Ok(CommandOutput {
                text: self.render_context(),
                should_quit: false,
            }),
            HeadlessCommand::Debug => Ok(CommandOutput {
                text: self.render_debug(),
                should_quit: false,
            }),
            HeadlessCommand::FocusManual => {
                self.focused_actor_id = self.game.manual_actor_id();
                Ok(CommandOutput {
                    text: format!(
                        "Focused actor reset to actor#{}.\n\n{}",
                        self.focused_actor_id.index(),
                        self.render_snapshot()
                    ),
                    should_quit: false,
                })
            }
            HeadlessCommand::FocusActor { actor_id } => {
                if !self.game.actor_exists(actor_id) {
                    bail!("actor#{} does not exist", actor_id.index());
                }
                let snapshot = self.game.snapshot_for(actor_id);
                self.focused_actor_id = actor_id;
                Ok(CommandOutput {
                    text: format!(
                        "Focused actor is now {}.\n\n{}",
                        actor_label(&snapshot, actor_id),
                        self.render_snapshot()
                    ),
                    should_quit: false,
                })
            }
            HeadlessCommand::Travel { selector } => {
                let snapshot = self.snapshot();
                let destination = resolve_route_selector(&snapshot, selector)?;
                let result = self
                    .game
                    .apply_action(ActionRequest {
                        actor_id: self.focused_actor_id,
                        action: ActionKind::MoveTo { destination },
                    })
                    .await?;
                Ok(self.render_action_result(result))
            }
            HeadlessCommand::Say { selector, text } => {
                let snapshot = self.snapshot();
                let target = resolve_actor_selector(&snapshot, selector)?;
                let result = self
                    .game
                    .apply_action(ActionRequest {
                        actor_id: self.focused_actor_id,
                        action: ActionKind::Speak { target, text },
                    })
                    .await?;
                Ok(self.render_action_result(result))
            }
            HeadlessCommand::Inspect { selector } => {
                let snapshot = self.snapshot();
                let entity_id = resolve_entity_selector(&snapshot, selector)?;
                let result = self
                    .game
                    .apply_action(ActionRequest {
                        actor_id: self.focused_actor_id,
                        action: ActionKind::InspectEntity { entity_id },
                    })
                    .await?;
                Ok(self.render_action_result(result))
            }
            HeadlessCommand::Wait { duration } => {
                let result = self
                    .game
                    .apply_action(ActionRequest {
                        actor_id: self.focused_actor_id,
                        action: ActionKind::Wait { duration },
                    })
                    .await?;
                Ok(self.render_action_result(result))
            }
            HeadlessCommand::AgentTurn { actor_id } => {
                let Some(request) = self.game.choose_autonomous_action(actor_id).await? else {
                    let snapshot = self.snapshot();
                    return Ok(CommandOutput {
                        text: format!(
                            "{} had no autonomous action to take.\n\n{}",
                            actor_label(&snapshot, actor_id),
                            self.render_debug()
                        ),
                        should_quit: false,
                    });
                };
                let result = self.game.apply_action(request).await?;
                Ok(self.render_action_result(result))
            }
            HeadlessCommand::Save { path } => {
                self.game.save(&path)?;
                Ok(CommandOutput {
                    text: format!("Saved game state to {}.", path.display()),
                    should_quit: false,
                })
            }
            HeadlessCommand::Load { path } => {
                self.game.load(&path)?;
                if !self.game.actor_exists(self.focused_actor_id) {
                    self.focused_actor_id = self.game.manual_actor_id();
                }
                Ok(CommandOutput {
                    text: format!(
                        "Loaded game state from {}.\n\n{}",
                        path.display(),
                        self.render_snapshot()
                    ),
                    should_quit: false,
                })
            }
            HeadlessCommand::Quit => Ok(CommandOutput {
                text: "Quitting headless session.".to_string(),
                should_quit: true,
            }),
            HeadlessCommand::Source { .. } => {
                bail!("source commands are only supported from the top-level parser")
            }
        }
    }

    async fn execute_script(&mut self, path: &Path) -> Result<String> {
        let contents = fs::read_to_string(path)?;
        let mut sections = Vec::new();

        for (line_no, line) in contents.lines().enumerate() {
            let rendered = line.trim();
            if rendered.is_empty() || rendered.starts_with('#') {
                continue;
            }
            let Some(command) = parse_command(rendered, self.game.manual_actor_id())? else {
                continue;
            };
            if matches!(command, HeadlessCommand::Source { .. }) {
                bail!("nested source commands are not supported");
            }
            let output = self.execute_non_source_command(command).await?;
            sections.push(format!("> {}:{} {}", path.display(), line_no + 1, rendered));
            if !output.text.trim().is_empty() {
                sections.push(output.text);
            }
            if output.should_quit {
                break;
            }
        }

        if sections.is_empty() {
            Ok(format!(
                "Script {} contained no executable commands.",
                path.display()
            ))
        } else {
            Ok(sections.join("\n\n"))
        }
    }

    fn render_action_result(&self, result: ActionResult) -> CommandOutput {
        let snapshot = self.snapshot();
        let mut sections = Vec::new();
        if result.events.is_empty() {
            sections.push("Events:\n  (none)".to_string());
        } else {
            let event_lines = result
                .events
                .iter()
                .filter_map(|event| render_event_text(&snapshot, event))
                .collect::<Vec<_>>();
            if event_lines.is_empty() {
                sections.push("Events:\n  (none worth rendering)".to_string());
            } else {
                sections.push(format!("Events:\n{}", event_lines.join("\n")));
            }
        }
        sections.push(self.render_snapshot());
        CommandOutput {
            text: sections.join("\n\n"),
            should_quit: result.should_quit,
        }
    }

    fn render_snapshot(&self) -> String {
        let snapshot = self.snapshot();
        let mut lines = vec![
            format!(
                "Focused actor: {}",
                actor_view_label(&snapshot, snapshot.focused_actor)
            ),
            format!(
                "Location: {} in {}",
                place_label(
                    snapshot.world_seed,
                    snapshot.place.id,
                    snapshot.place.city_id,
                    snapshot.place.kind,
                ),
                snapshot.city.id.name(snapshot.world_seed),
            ),
            format!(
                "Time: {} | Known cities: {}",
                snapshot.status.clock.format(),
                snapshot.status.known_city_count
            ),
            format!(
                "City mood: {}, {}, {}",
                snapshot.city.biome.label(),
                snapshot.city.economy.label(),
                snapshot.city.culture.label()
            ),
        ];

        let connected_cities = snapshot
            .city
            .connected_cities
            .iter()
            .map(|city_id| {
                format!(
                    "city#{} {}",
                    city_id.index(),
                    city_id.name(snapshot.world_seed)
                )
            })
            .collect::<Vec<_>>();
        lines.push(format_list("Connected cities", connected_cities));

        lines.push("People here:".to_string());
        let talk_targets = talk_targets(&snapshot);
        if talk_targets.is_empty() {
            lines.push("  (none)".to_string());
        } else {
            for (index, actor) in talk_targets.iter().enumerate() {
                lines.push(format!(
                    "  [{}] {}",
                    index,
                    actor_view_label(&snapshot, *actor)
                ));
            }
        }

        lines.push("Routes:".to_string());
        if snapshot.routes.is_empty() {
            lines.push("  (none)".to_string());
        } else {
            for (index, route) in snapshot.routes.iter().enumerate() {
                lines.push(format!(
                    "  [{}] place#{} {}",
                    index,
                    route.destination.id.index(),
                    render_route_label(snapshot.world_seed, route)
                ));
            }
        }

        lines.push("Entities:".to_string());
        let entities = inspect_targets(&snapshot);
        if entities.is_empty() {
            lines.push("  (none)".to_string());
        } else {
            for (index, entity) in entities.iter().enumerate() {
                lines.push(format!(
                    "  [{}] entity#{} {} ({})",
                    index,
                    entity.id.index(),
                    entity_name_from_parts(snapshot.world_seed, entity.id, entity.kind),
                    entity.kind.label()
                ));
            }
        }

        lines.push(String::new());
        lines.push(self.render_actions());
        lines.push(String::new());
        lines.push(self.render_context());
        lines.join("\n")
    }

    fn render_actions(&self) -> String {
        let snapshot = self.snapshot();
        let mut lines = vec!["Available actions:".to_string()];
        if snapshot.available_actions.is_empty() {
            lines.push("  (none)".to_string());
        } else {
            for action in &snapshot.available_actions {
                lines.push(format!("  - {}", available_action_label(&snapshot, action)));
            }
        }
        lines.join("\n")
    }

    fn render_people(&self) -> String {
        let snapshot = self.snapshot();
        let mut lines = vec!["People here:".to_string()];
        let talk_targets = talk_targets(&snapshot);
        if talk_targets.is_empty() {
            lines.push("  (none)".to_string());
        } else {
            for (index, actor) in talk_targets.iter().enumerate() {
                lines.push(format!(
                    "  [{}] {}",
                    index,
                    actor_view_label(&snapshot, *actor)
                ));
            }
        }
        lines.join("\n")
    }

    fn render_routes(&self) -> String {
        let snapshot = self.snapshot();
        let mut lines = vec!["Routes:".to_string()];
        if snapshot.routes.is_empty() {
            lines.push("  (none)".to_string());
        } else {
            for (index, route) in snapshot.routes.iter().enumerate() {
                lines.push(format!(
                    "  [{}] place#{} {}",
                    index,
                    route.destination.id.index(),
                    render_route_label(snapshot.world_seed, route)
                ));
            }
        }
        lines.join("\n")
    }

    fn render_entities(&self) -> String {
        let snapshot = self.snapshot();
        let mut lines = vec!["Entities:".to_string()];
        let entities = inspect_targets(&snapshot);
        if entities.is_empty() {
            lines.push("  (none)".to_string());
        } else {
            for (index, entity) in entities.iter().enumerate() {
                lines.push(format!(
                    "  [{}] entity#{} {} ({})",
                    index,
                    entity.id.index(),
                    entity_name_from_parts(snapshot.world_seed, entity.id, entity.kind),
                    entity.kind.label()
                ));
            }
        }
        lines.join("\n")
    }

    fn render_context(&self) -> String {
        let snapshot = self.snapshot();
        let mut lines = vec!["Recent activity:".to_string()];
        if snapshot.context_feed.is_empty() {
            lines.push("  (none)".to_string());
        } else {
            for entry in &snapshot.context_feed {
                lines.push(format!("  {}", render_context_entry(&snapshot, entry)));
            }
        }
        lines.join("\n")
    }

    fn render_debug(&self) -> String {
        let snapshot = self.snapshot();
        let mut lines = Vec::new();
        if let Some(trace) = self.game.agent_debug_trace(snapshot.focused_actor_id) {
            lines.push(render_agent_debug_block(
                &snapshot,
                snapshot.focused_actor,
                Some(&trace),
            ));
        }
        for agent in &snapshot.agent_debug {
            lines.push(render_agent_debug_block(
                &snapshot,
                agent.actor,
                agent.trace.as_ref(),
            ));
        }
        if lines.is_empty() {
            "No agent debug traces available yet.".to_string()
        } else {
            lines.join("\n\n")
        }
    }
}

pub async fn run_cli(args: impl IntoIterator<Item = String>) -> Result<()> {
    let options = CliOptions::parse(args.into_iter())?;
    if options.print_help {
        println!("{}", cli_help_text());
        return Ok(());
    }

    let backend = if options.force_mock {
        AnyBackend::Mock(MockBackend)
    } else {
        AnyBackend::from_env()?
    };
    let backend_label = backend.label();
    let game = GameService::new(backend)?;
    let mut session = HeadlessSession::new(game);
    info!(backend = %backend_label, "starting headless session");

    if let Some(script_path) = options.script {
        let output = session
            .execute_parsed_command(HeadlessCommand::Source { path: script_path })
            .await?;
        if !output.text.is_empty() {
            println!("{}", output.text);
        }
        return Ok(());
    }

    if !options.commands.is_empty() {
        for command in options.commands {
            let Some(output) = session.execute_line(&command).await? else {
                continue;
            };
            if !output.text.is_empty() {
                println!("{}", output.text);
            }
            if output.should_quit {
                break;
            }
        }
        return Ok(());
    }

    println!("riggy headless session");
    println!("backend: {}", backend_label);
    println!("{}", help_text());
    println!();
    println!("{}", session.render_snapshot());
    print!("\nriggy> ");
    io::stdout().flush()?;

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        let Some(output) = session.execute_line(&line).await? else {
            continue;
        };
        if !output.text.is_empty() {
            writeln!(stdout, "{}", output.text)?;
        }
        if output.should_quit {
            break;
        }
        write!(stdout, "\nriggy> ")?;
        stdout.flush()?;
    }

    Ok(())
}

#[derive(Default)]
struct CliOptions {
    commands: Vec<String>,
    script: Option<PathBuf>,
    force_mock: bool,
    print_help: bool,
}

impl CliOptions {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self> {
        let mut options = Self::default();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--mock" => options.force_mock = true,
                "--command" | "-c" => {
                    let Some(command) = args.next() else {
                        bail!("--command requires a value");
                    };
                    options.commands.push(command);
                }
                "--script" | "-s" => {
                    let Some(path) = args.next() else {
                        bail!("--script requires a value");
                    };
                    options.script = Some(PathBuf::from(path));
                }
                "--help" | "-h" => options.print_help = true,
                other => bail!("unknown argument: {other}"),
            }
        }
        Ok(options)
    }
}

fn parse_command(line: &str, manual_actor_id: ActorId) -> Result<Option<HeadlessCommand>> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Ok(None);
    }

    let mut parts = trimmed.split_whitespace();
    let Some(command) = parts.next() else {
        return Ok(None);
    };

    let parsed = match command {
        "help" | "h" | "?" => HeadlessCommand::Help,
        "look" | "l" | "snapshot" => HeadlessCommand::Look,
        "actions" => HeadlessCommand::Actions,
        "people" | "actors" => HeadlessCommand::People,
        "routes" => HeadlessCommand::Routes,
        "entities" | "items" => HeadlessCommand::Entities,
        "context" => HeadlessCommand::Context,
        "debug" => HeadlessCommand::Debug,
        "focus" => {
            let Some(target) = parts.next() else {
                bail!("focus requires `me` or an actor id");
            };
            if target == "me" {
                HeadlessCommand::FocusManual
            } else {
                HeadlessCommand::FocusActor {
                    actor_id: parse_actor_id(target)?,
                }
            }
        }
        "travel" | "move" => HeadlessCommand::Travel {
            selector: parse_selector_token(parts.next(), "travel")?,
        },
        "say" | "talk" => {
            let remainder = trimmed[command.len()..].trim_start();
            let Some((selector, text)) = remainder.split_once(char::is_whitespace) else {
                bail!("say requires a target index/id and text");
            };
            HeadlessCommand::Say {
                selector: selector
                    .parse::<usize>()
                    .map_err(|_| anyhow!("invalid say target selector: {selector}"))?,
                text: text.trim().to_string(),
            }
        }
        "inspect" => HeadlessCommand::Inspect {
            selector: parse_selector_token(parts.next(), "inspect")?,
        },
        "wait" => HeadlessCommand::Wait {
            duration: parse_duration_token(parts.next())?,
        },
        "agent" => {
            let Some(target) = parts.next() else {
                bail!("agent requires an actor id");
            };
            HeadlessCommand::AgentTurn {
                actor_id: parse_actor_id(target)?,
            }
        }
        "save" => HeadlessCommand::Save {
            path: parse_path_argument(trimmed, command, "save")?,
        },
        "load" => HeadlessCommand::Load {
            path: parse_path_argument(trimmed, command, "load")?,
        },
        "source" => HeadlessCommand::Source {
            path: parse_path_argument(trimmed, command, "source")?,
        },
        "quit" | "exit" => HeadlessCommand::Quit,
        other => bail!("unknown command: {other}"),
    };

    if matches!(parsed, HeadlessCommand::FocusActor { actor_id } if actor_id == manual_actor_id) {
        return Ok(Some(HeadlessCommand::FocusManual));
    }

    Ok(Some(parsed))
}

fn parse_selector_token(token: Option<&str>, command: &str) -> Result<usize> {
    let Some(token) = token else {
        bail!("{command} requires a numeric selector");
    };
    token
        .parse::<usize>()
        .map_err(|_| anyhow!("invalid numeric selector for {command}: {token}"))
}

fn parse_actor_id(token: &str) -> Result<ActorId> {
    Ok(ActorId(petgraph::stable_graph::NodeIndex::new(
        token
            .parse::<usize>()
            .map_err(|_| anyhow!("invalid actor id: {token}"))?,
    )))
}

fn parse_duration_token(token: Option<&str>) -> Result<TimeDelta> {
    let Some(token) = token else {
        bail!("wait requires a duration like `30s`, `2m`, or `1h`");
    };
    let (value, unit) = match token.chars().last() {
        Some('s') | Some('m') | Some('h') => {
            (&token[..token.len() - 1], token.chars().last().unwrap())
        }
        _ => (token, 's'),
    };
    let amount = value
        .parse::<u32>()
        .map_err(|_| anyhow!("invalid wait duration: {token}"))?;
    Ok(match unit {
        's' => TimeDelta::from_seconds(amount),
        'm' => TimeDelta::from_minutes(amount),
        'h' => TimeDelta::from_hours(amount),
        _ => unreachable!("unit should already be normalized"),
    })
}

fn parse_path_argument(trimmed: &str, command: &str, label: &str) -> Result<PathBuf> {
    let path = trimmed[command.len()..].trim();
    if path.is_empty() {
        bail!("{label} requires a path");
    }
    Ok(PathBuf::from(path))
}

fn talk_targets(snapshot: &UiSnapshot) -> Vec<ActorView> {
    snapshot
        .interactables
        .iter()
        .filter_map(|interactable| match interactable {
            Interactable::Talk(actor) => Some(*actor),
            Interactable::Inspect(_) => None,
        })
        .collect()
}

fn inspect_targets(snapshot: &UiSnapshot) -> Vec<crate::domain::events::EntitySummary> {
    snapshot
        .interactables
        .iter()
        .filter_map(|interactable| match interactable {
            Interactable::Inspect(entity) => Some(*entity),
            Interactable::Talk(_) => None,
        })
        .collect()
}

fn resolve_route_selector(snapshot: &UiSnapshot, selector: usize) -> Result<PlaceId> {
    if let Some(route) = snapshot.routes.get(selector) {
        return Ok(route.destination.id);
    }
    snapshot
        .routes
        .iter()
        .find(|route| route.destination.id.index() == selector)
        .map(|route| route.destination.id)
        .ok_or_else(|| anyhow!("no route for selector {selector}"))
}

fn resolve_actor_selector(snapshot: &UiSnapshot, selector: usize) -> Result<ActorId> {
    let talk_targets = talk_targets(snapshot);
    if let Some(actor) = talk_targets.get(selector) {
        return Ok(actor.id);
    }
    talk_targets
        .into_iter()
        .find(|actor| actor.id.index() == selector)
        .map(|actor| actor.id)
        .ok_or_else(|| anyhow!("no actor for selector {selector}"))
}

fn resolve_entity_selector(snapshot: &UiSnapshot, selector: usize) -> Result<EntityId> {
    let entities = inspect_targets(snapshot);
    if let Some(entity) = entities.get(selector) {
        return Ok(entity.id);
    }
    entities
        .into_iter()
        .find(|entity| entity.id.index() == selector)
        .map(|entity| entity.id)
        .ok_or_else(|| anyhow!("no entity for selector {selector}"))
}

fn render_event_text(snapshot: &UiSnapshot, event: &GameEvent) -> Option<String> {
    render_event_notice(snapshot.world_seed, snapshot.focused_actor_id, event)
        .map(|text| format!("  - {}", text))
        .or_else(|| match event {
            GameEvent::ContextAppended { .. } => None,
            other => Some(format!("  - {:?}", other)),
        })
}

fn render_context_entry(snapshot: &UiSnapshot, entry: &ContextEntry) -> String {
    match entry {
        ContextEntry::System { timestamp, context } => {
            format!(
                "[{}] {} {}",
                timestamp.format(),
                context.label(),
                render_system_context(snapshot, context)
            )
        }
        ContextEntry::Dialogue(line) => format!(
            "[{}] {}: {}",
            line.timestamp.format(),
            speaker_label(snapshot, line.speaker),
            line.text
        ),
    }
}

fn render_system_context(
    snapshot: &UiSnapshot,
    context: &crate::domain::events::SystemContext,
) -> String {
    match context {
        crate::domain::events::SystemContext::Travel {
            destination,
            duration,
        } => format!(
            "Arrived at {} after {}.",
            place_label(
                snapshot.world_seed,
                destination.id,
                destination.city_id,
                destination.kind,
            ),
            duration.format()
        ),
        crate::domain::events::SystemContext::Inspect { entity } => format!(
            "You inspect {}. It looks like a {} left out in plain view.",
            entity_label(snapshot, *entity),
            entity.kind.label()
        ),
        crate::domain::events::SystemContext::Wait {
            duration,
            current_time,
        } => format!(
            "You wait for {}. The time is now {}.",
            duration.format(),
            current_time.format()
        ),
    }
}

fn speaker_label(snapshot: &UiSnapshot, speaker: DialogueSpeaker) -> String {
    match speaker {
        DialogueSpeaker::Actor(actor_id) if actor_id == snapshot.focused_actor_id => {
            format!("You (actor#{})", actor_id.index())
        }
        DialogueSpeaker::Actor(actor_id) => {
            format!(
                "{} (actor#{})",
                actor_id.name(snapshot.world_seed),
                actor_id.index()
            )
        }
        DialogueSpeaker::System => "System".to_string(),
    }
}

fn actor_label(snapshot: &UiSnapshot, actor_id: ActorId) -> String {
    if actor_id == snapshot.focused_actor_id {
        format!("You (actor#{})", actor_id.index())
    } else {
        format!(
            "{} (actor#{})",
            actor_id.name(snapshot.world_seed),
            actor_id.index()
        )
    }
}

fn actor_view_label(snapshot: &UiSnapshot, actor: ActorView) -> String {
    format!(
        "{} - {}, {}",
        actor_label(snapshot, actor.id),
        actor.occupation.label(),
        actor.archetype.label()
    )
}

fn available_action_label(
    snapshot: &UiSnapshot,
    action: &crate::domain::commands::AvailableAction,
) -> String {
    match action {
        crate::domain::commands::AvailableAction::MoveTo { destination } => snapshot
            .routes
            .iter()
            .find(|route| route.destination.id == *destination)
            .map(|route| {
                format!(
                    "travel to place#{} {}",
                    destination.index(),
                    render_route_label(snapshot.world_seed, route)
                )
            })
            .unwrap_or_else(|| format!("travel to place#{}", destination.index())),
        crate::domain::commands::AvailableAction::SpeakTo { target } => {
            format!(
                "say to {} (actor#{})",
                target.name(snapshot.world_seed),
                target.index()
            )
        }
        crate::domain::commands::AvailableAction::InspectEntity { entity_id } => {
            format!("inspect entity#{}", entity_id.index())
        }
        crate::domain::commands::AvailableAction::Wait => "wait".to_string(),
    }
}

fn render_agent_debug_block(
    snapshot: &UiSnapshot,
    actor: ActorView,
    trace: Option<&crate::llm::AgentDebugTrace>,
) -> String {
    let mut lines = vec![format!("Agent {}", actor_view_label(snapshot, actor))];
    let Some(trace) = trace else {
        lines.push("  No trace yet.".to_string());
        return lines.join("\n");
    };
    lines.push(format!("  Backend: {}", trace.backend_name));
    lines.push(format!(
        "  Selected: {}",
        trace
            .selected_action
            .as_ref()
            .map(|action| format!("{action:?}"))
            .unwrap_or_else(|| "none".to_string())
    ));
    if let Some(error) = &trace.error {
        lines.push(format!("  Error: {}", error));
    }
    lines.push(format!(
        "  Available actions: {}",
        if trace.available_actions.is_empty() {
            "none".to_string()
        } else {
            trace
                .available_actions
                .iter()
                .map(|action| format!("{action:?}"))
                .collect::<Vec<_>>()
                .join(" | ")
        }
    ));
    if !trace.recent_speech.is_empty() {
        lines.push("  Recent speech:".to_string());
        for line in &trace.recent_speech {
            lines.push(format!("    [{}] {}", line.timestamp.format(), line.text));
        }
    }
    if let Some(model_output) = &trace.model_output {
        lines.push("  Model output:".to_string());
        for line in model_output.lines() {
            lines.push(format!("    {}", line));
        }
    }
    if trace.tool_calls.is_empty() {
        lines.push("  Tool calls: none".to_string());
    } else {
        lines.push("  Tool calls:".to_string());
        for call in &trace.tool_calls {
            lines.push(format!("    {}", call.tool_name));
            lines.push(format!("      args: {}", call.arguments));
            if let Some(result) = &call.result {
                lines.push(format!("      result: {}", result));
            }
            if let Some(error) = &call.error {
                lines.push(format!("      error: {}", error));
            }
        }
    }
    lines.join("\n")
}

fn place_label(
    world_seed: crate::domain::seed::WorldSeed,
    place_id: PlaceId,
    city_id: crate::world::CityId,
    kind: crate::world::PlaceKind,
) -> String {
    format!(
        "place#{} {} ({})",
        place_id.index(),
        place_name_from_parts(world_seed, place_id, city_id, kind),
        kind.label()
    )
}

fn entity_label(snapshot: &UiSnapshot, entity: crate::domain::events::EntitySummary) -> String {
    format!(
        "entity#{} {} ({})",
        entity.id.index(),
        entity_name_from_parts(snapshot.world_seed, entity.id, entity.kind),
        entity.kind.label()
    )
}

fn format_list(label: &str, values: Vec<String>) -> String {
    if values.is_empty() {
        format!("{label}: (none)")
    } else {
        format!("{label}: {}", values.join(", "))
    }
}

fn help_text() -> String {
    [
        "Commands:",
        "  look | l",
        "  actions",
        "  people | actors",
        "  routes",
        "  entities | items",
        "  context",
        "  debug",
        "  focus me | focus <actor-id>",
        "  travel <route-index-or-place-id>",
        "  say <person-index-or-actor-id> <text>",
        "  inspect <entity-index-or-entity-id>",
        "  wait <30s|2m|1h>",
        "  agent <actor-id>",
        "  save <path>",
        "  load <path>",
        "  source <script-path>",
        "  quit",
    ]
    .join("\n")
}

fn cli_help_text() -> String {
    [
        "Usage: cargo run --bin riggy_headless -- [options]",
        "",
        "Options:",
        "  --mock               force the mock backend",
        "  --command, -c CMD    run one command, can be repeated",
        "  --script, -s PATH    run commands from a script file",
        "  --help, -h           show this help",
        "",
        "If no command or script is provided, the binary starts an interactive headless session.",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::{HeadlessSession, parse_command};
    use crate::app::service::GameService;
    use crate::domain::seed::WorldSeed;
    use crate::llm::MockBackend;

    fn test_game() -> GameService<MockBackend> {
        GameService::new_with_seed(MockBackend, WorldSeed::new(42)).unwrap()
    }

    #[test]
    fn parse_say_command_keeps_full_text() {
        let manual_actor = test_game().manual_actor_id();
        let command = parse_command("say 0 hello there", manual_actor)
            .unwrap()
            .unwrap();
        assert!(
            matches!(command, super::HeadlessCommand::Say { selector: 0, text } if text == "hello there")
        );
    }

    #[tokio::test]
    async fn headless_session_can_drive_mock_conversation() {
        let game = test_game();
        let mut session = HeadlessSession::new(game);

        let output = session.execute_line("say 0 hello").await.unwrap().unwrap();

        assert!(output.text.contains("Events:"));
        assert!(output.text.contains("says:"));
        assert!(
            session
                .snapshot()
                .context_feed
                .iter()
                .any(|entry| matches!(entry, crate::domain::events::ContextEntry::Dialogue(_)))
        );
    }

    #[tokio::test]
    async fn focus_command_switches_snapshot_actor() {
        let game = test_game();
        let mut session = HeadlessSession::new(game);
        let target_id = super::talk_targets(&session.snapshot())[0].id;

        session
            .execute_line(&format!("focus {}", target_id.index()))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(session.snapshot().focused_actor_id, target_id);
    }
}
