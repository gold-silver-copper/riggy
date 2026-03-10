use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema, Default)]
pub struct RelationshipMemory {
    pub trust_delta_summary: i32,
    #[serde(default)]
    pub known_topics: Vec<String>,
    #[serde(default)]
    pub unresolved_threads: Vec<String>,
    #[serde(default)]
    pub freeform_summary: String,
}

impl RelationshipMemory {
    pub fn normalized(mut self) -> Self {
        self.known_topics = normalize_entries(self.known_topics);
        self.unresolved_threads = normalize_entries(self.unresolved_threads);
        self.freeform_summary = self.freeform_summary.trim().to_string();
        self
    }

    pub fn is_empty(&self) -> bool {
        self.trust_delta_summary == 0
            && self.known_topics.is_empty()
            && self.unresolved_threads.is_empty()
            && self.freeform_summary.trim().is_empty()
    }

    pub fn merge_update(&mut self, update: RelationshipMemory) {
        self.trust_delta_summary += update.trust_delta_summary;
        self.known_topics.extend(update.known_topics);
        self.unresolved_threads.extend(update.unresolved_threads);
        if !update.freeform_summary.trim().is_empty() {
            self.freeform_summary = update.freeform_summary;
        }
        *self = self.clone().normalized();
    }
}

fn normalize_entries(values: Vec<String>) -> Vec<String> {
    let mut normalized = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

#[cfg(test)]
mod tests {
    use super::RelationshipMemory;

    #[test]
    fn normalization_trims_and_deduplicates_entries() {
        let memory = RelationshipMemory {
            trust_delta_summary: 1,
            known_topics: vec![
                " local work ".to_string(),
                String::new(),
                "local work".to_string(),
            ],
            unresolved_threads: vec!["  call back  ".to_string(), "call back".to_string()],
            freeform_summary: "  stayed cautious ".to_string(),
        }
        .normalized();

        assert_eq!(memory.known_topics, vec!["local work".to_string()]);
        assert_eq!(memory.unresolved_threads, vec!["call back".to_string()]);
        assert_eq!(memory.freeform_summary, "stayed cautious".to_string());
    }

    #[test]
    fn merge_update_accumulates_structured_memory() {
        let mut memory = RelationshipMemory {
            trust_delta_summary: 1,
            known_topics: vec!["local work".to_string()],
            unresolved_threads: vec!["call back".to_string()],
            freeform_summary: "The player seemed reliable.".to_string(),
        };
        memory.merge_update(RelationshipMemory {
            trust_delta_summary: -1,
            known_topics: vec!["city layout".to_string(), "local work".to_string()],
            unresolved_threads: vec!["check the station".to_string()],
            freeform_summary: "The NPC stayed guarded.".to_string(),
        });

        assert_eq!(memory.trust_delta_summary, 0);
        assert_eq!(
            memory.known_topics,
            vec!["city layout".to_string(), "local work".to_string()]
        );
        assert_eq!(
            memory.unresolved_threads,
            vec!["call back".to_string(), "check the station".to_string()]
        );
        assert_eq!(
            memory.freeform_summary,
            "The NPC stayed guarded.".to_string()
        );
    }
}
