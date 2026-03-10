use crate::ai::context::NpcDialogueContextV1;

pub fn build_dialogue_prompt_v1(context: &NpcDialogueContextV1) -> String {
    format!(
        "Context contract: NpcDialogueContextV1 (v{version})\nWorld seed: {world_seed}\nTime: {time_label} ({time_seconds} seconds)\nCity: {city} ({biome}, {economy}, {culture})\nDistricts: {districts}\nLandmarks: {landmarks}\nConnected cities: {connected_cities}\nNPC: {npc}, a {occupation} and {archetype}\nHome district: {home_district}\nTraits: {traits}\nGoal: {goal}\nDisposition toward player: {disposition}\nRelationship memory: {memory}\n\nPlayer says: {player_input}\n\nReply as the NPC in 2-4 sentences. Stay grounded in the city and the NPC's motives. Refer only to facts present in this context or naturally implied by them.",
        version = context.version,
        world_seed = context.world_seed,
        time_label = context.clock.current_time_label,
        time_seconds = context.clock.current_time_seconds,
        city = context.city.name,
        biome = context.city.biome.label(),
        economy = context.city.economy.label(),
        culture = context.city.culture.label(),
        districts = if context.city.districts.is_empty() {
            "none".to_string()
        } else {
            context.city.districts.join(", ")
        },
        landmarks = if context.city.landmarks.is_empty() {
            "none".to_string()
        } else {
            context.city.landmarks.join(", ")
        },
        connected_cities = if context.city.connected_cities.is_empty() {
            "none".to_string()
        } else {
            context.city.connected_cities.join(", ")
        },
        npc = context.npc.name,
        occupation = context.npc.occupation.label(),
        archetype = context.npc.archetype.label(),
        home_district = context.npc.home_district,
        traits = if context.npc.traits.is_empty() {
            "none".to_string()
        } else {
            context
                .npc
                .traits
                .iter()
                .map(|trait_tag| trait_tag.label())
                .collect::<Vec<_>>()
                .join(", ")
        },
        goal = context.npc.goal.label(),
        disposition = context.relationship.disposition,
        memory = render_relationship_memory(context),
        player_input = context.turn.player_input
    )
}

pub fn build_proposal_prompt_v1(context: &NpcDialogueContextV1, text: &str) -> String {
    format!(
        "Context contract: NpcDialogueContextV1 (v{version})\nYou are extracting conservative AI proposals from an NPC response.\nAllowed proposals: relationship_adjustment, no_change.\nOnly emit proposals justified by the NPC response and keep them conservative.\n\nCity: {city}\nNPC: {npc}\nPlayer input: {player_input}\nNPC response: {response}",
        version = context.version,
        city = context.city.name,
        npc = context.npc.name,
        player_input = context.turn.player_input,
        response = text
    )
}

fn render_relationship_memory(context: &NpcDialogueContextV1) -> String {
    [
        format!(
            "trust delta summary: {}",
            context.relationship.trust_delta_summary
        ),
        format!(
            "known topics: {}",
            if context.relationship.known_topics.is_empty() {
                "none".to_string()
            } else {
                context.relationship.known_topics.join(", ")
            }
        ),
        format!(
            "unresolved threads: {}",
            if context.relationship.unresolved_threads.is_empty() {
                "none".to_string()
            } else {
                context.relationship.unresolved_threads.join(", ")
            }
        ),
        format!(
            "freeform summary: {}",
            if context.relationship.freeform_summary.trim().is_empty() {
                "none".to_string()
            } else {
                context.relationship.freeform_summary.clone()
            }
        ),
    ]
    .join(" | ")
}

#[cfg(test)]
mod tests {
    use crate::ai::context::{
        CityContextV1, DialogueClockV1, DialogueTranscriptLineV1, DialogueTranscriptSpeakerV1,
        DialogueTurnContextV1, NpcContextV1, NpcDialogueContextV1, RelationshipMemoryViewV1,
    };
    use crate::domain::vocab::{
        Biome, Culture, Economy, GoalTag, NpcArchetype, Occupation, TraitTag,
    };

    use super::{build_dialogue_prompt_v1, build_proposal_prompt_v1};

    #[test]
    fn dialogue_prompt_renders_from_context_fixture() {
        let prompt = build_dialogue_prompt_v1(&sample_context());

        assert!(prompt.contains("NpcDialogueContextV1"));
        assert!(prompt.contains("Ashcrest"));
        assert!(prompt.contains("journalist"));
        assert!(prompt.contains("watcher"));
        assert!(prompt.contains("civic-minded"));
        assert!(prompt.contains("What is this city like?"));
        assert!(prompt.contains("known topics: local lead"));
        assert!(prompt.contains("unresolved threads: Call back tomorrow"));
    }

    #[test]
    fn proposal_prompt_renders_from_context_fixture() {
        let prompt =
            build_proposal_prompt_v1(&sample_context(), "I might trust you with this later.");

        assert!(prompt.contains("NpcDialogueContextV1"));
        assert!(prompt.contains("Allowed proposals"));
        assert!(prompt.contains("Ashcrest"));
        assert!(prompt.contains("Yana Orchard"));
        assert!(prompt.contains("I might trust you with this later."));
    }

    fn sample_context() -> NpcDialogueContextV1 {
        NpcDialogueContextV1 {
            version: 1,
            world_seed: 42,
            clock: DialogueClockV1 {
                current_time_seconds: 29_400,
                current_time_label: "Day 1 08:10:00".to_string(),
            },
            city: CityContextV1 {
                name: "Ashcrest".to_string(),
                biome: Biome::Coastal,
                economy: Economy::Trade,
                culture: Culture::CivicMinded,
                districts: vec!["Market District".to_string(), "Station Quarter".to_string()],
                landmarks: vec!["Old Exchange".to_string()],
                connected_cities: vec!["Lowharbor".to_string()],
            },
            npc: NpcContextV1 {
                name: "Yana Orchard".to_string(),
                archetype: NpcArchetype::Watcher,
                occupation: Occupation::Journalist,
                traits: vec![TraitTag::Guarded, TraitTag::Ambitious],
                goal: GoalTag::ExposeRecordsLeak,
                home_district: "Station Quarter".to_string(),
            },
            relationship: RelationshipMemoryViewV1 {
                disposition: 2,
                trust_delta_summary: 1,
                known_topics: vec!["local lead".to_string()],
                unresolved_threads: vec!["Call back tomorrow".to_string()],
                freeform_summary: "The player followed up on a local lead.".to_string(),
            },
            turn: DialogueTurnContextV1 {
                transcript: vec![DialogueTranscriptLineV1 {
                    speaker: DialogueTranscriptSpeakerV1::Player,
                    text: "hello".to_string(),
                }],
                player_input: "What is this city like?".to_string(),
            },
        }
    }
}
