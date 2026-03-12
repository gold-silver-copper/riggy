use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

use crate::domain::events::{
    ContextEntry, DialogueSpeaker, EntitySummary, GameEvent, PlaceSummary, SystemContext,
};
use crate::domain::seed::WorldSeed;
use crate::simulation::{
    InteractableOption, InteractableSubjectView, InteractionVerb, RouteView, UiSnapshot,
};
use crate::world::{entity_name_from_parts, place_name_from_parts};

pub fn build_world_title(snapshot: &UiSnapshot) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!(
                "{} ({})",
                place_name(snapshot.world_seed, &snapshot.place),
                snapshot.place.kind.label()
            ),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            snapshot.city.id.name(snapshot.world_seed),
            Style::default().fg(Color::Green),
        ),
    ])
}

pub fn build_world_text(snapshot: &UiSnapshot, notices: &[String]) -> Text<'static> {
    let mut lines = vec![
        Line::from(vec![
            Span::raw("You are in "),
            highlighted(
                place_name(snapshot.world_seed, &snapshot.place),
                Color::Yellow,
            ),
            Span::raw(" in "),
            highlighted(snapshot.city.id.name(snapshot.world_seed), Color::Green),
            Span::raw(", a "),
            highlighted(
                format!(
                    "{} city with a {} economy and {} culture",
                    snapshot.city.biome.label(),
                    snapshot.city.economy.label(),
                    snapshot.city.culture.label()
                ),
                Color::Cyan,
            ),
            Span::raw("."),
        ]),
        Line::from(vec![
            Span::raw("This area is a "),
            highlighted(snapshot.place.kind.label().to_string(), Color::Yellow),
            Span::raw("."),
        ]),
        Line::from(vec![
            Span::raw("Time: "),
            highlighted(snapshot.status.clock.format(), Color::Cyan),
            Span::raw("  |  Transport: "),
            highlighted(
                snapshot.status.transport_mode.label().to_string(),
                Color::Yellow,
            ),
            Span::raw("  |  Known cities: "),
            highlighted(snapshot.status.known_city_count.to_string(), Color::Green),
        ]),
        Line::from(""),
    ];

    if let Some(partner) = &snapshot.dialogue_partner {
        lines.push(Line::from(vec![
            Span::raw("Current conversation: "),
            highlighted(partner.actor.id.name(snapshot.world_seed), Color::Magenta),
            Span::raw("  |  Job: "),
            highlighted(partner.actor.occupation.label().to_string(), Color::Yellow),
            Span::raw("."),
        ]));
        if let Some(memory) = &partner.memory {
            lines.push(Line::from(vec![
                Span::raw("Conversation memory: "),
                Span::raw(clean_inline_text(&render_conversation_memory(memory))),
                Span::raw("."),
            ]));
        }
    }

    if !snapshot.city.districts.is_empty() {
        push_list_section(
            &mut lines,
            "Districts nearby",
            snapshot
                .city
                .districts
                .iter()
                .map(|district| district.name(snapshot.world_seed)),
            Color::Green,
            ", ",
        );
    }

    if !snapshot.nearby_actors.is_empty() {
        push_list_section(
            &mut lines,
            "People here",
            snapshot.nearby_actors.iter().map(|person| {
                format!(
                    "{} - {}, {}",
                    person.id.name(snapshot.world_seed),
                    person.occupation.label(),
                    person.archetype.label()
                )
            }),
            Color::Magenta,
            " | ",
        );
    }

    if !snapshot.nearby_cars.is_empty() {
        push_list_section(
            &mut lines,
            "Vehicles within reach",
            snapshot
                .nearby_cars
                .iter()
                .map(|entity| entity_name(snapshot.world_seed, entity)),
            Color::Yellow,
            " | ",
        );
    }

    if !snapshot.nearby_entities.is_empty() {
        push_list_section(
            &mut lines,
            "Other notable details",
            snapshot.nearby_entities.iter().map(|entity| {
                format!(
                    "{} ({})",
                    entity_name(snapshot.world_seed, entity),
                    entity.kind.label()
                )
            }),
            Color::Cyan,
            " | ",
        );
    }

    if !snapshot.city.landmarks.is_empty() {
        push_list_section(
            &mut lines,
            "Landmarks",
            snapshot
                .city
                .landmarks
                .iter()
                .map(|landmark| landmark.name(snapshot.world_seed)),
            Color::Cyan,
            ", ",
        );
    }

    if !snapshot.routes.is_empty() {
        push_list_section(
            &mut lines,
            "Routes from here",
            snapshot
                .routes
                .iter()
                .map(|route| render_route_label(snapshot.world_seed, route)),
            Color::Yellow,
            ", ",
        );
    }

    let recent_context = build_recent_context_lines(snapshot, notices);
    if !recent_context.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "Recent Context",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )]));
        lines.extend(recent_context);
    }

    Text::from(lines)
}

