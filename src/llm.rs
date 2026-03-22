use std::fmt;

use anyhow::Result;
use rig::client::{CompletionClient, Nothing, ProviderClient};
use rig::completion::{Chat, Prompt};
use rig::message::Message;
use rig::providers::{ollama, openai};

use crate::ai::context::ActorDialogueContext;
use crate::ai::prompting::build_dialogue_prompt;
use crate::domain::events::{DialogueLine, DialogueSpeaker};
use crate::domain::memory::ConversationMemory;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogueResponse {
    pub text: String,
}

#[allow(async_fn_in_trait)]
pub trait LlmBackend {
    async fn generate_dialogue(&self, context: &ActorDialogueContext) -> Result<DialogueResponse>;

    async fn summarize_memory(&self, transcript: &[DialogueLine]) -> Result<ConversationMemory>;

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
    async fn generate_dialogue(&self, context: &ActorDialogueContext) -> Result<DialogueResponse> {
        match self {
            Self::Mock(backend) => backend.generate_dialogue(context).await,
            Self::Rig(backend) => backend.generate_dialogue(context).await,
        }
    }

    async fn summarize_memory(&self, transcript: &[DialogueLine]) -> Result<ConversationMemory> {
        match self {
            Self::Mock(backend) => backend.summarize_memory(transcript).await,
            Self::Rig(backend) => backend.summarize_memory(transcript).await,
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
    async fn generate_dialogue(&self, context: &ActorDialogueContext) -> Result<DialogueResponse> {
        let lower = context.turn.speaker_input.to_lowercase();
        let mut lines = vec![format!(
            "{} the {} leans in, measuring your tone before answering.",
            context.actor.name(context.world_seed),
            context.actor.occupation.label()
        )];

        if lower.contains("job") || lower.contains("work") || lower.contains("favor") {
            lines.push(format!(
                "\"I might have something for you if you're reliable. Check around {} and see whether anything looks out of place.\"",
                crate::world::place_name_from_parts(
                    context.world_seed,
                    context.current_place.id,
                    context.current_place.city_id,
                    context.current_place.kind,
                )
            ));
        } else if lower.contains("where") || lower.contains("city") || lower.contains("travel") {
            lines.push(format!(
                "\"{} is a {} place built on {} and {}. I keep mostly to {}. From here you can push on toward {} if you've got a reason.\"",
                context.city.name(context.world_seed),
                context.city.biome.label(),
                context.city.economy.label(),
                context.city.culture.label(),
                context.actor.home_place_name(context.world_seed),
                if context.city.connected_cities.is_empty() {
                    "nowhere worth naming".to_string()
                } else {
                    context
                        .city
                        .connected_cities
                        .iter()
                        .map(|city| city.name(context.world_seed))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ));
        } else {
            lines.push(format!(
                "\"You don't sound like most people passing through {}. That can be useful or it can get noticed. I spend most of my time around {}, so I hear things early.\"",
                context.city.name(context.world_seed),
                context.actor.home_place_name(context.world_seed)
            ));
        }

        Ok(DialogueResponse {
            text: lines.join(" "),
        })
    }

    async fn summarize_memory(&self, transcript: &[DialogueLine]) -> Result<ConversationMemory> {
        Ok(ConversationMemory {
            summary: fallback_summary(transcript),
        }
        .normalized())
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

    async fn prompt_text(&self, context: &ActorDialogueContext) -> Result<String> {
        let history = context
            .turn
            .transcript
            .iter()
            .map(|line| match line.speaker {
                DialogueSpeaker::Actor(actor_id) if actor_id == context.counterpart.id => {
                    Message::user(line.text.clone())
                }
                DialogueSpeaker::Actor(_) | DialogueSpeaker::System => {
                    Message::assistant(line.text.clone())
                }
            })
            .collect::<Vec<_>>();
        let prompt = build_dialogue_prompt(context);

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

    async fn prompt_memory_summary_text(&self, transcript: &str) -> Result<String> {
        let prompt = format!(
            "Summarize what these two actors talked about in 1-2 durable sentences.\n\nConversation:\n{}",
            transcript
        );

        match &self.provider {
            RigProvider::Ollama { client, model } => {
                let agent = client
                    .agent(model.clone())
                    .preamble(MEMORY_PREAMBLE)
                    .temperature(0.2)
                    .build();
                Ok(agent.prompt(prompt).await?)
            }
            RigProvider::OpenAiCompatible { client, model } => {
                let agent = client
                    .agent(model.clone())
                    .preamble(MEMORY_PREAMBLE)
                    .temperature(0.2)
                    .build();
                Ok(agent.prompt(prompt).await?)
            }
        }
    }
}

impl LlmBackend for RigBackend {
    async fn generate_dialogue(&self, context: &ActorDialogueContext) -> Result<DialogueResponse> {
        Ok(DialogueResponse {
            text: self.prompt_text(context).await?,
        })
    }

    async fn summarize_memory(&self, transcript: &[DialogueLine]) -> Result<ConversationMemory> {
        let transcript_text = transcript
            .iter()
            .map(|line| format!("{}: {}", speaker_label(line), line.text))
            .collect::<Vec<_>>()
            .join("\n");

        let summary = self
            .prompt_memory_summary_text(&transcript_text)
            .await
            .unwrap_or_else(|_| fallback_summary(transcript));

        Ok(ConversationMemory { summary }.normalized())
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
        DialogueSpeaker::Actor(_) => "Actor".to_string(),
        DialogueSpeaker::System => "System".to_string(),
    }
}

const DIALOGUE_PREAMBLE: &str = "You are roleplaying a resident of a procedurally generated city in a turn-based text game. Speak in first person as the character, stay consistent with the provided setting and personal motive, and do not narrate as a game master.";
const MEMORY_PREAMBLE: &str =
    "Summarize conversations for a text game. Keep only durable memory of what was discussed.";

fn fallback_summary(transcript: &[DialogueLine]) -> String {
    let summary = transcript
        .iter()
        .rev()
        .take(6)
        .rev()
        .map(|line| format!("{}: {}", speaker_label(line), line.text))
        .collect::<Vec<_>>()
        .join(" | ");

    if summary.is_empty() {
        "No memorable conversation yet.".to_string()
    } else {
        summary
    }
}

#[cfg(test)]
mod tests {
    use crate::ai::context::build_actor_dialogue_context;
    use crate::domain::events::{DialogueLine, DialogueSpeaker};
    use crate::domain::memory::ConversationMemory;
    use crate::domain::time::GameTime;
    use crate::world::World;

    use super::{LlmBackend, MockBackend};

    #[tokio::test]
    async fn mock_backend_generates_dialogue() {
        let mut world = World::generate(crate::domain::seed::WorldSeed::new(2), 16);
        let actor_id = world.manual_actor_id().unwrap();
        let city_id = world.actor_city_id(actor_id).unwrap();
        let place_id = world.actor_place_id(actor_id).unwrap();
        let counterpart_id = world
            .place_actors(place_id)
            .into_iter()
            .find(|candidate| *candidate != actor_id)
            .unwrap_or_else(|| {
                let counterpart_id = world
                    .actor_ids()
                    .into_iter()
                    .find(|candidate| *candidate != actor_id)
                    .unwrap();
                world.move_actor(counterpart_id, place_id);
                counterpart_id
            });
        world.record_speech_process(
            actor_id,
            counterpart_id,
            place_id,
            GameTime::from_seconds(0),
            crate::domain::time::TimeDelta::from_seconds(10),
            vec![DialogueLine {
                timestamp: GameTime::from_seconds(0),
                speaker: DialogueSpeaker::Actor(actor_id),
                text: "Hello".to_string(),
            }],
        );
        let context = build_actor_dialogue_context(
            &world,
            GameTime::from_seconds(0),
            city_id,
            counterpart_id,
            actor_id,
            &ConversationMemory::default(),
            "Hello there.".to_string(),
        )
        .unwrap();

        let response = MockBackend.generate_dialogue(&context).await.unwrap();
        assert!(!response.text.is_empty());
    }
}
