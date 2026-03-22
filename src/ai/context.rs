use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::app::projection::{city_context, npc_context, place_summary};
use crate::domain::events::{DialogueLine, PlaceSummary};
use crate::domain::memory::ConversationMemory;
use crate::domain::seed::WorldSeed;
use crate::domain::time::GameTime;
use crate::domain::vocab::{Biome, Culture, Economy, GoalTag, NpcArchetype, Occupation, TraitTag};
use crate::world::{CityId, NpcId, ProcessId, World, place_name_from_parts};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NpcDialogueContext {
    pub world_seed: WorldSeed,
    pub current_time: GameTime,
    pub city: CityContext,
    pub current_place: PlaceSummary,
    pub npc: NpcContext,
    pub memory: ConversationMemory,
    pub turn: DialogueTurnContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CityContext {
    pub id: CityId,
    pub biome: Biome,
    pub economy: Economy,
    pub culture: Culture,
    pub connected_cities: Vec<CityId>,
}

impl CityContext {
    pub fn name(&self, world_seed: WorldSeed) -> String {
        self.id.name(world_seed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NpcContext {
    pub id: NpcId,
    pub archetype: NpcArchetype,
    pub occupation: Occupation,
    pub traits: Vec<TraitTag>,
    pub goal: GoalTag,
    pub home_place: PlaceSummary,
}

impl NpcContext {
    pub fn name(&self, world_seed: WorldSeed) -> String {
        self.id.name(world_seed)
    }

    pub fn home_place_name(&self, world_seed: WorldSeed) -> String {
        place_name_from_parts(
            world_seed,
            self.home_place.id,
            self.home_place.city_id,
            self.home_place.kind,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DialogueTurnContext {
    pub transcript: Vec<DialogueLine>,
    pub player_input: String,
}

pub fn build_npc_dialogue_context(
    world: &World,
    current_time: GameTime,
    city_id: CityId,
    memory: &ConversationMemory,
    process_id: ProcessId,
    player_input: String,
) -> Result<NpcDialogueContext> {
    let npc_id = world
        .dialogue_npc_id(process_id)
        .ok_or_else(|| anyhow::anyhow!("dialogue context process is missing an NPC participant"))?;
    let place_id = world
        .dialogue_place_id(process_id)
        .ok_or_else(|| anyhow::anyhow!("dialogue context process is missing a place"))?;
    if !world.city_ids().contains(&city_id) {
        bail!("dialogue context city does not exist");
    }
    if !world.npc_ids().contains(&npc_id) {
        bail!("dialogue context npc does not exist");
    }
    if !world.city_npcs(city_id).contains(&npc_id) {
        bail!("dialogue context npc does not belong to the provided city");
    }
    if world.place_city_id(place_id) != Some(city_id) {
        bail!("dialogue context place does not belong to the provided city");
    }

    Ok(NpcDialogueContext {
        world_seed: world.seed,
        current_time,
        city: city_context(world, city_id),
        current_place: place_summary(world, place_id),
        npc: npc_context(world, npc_id),
        memory: memory.clone(),
        turn: DialogueTurnContext {
            transcript: world.dialogue_lines(process_id),
            player_input,
        },
    })
}

#[cfg(test)]
mod tests {
    use crate::domain::events::{DialogueLine, DialogueSpeaker};
    use crate::domain::memory::ConversationMemory;
    use crate::domain::time::GameTime;
    use crate::world::{PlaceKind, World};

    use super::build_npc_dialogue_context;

    #[test]
    fn builder_creates_context_from_world_state() {
        let world = World::generate(crate::domain::seed::WorldSeed::new(9), 16);
        let city_id = world.city_ids()[0];
        let npc_id = world.city_npcs(city_id)[0];
        let player_id = world.player_id().expect("world should contain a player");
        let memory = ConversationMemory {
            summary: "The player kept their word once before.".to_string(),
        };
        let mut world = world;
        let place_id = world.city_places(city_id)[0];
        let process_id =
            world.start_dialogue_process(player_id, npc_id, place_id, GameTime::from_seconds(4));
        world.append_dialogue_utterance(
            process_id,
            player_id,
            DialogueLine {
                timestamp: GameTime::from_seconds(4),
                speaker: DialogueSpeaker::Player,
                text: "hello".to_string(),
            },
        );

        let context = build_npc_dialogue_context(
            &world,
            GameTime::from_seconds(34),
            city_id,
            &memory,
            process_id,
            "What is this city like?".to_string(),
        )
        .unwrap();

        assert_eq!(context.current_time, GameTime::from_seconds(34));
        assert_eq!(context.current_time.format(), "Day 1 00:00:34");
        assert_eq!(context.city.id, city_id);
        assert_eq!(context.current_place.city_id, city_id);
        assert_eq!(context.npc.id, npc_id);
        assert_eq!(
            context.memory.summary,
            "The player kept their word once before."
        );
        assert_eq!(context.npc.home_place.city_id, city_id);
        assert_eq!(context.turn.player_input, "What is this city like?");
        assert!(!context.city.connected_cities.is_empty());
        assert_eq!(context.turn.transcript.len(), 1);
        assert_eq!(context.turn.transcript[0].speaker, DialogueSpeaker::Player);
        assert!(matches!(
            context.current_place.kind,
            PlaceKind::Residence | PlaceKind::Street | PlaceKind::Venue | PlaceKind::Station
        ));
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
        let player_id = world.player_id().expect("world should contain a player");
        let memory = ConversationMemory::default();
        let mut world = world;
        let place_id = world.city_places(city_id)[0];
        let process_id =
            world.start_dialogue_process(player_id, npc_id, place_id, GameTime::from_seconds(0));

        let error = build_npc_dialogue_context(
            &world,
            GameTime::from_seconds(90),
            other_city_id,
            &memory,
            process_id,
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