pub fn render_route_label(world_seed: WorldSeed, option: &RouteView) -> String {
    let destination_name = place_name(world_seed, &option.destination);
    match option.travel_time {
        Some(duration) => format!(
            "{} via {} ({})",
            destination_name,
            option.route.kind.label(),
            format_duration(duration),
        ),
        None => format!(
            "{} via {} (unavailable)",
            destination_name,
            option.route.kind.label(),
        ),
    }
}

pub fn render_interactable_label(world_seed: WorldSeed, option: &InteractableOption) -> String {
    match (&option.subject, option.verb) {
        (InteractableSubjectView::Actor(actor), InteractionVerb::Talk) => {
            format!(
                "{} - talk ({}, {})",
                actor.id.name(world_seed),
                actor.occupation.label(),
                actor.archetype.label()
            )
        }
        (InteractableSubjectView::Entity(entity), InteractionVerb::EnterVehicle) => {
            format!("{} - enter vehicle", entity_name(world_seed, entity))
        }
        (InteractableSubjectView::Entity(entity), InteractionVerb::ExitVehicle) => {
            format!("{} - exit vehicle", entity_name(world_seed, entity))
        }
        (InteractableSubjectView::Entity(entity), InteractionVerb::Inspect) => {
            format!(
                "{} - inspect {}",
                entity_name(world_seed, entity),
                entity.kind.label()
            )
        }
        _ => "Unavailable interaction".to_string(),
    }
}

pub fn render_event_notice(world_seed: WorldSeed, event: &GameEvent) -> Option<String> {
    match event {
        GameEvent::DialogueStarted { npc_id } => Some(format!(
            "You approach {}. Type normally to speak, or press Esc to end the conversation.",
            npc_id.name(world_seed)
        )),
        GameEvent::DialogueLineRecorded { .. } => None,
        GameEvent::DialogueEnded { npc_id } => {
            Some(format!("You step away from {}.", npc_id.name(world_seed)))
        }
        GameEvent::TravelCompleted {
            destination,
            transport_mode,
            route,
            duration,
        } => Some(format!(
            "You travel to {} by {} on {} in {}.",
            place_name_from_parts(
                world_seed,
                destination.id,
                destination.district_id,
                destination.kind
            ),
            transport_mode.label(),
            route.kind.label(),
            format_duration(*duration)
        )),
        GameEvent::VehicleEntered { entity } => Some(format!(
            "You get into the {}.",
            entity_name_from_parts(world_seed, entity.id, entity.kind)
        )),
        GameEvent::VehicleExited { entity } => Some(format!(
            "You get out of the {}.",
            entity_name_from_parts(world_seed, entity.id, entity.kind)
        )),
        GameEvent::EntityInspected { entity } => Some(format!(
            "You inspect {}. It looks like a {} left out in plain view.",
            entity_name_from_parts(world_seed, entity.id, entity.kind),
            entity.kind.label()
        )),
        GameEvent::WaitCompleted {
            duration,
            current_time,
        } => Some(format!(
            "You wait for {}. The time is now {}.",
            format_duration(*duration),
            current_time.format()
        )),
        GameEvent::ContextAppended { .. } => None,
    }
}

fn build_recent_context_lines(snapshot: &UiSnapshot, notices: &[String]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    for entry in &snapshot.context_feed {
        match entry {
            ContextEntry::System { timestamp, context } => {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!("[{}]", timestamp.format()),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        context.label().to_string(),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::raw(clean_inline_text(&render_system_context(
                        snapshot.world_seed,
                        context,
                    ))),
                ]));
            }
            ContextEntry::Dialogue(line) => {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        dialogue_speaker_label(snapshot.world_seed, line.speaker),
                        Style::default()
                            .fg(dialogue_speaker_color(line.speaker))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::raw(clean_inline_text(&line.text)),
                ]));
            }
        }
    }

    for notice in notices.iter().rev().take(4).rev() {
        for line in notice
            .lines()
            .map(clean_inline_text)
            .filter(|line| !line.is_empty())
        {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "note",
                    Style::default()
                        .fg(Color::LightBlue)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::raw(line),
            ]));
        }
    }

    lines
}

fn render_system_context(world_seed: WorldSeed, context: &SystemContext) -> String {
    match context {
        SystemContext::Start => {
            "You arrived in a starter apartment with a need for useful names and a parked car somewhere close by.".to_string()
        }
        SystemContext::Travel {
            destination,
            transport_mode,
            duration,
            ..
        } => format!(
            "Arrived at {} via {} after {}.",
            place_name_from_parts(world_seed, destination.id, destination.district_id, destination.kind),
            transport_mode.label(),
            format_duration(*duration)
        ),
    }
}

