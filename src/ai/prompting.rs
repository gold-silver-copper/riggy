use crate::ai::context::NpcDialogueContext;
use crate::domain::events::PlaceSummary;
use crate::domain::memory::ConversationMemory;
use crate::domain::seed::WorldSeed;
use crate::world::place_name_from_parts;

pub fn build_dialogue_prompt(context: &NpcDialogueContext) -> String {
    format!(
        "World seed: {world_seed}\nTime: {time_label} ({time_seconds} seconds)\nCity: {city} ({biome}, {economy}, {culture})\nConnected cities: {connected_cities}\nCurrent place: {current_place}\nNPC: {npc}, a {occupation} and {archetype}\nHome place: {home_place}\nTraits: {traits}\nGoal: {goal}\nConversation memory: {memory}\n\nPlayer says: {player_input}\n\nReply as the NPC in 2-4 sentences. Stay grounded in the city, the current place, and the NPC's motives. Refer only to facts present in this context or naturally implied by them.",
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
        npc = context.npc.name(context.world_seed),
        occupation = context.npc.occupation.label(),
        archetype = context.npc.archetype.label(),
        home_place = context.npc.home_place_name(context.world_seed),
        traits = render_list(
            context
                .npc
                .traits
                .iter()
                .map(|trait_tag| trait_tag.label().to_string())
        ),
        goal = context.npc.goal.label(),
        memory = render_conversation_memory(&context.memory),
        player_input = context.turn.player_input
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

#[cfg(test)]
mod tests {
    use super::build_dialogue_prompt;

    use crate::ai::context::{CityContext, DialogueTurnContext, NpcContext, NpcDialogueContext};
    use crate::domain::events::{DialogueLine, DialogueSpeaker, PlaceSummary};
    use crate::domain::memory::ConversationMemory;
    use crate::domain::seed::WorldSeed;
    use crate::domain::vocab::{
        Biome, Culture, Economy, GoalTag, NpcArchetype, Occupation, TraitTag,
    };
    use crate::world::{CityId, NpcId, PlaceKind, PlaceId};

    #[test]
    fn dialogue_prompt_renders_from_context_fixture() {
        let context = sample_context();
        let prompt = build_dialogue_prompt(&context);

        assert!(prompt.contains(&context.city.name(context.world_seed)));
        assert!(prompt.contains("journalist"));
        assert!(prompt.contains("watcher"));
        assert!(prompt.contains("civic-minded"));
        assert!(prompt.contains("What is this city like?"));
        assert!(prompt.contains(&context.current_time.format()));
        assert!(prompt.contains(&context.npc.name(context.world_seed)));
        assert!(prompt.contains(&context.npc.home_place_name(context.world_seed)));
        assert!(prompt.contains("Current place"));
        assert!(prompt.contains("Conversation memory"));
        assert!(prompt.contains("The player followed up on a local lead."));
    }

    fn sample_context() -> NpcDialogueContext {
        NpcDialogueContext {
            world_seed: WorldSeed::new(42),
            current_time: crate::domain::time::GameTime::from_seconds(29_400),
            city: CityContext {
                id: CityId(1.into()),
                biome: Biome::Coastal,
                economy: Economy::Trade,
                culture: Culture::CivicMinded,
                connected_cities: vec![CityId(7.into())],
            },
            current_place: PlaceSummary {
                id: PlaceId(3.into()),
                city_id: CityId(1.into()),
                kind: PlaceKind::Venue,
            },
            npc: NpcContext {
                id: NpcId(9.into()),
                archetype: NpcArchetype::Watcher,
                occupation: Occupation::Journalist,
                traits: vec![TraitTag::Guarded, TraitTag::Ambitious],
                goal: GoalTag::ExposeRecordsLeak,
                home_place: PlaceSummary {
                    id: PlaceId(1.into()),
                    city_id: CityId(1.into()),
                    kind: PlaceKind::Residence,
                },
            },
            memory: ConversationMemory {
                summary: "The player followed up on a local lead.".to_string(),
            },
            turn: DialogueTurnContext {
                transcript: vec![DialogueLine {
                    timestamp: crate::domain::time::GameTime::from_seconds(29_390),
                    speaker: DialogueSpeaker::Player,
                    text: "hello".to_string(),
                }],
                player_input: "What is this city like?".to_string(),
            },
        }
    }
}
