use crate::ai::context::{CityContext, NpcContext};
use crate::app::query::{current_transport_mode, current_vehicle_id, reachable_car_ids};
use crate::domain::events::{EntitySummary, PlaceSummary};
use crate::domain::memory::ConversationMemory;
use crate::simulation::{
    ActorView, CityView, DialoguePartnerView, GameState, Interactable, RouteView,
};
use crate::world::{CityId, EntityId, EntityKind, NpcId, PlaceId, World};

pub fn place_summary(world: &World, place_id: PlaceId) -> PlaceSummary {
    let place = world.place(place_id);
    PlaceSummary {
        id: place_id,
        district_id: place.district_id,
        kind: place.kind,
    }
}

pub fn entity_summary(world: &World, entity_id: EntityId) -> EntitySummary {
    let entity = world.entity(entity_id);
    EntitySummary {
        id: entity_id,
        kind: entity.kind,
    }
}

pub fn actor_view(world: &World, npc_id: NpcId) -> ActorView {
    let npc = world.npc(npc_id);
    ActorView {
        id: npc_id,
        occupation: npc.occupation,
        archetype: npc.archetype,
    }
}

pub fn city_view(world: &World, city_id: CityId) -> CityView {
    let city = world.city(city_id);
    CityView {
        id: city_id,
        biome: city.biome,
        economy: city.economy,
        culture: city.culture,
        districts: city.districts.iter().map(|district| district.id).collect(),
        landmarks: city.landmarks.iter().map(|landmark| landmark.id).collect(),
    }
}

pub fn dialogue_partner_view(state: &GameState) -> Option<DialoguePartnerView> {
    state.active_dialogue.as_ref().map(|session| {
        let npc_id = session.npc_id;
        DialoguePartnerView {
            actor: actor_view(&state.world, npc_id),
            memory: state
                .npc_memories
                .get(&npc_id)
                .filter(|memory| !memory.is_empty())
                .cloned(),
        }
    })
}

pub fn route_views(state: &GameState) -> Vec<RouteView> {
    let transport_mode = current_transport_mode(state);
    state
        .world
        .place_routes(state.player_place_id)
        .iter()
        .map(|(place_id, route)| RouteView {
            destination: place_summary(&state.world, *place_id),
            route: *route,
            travel_time: route.travel_time(transport_mode),
        })
        .collect()
}

pub fn interactables(state: &GameState) -> Vec<Interactable> {
    let mut interactables = state
        .world
        .place_npcs(state.player_place_id)
        .into_iter()
        .map(|npc_id| Interactable::Talk(actor_view(&state.world, npc_id)))
        .collect::<Vec<_>>();
    interactables.extend(
        reachable_car_ids(state)
            .into_iter()
            .map(|entity_id| {
                let entity = entity_summary(&state.world, entity_id);
                if current_vehicle_id(state) == Some(entity.id) {
                    Interactable::ExitVehicle(entity)
                } else {
                    Interactable::EnterVehicle(entity)
                }
            }),
    );
    interactables.extend(
        state
            .world
            .place_entities(state.player_place_id)
            .into_iter()
            .filter(|entity_id| !matches!(state.world.entity(*entity_id).kind, EntityKind::Car))
            .map(|entity_id| Interactable::Inspect(entity_summary(&state.world, entity_id))),
    );
    interactables
}

pub fn city_context(world: &World, city_id: CityId) -> CityContext {
    let city = world.city(city_id);
    CityContext {
        id: city_id,
        biome: city.biome,
        economy: city.economy,
        culture: city.culture,
        districts: city.districts.iter().map(|district| district.id).collect(),
        landmarks: city.landmarks.iter().map(|landmark| landmark.id).collect(),
        connected_cities: world.city_connections(city_id),
    }
}

pub fn npc_context(world: &World, npc_id: NpcId) -> NpcContext {
    let npc = world.npc(npc_id);
    NpcContext {
        id: npc_id,
        archetype: npc.archetype,
        occupation: npc.occupation,
        traits: npc.personality_traits.clone(),
        goal: npc.goal,
        home_district: npc.home_district,
    }
}

pub fn conversation_memory<'a>(
    memories: &'a std::collections::BTreeMap<NpcId, ConversationMemory>,
    npc_id: NpcId,
) -> Option<&'a ConversationMemory> {
    memories.get(&npc_id).filter(|memory| !memory.is_empty())
}
