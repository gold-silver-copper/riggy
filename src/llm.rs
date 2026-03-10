use std::fmt;

use anyhow::Result;
use rig::client::{CompletionClient, Nothing, ProviderClient};
use rig::completion::Chat;
use rig::extractor::ExtractionError;
use rig::message::Message;
use rig::providers::{ollama, openai};

use crate::ai::context::{DialogueTranscriptSpeakerV1, NpcDialogueContextV1};
use crate::ai::prompting::{build_dialogue_prompt_v1, build_proposal_prompt_v1};
use crate::ai::proposals::{AiProposal, ProposedProposals, RelationshipAdjustmentProposal};
use crate::domain::relationship::RelationshipMemory;
use crate::simulation::{DialogueLine, DialogueSession, Speaker};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogueResponse {
    pub text: String,
    pub proposals: Vec<AiProposal>,
}

#[allow(async_fn_in_trait)]
pub trait LlmBackend {
    async fn generate_dialogue(&self, context: &NpcDialogueContextV1) -> Result<DialogueResponse>;

    async fn summarize_memory(&self, session: &DialogueSession) -> Result<RelationshipMemory>;

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
    async fn generate_dialogue(&self, context: &NpcDialogueContextV1) -> Result<DialogueResponse> {
        match self {
            Self::Mock(backend) => backend.generate_dialogue(context).await,
            Self::Rig(backend) => backend.generate_dialogue(context).await,
        }
    }

