use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::domain::vocab::{Biome, Culture, Economy, GoalTag, NpcArchetype, Occupation, TraitTag};
use crate::simulation::{DialogueSession, RelationshipState, Speaker};
use crate::world::{CityId, World};

pub const NPC_DIALOGUE_CONTEXT_V1_VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NpcDialogueContextV1 {
    pub version: u8,
    pub world_seed: u64,
    pub clock: DialogueClockV1,
    pub city: CityContextV1,
    pub npc: NpcContextV1,
    pub relationship: RelationshipMemoryViewV1,
    pub turn: DialogueTurnContextV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DialogueClockV1 {
    pub current_time_seconds: u64,
    pub current_time_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CityContextV1 {
    pub name: String,
    pub biome: Biome,
    pub economy: Economy,
    pub culture: Culture,
    pub districts: Vec<String>,
    pub landmarks: Vec<String>,
    pub connected_cities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NpcContextV1 {
    pub name: String,
    pub archetype: NpcArchetype,
    pub occupation: Occupation,
    pub traits: Vec<TraitTag>,
    pub goal: GoalTag,
    pub home_district: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelationshipMemoryViewV1 {
    pub disposition: i32,
    pub trust_delta_summary: i32,
    pub known_topics: Vec<String>,
    pub unresolved_threads: Vec<String>,
    pub freeform_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DialogueTurnContextV1 {
    pub transcript: Vec<DialogueTranscriptLineV1>,
    pub player_input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DialogueTranscriptLineV1 {
    pub speaker: DialogueTranscriptSpeakerV1,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DialogueTranscriptSpeakerV1 {
    Player,
    Npc,
    System,
}

pub fn build_npc_dialogue_context_v1(
    world: &World,
    current_time_seconds: u64,
    city_id: CityId,
    relationship: &RelationshipState,
    session: &DialogueSession,
    player_input: String,
) -> Result<NpcDialogueContextV1> {
    if !world.city_ids().contains(&city_id) {
        bail!("dialogue context city does not exist");
    }
    if !world.npc_ids().contains(&session.npc_id) {
        bail!("dialogue context npc does not exist");
    }
    if !world.city_npcs(city_id).contains(&session.npc_id) {
        bail!("dialogue context npc does not belong to the provided city");
    }

    let city = world.city(city_id);
    let npc = world.npc(session.npc_id);

    Ok(NpcDialogueContextV1 {
        version: NPC_DIALOGUE_CONTEXT_V1_VERSION,
        world_seed: world.seed,
        clock: DialogueClockV1 {
            current_time_seconds,
            current_time_label: format_clock_label(current_time_seconds),
        },
        city: CityContextV1 {
            name: city.name.clone(),
            biome: city.biome,
            economy: city.economy,
            culture: city.culture,
            districts: city
                .districts
                .iter()
                .map(|district| district.name.clone())
                .collect(),
            landmarks: city.landmarks.clone(),
            connected_cities: world
                .city_connections(city_id)
                .iter()
                .map(|connected_city_id| world.city(*connected_city_id).name.clone())
                .collect(),
        },
        npc: NpcContextV1 {
            name: npc.name.clone(),
            archetype: npc.archetype,
            occupation: npc.occupation,
            traits: npc.personality_traits.clone(),
            goal: npc.goal,
            home_district: npc.home_district.clone(),
        },
        relationship: RelationshipMemoryViewV1 {
            disposition: relationship.disposition,
            trust_delta_summary: relationship.memory.trust_delta_summary,
            known_topics: relationship.memory.known_topics.clone(),
            unresolved_threads: relationship.memory.unresolved_threads.clone(),
            freeform_summary: relationship.memory.freeform_summary.clone(),
        },
        turn: DialogueTurnContextV1 {
            transcript: session
                .transcript
                .iter()
                .map(|line| DialogueTranscriptLineV1 {
                    speaker: match line.speaker {
                        Speaker::Player => DialogueTranscriptSpeakerV1::Player,
                        Speaker::Npc(_) => DialogueTranscriptSpeakerV1::Npc,
                        Speaker::System => DialogueTranscriptSpeakerV1::System,
                    },
                    text: line.text.clone(),
                })
                .collect(),
            player_input,
        },
    })
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

#[cfg(test)]
mod tests {
    use crate::domain::relationship::RelationshipMemory;
    use crate::simulation::{DialogueLine, DialogueSession, RelationshipState, Speaker};
    use crate::world::World;

    use super::{NPC_DIALOGUE_CONTEXT_V1_VERSION, build_npc_dialogue_context_v1};

    #[test]
    fn builder_creates_versioned_context_from_world_state() {
        let world = World::generate(9, 16);
        let city_id = world.city_ids()[0];
        let npc_id = world.city_npcs(city_id)[0];
        let relationship = RelationshipState {
            disposition: 2,
            memory: RelationshipMemory {
                trust_delta_summary: 1,
                known_topics: vec!["local records".to_string()],
                unresolved_threads: vec!["Follow up at city hall".to_string()],
                freeform_summary: "The player kept their word once before.".to_string(),
            },
            last_interaction_at: 3,
        };
        let session = DialogueSession {
            npc_id,
            started_at: 4,
            transcript: vec![DialogueLine {
                speaker: Speaker::Player,
                text: "hello".to_string(),
            }],
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

        assert_eq!(context.version, NPC_DIALOGUE_CONTEXT_V1_VERSION);
        assert_eq!(context.clock.current_time_seconds, 34);
        assert_eq!(context.clock.current_time_label, "Day 1 00:00:34");
        assert_eq!(context.city.name, world.city(city_id).name);
        assert_eq!(context.npc.name, world.npc(npc_id).name);
        assert_eq!(context.relationship.disposition, relationship.disposition);
        assert_eq!(context.relationship.trust_delta_summary, 1);
        assert_eq!(
            context.relationship.known_topics,
            vec!["local records".to_string()]
        );
        assert_eq!(context.turn.player_input, "What is this city like?");
        assert!(!context.city.connected_cities.is_empty());
        assert_eq!(context.turn.transcript.len(), 1);
        assert_eq!(
            context.turn.transcript[0].speaker,
            super::DialogueTranscriptSpeakerV1::Player
        );
    }

    #[test]
    fn builder_rejects_incoherent_city_and_npc_inputs() {
        let world = World::generate(9, 16);
        let city_id = world.city_ids()[0];
        let other_city_id = world
            .city_ids()
            .into_iter()
            .find(|candidate| *candidate != city_id)
            .expect("world should contain at least two cities");
        let npc_id = world.city_npcs(city_id)[0];
        let relationship = RelationshipState {
            disposition: 0,
            memory: RelationshipMemory::default(),
            last_interaction_at: 0,
        };
        let session = DialogueSession {
            npc_id,
            started_at: 0,
            transcript: Vec::new(),
        };

        let error = build_npc_dialogue_context_v1(
            &world,
            90,
            other_city_id,
            &relationship,
            &session,
            "hello".to_string(),
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("npc does not belong to the provided city")
        );
    }
}
