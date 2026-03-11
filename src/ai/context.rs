use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::domain::seed::WorldSeed;
use crate::domain::time::GameTime;
use crate::domain::vocab::{Biome, Culture, Economy, GoalTag, NpcArchetype, Occupation, TraitTag};
use crate::simulation::{DialogueSession, NpcMemoryState, Speaker};
use crate::world::{CityId, DistrictId, LandmarkId, NpcId, World};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NpcDialogueContext {
    pub world_seed: WorldSeed,
    pub clock: DialogueClock,
    pub city: CityContext,
    pub npc: NpcContext,
    pub memory: ConversationMemoryView,
    pub turn: DialogueTurnContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DialogueClock {
    pub current_time: GameTime,
}

impl DialogueClock {
    pub fn label(&self) -> String {
        self.current_time.format()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CityContext {
    pub id: CityId,
    pub biome: Biome,
    pub economy: Economy,
    pub culture: Culture,
    pub districts: Vec<DistrictContext>,
    pub landmarks: Vec<LandmarkContext>,
    pub connected_cities: Vec<ConnectedCityContext>,
}

impl CityContext {
    pub fn name(&self, world_seed: WorldSeed) -> String {
        self.id.name(world_seed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DistrictContext {
    pub id: DistrictId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LandmarkContext {
    pub id: LandmarkId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectedCityContext {
    pub id: CityId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NpcContext {
    pub id: NpcId,
    pub archetype: NpcArchetype,
    pub occupation: Occupation,
    pub traits: Vec<TraitTag>,
    pub goal: GoalTag,
    pub home_district: DistrictId,
}

impl ConnectedCityContext {
    pub fn name(&self, world_seed: WorldSeed) -> String {
        self.id.name(world_seed)
    }
}

impl NpcContext {
    pub fn name(&self, world_seed: WorldSeed) -> String {
        self.id.name(world_seed)
    }

    pub fn home_district_name(&self, world_seed: WorldSeed) -> String {
        self.home_district.name(world_seed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConversationMemoryView {
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DialogueTurnContext {
    pub transcript: Vec<DialogueTranscriptLine>,
    pub player_input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DialogueTranscriptLine {
    pub speaker: DialogueTranscriptSpeaker,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DialogueTranscriptSpeaker {
    Player,
    Npc,
    System,
}

pub fn build_npc_dialogue_context(
    world: &World,
    current_time: GameTime,
    city_id: CityId,
    memory: &NpcMemoryState,
    session: &DialogueSession,
    player_input: String,
) -> Result<NpcDialogueContext> {
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
    Ok(NpcDialogueContext {
        world_seed: world.seed,
        clock: DialogueClock {
            current_time,
        },
        city: CityContext {
            id: city_id,
            biome: city.biome,
            economy: city.economy,
            culture: city.culture,
            districts: city
                .districts
                .iter()
                .map(|district| DistrictContext { id: district.id })
                .collect(),
            landmarks: city
                .landmarks
                .iter()
                .map(|landmark| LandmarkContext { id: landmark.id })
                .collect(),
            connected_cities: world
                .city_connections(city_id)
                .iter()
                .map(|connected_city_id| ConnectedCityContext {
                    id: *connected_city_id,
                })
                .collect(),
        },
        npc: NpcContext {
            id: session.npc_id,
            archetype: npc.archetype,
            occupation: npc.occupation,
            traits: npc.personality_traits.clone(),
            goal: npc.goal,
            home_district: npc.home_district,
        },
        memory: ConversationMemoryView {
            summary: memory.memory.summary.clone(),
        },
        turn: DialogueTurnContext {
            transcript: session
                .transcript
                .iter()
                .map(|line| DialogueTranscriptLine {
                    speaker: match line.speaker {
                        Speaker::Player => DialogueTranscriptSpeaker::Player,
                        Speaker::Npc(_) => DialogueTranscriptSpeaker::Npc,
                        Speaker::System => DialogueTranscriptSpeaker::System,
                    },
                    text: line.text.clone(),
                })
                .collect(),
            player_input,
        },
    })
}

#[cfg(test)]
mod tests {
    use crate::domain::memory::ConversationMemory;
    use crate::domain::time::GameTime;
    use crate::simulation::{DialogueLine, DialogueSession, NpcMemoryState, Speaker};
    use crate::world::World;

    use super::{DialogueTranscriptSpeaker, build_npc_dialogue_context};

    #[test]
    fn builder_creates_context_from_world_state() {
        let world = World::generate(crate::domain::seed::WorldSeed::new(9), 16);
        let city_id = world.city_ids()[0];
        let npc_id = world.city_npcs(city_id)[0];
        let memory = NpcMemoryState {
            memory: ConversationMemory {
                summary: "The player kept their word once before.".to_string(),
            },
        };
        let session = DialogueSession {
            npc_id,
            started_at: GameTime::from_seconds(4),
            transcript: vec![DialogueLine {
                speaker: Speaker::Player,
                text: "hello".to_string(),
            }],
        };

        let context = build_npc_dialogue_context(
            &world,
            GameTime::from_seconds(34),
            city_id,
            &memory,
            &session,
            "What is this city like?".to_string(),
        )
        .unwrap();

        assert_eq!(context.clock.current_time, GameTime::from_seconds(34));
        assert_eq!(context.clock.label(), "Day 1 00:00:34");
        assert_eq!(context.city.id, city_id);
        assert_eq!(context.npc.id, npc_id);
        assert_eq!(context.memory.summary, "The player kept their word once before.");
        assert!(
            !context.city.districts[0]
                .id
                .description(context.world_seed)
                .is_empty()
        );
        assert!(
            !context.city.landmarks[0]
                .id
                .name(context.world_seed)
                .is_empty()
        );
        assert_eq!(context.npc.home_district.city_id, city_id);
        assert_eq!(context.turn.player_input, "What is this city like?");
        assert!(!context.city.connected_cities.is_empty());
        assert_eq!(context.turn.transcript.len(), 1);
        assert_eq!(
            context.turn.transcript[0].speaker,
            DialogueTranscriptSpeaker::Player
        );
    }

    #[test]
    fn builder_rejects_incoherent_city_and_npc_inputs() {
        let world = World::generate(crate::domain::seed::WorldSeed::new(9), 16);
        let city_id = world.city_ids()[0];
        let other_city_id = world
            .city_ids()
            .into_iter()
            .find(|candidate| *candidate != city_id)
            .expect("world should contain at least two cities");
        let npc_id = world.city_npcs(city_id)[0];
        let memory = NpcMemoryState {
            memory: ConversationMemory::default(),
        };
        let session = DialogueSession {
            npc_id,
            started_at: GameTime::from_seconds(0),
            transcript: Vec::new(),
        };

        let error = build_npc_dialogue_context(
            &world,
            GameTime::from_seconds(90),
            other_city_id,
            &memory,
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
