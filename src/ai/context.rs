use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::app::projection::{actor_context, city_context, entity_summary, place_summary};
use crate::domain::commands::AgentAvailableAction;
use crate::domain::events::{DialogueLine, EntitySummary, PlaceSummary};
use crate::domain::memory::ConversationMemory;
use crate::domain::seed::WorldSeed;
use crate::domain::time::{GameTime, TimeDelta};
use crate::domain::vocab::{Biome, Culture, Economy, GoalTag, NpcArchetype, Occupation, TraitTag};
use crate::world::{ActorId, CityId, ControllerMode, World, place_name_from_parts};

const RECENT_SPEECH_LIMIT: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActorTurnContext {
    pub world_seed: WorldSeed,
    pub current_time: GameTime,
    pub city: CityContext,
    pub current_place: PlaceSummary,
    pub actor: ActorContext,
    pub memory: ConversationMemory,
    pub local_state: LocalStateContext,
    pub recent_speech: Vec<DialogueLine>,
    pub available_actions: Vec<AgentAvailableAction>,
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
pub struct LocalStateContext {
    pub nearby_actors: Vec<ActorContext>,
    pub nearby_entities: Vec<EntitySummary>,
    pub routes: Vec<RouteContext>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteContext {
    pub destination: PlaceSummary,
    pub travel_time: TimeDelta,
}

pub fn build_actor_turn_context(
    world: &World,
    current_time: GameTime,
    actor_id: ActorId,
    available_actions: Vec<AgentAvailableAction>,
) -> Result<ActorTurnContext> {
    let place_id = world
        .actor_place_id(actor_id)
        .ok_or_else(|| anyhow::anyhow!("turn actor is missing a place"))?;
    let city_id = world
        .place_city_id(place_id)
        .ok_or_else(|| anyhow::anyhow!("turn actor place is missing a city"))?;
    if !world.city_ids().contains(&city_id) {
        bail!("turn context city does not exist");
    }
    if !world.actor_ids().contains(&actor_id) {
        bail!("turn context actor does not exist");
    }

    let nearby_actor_ids = world
        .place_actors(place_id)
        .into_iter()
        .filter(|candidate| *candidate != actor_id)
        .collect::<Vec<_>>();
    let local_state = LocalStateContext {
        nearby_actors: nearby_actor_ids
            .iter()
            .copied()
            .map(|nearby_actor_id| actor_context(world, nearby_actor_id))
            .collect(),
        nearby_entities: world
            .place_entities(place_id)
            .into_iter()
            .map(|entity_id| entity_summary(world, entity_id))
            .collect(),
        routes: world
            .place_routes(place_id)
            .into_iter()
            .map(|(destination_id, route)| RouteContext {
                destination: place_summary(world, destination_id),
                travel_time: route.travel_time,
            })
            .collect(),
    };

    let mut recent_speech = nearby_actor_ids
        .iter()
        .copied()
        .flat_map(|nearby_actor_id| {
            world.speech_lines_between(actor_id, nearby_actor_id, RECENT_SPEECH_LIMIT)
        })
        .collect::<Vec<_>>();
    recent_speech.sort_by_key(|line| line.timestamp);
    let speech_len = recent_speech.len();
    let recent_speech = recent_speech
        .into_iter()
        .skip(speech_len.saturating_sub(RECENT_SPEECH_LIMIT))
        .collect();

    Ok(ActorTurnContext {
        world_seed: world.seed,
        current_time,
        city: city_context(world, city_id),
        current_place: place_summary(world, place_id),
        actor: actor_context(world, actor_id),
        memory: world
            .actor_conversation_memory(actor_id)
            .unwrap_or_default(),
        local_state,
        recent_speech,
        available_actions,
    })
}
