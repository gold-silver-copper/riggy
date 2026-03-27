use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

use crate::domain::events::{
    ContextEntry, DialogueSpeaker, EntitySummary, GameEvent, PlaceSummary, SystemContext,
};
use crate::domain::seed::WorldSeed;
use crate::simulation::{ActorView, Interactable, RouteView, UiSnapshot};
use crate::world::{entity_name_from_parts, place_name_from_parts};

pub fn build_world_title(snapshot: &UiSnapshot) -> Line<'static> {
    let formatter = WorldFormatter::new(snapshot.world_seed, snapshot.focused_actor_id);
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
    let formatter = WorldFormatter::new(snapshot.world_seed, snapshot.focused_actor_id);
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
            Span::raw("You are "),
            highlighted(
                snapshot.focused_actor.id.name(snapshot.world_seed),
                Color::Yellow,
            ),
            Span::raw(", a "),
            highlighted(
                format!(
                    "{} and {}",
                    snapshot.focused_actor.occupation.label(),
                    snapshot.focused_actor.archetype.label()
                ),
                Color::Magenta,
            ),
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
                actor_display_name(snapshot, actor),
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

    let recent_context = build_recent_context_lines(snapshot);
    if !recent_context.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "Recent Activity",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )]));
        lines.extend(recent_context);
    }

    let status_lines = build_status_lines(notices);
    if !status_lines.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "Status",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )]));
        lines.extend(status_lines);
    }

    Text::from(lines)
}

pub fn render_route_label(world_seed: WorldSeed, option: &RouteView) -> String {
    let destination_name =
        WorldFormatter::new(world_seed, crate::world::ActorId(0.into())).place(&option.destination);
    format!(
        "{} via {} ({})",
        destination_name,
        option.route.kind.label(),
        format_duration(option.travel_time),
    )
}

pub fn render_interactable_label(snapshot: &UiSnapshot, interactable: &Interactable) -> String {
    let formatter = WorldFormatter::new(snapshot.world_seed, snapshot.focused_actor_id);
    match interactable {
        Interactable::Talk(actor) => format!(
            "{} - talk ({}, {})",
            actor_display_name(snapshot, actor),
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

pub fn render_event_notice(
    world_seed: WorldSeed,
    actor_id: crate::world::ActorId,
    event: &GameEvent,
) -> Option<String> {
    WorldFormatter::new(world_seed, actor_id).event_notice(event)
}

fn build_recent_context_lines(snapshot: &UiSnapshot) -> Vec<Line<'static>> {
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
                        snapshot.focused_actor_id,
                        context,
                    ))),
                ]));
            }
            ContextEntry::Dialogue(line) => {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        dialogue_speaker_label(
                            snapshot.world_seed,
                            snapshot.focused_actor_id,
                            line.speaker,
                        ),
                        Style::default()
                            .fg(dialogue_speaker_color(
                                snapshot.focused_actor_id,
                                line.speaker,
                            ))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::raw(clean_inline_text(&line.text)),
                ]));
            }
        }
    }

    lines
}

fn build_status_lines(notices: &[String]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    for notice in notices.iter().rev().take(3).rev() {
        for line in notice
            .lines()
            .map(clean_inline_text)
            .filter(|line| !line.is_empty())
        {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "!",
                    Style::default()
                        .fg(Color::LightRed)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::raw(line),
            ]));
        }
    }

    lines
}

fn render_system_context(
    world_seed: WorldSeed,
    actor_id: crate::world::ActorId,
    context: &SystemContext,
) -> String {
    WorldFormatter::new(world_seed, actor_id).system_context(context)
}

fn dialogue_speaker_label(
    world_seed: WorldSeed,
    manual_actor_id: crate::world::ActorId,
    speaker: DialogueSpeaker,
) -> String {
    WorldFormatter::new(world_seed, manual_actor_id).speaker(speaker)
}

fn dialogue_speaker_color(
    manual_actor_id: crate::world::ActorId,
    speaker: DialogueSpeaker,
) -> Color {
    match speaker {
        DialogueSpeaker::Actor(actor_id) if actor_id == manual_actor_id => Color::Yellow,
        DialogueSpeaker::Actor(_) => Color::Magenta,
        DialogueSpeaker::System => Color::Cyan,
    }
}

