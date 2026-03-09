use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::cli;
use crate::llm::{DialogueResponse, LlmBackend, WorldAction, build_request};
use crate::world::{CityId, NpcId, World};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameState {
    pub world: World,
    pub turn: u32,
    pub player_city_id: CityId,
    pub known_city_ids: Vec<CityId>,
    pub relationships: BTreeMap<NpcId, RelationshipState>,
    pub journal: Vec<PlayerJournalEntry>,
    pub active_dialogue: Option<DialogueSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelationshipState {
    pub disposition: i32,
    pub memory_summary: String,
    pub last_interaction_turn: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlayerJournalEntry {
    pub turn: u32,
    pub category: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DialogueSession {
    pub npc_id: NpcId,
    pub started_turn: u32,
    pub transcript: Vec<DialogueLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DialogueLine {
    pub speaker: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub text: String,
    pub should_quit: bool,
}

#[derive(Debug)]
pub struct Game<B> {
    state: GameState,
    backend: B,
}

impl<B: LlmBackend> Game<B> {
    pub fn new(backend: B) -> Self {
        let seed = 42;
        let world = World::generate(seed, 18);
        let start_city_id = 0;
        let known_city_ids = {
            let mut ids = vec![start_city_id];
            ids.extend(world.city(start_city_id).connected_city_ids.iter().copied());
            ids.sort_unstable();
            ids.dedup();
            ids
        };

        Self {
            state: GameState {
                world,
                turn: 0,
                player_city_id: start_city_id,
                known_city_ids,
                relationships: BTreeMap::new(),
                journal: vec![PlayerJournalEntry {
                    turn: 0,
                    category: "start".to_string(),
                    text: "You arrived with a blank journal and a need for useful names."
                        .to_string(),
                }],
                active_dialogue: None,
            },
            backend,
        }
    }

    pub fn backend_name(&self) -> &'static str {
        self.backend.name()
    }

    pub fn dialogue_partner_name(&self) -> Option<&str> {
        self.state
            .active_dialogue
            .as_ref()
            .map(|session| self.state.world.npc(session.npc_id).name.as_str())
    }

    pub fn render_location_summary(&self) -> String {
        let city = self.current_city();
        format!(
            "You are in {}. {} city, {} economy, {} culture.\nConnected cities: {}\nType `look`, `people`, or `travel`.",
            city.name,
            city.biome,
            city.economy,
            city.culture,
            city.connected_city_ids
                .iter()
                .map(|id| self.state.world.city(*id).name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    pub async fn handle_input(&mut self, input: &str) -> Result<CommandOutput> {
        if self.state.active_dialogue.is_some() {
            return self.handle_dialogue_input(input).await;
        }

        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(CommandOutput {
                text: String::new(),
                should_quit: false,
            });
        }

        let (command, rest) = split_command(trimmed);
        let output = match command {
            "help" => self.help_text(),
            "look" => self.look(),
            "where" => self.where_am_i(),
            "travel" => self.travel(rest)?,
            "people" => self.people(),
            "talk" => self.start_dialogue(rest)?,
            "ask" => {
                "Use `talk <name>` first, then type natural language in dialogue mode.".to_string()
            }
            "journal" => self.journal(),
            "wait" => self.wait_turn(),
            "save" => {
                let path = normalize_save_path(rest);
                self.save(path.as_path())?;
                format!("Saved game to {}.", path.display())
            }
            "load" => {
                let path = normalize_save_path(rest);
                self.load(path.as_path())?;
                format!(
                    "Loaded game from {}.\n{}",
                    path.display(),
                    self.render_location_summary()
                )
            }
            "quit" | "exit" => {
                return Ok(CommandOutput {
                    text: "Goodbye.".to_string(),
                    should_quit: true,
                });
            }
            _ => format!("Unknown command `{}`. Type `help`.", command),
        };

        Ok(CommandOutput {
            text: output,
            should_quit: false,
        })
    }

    fn help_text(&self) -> String {
        format!(
            "Commands:\n  help\n  look\n  where\n  travel [city]\n  people\n  talk <npc>\n  journal\n  wait\n  save [path]\n  load [path]\n  quit\n\nDialogue mode commands:\n  /leave\n  /people\n  /repeat\n\nBackend: {}",
            self.backend.name()
        )
    }

    fn look(&self) -> String {
        let city = self.current_city();
        let districts = city
            .districts
            .iter()
            .map(|district| format!("{} ({})", district.name, district.description))
            .collect::<Vec<_>>()
            .join("; ");
        let landmarks = city.landmarks.join(", ");
        format!(
            "{} is a {} city shaped by {} and {}.\nLandmarks: {}\nDistricts: {}",
            city.name, city.biome, city.economy, city.culture, landmarks, districts
        )
    }

    fn where_am_i(&self) -> String {
        let city = self.current_city();
        format!("Turn {}. You are in {}.", self.state.turn, city.name)
    }

    fn travel(&mut self, rest: &str) -> Result<String> {
        let city = self.current_city();
        if rest.is_empty() {
            let connections = city
                .connected_city_ids
                .iter()
                .map(|id| self.state.world.city(*id).name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            return Ok(format!("Connected destinations: {}", connections));
        }

        let destination_id = self
            .match_city(rest)
            .ok_or_else(|| anyhow::anyhow!("No connected city matched `{}`.", rest))?;
        if !city.connected_city_ids.contains(&destination_id) {
            bail!("You can only travel to directly connected cities from here.");
        }

        self.state.player_city_id = destination_id;
        self.advance_turns(2);
        self.learn_city(destination_id);
        let destination_name = self.current_city().name.clone();
        self.state.journal.push(PlayerJournalEntry {
            turn: self.state.turn,
            category: "travel".to_string(),
            text: format!("Arrived in {}.", destination_name),
        });
        Ok(format!(
            "You travel to {}.\n{}",
            destination_name,
            self.render_location_summary()
        ))
    }

    fn people(&self) -> String {
        let city = self.current_city();
        let people = city
            .npc_ids
            .iter()
            .map(|npc_id| {
                let npc = self.state.world.npc(*npc_id);
                let disposition = self.relationship(*npc_id).disposition;
                format!(
                    "{} - {}, {} ({})",
                    npc.name, npc.occupation, npc.archetype, disposition
                )
            })
            .collect::<Vec<_>>();
        format!("People in {}:\n{}", city.name, people.join("\n"))
    }

    fn start_dialogue(&mut self, rest: &str) -> Result<String> {
        if rest.is_empty() {
            bail!("Use `talk <npc>`.");
        }
        let npc_id = self
            .match_npc_in_city(rest)
            .ok_or_else(|| anyhow::anyhow!("No one here matched `{}`.", rest))?;
        self.state.active_dialogue = Some(DialogueSession {
            npc_id,
            started_turn: self.state.turn,
            transcript: vec![DialogueLine {
                speaker: self.state.world.npc(npc_id).name.clone(),
                text: format!(
                    "What do you want to know about {}?",
                    self.current_city().name
                ),
            }],
        });
        Ok(format!(
            "You approach {}.\nType normally to speak, or `/leave` to end the conversation.",
            self.state.world.npc(npc_id).name
        ))
    }

    fn journal(&self) -> String {
        let entries = self
            .state
            .journal
            .iter()
            .rev()
            .take(12)
            .map(|entry| format!("[{}] {}: {}", entry.turn, entry.category, entry.text))
            .collect::<Vec<_>>();
        if entries.is_empty() {
            "Journal is empty.".to_string()
        } else {
            format!("Journal:\n{}", entries.join("\n"))
        }
    }

    fn wait_turn(&mut self) -> String {
        self.advance_turns(1);
        "You wait and watch the city shift around you.".to_string()
    }

    async fn handle_dialogue_input(&mut self, input: &str) -> Result<CommandOutput> {
        let trimmed = input.trim();

        match trimmed {
            "/leave" => {
                let session = self.state.active_dialogue.clone().expect("dialogue exists");
                let summary = self.backend.summarize_memory(&session).await?;
                let npc_id = session.npc_id;
                self.relationship_mut(npc_id).memory_summary = summary;
                self.state.active_dialogue = None;
                return Ok(CommandOutput {
                    text: "You step away from the conversation.".to_string(),
                    should_quit: false,
                });
            }
            "/people" => {
                return Ok(CommandOutput {
                    text: self.people(),
                    should_quit: false,
                });
            }
            "/repeat" => {
                let last_line = self
                    .state
                    .active_dialogue
                    .as_ref()
                    .expect("dialogue exists")
                    .transcript
                    .last()
                    .map(|line| format!("{}: {}", line.speaker, line.text))
                    .unwrap_or_else(|| "No dialogue yet.".to_string());
                return Ok(CommandOutput {
                    text: last_line,
                    should_quit: false,
                });
            }
            "" => {
                return Ok(CommandOutput {
                    text: String::new(),
                    should_quit: false,
                });
            }
            _ => {}
        }

        {
            let session = self
                .state
                .active_dialogue
                .as_mut()
                .expect("dialogue exists");
            session.transcript.push(DialogueLine {
                speaker: "You".to_string(),
                text: trimmed.to_string(),
            });
        }

        let session_snapshot = self.state.active_dialogue.clone().expect("dialogue exists");
        let npc_id = session_snapshot.npc_id;
        let city = self.current_city().clone();
        let npc = self.state.world.npc(npc_id).clone();
        let relationship = self.relationship(npc_id).clone();
        let journal_context = self
            .state
            .journal
            .iter()
            .rev()
            .take(5)
            .rev()
            .map(|entry| format!("[{}] {}: {}", entry.turn, entry.category, entry.text))
            .collect::<Vec<_>>();
        let request = build_request(
            &self.state.world,
            &city,
            &npc,
            &relationship,
            journal_context,
            &session_snapshot,
            trimmed.to_string(),
        );
        let streamed = self.backend.stream_dialogue(&request).await?;
        let DialogueResponse { text, actions } = streamed.response;

        {
            let session = self
                .state
                .active_dialogue
                .as_mut()
                .expect("dialogue exists");
            session.transcript.push(DialogueLine {
                speaker: npc.name.clone(),
                text: text.clone(),
            });
        }
        self.advance_turns(1);
        let applied = self.apply_actions(npc_id, actions);

        let mut rendered = String::new();
        for chunk in streamed.chunks {
            rendered.push_str(&chunk);
            rendered.push('\n');
        }
        if !applied.is_empty() {
            rendered.push_str("\nUpdates:\n");
            rendered.push_str(&applied.join("\n"));
        }

        Ok(CommandOutput {
            text: rendered.trim_end().to_string(),
            should_quit: false,
        })
    }

    fn apply_actions(&mut self, npc_id: NpcId, actions: Vec<WorldAction>) -> Vec<String> {
        let mut applied = Vec::new();
        for action in actions {
            match action {
                WorldAction::RevealRumor { text } => {
                    if self
                        .state
                        .journal
                        .iter()
                        .any(|entry| entry.category == "rumor" && entry.text == text)
                    {
                        continue;
                    }
                    self.state.journal.push(PlayerJournalEntry {
                        turn: self.state.turn,
                        category: "rumor".to_string(),
                        text: text.clone(),
                    });
                    applied.push(format!("- Rumor logged: {}", text));
                }
                WorldAction::UpdateRelationship { delta, note } => {
                    let turn = self.state.turn;
                    let npc_name = self.state.world.npc(npc_id).name.clone();
                    let disposition = {
                        let relationship = self.relationship_mut(npc_id);
                        relationship.disposition =
                            (relationship.disposition + delta.clamp(-2, 2)).clamp(-10, 10);
                        relationship.last_interaction_turn = turn;
                        relationship.disposition
                    };
                    if !note.trim().is_empty() {
                        self.state.journal.push(PlayerJournalEntry {
                            turn,
                            category: "relationship".to_string(),
                            text: format!("{}: {}", npc_name, note.trim()),
                        });
                    }
                    applied.push(format!(
                        "- Relationship with {} is now {}.",
                        npc_name, disposition
                    ));
                }
                WorldAction::OfferFavor { summary } => {
                    self.state.journal.push(PlayerJournalEntry {
                        turn: self.state.turn,
                        category: "lead".to_string(),
                        text: summary.clone(),
                    });
                    applied.push(format!("- New lead: {}", summary));
                }
                WorldAction::LearnLocation { city_name } => {
                    if let Some(city_id) = self.match_any_city(&city_name) {
                        self.learn_city(city_id);
                        applied.push(format!(
                            "- Learned about {}.",
                            self.state.world.city(city_id).name
                        ));
                    }
                }
                WorldAction::ReceiveItem { item } => {
                    self.state.journal.push(PlayerJournalEntry {
                        turn: self.state.turn,
                        category: "item".to_string(),
                        text: format!("Received {}.", item),
                    });
                    applied.push(format!("- Received {}.", item));
                }
                WorldAction::ScheduleMeeting {
                    summary,
                    turns_from_now,
                } => {
                    self.state.journal.push(PlayerJournalEntry {
                        turn: self.state.turn,
                        category: "meeting".to_string(),
                        text: format!("In {} turns: {}", turns_from_now.max(1), summary),
                    });
                    applied.push(format!("- Meeting noted: {}", summary));
                }
            }
        }
        applied
    }

    fn advance_turns(&mut self, steps: u32) {
        self.state.turn += steps;
        for relationship in self.state.relationships.values_mut() {
            let idle = self
                .state
                .turn
                .saturating_sub(relationship.last_interaction_turn);
            if idle > 8 && relationship.disposition > 0 {
                relationship.disposition -= 1;
            }
        }
    }

    fn learn_city(&mut self, city_id: CityId) {
        self.state.known_city_ids.push(city_id);
        self.state.known_city_ids.extend(
            self.state
                .world
                .city(city_id)
                .connected_city_ids
                .iter()
                .copied(),
        );
        self.state.known_city_ids.sort_unstable();
        self.state.known_city_ids.dedup();
    }

    fn relationship(&self, npc_id: NpcId) -> &RelationshipState {
        self.state
            .relationships
            .get(&npc_id)
            .unwrap_or(&DEFAULT_RELATIONSHIP)
    }

    fn relationship_mut(&mut self, npc_id: NpcId) -> &mut RelationshipState {
        self.state
            .relationships
            .entry(npc_id)
            .or_insert_with(|| RelationshipState {
                disposition: 0,
                memory_summary: String::new(),
                last_interaction_turn: self.state.turn,
            })
    }

    fn current_city(&self) -> &crate::world::City {
        self.state.world.city(self.state.player_city_id)
    }

    fn match_city(&self, query: &str) -> Option<CityId> {
        let lower = query.to_lowercase();
        self.current_city()
            .connected_city_ids
            .iter()
            .copied()
            .find(|city_id| {
                self.state
                    .world
                    .city(*city_id)
                    .name
                    .to_lowercase()
                    .contains(&lower)
            })
    }

    fn match_any_city(&self, query: &str) -> Option<CityId> {
        let lower = query.to_lowercase();
        self.state
            .world
            .cities
            .iter()
            .find(|city| city.name.to_lowercase().contains(&lower))
            .map(|city| city.id)
    }

    fn match_npc_in_city(&self, query: &str) -> Option<NpcId> {
        let lower = query.to_lowercase();
        self.current_city().npc_ids.iter().copied().find(|npc_id| {
            self.state
                .world
                .npc(*npc_id)
                .name
                .to_lowercase()
                .contains(&lower)
        })
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let data = serde_json::to_string_pretty(&self.state)?;
        fs::write(path, data)?;
        Ok(())
    }

    pub fn load(&mut self, path: &Path) -> Result<()> {
        let data = fs::read_to_string(path)?;
        self.state = serde_json::from_str(&data)?;
        Ok(())
    }
}

fn split_command(input: &str) -> (&str, &str) {
    match input.split_once(' ') {
        Some((command, rest)) => (command, rest.trim()),
        None => (input, ""),
    }
}

fn normalize_save_path(rest: &str) -> std::path::PathBuf {
    if rest.is_empty() {
        cli::default_save_path()
    } else {
        rest.into()
    }
}

static DEFAULT_RELATIONSHIP: RelationshipState = RelationshipState {
    disposition: 0,
    memory_summary: String::new(),
    last_interaction_turn: 0,
};

#[cfg(test)]
mod tests {
    use super::Game;
    use crate::llm::MockBackend;

    #[tokio::test]
    async fn parser_handles_dialogue_escape_commands() {
        let mut game = Game::new(MockBackend);
        let people = game.handle_input("people").await.unwrap();
        let first_name = people
            .text
            .lines()
            .nth(1)
            .and_then(|line| line.split(" - ").next())
            .unwrap()
            .to_string();
        game.handle_input(&format!("talk {}", first_name))
            .await
            .unwrap();
        let repeat = game.handle_input("/repeat").await.unwrap();
        assert!(repeat.text.contains(':'));
        let leave = game.handle_input("/leave").await.unwrap();
        assert!(leave.text.contains("step away"));
    }

    #[tokio::test]
    async fn save_and_load_round_trip() {
        let mut game = Game::new(MockBackend);
        game.handle_input("wait").await.unwrap();
        game.handle_input("save /tmp/riggy-test-save.json")
            .await
            .unwrap();

        let mut loaded = Game::new(MockBackend);
        loaded
            .handle_input("load /tmp/riggy-test-save.json")
            .await
            .unwrap();
        assert_eq!(game.state.turn, loaded.state.turn);
        assert_eq!(game.state.player_city_id, loaded.state.player_city_id);
    }
}
