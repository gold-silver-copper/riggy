use std::collections::BTreeMap;

use crate::app::query::{current_transport_mode, current_vehicle_id, reachable_car_ids};
use crate::simulation::{
    ActorRefView, ActorView, CityView, ContextEntryKind, ContextFeedEntryView, DialoguePartnerView,
    DialogueSpeakerView, DistrictView, EntityView, GameState, InteractableOption,
    InteractableSubjectView, InteractionTarget, InteractionVerb, PlaceView, PlayerStatusView,
    RouteView, Speaker, UiMode, UiSnapshot,
};
use crate::world::EntityKind;

const RECENT_CONTEXT_LIMIT: usize = 64;

pub fn build_ui_snapshot(state: &GameState) -> UiSnapshot {
    let city = state.world.city(state.player_city_id);
    let place = state.world.place(state.player_place_id);
    let dialogue_npc = state
        .active_dialogue
        .as_ref()
        .map(|session| state.world.npc(session.npc_id));
    let status = PlayerStatusView {
        clock_seconds: state.clock_seconds,
        transport_mode: current_transport_mode(state),
        known_city_count: state.known_city_ids.len(),
    };
    let city_view = CityView {
        name: city.name.clone(),
        biome: city.biome,
        economy: city.economy,
        culture: city.culture,
        districts: city
            .districts
            .iter()
            .map(|district| DistrictView {
                name: district.name.clone(),
            })
            .collect(),
        landmarks: city.landmarks.clone(),
    };
    let place_view = PlaceView {
        id: state.player_place_id,
        name: place.name.clone(),
        kind: place.kind,
    };
    let dialogue_partner = dialogue_npc.map(|npc| {
        let npc_id = state
            .active_dialogue
            .as_ref()
            .map(|session| session.npc_id)
            .expect("dialogue partner should have active dialogue");
        let relationship = state.relationships.get(&npc_id);
        DialoguePartnerView {
            actor: ActorView {
                id: npc_id,
                name: npc.name.clone(),
                occupation: npc.occupation,
                archetype: npc.archetype,
            },
            disposition: relationship.map_or(0, |entry| entry.disposition),
            memory: relationship
                .and_then(|entry| (!entry.memory.is_empty()).then(|| entry.memory.clone())),
        }
    });
    let routes = state
        .world
        .place_routes(state.player_place_id)
        .iter()
        .map(|(place_id, route)| {
            let target = state.world.place(*place_id);
            let travel_seconds = route.travel_seconds(current_transport_mode(state));
            RouteView {
                destination: PlaceView {
                    id: *place_id,
                    name: target.name.clone(),
                    kind: target.kind,
                },
                route: *route,
                travel_seconds,
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
                name: npc.name.clone(),
                occupation: npc.occupation,
                archetype: npc.archetype,
            }
        })
        .collect::<Vec<_>>();
    let nearby_actors_by_id = nearby_actors
        .iter()
        .cloned()
        .map(|actor| (actor.id, actor))
        .collect::<BTreeMap<_, _>>();
    let nearby_cars = reachable_car_ids(state)
        .iter()
        .map(|entity_id| {
            let entity = state.world.entity(*entity_id);
            EntityView {
                id: *entity_id,
                name: entity.name.clone(),
                kind: entity.kind,
            }
        })
        .collect::<Vec<_>>();
    let nearby_cars_by_id = nearby_cars
        .iter()
        .cloned()
        .map(|entity| (entity.id, entity))
        .collect::<BTreeMap<_, _>>();
    let mut interactables = state
        .world
        .place_npcs(state.player_place_id)
        .iter()
        .map(|npc_id| InteractableOption {
            target: InteractionTarget::Npc(*npc_id),
            verb: InteractionVerb::Talk,
            subject: InteractableSubjectView::Actor(
                nearby_actors_by_id
                    .get(npc_id)
                    .cloned()
                    .expect("nearby actor should exist for interactable npc"),
            ),
        })
        .collect::<Vec<_>>();
    interactables.extend(reachable_car_ids(state).into_iter().map(|entity_id| {
        let is_current_car = current_vehicle_id(state) == Some(entity_id);
        InteractableOption {
            target: InteractionTarget::Entity(entity_id),
            verb: if is_current_car {
                InteractionVerb::ExitVehicle
            } else {
                InteractionVerb::EnterVehicle
            },
            subject: InteractableSubjectView::Entity(
                nearby_cars_by_id
                    .get(&entity_id)
                    .cloned()
                    .expect("nearby car should exist for interactable entity"),
            ),
        }
    }));
    let nearby_entities = state
        .world
        .place_entities(state.player_place_id)
        .into_iter()
        .filter(|entity_id| !matches!(state.world.entity(*entity_id).kind, EntityKind::Car))
        .map(|entity_id| {
            let entity = state.world.entity(entity_id);
            EntityView {
                id: entity_id,
                name: entity.name.clone(),
                kind: entity.kind,
            }
        })
        .collect::<Vec<_>>();
    let nearby_entities_by_id = nearby_entities
        .iter()
        .cloned()
        .map(|entity| (entity.id, entity))
        .collect::<BTreeMap<_, _>>();
    interactables.extend(nearby_entities_by_id.keys().copied().map(|entity_id| {
        InteractableOption {
            target: InteractionTarget::Entity(entity_id),
            verb: InteractionVerb::Inspect,
            subject: InteractableSubjectView::Entity(
                nearby_entities_by_id
                    .get(&entity_id)
                    .cloned()
                    .expect("nearby entity should exist for interactable entity"),
            ),
        }
    }));
    let context_feed = state
        .context_feed
        .iter()
        .rev()
        .take(RECENT_CONTEXT_LIMIT)
        .rev()
        .map(|entry| match &entry.kind {
            ContextEntryKind::System(context) => ContextFeedEntryView::System {
                timestamp_seconds: entry.timestamp_seconds,
                context: context.clone(),
            },
            ContextEntryKind::Dialogue { speaker, text } => ContextFeedEntryView::Dialogue {
                timestamp_seconds: entry.timestamp_seconds,
                speaker: match speaker {
                    Speaker::Player => DialogueSpeakerView::Player,
                    Speaker::Npc(npc_id) => DialogueSpeakerView::Npc(ActorRefView {
                        id: *npc_id,
                        name: state.world.npc(*npc_id).name.clone(),
                    }),
                    Speaker::System => DialogueSpeakerView::System,
                },
                text: text.clone(),
            },
        })
        .collect();

    UiSnapshot {
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
