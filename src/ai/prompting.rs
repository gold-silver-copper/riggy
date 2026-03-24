use crate::ai::context::{ActorTurnContext, LocalStateContext};
use crate::domain::commands::AgentAvailableAction;
use crate::domain::events::{DialogueSpeaker, PlaceSummary};
use crate::domain::memory::ConversationMemory;
use crate::domain::seed::WorldSeed;
use crate::world::place_name_from_parts;

pub fn build_turn_prompt(context: &ActorTurnContext) -> String {
    format!(
        "Time: {time_label} ({time_seconds}s)\nCurrent place: {current_place}\nYou are actor #{actor_id}: {actor} ({occupation}, {archetype})\nHome: {home_place}\nTraits: {traits}\nGoal: {goal}\nMemory: {memory}\nNearby actors: {nearby_actors}\nNearby entities: {nearby_entities}\nRoutes: {routes}\nAvailable actions:\n{available_actions}\nRecent speech:\n{recent_speech}\n\nDecide the next action for this actor. You may initiate a conversation, continue one, move to another room, inspect something nearby, or stay idle. Recent speech matters, but does not force a reply. The tools are already scoped to you, so never invent or repeat your own actor_id. Call exactly one available tool: speak_to, move_to, inspect_entity, or do_nothing. If you use speak_to, provide only the exact words spoken, 1-3 short sentences, no narration, no speaker labels, and no stage directions. Do not narrate. Do not explain. Call one tool and stop.",
        time_label = context.current_time.format(),
        time_seconds = context.current_time.seconds(),
        current_place = render_place(context.world_seed, context.current_place),
        actor_id = context.actor.id.index(),
        actor = context.actor.name(context.world_seed),
        occupation = context.actor.occupation.label(),
        archetype = context.actor.archetype.label(),
        home_place = context.actor.home_place_name(context.world_seed),
        traits = render_list(
            context
                .actor
                .traits
                .iter()
                .map(|trait_tag| trait_tag.label().to_string())
        ),
        goal = context.actor.goal.label(),
        memory = render_conversation_memory(&context.memory),
        recent_speech = render_recent_speech(context),
        nearby_actors = render_nearby_actors(context.world_seed, &context.local_state),
        nearby_entities = render_nearby_entities(&context.local_state),
        routes = render_routes(context.world_seed, &context.local_state),
        available_actions = render_available_actions(context.world_seed, context),
    )
}

fn render_place(world_seed: WorldSeed, place: PlaceSummary) -> String {
    format!(
        "{} ({})",
        place_name_from_parts(world_seed, place.id, place.city_id, place.kind),
        place.kind.label()
    )
}

fn render_list(values: impl Iterator<Item = String>) -> String {
    let rendered = values.collect::<Vec<_>>();
    if rendered.is_empty() {
        "none".to_string()
    } else {
        rendered.join(", ")
    }
}

fn render_conversation_memory(memory: &ConversationMemory) -> String {
    if memory.summary.trim().is_empty() {
        "none".to_string()
    } else {
        memory.summary.clone()
    }
}

fn render_recent_speech(context: &ActorTurnContext) -> String {
    if context.recent_speech.is_empty() {
        return "none".to_string();
    }

    context
        .recent_speech
        .iter()
        .map(|line| {
            let speaker = match line.speaker {
                DialogueSpeaker::Actor(actor_id) if actor_id == context.actor.id => {
                    "you".to_string()
                }
                DialogueSpeaker::Actor(actor_id) => actor_id.name(context.world_seed),
                DialogueSpeaker::System => "system".to_string(),
            };
            format!(
                "- {} at {}: {}",
                speaker,
                line.timestamp.format(),
                line.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_nearby_actors(world_seed: WorldSeed, local_state: &LocalStateContext) -> String {
    if local_state.nearby_actors.is_empty() {
        "none".to_string()
    } else {
        local_state
            .nearby_actors
            .iter()
            .map(|actor| {
                format!(
                    "{} actor#{} ({}, {})",
                    actor.name(world_seed),
                    actor.id.index(),
                    actor.occupation.label(),
                    actor.archetype.label(),
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn render_nearby_entities(local_state: &LocalStateContext) -> String {
    if local_state.nearby_entities.is_empty() {
        "none".to_string()
    } else {
        local_state
            .nearby_entities
            .iter()
            .map(|entity| format!("entity#{} ({})", entity.id.index(), entity.kind.label()))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn render_routes(world_seed: WorldSeed, local_state: &LocalStateContext) -> String {
    if local_state.routes.is_empty() {
        "none".to_string()
    } else {
        local_state
            .routes
            .iter()
            .map(|route| {
                format!(
                    "place#{} {} in {}s",
                    route.destination.id.index(),
                    render_place(world_seed, route.destination),
                    route.travel_time.seconds()
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn render_available_actions(world_seed: WorldSeed, context: &ActorTurnContext) -> String {
    context
        .available_actions
        .iter()
        .map(|action| match action {
            AgentAvailableAction::MoveTo { destination } => format!(
                "- move_to destination_place_id={} ({})",
                destination.index(),
                render_place(
                    world_seed,
                    context
                        .local_state
                        .routes
                        .iter()
                        .find_map(|route| {
                            (route.destination.id == *destination).then_some(route.destination)
                        })
                        .unwrap_or(PlaceSummary {
                            id: *destination,
                            city_id: context.current_place.city_id,
                            kind: context.current_place.kind,
                        })
                )
            ),
            AgentAvailableAction::SpeakTo { target } => {
                format!(
                    "- speak_to target_actor_id={} ({})",
                    target.index(),
                    target.name(world_seed)
                )
            }
            AgentAvailableAction::InspectEntity { entity_id } => {
                format!("- inspect_entity entity_id={}", entity_id.index())
            }
            AgentAvailableAction::DoNothing => "- do_nothing".to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use crate::ai::context::build_actor_turn_context;
    use crate::domain::seed::WorldSeed;
    use crate::domain::time::GameTime;
    use crate::world::World;

    use super::build_turn_prompt;

    #[test]
    fn turn_prompt_mentions_generic_tools_and_not_legacy_names() {
        let world = World::generate(WorldSeed::new(42), 18);
        let actor_id = world
            .actor_ids()
            .into_iter()
            .find(|candidate| {
                world.actor(*candidate).controller == crate::world::ControllerMode::AiAgent
            })
            .expect("expected an ai actor");
        let context = build_actor_turn_context(
            &world,
            GameTime::from_seconds(0),
            actor_id,
            vec![
                crate::domain::commands::AgentAvailableAction::SpeakTo {
                    target: world.manual_actor_id().unwrap(),
                },
                crate::domain::commands::AgentAvailableAction::MoveTo {
                    destination: world.place_routes(world.actor_place_id(actor_id).unwrap())[0].0,
                },
                crate::domain::commands::AgentAvailableAction::DoNothing,
            ],
        )
        .unwrap();

        let prompt = build_turn_prompt(&context);

        assert!(prompt.contains("speak_to"));
        assert!(prompt.contains("move_to"));
        assert!(prompt.contains("do_nothing"));
        assert!(!prompt.contains("reply_to_speaker"));
        assert!(!prompt.contains("perform_action"));
    }
}
