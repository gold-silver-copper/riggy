use crate::domain::time::GameTime;
use crate::simulation::GameState;
use crate::world::{CityId, NpcId, PlaceId, PlayerId, ProcessId};

pub fn player_id(state: &GameState) -> PlayerId {
    state
        .world
        .player_id()
        .expect("player should exist in world")
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

pub fn active_dialogue_process_id(state: &GameState) -> Option<ProcessId> {
    state.world.active_dialogue_process_id(player_id(state))
}

pub fn active_dialogue_npc_id(state: &GameState) -> Option<NpcId> {
    state.world.active_dialogue_npc_id(player_id(state))
}