fn dialogue_speaker_label(world_seed: WorldSeed, speaker: DialogueSpeaker) -> String {
    match speaker {
        DialogueSpeaker::Player => "You".to_string(),
        DialogueSpeaker::Npc(npc_id) => npc_id.name(world_seed),
        DialogueSpeaker::System => "System".to_string(),
    }
}

fn dialogue_speaker_color(speaker: DialogueSpeaker) -> Color {
    match speaker {
        DialogueSpeaker::Player => Color::Yellow,
        DialogueSpeaker::Npc(_) => Color::Magenta,
        DialogueSpeaker::System => Color::Cyan,
    }
}

fn clean_inline_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn render_conversation_memory(memory: &crate::domain::memory::ConversationMemory) -> String {
    if memory.summary.trim().is_empty() {
        "none".to_string()
    } else {
        memory.summary.trim().to_string()
    }
}

pub fn format_duration(duration: crate::domain::time::TimeDelta) -> String {
    duration.format()
}

fn push_list_section<I>(
    lines: &mut Vec<Line<'static>>,
    label: &str,
    values: I,
    color: Color,
    separator: &str,
) where
    I: Iterator<Item = String>,
{
    lines.push(Line::from(vec![
        Span::raw(format!("{label}: ")),
        highlighted(values.collect::<Vec<_>>().join(separator), color),
        Span::raw("."),
    ]));
}

fn place_name(world_seed: WorldSeed, place: &PlaceSummary) -> String {
    place_name_from_parts(world_seed, place.id, place.district_id, place.kind)
}

fn entity_name(world_seed: WorldSeed, entity: &EntitySummary) -> String {
    entity_name_from_parts(world_seed, entity.id, entity.kind)
}

