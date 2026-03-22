use crate::ai::context::{ActorContext, CityContext};
use crate::app::query::{current_place_id, manual_actor_id};
use crate::domain::events::{EntitySummary, PlaceSummary};
use crate::simulation::{ActorView, CityView, GameState, Interactable, RouteView};
use crate::world::{ActorId, CityId, EntityId, PlaceId, World};

pub fn place_summary(world: &World, place_id: PlaceId) -> PlaceSummary {
    let place = world.place(place_id);
    PlaceSummary {
        id: place_id,
        city_id: world
            .place_city_id(place_id)
            .expect("place should belong to a city"),
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

pub fn actor_view(world: &World, actor_id: ActorId) -> ActorView {
    let profile = world.actor_profile(actor_id);
    ActorView {
        id: actor_id,
        occupation: profile.occupation,
        archetype: profile.archetype,
    }
}

pub fn city_view(world: &World, city_id: CityId) -> CityView {
    let city = world.city(city_id);
    CityView {
        id: city_id,
        biome: city.biome,
        economy: city.economy,
        culture: city.culture,
        connected_cities: world.city_connections(city_id),
    }
}

pub fn route_views(state: &GameState) -> Vec<RouteView> {
    state
        .world
        .place_routes(current_place_id(state))
        .iter()
        .map(|(place_id, route)| RouteView {
            destination: place_summary(&state.world, *place_id),
            route: *route,
            travel_time: route.travel_time,
        })
        .collect()
}

pub fn interactables(state: &GameState) -> Vec<Interactable> {
    let manual_actor_id = manual_actor_id(state);
    let mut interactables = state
        .world
        .place_actors(current_place_id(state))
        .into_iter()
        .filter(|actor_id| *actor_id != manual_actor_id)
        .map(|actor_id| Interactable::Talk(actor_view(&state.world, actor_id)))
        .collect::<Vec<_>>();
    interactables.extend(
        state
            .world
            .place_entities(current_place_id(state))
            .into_iter()
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
        connected_cities: world.city_connections(city_id),
    }
}

pub fn actor_context(world: &World, actor_id: ActorId) -> ActorContext {
    let profile = world.actor_profile(actor_id);
    ActorContext {
        id: actor_id,
        controller: profile.controller,
        archetype: profile.archetype,
        occupation: profile.occupation,
        traits: profile.traits,
        goal: profile.goal,
        home_place: place_summary(world, profile.home_place_id),
    }
}
