use crate::ai::context::ActorDialogueContext;
use crate::domain::events::PlaceSummary;
use crate::domain::memory::ConversationMemory;
use crate::domain::seed::WorldSeed;
use crate::world::place_name_from_parts;

pub fn build_dialogue_prompt(context: &ActorDialogueContext) -> String {
    format!(
        "World seed: {world_seed}\nTime: {time_label} ({time_seconds} seconds)\nCity: {city} ({biome}, {economy}, {culture})\nConnected cities: {connected_cities}\nCurrent place: {current_place}\nYou are: {actor}, a {occupation} and {archetype}\nHome place: {home_place}\nTraits: {traits}\nGoal: {goal}\nConversation memory: {memory}\nCounterpart: {counterpart}\n\nCounterpart says: {speaker_input}\n\nReply as this character in 2-4 sentences. Stay grounded in the city, the current place, and the character's motives. Refer only to facts present in this context or naturally implied by them.",
        world_seed = context.world_seed,
        time_label = context.current_time.format(),
        time_seconds = context.current_time.seconds(),
        city = context.city.name(context.world_seed),
        biome = context.city.biome.label(),
        economy = context.city.economy.label(),
        culture = context.city.culture.label(),
        connected_cities = render_list(
            context
                .city
                .connected_cities
                .iter()
                .map(|city| city.name(context.world_seed))
        ),
        current_place = render_place(context.world_seed, context.current_place),
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
        counterpart = context.counterpart.name(context.world_seed),
        speaker_input = context.turn.speaker_input
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