    async fn summarize_memory(&self, session: &DialogueSession) -> Result<RelationshipMemory> {
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
    async fn generate_dialogue(&self, context: &NpcDialogueContextV1) -> Result<DialogueResponse> {
        let lower = context.turn.player_input.to_lowercase();
        let mut proposals = Vec::new();
        let mut lines = vec![format!(
            "{} the {} leans in, measuring your tone before answering.",
            context.npc.name,
            context.npc.occupation.label()
        )];

        if lower.contains("job") || lower.contains("work") || lower.contains("favor") {
            let landmark = context
                .city
                .landmarks
                .first()
                .cloned()
                .unwrap_or_else(|| "the transit station".to_string());
            lines.push(format!(
                "\"I might have something for you if you're reliable. Check around {} and see whether anything looks out of place.\"",
                landmark
            ));
            proposals.push(AiProposal::RelationshipAdjustment(
                RelationshipAdjustmentProposal {
                    delta: 1,
                    note: "Opened up about local work".to_string(),
                },
            ));
        } else if lower.contains("where") || lower.contains("city") || lower.contains("travel") {
            lines.push(format!(
                "\"{} is a {} place built on {} and {}. I keep mostly to {}. From here you can push on toward {} if you've got a reason.\"",
                context.city.name,
                context.city.biome.label(),
                context.city.economy.label(),
                context.city.culture.label(),
                context.npc.home_district,
                if context.city.connected_cities.is_empty() {
                    "nowhere worth naming".to_string()
                } else {
                    context.city.connected_cities.join(", ")
                }
            ));
        } else {
            lines.push(format!(
                "\"You don't sound like most people passing through {}. That can be useful or it can get noticed. I spend most of my time around {}, so I hear things early.\"",
                context.city.name, context.npc.home_district
            ));
            if context.relationship.disposition < 2 {
                proposals.push(AiProposal::RelationshipAdjustment(
                    RelationshipAdjustmentProposal {
                        delta: 1,
                        note: "Held a decent conversation".to_string(),
                    },
                ));
            }
        }

        Ok(DialogueResponse {
            text: lines.join(" "),
            proposals,
        })
    }

    async fn summarize_memory(&self, session: &DialogueSession) -> Result<RelationshipMemory> {
        let summary = session
            .transcript
            .iter()
            .rev()
            .take(4)
            .rev()
            .map(|line| format!("{}: {}", speaker_label(line), line.text))
            .collect::<Vec<_>>()
            .join(" | ");
        Ok(RelationshipMemory {
            trust_delta_summary: infer_mock_trust_delta(session),
            known_topics: infer_mock_known_topics(session),
            unresolved_threads: infer_mock_unresolved_threads(session),
            freeform_summary: if summary.is_empty() {
                "No memorable conversation yet.".to_string()
            } else {
                summary
            },
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

    async fn prompt_text(&self, context: &NpcDialogueContextV1) -> Result<String> {
        let history = context
            .turn
            .transcript
            .iter()
            .map(|line| match line.speaker {
                DialogueTranscriptSpeakerV1::Player => Message::user(line.text.clone()),
                DialogueTranscriptSpeakerV1::Npc | DialogueTranscriptSpeakerV1::System => {
                    Message::assistant(line.text.clone())
                }
            })
            .collect::<Vec<_>>();
        let prompt = build_dialogue_prompt_v1(context);

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

    async fn extract_proposals(
        &self,
        context: &NpcDialogueContextV1,
        text: &str,
    ) -> Result<Vec<AiProposal>> {
        let extraction_prompt = build_proposal_prompt_v1(context, text);
        let parsed: Result<ProposedProposals> = match &self.provider {
            RigProvider::Ollama { client, model } => client
                .extractor::<ProposedProposals>(model.clone())
                .build()
                .extract(extraction_prompt)
                .await
                .map_err(|err| anyhow::anyhow!(err.to_string())),
            RigProvider::OpenAiCompatible { client, model } => {
                let agent = client
                    .agent(model.clone())
                    .preamble(ACTION_PREAMBLE)
                    .temperature(0.1)
                    .output_schema::<ProposedProposals>()
                    .build();
                rig::prelude::TypedPrompt::prompt_typed::<ProposedProposals>(
                    &agent,
                    extraction_prompt,
                )
                .await
                .map_err(|err| anyhow::anyhow!(err.to_string()))
            }
        };

        match parsed {
            Ok(proposals) => Ok(proposals.proposals),
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
    async fn generate_dialogue(&self, context: &NpcDialogueContextV1) -> Result<DialogueResponse> {
        let text = self.prompt_text(context).await?;
        let proposals = self
            .extract_proposals(context, &text)
            .await
            .unwrap_or_default();
        Ok(DialogueResponse { text, proposals })
    }

    async fn summarize_memory(&self, session: &DialogueSession) -> Result<RelationshipMemory> {
        let transcript = session
            .transcript
            .iter()
            .map(|line| format!("{}: {}", speaker_label(line), line.text))
            .collect::<Vec<_>>()
            .join("\n");
        let prompt = format!(
            "Return structured relationship memory from this conversation.\nFields:\n- trust_delta_summary: integer describing overall trust movement during the conversation\n- known_topics: short durable topics the NPC and player discussed\n- unresolved_threads: short leads, promises, or follow-ups left open\n- freeform_summary: 1-2 sentence durable summary\n\nConversation:\n{}",
            transcript
        );

        let parsed: Result<RelationshipMemory> = match &self.provider {
            RigProvider::Ollama { client, model } => client
                .extractor::<RelationshipMemory>(model.clone())
                .build()
                .extract(prompt)
                .await
                .map_err(|err| anyhow::anyhow!(err.to_string())),
            RigProvider::OpenAiCompatible { client, model } => {
                let agent = client
                    .agent(model.clone())
                    .preamble(MEMORY_PREAMBLE)
                    .temperature(0.1)
                    .output_schema::<RelationshipMemory>()
                    .build();
                rig::prelude::TypedPrompt::prompt_typed::<RelationshipMemory>(&agent, prompt)
                    .await
                    .map_err(|err| anyhow::anyhow!(err.to_string()))
            }
        };

        let memory = parsed?;
        Ok(memory.normalized())
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

const DIALOGUE_PREAMBLE: &str = "You are roleplaying a resident of a procedurally generated city in a turn-based text game. Speak in first person as the NPC, stay consistent with the provided setting and personal motive, and do not narrate as a game master.";
const ACTION_PREAMBLE: &str = "Convert NPC dialogue into conservative structured AI proposals. If nothing durable changes, return an empty proposals list or a no_change proposal.";
const MEMORY_PREAMBLE: &str = "Summarize conversations for a text game into structured relationship memory. Keep only durable topics, unresolved threads, and a conservative trust shift summary.";

fn infer_mock_trust_delta(session: &DialogueSession) -> i32 {
    let transcript = session
        .transcript
        .iter()
        .map(|line| line.text.to_lowercase())
        .collect::<Vec<_>>();
    if transcript
        .iter()
        .any(|line| line.contains("job") || line.contains("work") || line.contains("favor"))
    {
        1
    } else {
        0
    }
}

fn infer_mock_known_topics(session: &DialogueSession) -> Vec<String> {
    let transcript = session
        .transcript
        .iter()
        .map(|line| line.text.to_lowercase())
        .collect::<Vec<_>>();
    let mut topics = Vec::new();
    if transcript
        .iter()
        .any(|line| line.contains("city") || line.contains("travel") || line.contains("where"))
    {
        topics.push("city layout".to_string());
    }
    if transcript
        .iter()
        .any(|line| line.contains("job") || line.contains("work") || line.contains("favor"))
    {
        topics.push("local work".to_string());
    }
    topics
}

fn infer_mock_unresolved_threads(session: &DialogueSession) -> Vec<String> {
    let transcript = session
        .transcript
        .iter()
        .map(|line| line.text.to_lowercase())
        .collect::<Vec<_>>();
    let mut threads = Vec::new();
    if transcript
        .iter()
        .any(|line| line.contains("job") || line.contains("work") || line.contains("favor"))
    {
        threads.push("Follow up on possible local work".to_string());
    }
    threads
}

#[cfg(test)]
mod tests {
    use crate::ai::context::build_npc_dialogue_context_v1;
    use crate::ai::proposals::AiProposal;
    use crate::domain::relationship::RelationshipMemory;
    use crate::llm::{LlmBackend, MockBackend};
    use crate::simulation::{DialogueLine, DialogueSession, RelationshipState, Speaker};
    use crate::world::World;

    #[tokio::test]
    async fn mock_backend_updates_relationship_on_normal_conversation() {
        let world = World::generate(2, 16);
        let city_id = world.city_ids()[0];
        let npc_id = world.city_npcs(city_id)[0];
        let relationship = RelationshipState {
            disposition: 0,
            memory: RelationshipMemory::default(),
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
        let context = build_npc_dialogue_context_v1(
            &world,
            0,
            city_id,
            &relationship,
            &session,
            "Hello there.".to_string(),
        )
        .unwrap();

        let response = MockBackend.generate_dialogue(&context).await.unwrap();
        assert!(
            response
                .proposals
                .iter()
                .any(|proposal| matches!(proposal, AiProposal::RelationshipAdjustment(_)))
        );
    }

    #[test]
    fn context_contains_npc_and_city_state() {
        let world = World::generate(9, 16);
        let city_id = world.city_ids()[0];
        let npc_id = world.city_npcs(city_id)[0];
        let relationship = RelationshipState {
            disposition: 2,
            memory: RelationshipMemory {
                trust_delta_summary: 1,
                known_topics: vec!["follow-through".to_string()],
                unresolved_threads: Vec::new(),
                freeform_summary: "The player kept their word once before.".to_string(),
            },
            last_interaction_at: 3,
        };
        let session = DialogueSession {
            npc_id,
            started_at: 4,
            transcript: Vec::new(),
        };

        let context = build_npc_dialogue_context_v1(
            &world,
            34,
            city_id,
            &relationship,
            &session,
            "What is this city like?".to_string(),
        )
        .unwrap();

        assert_eq!(context.npc.name, world.npc(npc_id).name);
        assert_eq!(context.npc.home_district, world.npc(npc_id).home_district);
        assert_eq!(context.city.name, world.city(city_id).name);
        assert_eq!(context.city.economy, world.city(city_id).economy);
        assert!(!context.city.districts.is_empty());
        assert!(!context.city.connected_cities.is_empty());
        assert_eq!(
            context.relationship.freeform_summary,
            relationship.memory.freeform_summary
        );
    }
}
