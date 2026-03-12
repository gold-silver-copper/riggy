use crate::domain::time::GameTime;
use crate::simulation::GameState;
use crate::world::{
    CityId, EntityId, NpcId, PlaceId, PlaceKind, PlayerId, ProcessId, TransportMode,
};

pub fn player_id(state: &GameState) -> PlayerId {
    state
        .world
        .player_id()
        .expect("player should exist in world graph")
}

pub fn current_time(state: &GameState) -> GameTime {
    state.world.current_time()
}

pub fn current_place_id(state: &GameState) -> PlaceId {
    state
        .world
        .player_place_id(player_id(state))
        .expect("player should occupy a place")
}

pub fn current_city_id(state: &GameState) -> CityId {
    state
        .world
        .player_city_id(player_id(state))
        .expect("player place should belong to a city")
}

pub fn current_vehicle_id(state: &GameState) -> Option<EntityId> {
    state.world.player_vehicle_id(player_id(state))
}

pub fn active_dialogue_process_id(state: &GameState) -> Option<ProcessId> {
    state.world.active_dialogue_process_id(player_id(state))
}

pub fn active_dialogue_npc_id(state: &GameState) -> Option<NpcId> {
    state.world.active_dialogue_npc_id(player_id(state))
}

pub fn current_transport_mode(state: &GameState) -> TransportMode {
    if current_vehicle_id(state).is_some() {
        TransportMode::Car
    } else {
        TransportMode::Walking
    }
}

pub fn reachable_car_ids(state: &GameState) -> Vec<EntityId> {
    let place_id = current_place_id(state);
    let mut cars = state.world.place_cars(place_id);
    cars.extend(
        state
            .world
            .place_routes(place_id)
            .into_iter()
            .filter(|(place_id, _)| {
                matches!(state.world.place(*place_id).kind, PlaceKind::RoadLane)
            })
            .flat_map(|(place_id, _)| state.world.place_cars(place_id)),
    );
    cars.sort_unstable();
    cars.dedup();
    cars
}
