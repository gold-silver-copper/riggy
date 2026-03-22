use crate::app::projection::{city_view, interactables, place_summary, route_views};
use crate::app::query::{current_city_id, current_place_id, current_time, manual_actor_id};
use crate::simulation::{ActorStatusView, GameState, UiSnapshot};

const RECENT_CONTEXT_LIMIT: usize = 64;

pub fn build_ui_snapshot(state: &GameState) -> UiSnapshot {
    let actor_id = manual_actor_id(state);
    let status = ActorStatusView {
        id: actor_id,
        clock: current_time(state),
        known_city_count: state.world.discovered_city_ids(actor_id).len(),
    };
    let context_feed = state.world.recent_context_entries(actor_id, RECENT_CONTEXT_LIMIT);

    UiSnapshot {
        world_seed: state.world.seed,
        actor_id,
        status,
        city: city_view(&state.world, current_city_id(state)),
        place: place_summary(&state.world, current_place_id(state)),
        routes: route_views(state),
        interactables: interactables(state),
        context_feed,
    }
}
