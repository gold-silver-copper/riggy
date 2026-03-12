use crate::app::projection::{
    city_view, dialogue_partner_view, interactables, place_summary, route_views,
};
use crate::app::query::{
    active_dialogue_process_id, current_city_id, current_place_id, current_time,
    current_transport_mode, player_id,
};
use crate::simulation::{GameState, PlayerStatusView, UiMode, UiSnapshot};

const RECENT_CONTEXT_LIMIT: usize = 64;

pub fn build_ui_snapshot(state: &GameState) -> UiSnapshot {
    let status = PlayerStatusView {
        clock: current_time(state),
        transport_mode: current_transport_mode(state),
        known_city_count: state.world.discovered_city_ids(player_id(state)).len(),
    };
    let context_feed = state
        .world
        .recent_context_entries(player_id(state), RECENT_CONTEXT_LIMIT);

    UiSnapshot {
        world_seed: state.world.seed,
        mode: if active_dialogue_process_id(state).is_some() {
            UiMode::Dialogue
        } else {
            UiMode::Explore
        },
        status,
        city: city_view(&state.world, current_city_id(state)),
        place: place_summary(&state.world, current_place_id(state)),
        dialogue_partner: dialogue_partner_view(state),
        routes: route_views(state),
        interactables: interactables(state),
        context_feed,
    }
}
