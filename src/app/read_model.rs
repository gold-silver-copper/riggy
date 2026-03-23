use crate::app::projection::{actor_view, city_view, interactables, place_summary, route_views};
use crate::app::query::{current_city_id, current_place_id, current_time};
use crate::domain::commands::AvailableAction;
use crate::simulation::{ActorStatusView, AgentDebugSnapshot, GameState, UiSnapshot};
use crate::world::ActorId;

const RECENT_CONTEXT_LIMIT: usize = 64;

pub fn build_ui_snapshot(
    state: &GameState,
    focused_actor_id: ActorId,
    available_actions: Vec<AvailableAction>,
    agent_debug: Vec<AgentDebugSnapshot>,
) -> UiSnapshot {
    let status = ActorStatusView {
        id: focused_actor_id,
        clock: current_time(state),
        known_city_count: state.world.discovered_city_ids(focused_actor_id).len(),
    };
    let context_feed = state
        .world
        .recent_context_entries(focused_actor_id, RECENT_CONTEXT_LIMIT);

    UiSnapshot {
        world_seed: state.world.seed,
        focused_actor_id,
        focused_actor: actor_view(&state.world, focused_actor_id),
        status,
        city: city_view(&state.world, current_city_id(state, focused_actor_id)),
        place: place_summary(&state.world, current_place_id(state, focused_actor_id)),
        routes: route_views(state, focused_actor_id),
        interactables: interactables(state, focused_actor_id),
        available_actions,
        context_feed,
        agent_debug,
    }
}
