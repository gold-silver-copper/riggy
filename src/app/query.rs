use crate::domain::time::GameTime;
use crate::simulation::GameState;
use crate::world::{ActorId, CityId, PlaceId};

pub fn current_time(state: &GameState) -> GameTime {
    state.world.current_time()
}

pub fn current_place_id(state: &GameState, actor_id: ActorId) -> PlaceId {
    state
        .world
        .actor_place_id(actor_id)
        .expect("focused actor should occupy a place")
}

pub fn current_city_id(state: &GameState, actor_id: ActorId) -> CityId {
    state
        .world
        .actor_city_id(actor_id)
        .expect("focused actor place should belong to a city")
}
