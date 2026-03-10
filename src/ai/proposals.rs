use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct ProposedProposals {
    #[serde(default)]
    pub proposals: Vec<AiProposal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AiProposal {
    NoChange,
    RelationshipAdjustment(RelationshipAdjustmentProposal),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct RelationshipAdjustmentProposal {
    pub delta: i32,
    #[serde(default)]
    pub note: String,
}
