use crate::app::query::{current_transport_mode, current_vehicle_id, reachable_car_ids};
use crate::domain::events::{EntitySummary, PlaceSummary};
use crate::simulation::{
    ActorView, CityView, DialoguePartnerView, GameState, InteractableOption,
    InteractableSubjectView, InteractionTarget, InteractionVerb, PlayerStatusView, RouteView,
    UiMode, UiSnapshot,
};
use crate::world::EntityKind;

const RECENT_CONTEXT_LIMIT: usize = 64;

pub fn build_ui_snapshot(state: &GameState) -> UiSnapshot {
    let city = state.world.city(state.player_city_id);
    let place = state.world.place(state.player_place_id);
    let status = PlayerStatusView {
        clock: state.clock,
        transport_mode: current_transport_mode(state),
        known_city_count: state.known_city_ids.len(),
    };
    let city_view = CityView {
        id: state.player_city_id,
        biome: city.biome,
        economy: city.economy,
        culture: city.culture,
        districts: city.districts.iter().map(|district| district.id).collect(),
        landmarks: city.landmarks.iter().map(|landmark| landmark.id).collect(),
    };
    let place_view = PlaceSummary {
        id: state.player_place_id,
        district_id: place.district_id,
        kind: place.kind,
    };
    let dialogue_partner = state.active_dialogue.as_ref().map(|session| {
        let npc_id = session.npc_id;
        let npc = state.world.npc(npc_id);
        DialoguePartnerView {
            actor: ActorView {
                id: npc_id,
                occupation: npc.occupation,
                archetype: npc.archetype,
            },
            memory: state
                .npc_memories
                .get(&npc_id)
                .filter(|memory| !memory.is_empty())
                .cloned(),
        }
    });
    let transport_mode = current_transport_mode(state);
    let routes = state
        .world
        .place_routes(state.player_place_id)
        .iter()
        .map(|(place_id, route)| {
            let target = state.world.place(*place_id);
            RouteView {
                destination: PlaceSummary {
                    id: *place_id,
                    district_id: target.district_id,
                    kind: target.kind,
                },
                route: *route,
                travel_time: route.travel_time(transport_mode),
            }
        })
        .collect::<Vec<_>>();
    let nearby_actors = state
        .world
        .place_npcs(state.player_place_id)
        .iter()
        .map(|npc_id| {
            let npc = state.world.npc(*npc_id);
            ActorView {
                id: *npc_id,
                occupation: npc.occupation,
                archetype: npc.archetype,
            }
        })
        .collect::<Vec<_>>();
    let nearby_cars = reachable_car_ids(state)
        .into_iter()
        .map(|entity_id| {
            let entity = state.world.entity(entity_id);
            EntitySummary {
                id: entity_id,
                kind: entity.kind,
            }
        })
        .collect::<Vec<_>>();
    let nearby_entities = state
        .world
        .place_entities(state.player_place_id)
        .into_iter()
        .filter(|entity_id| !matches!(state.world.entity(*entity_id).kind, EntityKind::Car))
        .map(|entity_id| {
            let entity = state.world.entity(entity_id);
            EntitySummary {
                id: entity_id,
                kind: entity.kind,
            }
        })
        .collect::<Vec<_>>();
    let mut interactables = nearby_actors
        .iter()
        .cloned()
        .map(|actor| InteractableOption {
            target: InteractionTarget::Npc(actor.id),
            verb: InteractionVerb::Talk,
            subject: InteractableSubjectView::Actor(actor),
        })
        .collect::<Vec<_>>();
    interactables.extend(
        nearby_cars
            .iter()
            .copied()
            .map(|entity| InteractableOption {
                target: InteractionTarget::Entity(entity.id),
                verb: if current_vehicle_id(state) == Some(entity.id) {
                    InteractionVerb::ExitVehicle
                } else {
                    InteractionVerb::EnterVehicle
                },
                subject: InteractableSubjectView::Entity(entity),
            }),
    );
    interactables.extend(
        nearby_entities
            .iter()
            .copied()
            .map(|entity| InteractableOption {
                target: InteractionTarget::Entity(entity.id),
                verb: InteractionVerb::Inspect,
                subject: InteractableSubjectView::Entity(entity),
            }),
    );
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
        city: city_view,
        place: place_view,
        dialogue_partner,
        routes,
        interactables,
        nearby_actors,
        nearby_cars,
        nearby_entities,
        context_feed,
    }
}
