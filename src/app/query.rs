use crate::simulation::{GameState, OccupancyState};
use crate::world::{EntityId, PlaceKind, TransportMode};

pub fn current_vehicle_id(state: &GameState) -> Option<EntityId> {
    match state.occupancy {
        OccupancyState::OnFoot => None,
        OccupancyState::InVehicle(entity_id) => Some(entity_id),
    }
}

pub fn current_transport_mode(state: &GameState) -> TransportMode {
    match state.occupancy {
        OccupancyState::OnFoot => TransportMode::Walking,
        OccupancyState::InVehicle(_) => TransportMode::Car,
    }
}

pub fn reachable_car_ids(state: &GameState) -> Vec<EntityId> {
    let mut cars = state.world.place_cars(state.player_place_id);
    cars.extend(
        state
            .world
            .place_routes(state.player_place_id)
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
