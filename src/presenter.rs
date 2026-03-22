use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

use crate::domain::events::{
    ContextEntry, DialogueSpeaker, EntitySummary, GameEvent, PlaceSummary, SystemContext,
};
use crate::domain::seed::WorldSeed;
use crate::simulation::{ActorView, Interactable, RouteView, UiSnapshot};
use crate::world::{entity_name_from_parts, place_name_from_parts};

pub fn build_world_title(snapshot: &UiSnapshot) -> Line<'static> {
    let formatter = WorldFormatter::new(snapshot.world_seed);
    Line::from(vec![
        Span::styled(
            format!(
                "{} ({})",
                formatter.place(&snapshot.place),
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
    let formatter = WorldFormatter::new(snapshot.world_seed);
    let mut lines = vec![
        Line::from(vec![
            Span::raw("You are in "),
            highlighted(formatter.place(&snapshot.place), Color::Yellow),
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
            Span::raw("  |  Known cities: "),
            highlighted(snapshot.status.known_city_count.to_string(), Color::Green),
        ]),
        Line::from(""),
    ];

    if let Some(partner) = &snapshot.dialogue_partner {
        lines.push(Line::from(vec![
            Span::raw("Current conversation: "),
            highlighted(formatter.actor(&partner.actor), Color::Magenta),
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

    if !snapshot.city.connected_cities.is_empty() {
        push_list_section(
            &mut lines,
            "Connected cities",
            snapshot
                .city
                .connected_cities
                .iter()
                .map(|city_id| city_id.name(snapshot.world_seed)),
            Color::Green,
            ", ",
        );
    }

    let people_here = snapshot
        .interactables
        .iter()
        .filter_map(|interactable| match interactable {
            Interactable::Talk(actor) => Some(format!(
                "{} - {}, {}",
                formatter.actor(actor),
                actor.occupation.label(),
                actor.archetype.label()
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !people_here.is_empty() {
        push_list_section(
            &mut lines,
            "People here",
            people_here.into_iter(),
            Color::Magenta,
            " | ",
        );
    }

    let other_details = snapshot
        .interactables
        .iter()
        .filter_map(|interactable| match interactable {
            Interactable::Inspect(entity) => Some(format!(
                "{} ({})",
                formatter.entity(entity),
                entity.kind.label()
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !other_details.is_empty() {
        push_list_section(
            &mut lines,
            "Other notable details",
            other_details.into_iter(),
            Color::Cyan,
            " | ",
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
    let destination_name = WorldFormatter::new(world_seed).place(&option.destination);
    format!(
        "{} via {} ({})",
        destination_name,
        option.route.kind.label(),
        format_duration(option.travel_time),
    )
}

pub fn render_interactable_label(world_seed: WorldSeed, interactable: &Interactable) -> String {
    let formatter = WorldFormatter::new(world_seed);
    match interactable {
        Interactable::Talk(actor) => format!(
            "{} - talk ({}, {})",
            formatter.actor(actor),
            actor.occupation.label(),
            actor.archetype.label()
        ),
        Interactable::Inspect(entity) => {
            format!(
                "{} - inspect {}",
                formatter.entity(entity),
                entity.kind.label()
            )
        }
    }
}

pub fn render_event_notice(world_seed: WorldSeed, event: &GameEvent) -> Option<String> {
    WorldFormatter::new(world_seed).event_notice(event)
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
    WorldFormatter::new(world_seed).system_context(context)
}

fn dialogue_speaker_label(world_seed: WorldSeed, speaker: DialogueSpeaker) -> String {
    WorldFormatter::new(world_seed).speaker(speaker)
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

fn highlighted(value: String, color: Color) -> Span<'static> {
    Span::styled(
        value,
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

struct WorldFormatter {
    seed: WorldSeed,
}

impl WorldFormatter {
    fn new(seed: WorldSeed) -> Self {
        Self { seed }
    }

    fn place(&self, place: &PlaceSummary) -> String {
        place_name_from_parts(self.seed, place.id, place.city_id, place.kind)
    }

    fn entity(&self, entity: &EntitySummary) -> String {
        entity_name_from_parts(self.seed, entity.id, entity.kind)
    }

    fn actor(&self, actor: &ActorView) -> String {
        actor.id.name(self.seed)
    }

    fn speaker(&self, speaker: DialogueSpeaker) -> String {
        match speaker {
            DialogueSpeaker::Player => "You".to_string(),
            DialogueSpeaker::Npc(npc_id) => npc_id.name(self.seed),
            DialogueSpeaker::System => "System".to_string(),
        }
    }

    fn system_context(&self, context: &SystemContext) -> String {
        match context {
            SystemContext::Start => {
                "You arrived in a starter residence with a need for useful names.".to_string()
            }
            SystemContext::Travel { destination, duration } => format!(
                "Arrived at {} after {}.",
                self.place(destination),
                format_duration(*duration)
            ),
        }
    }

    fn event_notice(&self, event: &GameEvent) -> Option<String> {
        match event {
            GameEvent::DialogueStarted { npc_id } => Some(format!(
                "You approach {}. Type normally to speak, or press Esc to end the conversation.",
                npc_id.name(self.seed)
            )),
            GameEvent::DialogueLineRecorded { .. } => None,
            GameEvent::DialogueEnded { npc_id } => {
                Some(format!("You step away from {}.", npc_id.name(self.seed)))
            }
            GameEvent::TravelCompleted {
                destination,
                route,
                duration,
            } => Some(format!(
                "You travel to {} using {} in {}.",
                self.place(destination),
                route.kind.label(),
                format_duration(*duration)
            )),
            GameEvent::EntityInspected { entity } => Some(format!(
                "You inspect {}. It looks like a {} left out in plain view.",
                self.entity(entity),
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
}

#[cfg(test)]
mod tests {
    use super::{
        build_world_text, render_event_notice, render_interactable_label, render_route_label,
    };
    use crate::domain::events::{
        ContextEntry, DialogueLine, DialogueSpeaker, EntitySummary, PlaceSummary, SystemContext,
    };
    use crate::domain::seed::WorldSeed;
    use crate::domain::time::{GameTime, TimeDelta};
    use crate::domain::vocab::{Biome, Culture, Economy, NpcArchetype, Occupation};
    use crate::simulation::{
        ActorView, CityView, DialoguePartnerView, Interactable, PlayerStatusView, RouteView,
        UiMode, UiSnapshot,
    };
    use crate::world::{
        CityId, EntityId, EntityKind, NpcId, PlaceId, PlaceKind, RouteKind, TravelRoute,
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
            snapshot.place.city_id,
            snapshot.place.kind
        )));
        assert!(rendered.contains(&snapshot.city.id.name(snapshot.world_seed)));
        assert!(rendered.contains("Current conversation:"));
        assert!(rendered.contains("Connected cities:"));
        assert!(rendered.contains("Routes from here:"));
        assert!(rendered.contains("Recent Context"));
    }

    #[test]
    fn interactable_labels_resolve_from_typed_views() {
        let snapshot = sample_snapshot();

        let talk_label = render_interactable_label(snapshot.world_seed, &snapshot.interactables[0]);
        let inspect_label =
            render_interactable_label(snapshot.world_seed, &snapshot.interactables[1]);

        assert_eq!(
            talk_label,
            format!(
                "{} - talk (journalist, watcher)",
                match snapshot.interactables[0] {
                    Interactable::Talk(actor) => actor.id.name(snapshot.world_seed),
                    _ => unreachable!("expected talk interactable"),
                }
            )
        );
        assert_eq!(
            inspect_label,
            format!(
                "{} - inspect bag",
                match snapshot.interactables[1] {
                    Interactable::Inspect(entity) => {
                        entity_name_from_parts(snapshot.world_seed, entity.id, entity.kind)
                    }
                    _ => unreachable!("expected inspect interactable"),
                }
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
                "{} via transit line (10m 00s)",
                place_name_from_parts(
                    snapshot.world_seed,
                    snapshot.routes[0].destination.id,
                    snapshot.routes[0].destination.city_id,
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
                    city_id: snapshot.routes[0].destination.city_id,
                    kind: snapshot.routes[0].destination.kind,
                },
                route: snapshot.routes[0].route,
                duration: TimeDelta::from_seconds(600),
            },
        );
        let expected = format!(
            "You travel to {} using transit line in 10m 00s.",
            place_name_from_parts(
                snapshot.world_seed,
                snapshot.routes[0].destination.id,
                snapshot.routes[0].destination.city_id,
                snapshot.routes[0].destination.kind,
            )
        );
        assert_eq!(travel_notice.as_deref(), Some(expected.as_str()));
    }

    fn sample_snapshot() -> UiSnapshot {
        let world_seed = WorldSeed::new(42);
        let city_id = CityId(0.into());
        let place_id = PlaceId(1.into());
        let route_destination = PlaceSummary {
            id: PlaceId(2.into()),
            city_id,
            kind: PlaceKind::Station,
        };
        let actor = ActorView {
            id: NpcId(3.into()),
            occupation: Occupation::Journalist,
            archetype: NpcArchetype::Watcher,
        };
        let bag = EntitySummary {
            id: EntityId(4.into()),
            kind: EntityKind::Bag,
        };

        UiSnapshot {
            world_seed,
            mode: UiMode::Dialogue,
            status: PlayerStatusView {
                clock: GameTime::from_seconds(29_400),
                known_city_count: 3,
            },
            city: CityView {
                id: city_id,
                biome: Biome::Coastal,
                economy: Economy::Trade,
                culture: Culture::CivicMinded,
                connected_cities: vec![CityId(1.into()), CityId(2.into())],
            },
            place: PlaceSummary {
                id: place_id,
                city_id,
                kind: PlaceKind::Street,
            },
            dialogue_partner: Some(DialoguePartnerView {
                actor,
                memory: Some(crate::domain::memory::ConversationMemory {
                    summary: "The player followed up on a local lead.".to_string(),
                }),
            }),
            routes: vec![RouteView {
                destination: route_destination,
                route: TravelRoute {
                    kind: RouteKind::Transit,
                    travel_time: crate::domain::time::TimeDelta::from_seconds(600),
                },
                travel_time: TimeDelta::from_seconds(600),
            }],
            interactables: vec![Interactable::Talk(actor), Interactable::Inspect(bag)],
            context_feed: vec![
                ContextEntry::System {
                    timestamp: GameTime::from_seconds(28_800),
                    context: SystemContext::Start,
                },
                ContextEntry::Dialogue(DialogueLine {
                    timestamp: GameTime::from_seconds(28_830),
                    speaker: DialogueSpeaker::Npc(NpcId(3.into())),
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
