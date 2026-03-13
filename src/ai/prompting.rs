use crate::ai::context::NpcDialogueContext;
use crate::domain::memory::ConversationMemory;

pub fn build_dialogue_prompt(context: &NpcDialogueContext) -> String {
    format!(
        "World seed: {world_seed}\nTime: {time_label} ({time_seconds} seconds)\nCity: {city} ({biome}, {economy}, {culture})\nDistricts: {districts}\nLandmarks: {landmarks}\nConnected cities: {connected_cities}\nNPC: {npc}, a {occupation} and {archetype}\nHome district: {home_district}\nTraits: {traits}\nGoal: {goal}\nConversation memory: {memory}\n\nPlayer says: {player_input}\n\nReply as the NPC in 2-4 sentences. Stay grounded in the city and the NPC's motives. Refer only to facts present in this context or naturally implied by them.",
        world_seed = context.world_seed,
        time_label = context.current_time.format(),
        time_seconds = context.current_time.seconds(),
        city = context.city.name(context.world_seed),
        biome = context.city.biome.label(),
        economy = context.city.economy.label(),
        culture = context.city.culture.label(),
        districts = render_list(
            context
                .city
                .districts
                .iter()
                .map(|district| district.name(context.world_seed))
        ),
        landmarks = render_list(
            context
                .city
                .landmarks
                .iter()
                .map(|landmark| landmark.name(context.world_seed))
        ),
        connected_cities = render_list(
            context
                .city
                .connected_cities
                .iter()
                .map(|city| city.name(context.world_seed))
        ),
        npc = context.npc.name(context.world_seed),
        occupation = context.npc.occupation.label(),
        archetype = context.npc.archetype.label(),
        home_district = context.npc.home_district_name(context.world_seed),
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
    use crate::ai::context::{CityContext, DialogueTurnContext, NpcContext, NpcDialogueContext};
    use crate::domain::events::{DialogueLine, DialogueSpeaker};
    use crate::domain::memory::ConversationMemory;
    use crate::domain::seed::WorldSeed;
    use crate::domain::vocab::{
        Biome, Culture, Economy, GoalTag, NpcArchetype, Occupation, TraitTag,
    };
    use crate::world::{DistrictId, LandmarkId};

    use super::build_dialogue_prompt;

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
        assert!(prompt.contains(&context.npc.home_district_name(context.world_seed)));
        assert!(prompt.contains("Conversation memory"));
        assert!(prompt.contains("The player followed up on a local lead."));
    }

    fn sample_context() -> NpcDialogueContext {
        NpcDialogueContext {
            world_seed: WorldSeed::new(42),
            current_time: crate::domain::time::GameTime::from_seconds(29_400),
            city: CityContext {
                id: crate::world::CityId(petgraph::stable_graph::NodeIndex::new(1)),
                biome: Biome::Coastal,
                economy: Economy::Trade,
                culture: Culture::CivicMinded,
                districts: vec![
                    DistrictId {
                        city_id: crate::world::CityId(petgraph::stable_graph::NodeIndex::new(1)),
                        district_index: 0,
                    },
                    DistrictId {
                        city_id: crate::world::CityId(petgraph::stable_graph::NodeIndex::new(1)),
                        district_index: 1,
                    },
                ],
                landmarks: vec![LandmarkId {
                    city_id: crate::world::CityId(petgraph::stable_graph::NodeIndex::new(1)),
                    landmark_index: 0,
                }],
                connected_cities: vec![crate::world::CityId(
                    petgraph::stable_graph::NodeIndex::new(7),
                )],
            },
            npc: NpcContext {
                id: crate::world::NpcId(petgraph::stable_graph::NodeIndex::new(9)),
                archetype: NpcArchetype::Watcher,
                occupation: Occupation::Journalist,
                traits: vec![TraitTag::Guarded, TraitTag::Ambitious],
                goal: GoalTag::ExposeRecordsLeak,
                home_district: DistrictId {
                    city_id: crate::world::CityId(petgraph::stable_graph::NodeIndex::new(1)),
                    district_index: 1,
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
