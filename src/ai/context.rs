use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::domain::seed::WorldSeed;
use crate::domain::time::GameTime;
use crate::domain::vocab::{Biome, Culture, Economy, GoalTag, NpcArchetype, Occupation, TraitTag};
use crate::simulation::{DialogueSession, NpcMemoryState, Speaker};
use crate::world::{CityId, NpcId, World};

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
        procgen_city_name(world_seed, self.id)
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DistrictId {
    pub city_id: CityId,
    pub district_index: u16,
}

impl DistrictId {
    pub fn name(self, world_seed: WorldSeed) -> String {
        let key = mix_seed(
            world_seed,
            &[1, self.city_id.index() as u64, self.district_index as u64],
        );
        format!(
            "{} {}",
            DISTRICT_PREFIXES[(key as usize) % DISTRICT_PREFIXES.len()],
            DISTRICT_SUFFIXES[((key >> 16) as usize) % DISTRICT_SUFFIXES.len()]
        )
    }

    pub fn description(self, world_seed: WorldSeed) -> String {
        let key = mix_seed(
            world_seed,
            &[2, self.city_id.index() as u64, self.district_index as u64],
        );
        format!(
            "{} with {}",
            DISTRICT_TEXTURES[(key as usize) % DISTRICT_TEXTURES.len()],
            DISTRICT_FUNCTIONS[((key >> 16) as usize) % DISTRICT_FUNCTIONS.len()]
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LandmarkId {
    pub city_id: CityId,
    pub landmark_index: u16,
}

impl LandmarkId {
    pub fn name(self, world_seed: WorldSeed) -> String {
        let key = mix_seed(
            world_seed,
            &[3, self.city_id.index() as u64, self.landmark_index as u64],
        );
        format!(
            "{} {}",
            LANDMARK_PREFIXES[(key as usize) % LANDMARK_PREFIXES.len()],
            LANDMARK_NOUNS[((key >> 16) as usize) % LANDMARK_NOUNS.len()]
        )
    }
}

impl ConnectedCityContext {
    pub fn name(&self, world_seed: WorldSeed) -> String {
        procgen_city_name(world_seed, self.id)
    }
}

impl NpcContext {
    pub fn name(&self, world_seed: WorldSeed) -> String {
        let key = mix_seed(world_seed, &[4, self.id.index() as u64]);
        format!(
            "{} {}",
            NPC_FIRST_NAMES[(key as usize) % NPC_FIRST_NAMES.len()],
            NPC_LAST_NAMES[((key >> 16) as usize) % NPC_LAST_NAMES.len()]
        )
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
    let home_district_index = city
        .districts
        .iter()
        .position(|district| district.name == npc.home_district)
        .ok_or_else(|| anyhow::anyhow!("dialogue context npc home district is missing"))?;

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
                .enumerate()
                .map(|(district_index, _district)| DistrictContext {
                    id: DistrictId {
                        city_id,
                        district_index: district_index as u16,
                    },
                })
                .collect(),
            landmarks: city
                .landmarks
                .iter()
                .enumerate()
                .map(|(landmark_index, _landmark)| LandmarkContext {
                    id: LandmarkId {
                        city_id,
                        landmark_index: landmark_index as u16,
                    },
                })
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
            home_district: DistrictId {
                city_id,
                district_index: home_district_index as u16,
            },
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

fn mix_seed(seed: WorldSeed, parts: &[u64]) -> u64 {
    let mut value = seed.raw() ^ 0x9E37_79B9_7F4A_7C15;
    for part in parts {
        value ^= part.wrapping_add(0x9E37_79B9_7F4A_7C15);
        value = value.rotate_left(27).wrapping_mul(0x94D0_49BB_1331_11EB);
    }
    value
}

fn procgen_city_name(world_seed: WorldSeed, city_id: CityId) -> String {
    let key = mix_seed(world_seed, &[5, city_id.index() as u64]);
    format!(
        "{}{}",
        CITY_PREFIXES[(key as usize) % CITY_PREFIXES.len()],
        CITY_SUFFIXES[((key >> 16) as usize) % CITY_SUFFIXES.len()]
    )
}

const DISTRICT_PREFIXES: [&str; 10] = [
    "Ash", "Market", "Harbor", "Station", "North", "South", "River", "Glass", "Union", "Cedar",
];
const DISTRICT_SUFFIXES: [&str; 10] = [
    "Quarter", "Heights", "Square", "Point", "Terrace", "Center", "Row", "Reach", "Gate", "Yard",
];
const DISTRICT_TEXTURES: [&str; 8] = [
    "dense midrise blocks",
    "retail-heavy streets",
    "quiet apartment corridors",
    "office-facing avenues",
    "warehouse edges",
    "night-shift storefronts",
    "mixed-use corners",
    "narrow commuter lanes",
];
const DISTRICT_FUNCTIONS: [&str; 8] = [
    "corner stores and takeout windows",
    "small offices and service counters",
    "loading bays and fenced lots",
    "apartment entries and laundromats",
    "transit foot traffic and kiosks",
    "cafes and repair shops",
    "late-night traffic and side parking",
    "municipal buildings and walk-ups",
];
const LANDMARK_PREFIXES: [&str; 8] = [
    "Old", "North", "Glass", "Moon", "Union", "Raven", "Low", "Civic",
];
const LANDMARK_NOUNS: [&str; 8] = [
    "Exchange",
    "Museum",
    "Data Center",
    "Overpass",
    "Terminal",
    "Arcade",
    "Park",
    "Archive",
];
const NPC_FIRST_NAMES: [&str; 12] = [
    "Yana", "Finn", "Mara", "Theo", "Iris", "Nico", "Leah", "Owen", "Tess", "Miles", "Juno", "Evan",
];
const NPC_LAST_NAMES: [&str; 12] = [
    "Orchard", "Ives", "Vale", "Morrow", "Hale", "Cross", "Rowan", "Keene", "Mercer", "Sable",
    "Dane", "Quill",
];
const CITY_PREFIXES: [&str; 10] = [
    "Ash", "Low", "Raven", "North", "Dawn", "Brae", "Quartz", "Moon", "Kings", "Harbor",
];
const CITY_SUFFIXES: [&str; 10] = [
    "crest", "harbor", "cross", "park", "view", "market", "ford", "center", "bridge", "field",
];

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
