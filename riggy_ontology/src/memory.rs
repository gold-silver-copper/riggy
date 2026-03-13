use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema, Default)]
pub struct ConversationMemory {
    #[serde(default)]
    pub summary: String,
}

impl ConversationMemory {
    pub fn normalized(mut self) -> Self {
        self.summary = self.summary.trim().to_string();
        self
    }

    pub fn is_empty(&self) -> bool {
        self.summary.trim().is_empty()
    }

    pub fn merge_update(&mut self, update: ConversationMemory) {
        let update_summary = update.summary.trim();
        if update_summary.is_empty() {
            return;
        }
        if self.summary.trim().is_empty() {
            self.summary = update_summary.to_string();
            return;
        }
        if self.summary.contains(update_summary) {
            return;
        }
        self.summary = format!("{} | {}", self.summary.trim(), update_summary);
    }
}

#[cfg(test)]
mod tests {
    use super::ConversationMemory;

    #[test]
    fn normalization_trims_summary() {
        let memory = ConversationMemory {
            summary: "  talked about local work ".to_string(),
        }
        .normalized();

        assert_eq!(memory.summary, "talked about local work");
    }

    #[test]
    fn merge_update_appends_new_summary_once() {
        let mut memory = ConversationMemory {
            summary: "Talked about city records.".to_string(),
        };
        memory.merge_update(ConversationMemory {
            summary: "Talked about station traffic.".to_string(),
        });
        memory.merge_update(ConversationMemory {
            summary: "Talked about station traffic.".to_string(),
        });

        assert_eq!(
            memory.summary,
            "Talked about city records. | Talked about station traffic."
        );
    }
}
