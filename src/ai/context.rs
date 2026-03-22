use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::app::projection::{actor_context, city_context, place_summary};
use crate::domain::events::{DialogueLine, PlaceSummary};
use crate::domain::memory::ConversationMemory;
use crate::domain::seed::WorldSeed;
use crate::domain::time::GameTime;
use crate::domain::vocab::{Biome, Culture, Economy, GoalTag, NpcArchetype, Occupation, TraitTag};
use crate::world::{ActorId, CityId, ControllerMode, World, place_name_from_parts};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActorDialogueContext {
    pub world_seed: WorldSeed,
    pub current_time: GameTime,
    pub city: CityContext,
    pub current_place: PlaceSummary,
    pub actor: ActorContext,
    pub counterpart: ActorContext,
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
pub struct ActorContext {
    pub id: ActorId,
    pub controller: ControllerMode,
    pub archetype: NpcArchetype,
    pub occupation: Occupation,
    pub traits: Vec<TraitTag>,
    pub goal: GoalTag,
    pub home_place: PlaceSummary,
}

impl ActorContext {
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
    pub speaker_input: String,
}

pub fn build_actor_dialogue_context(
    world: &World,
    current_time: GameTime,
    city_id: CityId,
    actor_id: ActorId,
    counterpart_id: ActorId,
    memory: &ConversationMemory,
    speaker_input: String,
) -> Result<ActorDialogueContext> {
    let place_id = world
        .actor_place_id(actor_id)
        .ok_or_else(|| anyhow::anyhow!("dialogue actor is missing a place"))?;
    if world.actor_place_id(counterpart_id) != Some(place_id) {
        bail!("dialogue counterpart is no longer in the same place");
    }
    if !world.city_ids().contains(&city_id) {
        bail!("dialogue context city does not exist");
    }
    if !world.actor_ids().contains(&actor_id) {
        bail!("dialogue context actor does not exist");
    }
    if !world.actor_ids().contains(&counterpart_id) {
        bail!("dialogue context counterpart does not exist");
    }
    if world.place_city_id(place_id) != Some(city_id) {
        bail!("dialogue context place does not belong to the provided city");
    }

    Ok(ActorDialogueContext {
        world_seed: world.seed,
        current_time,
        city: city_context(world, city_id),
        current_place: place_summary(world, place_id),
        actor: actor_context(world, actor_id),
        counterpart: actor_context(world, counterpart_id),
        memory: memory.clone(),
        turn: DialogueTurnContext {
            transcript: world.speech_lines_between(actor_id, counterpart_id, 64),
            speaker_input,
        },
    })
}
