use crate::app::projection::{
    city_view, dialogue_partner_view, interactables, place_summary, route_views,
};
use crate::app::query::current_transport_mode;
use crate::simulation::{GameState, PlayerStatusView, UiMode, UiSnapshot};

const RECENT_CONTEXT_LIMIT: usize = 64;

pub fn build_ui_snapshot(state: &GameState) -> UiSnapshot {
    let status = PlayerStatusView {
        clock: state.clock,
        transport_mode: current_transport_mode(state),
        known_city_count: state.known_city_ids.len(),
    };
    let context_feed = state
        .context_feed
        .iter()
        .rev()
        .take(RECENT_CONTEXT_LIMIT)
        .rev()
        .cloned()
        .collect();

    UiSnapshot {
        world_seed: state.world.seed,
        mode: if state.active_dialogue.is_some() {
            UiMode::Dialogue
        } else {
            UiMode::Explore
        },
        status,
        city: city_view(&state.world, state.player_city_id),
        place: place_summary(&state.world, state.player_place_id),
        dialogue_partner: dialogue_partner_view(state),
        routes: route_views(state),
        interactables: interactables(state),
        context_feed,
    }
}