fn highlighted(value: String, color: Color) -> Span<'static> {
    Span::styled(
        value,
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

#[cfg(test)]
mod tests {
    use petgraph::stable_graph::NodeIndex;

    use super::{
        build_world_text, render_event_notice, render_interactable_label, render_route_label,
    };
    use crate::domain::events::{
        ContextEntry, DialogueLine, DialogueSpeaker, EntitySummary, PlaceSummary, SystemContext,
    };
    use crate::domain::seed::WorldSeed;
    use crate::domain::time::{GameTime, TimeDelta};
    use crate::domain::vocab::{Biome, Culture, Economy, NpcArchetype, Occupation};
    use crate::graph_ecs::{EntityId, NpcId, PlaceId};
    use crate::simulation::{
        ActorView, CityView, DialoguePartnerView, InteractableOption, InteractionTarget,
        InteractionVerb, PlayerStatusView, RouteView, UiMode, UiSnapshot,
    };
    use crate::world::{
        CityId, DistrictId, EntityKind, PlaceKind, RouteKind, TransportMode, TravelRoute,
        entity_name_from_parts, place_name_from_parts,
    };

    #[test]
    fn world_text_renders_from_typed_snapshot() {
        let snapshot = sample_snapshot();
        let text = build_world_text(&snapshot, &[]);
        let rendered = flatten_text(&text);

        assert!(rendered.contains(&place_name_from_parts(
            snapshot.world_seed,
            snapshot.place.id,
            snapshot.place.district_id,
            snapshot.place.kind
        )));
        assert!(rendered.contains(&snapshot.city.id.name(snapshot.world_seed)));
        assert!(rendered.contains("Current conversation:"));
        assert!(rendered.contains(&snapshot.nearby_actors[0].id.name(snapshot.world_seed)));
        assert!(rendered.contains("Routes from here:"));
        assert!(rendered.contains("Recent Context"));
    }

    #[test]
    fn interactable_labels_resolve_from_typed_views() {
        let snapshot = sample_snapshot();

        let talk_label = render_interactable_label(snapshot.world_seed, &snapshot.interactables[0]);
        let inspect_label =
            render_interactable_label(snapshot.world_seed, &snapshot.interactables[2]);

        assert_eq!(
            talk_label,
            format!(
                "{} - talk (journalist, watcher)",
                snapshot.nearby_actors[0].id.name(snapshot.world_seed)
            )
        );
        assert_eq!(
            inspect_label,
            format!(
                "{} - inspect bag",
                entity_name_from_parts(
                    snapshot.world_seed,
                    snapshot.nearby_entities[0].id,
                    snapshot.nearby_entities[0].kind,
                )
            )
        );
    }

    #[test]
    fn route_labels_render_from_route_view() {
        let snapshot = sample_snapshot();
        let label = render_route_label(snapshot.world_seed, &snapshot.routes[0]);
        assert_eq!(
            label,
            format!(
                "{} via arterial road (10m 00s)",
                place_name_from_parts(
                    snapshot.world_seed,
                    snapshot.routes[0].destination.id,
                    snapshot.routes[0].destination.district_id,
                    snapshot.routes[0].destination.kind,
                )
            )
        );
    }

    #[test]
    fn event_notices_render_from_typed_events() {
        let snapshot = sample_snapshot();
        let travel_notice = render_event_notice(
            snapshot.world_seed,
            &crate::domain::events::GameEvent::TravelCompleted {
                destination: PlaceSummary {
                    id: snapshot.routes[0].destination.id,
                    district_id: snapshot.routes[0].destination.district_id,
                    kind: snapshot.routes[0].destination.kind,
                },
                transport_mode: TransportMode::Walking,
                route: snapshot.routes[0].route,
                duration: TimeDelta::from_seconds(600),
            },
        );
        let expected = format!(
            "You travel to {} by walk on arterial road in 10m 00s.",
            place_name_from_parts(
                snapshot.world_seed,
                snapshot.routes[0].destination.id,
                snapshot.routes[0].destination.district_id,
                snapshot.routes[0].destination.kind,
            )
        );
        assert_eq!(travel_notice.as_deref(), Some(expected.as_str()));
    }

    fn sample_snapshot() -> UiSnapshot {
        let world_seed = WorldSeed::new(42);
        let city_id = CityId(NodeIndex::new(0));
        let place_id = PlaceId(NodeIndex::new(1));
        let market_district = DistrictId {
            city_id,
            district_index: 0,
        };
        let station_district = DistrictId {
            city_id,
            district_index: 1,
        };
        let route_destination = PlaceSummary {
            id: PlaceId(NodeIndex::new(2)),
            district_id: station_district,
            kind: PlaceKind::StationPlatform,
        };
        let actor = ActorView {
            id: NpcId(NodeIndex::new(3)),
            occupation: Occupation::Journalist,
            archetype: NpcArchetype::Watcher,
        };
        let car = EntitySummary {
            id: EntityId(NodeIndex::new(4)),
            kind: EntityKind::Car,
        };
        let bag = EntitySummary {
            id: EntityId(NodeIndex::new(5)),
            kind: EntityKind::Bag,
        };

        UiSnapshot {
            world_seed,
            mode: UiMode::Dialogue,
            status: PlayerStatusView {
                clock: GameTime::from_seconds(29_400),
                transport_mode: TransportMode::Walking,
                known_city_count: 3,
            },
            city: CityView {
                id: city_id,
                biome: Biome::Coastal,
                economy: Economy::Trade,
                culture: Culture::CivicMinded,
                districts: vec![market_district, station_district],
                landmarks: vec![crate::world::LandmarkId {
                    city_id,
                    landmark_index: 0,
                }],
            },
            place: PlaceSummary {
                id: place_id,
                district_id: market_district,
                kind: PlaceKind::SidewalkLeft,
            },
            dialogue_partner: Some(DialoguePartnerView {
                actor: actor.clone(),
                memory: Some(crate::domain::memory::ConversationMemory {
                    summary: "The player followed up on a local lead.".to_string(),
                }),
            }),
            routes: vec![RouteView {
                destination: route_destination,
                route: TravelRoute {
                    kind: RouteKind::ArterialRoad,
                    walking_time: crate::domain::time::TimeDelta::from_seconds(600),
                    transit_time: Some(crate::domain::time::TimeDelta::from_seconds(240)),
                    driving_time: Some(crate::domain::time::TimeDelta::from_seconds(120)),
                },
                travel_time: Some(TimeDelta::from_seconds(600)),
            }],
            interactables: vec![
                InteractableOption {
                    target: InteractionTarget::Npc(actor.id),
                    verb: InteractionVerb::Talk,
                    subject: crate::simulation::InteractableSubjectView::Actor(actor.clone()),
                },
                InteractableOption {
                    target: InteractionTarget::Entity(car.id),
                    verb: InteractionVerb::EnterVehicle,
                    subject: crate::simulation::InteractableSubjectView::Entity(car.clone()),
                },
                InteractableOption {
                    target: InteractionTarget::Entity(bag.id),
                    verb: InteractionVerb::Inspect,
                    subject: crate::simulation::InteractableSubjectView::Entity(bag.clone()),
                },
            ],
            nearby_actors: vec![actor],
            nearby_cars: vec![car],
            nearby_entities: vec![bag],
            context_feed: vec![
                ContextEntry::System {
                    timestamp: GameTime::from_seconds(28_800),
                    context: SystemContext::Start,
                },
                ContextEntry::Dialogue(DialogueLine {
                    timestamp: GameTime::from_seconds(28_830),
                    speaker: DialogueSpeaker::Npc(NpcId(NodeIndex::new(3))),
                    text: "You should start at the station before the crowds thicken.".to_string(),
                }),
            ],
        }
    }

    fn flatten_text(text: &ratatui::text::Text<'_>) -> String {
        text.lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
