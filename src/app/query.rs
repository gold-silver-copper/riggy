use crate::domain::time::GameTime;
use crate::simulation::GameState;
use crate::world::{ActorId, CityId, PlaceId};

pub fn manual_actor_id(state: &GameState) -> ActorId {
    state
        .world
        .manual_actor_id()
        .expect("world should contain a manual actor")
}

pub fn current_time(state: &GameState) -> GameTime {
    state.world.current_time()
}

pub fn current_place_id(state: &GameState) -> PlaceId {
    state
        .world
        .actor_place_id(manual_actor_id(state))
        .expect("manual actor should occupy a place")
}

pub fn current_city_id(state: &GameState) -> CityId {
    state
        .world
        .actor_city_id(manual_actor_id(state))
        .expect("manual actor place should belong to a city")
}