fn clean_inline_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn actor_display_name(snapshot: &UiSnapshot, actor: &ActorView) -> String {
    if actor.id == snapshot.focused_actor_id {
        return "You".to_string();
    }

    let base_name = actor.id.name(snapshot.world_seed);
    let duplicate_count = std::iter::once(snapshot.focused_actor.id)
        .chain(
            snapshot
                .interactables
                .iter()
                .filter_map(|interactable| match interactable {
                    Interactable::Talk(actor) => Some(actor.id),
                    Interactable::Inspect(_) => None,
                }),
        )
        .filter(|actor_id| actor_id.name(snapshot.world_seed) == base_name)
        .count();

    if duplicate_count > 1 {
        format!("{base_name} (#{})", actor.id.index())
    } else {
        base_name
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
    manual_actor_id: crate::world::ActorId,
}

impl WorldFormatter {
    fn new(seed: WorldSeed, manual_actor_id: crate::world::ActorId) -> Self {
        Self {
            seed,
            manual_actor_id,
        }
    }

    fn place(&self, place: &PlaceSummary) -> String {
        place_name_from_parts(self.seed, place.id, place.city_id, place.kind)
    }

    fn entity(&self, entity: &EntitySummary) -> String {
        entity_name_from_parts(self.seed, entity.id, entity.kind)
    }

    fn speaker(&self, speaker: DialogueSpeaker) -> String {
        match speaker {
            DialogueSpeaker::Actor(actor_id) if actor_id == self.manual_actor_id => {
                "You".to_string()
            }
            DialogueSpeaker::Actor(actor_id) => actor_id.name(self.seed),
            DialogueSpeaker::System => "System".to_string(),
        }
    }

    fn actor_subject(&self, actor_id: crate::world::ActorId) -> String {
        if actor_id == self.manual_actor_id {
            "You".to_string()
        } else {
            actor_id.name(self.seed)
        }
    }

    fn system_context(&self, context: &SystemContext) -> String {
        match context {
            SystemContext::Travel {
                destination,
                duration,
            } => format!(
                "Arrived at {} after {}.",
                self.place(destination),
                format_duration(*duration)
            ),
            SystemContext::Inspect { entity } => format!(
                "You inspect {}. It looks like a {} left out in plain view.",
                self.entity(entity),
                entity.kind.label()
            ),
            SystemContext::Wait {
                duration,
                current_time,
            } => format!(
                "You wait for {}. The time is now {}.",
                format_duration(*duration),
                current_time.format()
            ),
        }
    }

    fn event_notice(&self, event: &GameEvent) -> Option<String> {
        match event {
            GameEvent::SpeechLineRecorded { line } => Some(format!(
                "{} says: {}",
                self.speaker(line.speaker),
                clean_inline_text(&line.text)
            )),
            GameEvent::TravelCompleted {
                actor_id,
                destination,
                route,
                duration,
            } => Some(format!(
                "{} {} to {} using {} in {}.",
                self.actor_subject(*actor_id),
                if *actor_id == self.manual_actor_id {
                    "travel"
                } else {
                    "travels"
                },
                self.place(destination),
                route.kind.label(),
                format_duration(*duration)
            )),
            GameEvent::EntityInspected { actor_id, entity } => Some(format!(
                "{} {} {}. It looks like a {} left out in plain view.",
                self.actor_subject(*actor_id),
                if *actor_id == self.manual_actor_id {
                    "inspect"
                } else {
                    "inspects"
                },
                self.entity(entity),
                entity.kind.label()
            )),
            GameEvent::WaitCompleted {
                actor_id,
                duration,
                current_time,
            } => Some(format!(
                "{} {} for {}. The time is now {}.",
                self.actor_subject(*actor_id),
                if *actor_id == self.manual_actor_id {
                    "wait"
                } else {
                    "waits"
                },
                format_duration(*duration),
                current_time.format()
            )),
            GameEvent::ContextAppended { .. } => None,
        }
    }
}
