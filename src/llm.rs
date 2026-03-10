use std::fmt;

use anyhow::Result;
use rig::client::{CompletionClient, Nothing, ProviderClient};
use rig::completion::{Chat, Prompt};
use rig::extractor::ExtractionError;
use rig::message::Message;
use rig::providers::{ollama, openai};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::domain::vocab::{Biome, Culture, Economy, GoalTag, NpcArchetype, Occupation, TraitTag};
use crate::simulation::{DialogueLine, DialogueSession, RelationshipState, Speaker};
use crate::world::{City, CityId, Npc, World};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DialogueRequest {
    pub world_seed: u64,
    pub current_time_seconds: u64,
    pub current_time_label: String,
    pub city_name: String,
    pub city_biome: Biome,
    pub city_economy: Economy,
    pub city_culture: Culture,
    pub city_districts: Vec<String>,
    pub city_landmarks: Vec<String>,
    pub connected_cities: Vec<String>,
    pub npc_name: String,
    pub npc_archetype: NpcArchetype,
    pub npc_occupation: Occupation,
    pub npc_traits: Vec<TraitTag>,
    pub npc_goal: GoalTag,
    pub npc_home_district: String,
    pub relationship_disposition: i32,
    pub relationship_memory: String,
    pub transcript: Vec<DialogueLine>,
    pub player_input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DialogueResponse {
    pub text: String,
    pub actions: Vec<WorldAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct ProposedActions {
    #[serde(default)]
    pub actions: Vec<WorldAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorldAction {
    UpdateRelationship { delta: i32, note: String },
}

#[allow(async_fn_in_trait)]
pub trait LlmBackend {
    async fn generate_dialogue(&self, request: &DialogueRequest) -> Result<DialogueResponse>;

    async fn summarize_memory(&self, session: &DialogueSession) -> Result<String>;

    fn name(&self) -> &'static str;
}

#[derive(Debug, Clone)]
pub enum AnyBackend {
    Mock(MockBackend),
    Rig(RigBackend),
}

impl AnyBackend {
    pub fn from_env() -> Result<Self> {
        if std::env::var("OLLAMA_MODEL").is_ok()
            || std::env::var("OPENAI_API_KEY").is_ok()
            || std::env::var("OPENAI_BASE_URL").is_ok()
        {
            Ok(Self::Rig(RigBackend::from_env()?))
        } else {
            Ok(Self::Mock(MockBackend))
        }
    }
}

impl LlmBackend for AnyBackend {
    async fn generate_dialogue(&self, request: &DialogueRequest) -> Result<DialogueResponse> {
        match self {
            Self::Mock(backend) => backend.generate_dialogue(request).await,
            Self::Rig(backend) => backend.generate_dialogue(request).await,
        }
    }

    async fn summarize_memory(&self, session: &DialogueSession) -> Result<String> {
        match self {
            Self::Mock(backend) => backend.summarize_memory(session).await,
            Self::Rig(backend) => backend.summarize_memory(session).await,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Mock(backend) => backend.name(),
            Self::Rig(backend) => backend.name(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MockBackend;

impl LlmBackend for MockBackend {
    async fn generate_dialogue(&self, request: &DialogueRequest) -> Result<DialogueResponse> {
        let lower = request.player_input.to_lowercase();
        let mut actions = Vec::new();
        let mut lines = vec![format!(
            "{} the {} leans in, measuring your tone before answering.",
            request.npc_name,
            request.npc_occupation.label()
        )];

        if lower.contains("job") || lower.contains("work") || lower.contains("favor") {
            let landmark = request
                .city_landmarks
                .first()
                .cloned()
                .unwrap_or_else(|| "the transit station".to_string());
            lines.push(format!(
                "\"I might have something for you if you're reliable. Check around {} and see whether anything looks out of place.\"",
                landmark
            ));
            actions.push(WorldAction::UpdateRelationship {
                delta: 1,
                note: "Opened up about local work".to_string(),
            });
        } else if lower.contains("where") || lower.contains("city") || lower.contains("travel") {
            lines.push(format!(
                "\"{} is a {} place built on {} and {}. I keep mostly to {}. From here you can push on toward {} if you've got a reason.\"",
                request.city_name,
                request.city_biome.label(),
                request.city_economy.label(),
                request.city_culture.label(),
                request.npc_home_district,
                if request.connected_cities.is_empty() {
                    "nowhere worth naming".to_string()
                } else {
                    request.connected_cities.join(", ")
                }
            ));
        } else {
            lines.push(format!(
                "\"You don't sound like most people passing through {}. That can be useful or it can get noticed. I spend most of my time around {}, so I hear things early.\"",
                request.city_name, request.npc_home_district
            ));
            if request.relationship_disposition < 2 {
                actions.push(WorldAction::UpdateRelationship {
                    delta: 1,
                    note: "Held a decent conversation".to_string(),
                });
            }
        }

        Ok(DialogueResponse {
            text: lines.join(" "),
            actions,
        })
    }

    async fn summarize_memory(&self, session: &DialogueSession) -> Result<String> {
        let summary = session
            .transcript
            .iter()
            .rev()
            .take(4)
            .rev()
            .map(|line| format!("{}: {}", speaker_label(line), line.text))
            .collect::<Vec<_>>()
            .join(" | ");
        Ok(if summary.is_empty() {
            "No memorable conversation yet.".to_string()
        } else {
            summary
        })
    }

    fn name(&self) -> &'static str {
        "mock"
    }
}

#[derive(Clone)]
pub struct RigBackend {
    provider: RigProvider,
}

#[derive(Clone)]
enum RigProvider {
    Ollama {
        client: ollama::Client,
        model: String,
    },
    OpenAiCompatible {
        client: openai::Client,
        model: String,
    },
}

impl fmt::Debug for RigBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.provider {
            RigProvider::Ollama { model, .. } => f
                .debug_struct("RigBackend")
                .field("provider", &"ollama")
                .field("model", model)
                .finish(),
            RigProvider::OpenAiCompatible { model, .. } => f
                .debug_struct("RigBackend")
                .field("provider", &"openai-compatible")
                .field("model", model)
                .finish(),
        }
    }
}

impl RigBackend {
    pub fn from_env() -> Result<Self> {
        if let Ok(model) = std::env::var("OLLAMA_MODEL") {
            let client = if std::env::var("OLLAMA_API_BASE_URL").is_ok() {
                ollama::Client::from_env()
            } else {
                ollama::Client::new(Nothing)?
            };
            return Ok(Self {
                provider: RigProvider::Ollama { client, model },
            });
        }

        let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
        let client = openai::Client::from_env();
        Ok(Self {
            provider: RigProvider::OpenAiCompatible { client, model },
        })
    }

    async fn prompt_text(&self, request: &DialogueRequest) -> Result<String> {
        let history = request
            .transcript
            .iter()
            .map(|line| match line.speaker {
                Speaker::Player => Message::user(line.text.clone()),
                Speaker::Npc(_) | Speaker::System => Message::assistant(line.text.clone()),
            })
            .collect::<Vec<_>>();
        let prompt = build_dialogue_prompt(request);

        match &self.provider {
            RigProvider::Ollama { client, model } => {
                let agent = client
                    .agent(model.clone())
                    .preamble(DIALOGUE_PREAMBLE)
                    .temperature(0.8)
                    .build();
                Ok(agent.chat(prompt, history).await?)
            }
            RigProvider::OpenAiCompatible { client, model } => {
                let agent = client
                    .agent(model.clone())
                    .preamble(DIALOGUE_PREAMBLE)
                    .temperature(0.8)
                    .build();
                Ok(agent.chat(prompt, history).await?)
            }
        }
    }

    async fn extract_actions(
        &self,
        request: &DialogueRequest,
        text: &str,
    ) -> Result<Vec<WorldAction>> {
        let extraction_prompt = build_action_prompt(request, text);
        let parsed: Result<ProposedActions> = match &self.provider {
            RigProvider::Ollama { client, model } => client
                .extractor::<ProposedActions>(model.clone())
                .build()
                .extract(extraction_prompt)
                .await
                .map_err(|err| anyhow::anyhow!(err.to_string())),
            RigProvider::OpenAiCompatible { client, model } => {
                let agent = client
                    .agent(model.clone())
                    .preamble(ACTION_PREAMBLE)
                    .temperature(0.1)
                    .output_schema::<ProposedActions>()
                    .build();
                rig::prelude::TypedPrompt::prompt_typed::<ProposedActions>(
                    &agent,
                    extraction_prompt,
                )
                .await
                .map_err(|err| anyhow::anyhow!(err.to_string()))
            }
        };

        match parsed {
            Ok(actions) => Ok(actions.actions),
            Err(err) => {
                if let Some(extraction_error) = err.downcast_ref::<ExtractionError>() {
                    return Err(anyhow::anyhow!(extraction_error.to_string()));
                }
                Ok(Vec::new())
            }
        }
    }
}

impl LlmBackend for RigBackend {
    async fn generate_dialogue(&self, request: &DialogueRequest) -> Result<DialogueResponse> {
        let text = self.prompt_text(request).await?;
        let actions = self
            .extract_actions(request, &text)
            .await
            .unwrap_or_default();
        Ok(DialogueResponse { text, actions })
    }

    async fn summarize_memory(&self, session: &DialogueSession) -> Result<String> {
        let transcript = session
            .transcript
            .iter()
            .map(|line| format!("{}: {}", speaker_label(line), line.text))
            .collect::<Vec<_>>()
            .join("\n");
        let prompt = format!(
            "Summarize the important facts, trust shifts, and unresolved leads from this conversation in 2 sentences.\n\n{}",
            transcript
        );

        match &self.provider {
            RigProvider::Ollama { client, model } => {
                let agent = client
                    .agent(model.clone())
                    .preamble("Summarize conversations for a text game. Keep only durable facts.")
                    .temperature(0.2)
                    .build();
                Ok(agent.prompt(prompt).await?)
            }
            RigProvider::OpenAiCompatible { client, model } => {
                let agent = client
                    .agent(model.clone())
                    .preamble("Summarize conversations for a text game. Keep only durable facts.")
                    .temperature(0.2)
                    .build();
                Ok(agent.prompt(prompt).await?)
            }
        }
    }

    fn name(&self) -> &'static str {
        match self.provider {
            RigProvider::Ollama { .. } => "rig/ollama",
            RigProvider::OpenAiCompatible { .. } => "rig/openai-compatible",
        }
    }
}

fn speaker_label(line: &DialogueLine) -> String {
    match line.speaker {
        Speaker::Player => "You".to_string(),
        Speaker::Npc(_) => "NPC".to_string(),
        Speaker::System => "System".to_string(),
    }
}

pub fn build_request(
    world: &World,
    city_id: CityId,
    city: &City,
    npc: &Npc,
    relationship: &RelationshipState,
    session: &DialogueSession,
    player_input: String,
) -> DialogueRequest {
    DialogueRequest {
        world_seed: world.seed,
        current_time_seconds: session.started_at + session.transcript.len() as u64 * 30,
        current_time_label: format_clock_label(
            session.started_at + session.transcript.len() as u64 * 30,
        ),
        city_name: city.name.clone(),
        city_biome: city.biome,
        city_economy: city.economy,
        city_culture: city.culture,
        city_districts: city
            .districts
            .iter()
            .map(|district| district.name.clone())
            .collect(),
        city_landmarks: city.landmarks.clone(),
        connected_cities: world
            .city_connections(city_id)
            .iter()
            .map(|city_id| world.city(*city_id).name.clone())
            .collect(),
        npc_name: npc.name.clone(),
        npc_archetype: npc.archetype,
        npc_occupation: npc.occupation,
        npc_traits: npc.personality_traits.clone(),
        npc_goal: npc.goal,
        npc_home_district: npc.home_district.clone(),
        relationship_disposition: relationship.disposition,
        relationship_memory: relationship.memory_summary.clone(),
        transcript: session.transcript.clone(),
        player_input,
    }
}

fn build_dialogue_prompt(request: &DialogueRequest) -> String {
    format!(
        "World seed: {world_seed}\nTime: {time_label} ({time_seconds} seconds)\nCity: {city} ({biome}, {economy}, {culture})\nDistricts: {districts}\nLandmarks: {landmarks}\nConnected cities: {connected_cities}\nNPC: {npc}, a {occupation} and {archetype}\nHome district: {home_district}\nTraits: {traits}\nGoal: {goal}\nDisposition toward player: {disposition}\nRelationship memory: {memory}\n\nPlayer says: {player_input}\n\nReply as the NPC in 2-4 sentences. Stay grounded in the city and the NPC's motives. Refer only to facts present in this context or naturally implied by them.",
        world_seed = request.world_seed,
        time_label = request.current_time_label,
        time_seconds = request.current_time_seconds,
        city = request.city_name,
        biome = request.city_biome.label(),
        economy = request.city_economy.label(),
        culture = request.city_culture.label(),
        districts = if request.city_districts.is_empty() {
            "none".to_string()
        } else {
            request.city_districts.join(", ")
        },
        landmarks = request.city_landmarks.join(", "),
        connected_cities = if request.connected_cities.is_empty() {
            "none".to_string()
        } else {
            request.connected_cities.join(", ")
        },
        npc = request.npc_name,
        occupation = request.npc_occupation.label(),
        archetype = request.npc_archetype.label(),
        home_district = request.npc_home_district,
        traits = request
            .npc_traits
            .iter()
            .map(|trait_tag| trait_tag.label())
            .collect::<Vec<_>>()
            .join(", "),
        goal = request.npc_goal.label(),
        disposition = request.relationship_disposition,
        memory = if request.relationship_memory.trim().is_empty() {
            "none".to_string()
        } else {
            request.relationship_memory.clone()
        },
        player_input = request.player_input
    )
}

fn format_clock_label(total_seconds: u64) -> String {
    let seconds_per_day = 24 * 60 * 60;
    let day = total_seconds / seconds_per_day + 1;
    let seconds_in_day = total_seconds % seconds_per_day;
    let hours = seconds_in_day / 3600;
    let minutes = (seconds_in_day % 3600) / 60;
    let seconds = seconds_in_day % 60;
    format!("Day {} {:02}:{:02}:{:02}", day, hours, minutes, seconds)
}

fn build_action_prompt(request: &DialogueRequest, text: &str) -> String {
    format!(
        "You are extracting game state updates from an NPC response.\nAllowed actions: update_relationship.\nOnly emit actions justified by the NPC response and keep them conservative.\n\nCity: {}\nNPC: {}\nPlayer input: {}\nNPC response: {}",
        request.city_name, request.npc_name, request.player_input, text
    )
}

const DIALOGUE_PREAMBLE: &str = "You are roleplaying a resident of a procedurally generated city in a turn-based text game. Speak in first person as the NPC, stay consistent with the provided setting and personal motive, and do not narrate as a game master.";
const ACTION_PREAMBLE: &str = "Convert NPC dialogue into conservative structured game actions. If nothing durable changes, return an empty actions list.";

#[cfg(test)]
mod tests {
    use super::{LlmBackend, MockBackend, WorldAction, build_request};
    use crate::simulation::{DialogueLine, DialogueSession, RelationshipState, Speaker};
    use crate::world::World;

    #[tokio::test]
    async fn mock_backend_updates_relationship_on_normal_conversation() {
        let world = World::generate(2, 16);
        let city_id = world.city_ids()[0];
        let city = world.city(city_id);
        let npc_id = world.city_npcs(city_id)[0];
        let npc = world.npc(npc_id);
        let relationship = RelationshipState {
            disposition: 0,
            memory_summary: String::new(),
            last_interaction_at: 0,
        };
        let session = DialogueSession {
            npc_id,
            started_at: 0,
            transcript: vec![DialogueLine {
                speaker: Speaker::Player,
                text: "Hello".to_string(),
            }],
        };
        let request = build_request(
            &world,
            city_id,
            city,
            npc,
            &relationship,
            &session,
            "Hello there.".to_string(),
        );

        let response = MockBackend.generate_dialogue(&request).await.unwrap();
        assert!(
            response
                .actions
                .iter()
                .any(|action| matches!(action, WorldAction::UpdateRelationship { .. }))
        );
    }

    #[test]
    fn request_contains_npc_and_city_context() {
        let world = World::generate(9, 16);
        let city_id = world.city_ids()[0];
        let city = world.city(city_id);
        let npc_id = world.city_npcs(city_id)[0];
        let npc = world.npc(npc_id);
        let relationship = RelationshipState {
            disposition: 2,
            memory_summary: "The player kept their word once before.".to_string(),
            last_interaction_at: 3,
        };
        let session = DialogueSession {
            npc_id,
            started_at: 4,
            transcript: Vec::new(),
        };

        let request = build_request(
            &world,
            city_id,
            city,
            npc,
            &relationship,
            &session,
            "What is this city like?".to_string(),
        );

        assert_eq!(request.npc_name, npc.name);
        assert_eq!(request.npc_home_district, npc.home_district);
        assert_eq!(request.city_name, city.name);
        assert_eq!(request.city_economy, city.economy);
        assert!(!request.city_districts.is_empty());
        assert!(!request.connected_cities.is_empty());
        assert_eq!(request.relationship_memory, relationship.memory_summary);
    }
}
