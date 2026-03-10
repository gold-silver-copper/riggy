use crate::ai::policy::{ApprovedRelationshipAdjustment, PolicyRejection, ProposalPolicy};
use crate::ai::proposals::{AiProposal, RelationshipAdjustmentProposal};
use crate::world::NpcId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalValidationContext {
    pub active_dialogue_npc_id: Option<NpcId>,
    pub target_npc_id: NpcId,
    pub target_exists: bool,
    pub current_disposition: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalReview {
    pub accepted: Vec<ValidatedProposal>,
    pub rejected: Vec<RejectedProposal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidatedProposal {
    NoChange,
    RelationshipAdjustment(ValidatedRelationshipAdjustment),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRelationshipAdjustment {
    pub target_npc_id: NpcId,
    pub delta: i32,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedProposal {
    pub proposal: AiProposal,
    pub reason: ProposalRejectionReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalRejectionReason {
    NoActiveDialogue,
    DialogueTargetMismatch,
    TargetMissing,
    NoMeaningfulChange,
    DeltaOutOfRange { delta: i32 },
    NoteTooLong { len: usize, max: usize },
}

pub fn validate_proposals<P: ProposalPolicy>(
    policy: &P,
    context: &ProposalValidationContext,
    proposals: Vec<AiProposal>,
) -> ProposalReview {
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();

    if context.active_dialogue_npc_id.is_none() {
        rejected.extend(proposals.into_iter().map(|proposal| RejectedProposal {
            proposal,
            reason: ProposalRejectionReason::NoActiveDialogue,
        }));
        return ProposalReview { accepted, rejected };
    }

    if context.active_dialogue_npc_id != Some(context.target_npc_id) {
        rejected.extend(proposals.into_iter().map(|proposal| RejectedProposal {
            proposal,
            reason: ProposalRejectionReason::DialogueTargetMismatch,
        }));
        return ProposalReview { accepted, rejected };
    }

    if !context.target_exists {
        rejected.extend(proposals.into_iter().map(|proposal| RejectedProposal {
            proposal,
            reason: ProposalRejectionReason::TargetMissing,
        }));
        return ProposalReview { accepted, rejected };
    }

    for proposal in proposals {
        match proposal.clone() {
            AiProposal::NoChange => accepted.push(ValidatedProposal::NoChange),
            AiProposal::RelationshipAdjustment(adjustment) => {
                match validate_relationship_adjustment(policy, context, &adjustment) {
                    Ok(approved) => {
                        accepted.push(ValidatedProposal::RelationshipAdjustment(
                            ValidatedRelationshipAdjustment {
                                target_npc_id: context.target_npc_id,
                                delta: approved.delta,
                                note: approved.note,
                            },
                        ));
                    }
                    Err(reason) => rejected.push(RejectedProposal { proposal, reason }),
                }
            }
        }
    }

    ProposalReview { accepted, rejected }
}

fn validate_relationship_adjustment<P: ProposalPolicy>(
    policy: &P,
    context: &ProposalValidationContext,
    proposal: &RelationshipAdjustmentProposal,
) -> Result<ApprovedRelationshipAdjustment, ProposalRejectionReason> {
    policy
        .approve_relationship_adjustment(context.current_disposition, proposal)
        .map_err(|reason| match reason {
            PolicyRejection::NoMeaningfulChange => ProposalRejectionReason::NoMeaningfulChange,
            PolicyRejection::DeltaOutOfRange { delta } => {
                ProposalRejectionReason::DeltaOutOfRange { delta }
            }
            PolicyRejection::NoteTooLong { len, max } => {
                ProposalRejectionReason::NoteTooLong { len, max }
            }
        })
}

#[cfg(test)]
mod tests {
    use petgraph::stable_graph::NodeIndex;

    use super::{
        ProposalRejectionReason, ProposalValidationContext, ValidatedProposal, validate_proposals,
    };
    use crate::ai::policy::ConservativeProposalPolicy;
    use crate::ai::proposals::{AiProposal, RelationshipAdjustmentProposal};
    use crate::graph_ecs::NpcId;

    #[test]
    fn accepts_conservative_relationship_adjustments() {
        let npc_id = NpcId(NodeIndex::new(7));
        let review = validate_proposals(
            &ConservativeProposalPolicy,
            &ProposalValidationContext {
                active_dialogue_npc_id: Some(npc_id),
                target_npc_id: npc_id,
                target_exists: true,
                current_disposition: 0,
            },
            vec![AiProposal::RelationshipAdjustment(
                RelationshipAdjustmentProposal {
                    delta: 1,
                    note: "Opened up a little".to_string(),
                },
            )],
        );

        assert_eq!(review.rejected.len(), 0);
        assert_eq!(review.accepted.len(), 1);
        assert!(matches!(
            review.accepted[0],
            ValidatedProposal::RelationshipAdjustment(_)
        ));
    }

    #[test]
    fn rejects_out_of_range_adjustments() {
        let npc_id = NpcId(NodeIndex::new(7));
        let review = validate_proposals(
            &ConservativeProposalPolicy,
            &ProposalValidationContext {
                active_dialogue_npc_id: Some(npc_id),
                target_npc_id: npc_id,
                target_exists: true,
                current_disposition: 0,
            },
            vec![AiProposal::RelationshipAdjustment(
                RelationshipAdjustmentProposal {
                    delta: 5,
                    note: "Too aggressive".to_string(),
                },
            )],
        );

        assert!(review.accepted.is_empty());
        assert_eq!(review.rejected.len(), 1);
        assert!(matches!(
            review.rejected[0].reason,
            ProposalRejectionReason::DeltaOutOfRange { delta: 5 }
        ));
    }

    #[test]
    fn rejects_when_dialogue_target_is_invalid() {
        let npc_id = NpcId(NodeIndex::new(7));
        let other_npc_id = NpcId(NodeIndex::new(8));
        let review = validate_proposals(
            &ConservativeProposalPolicy,
            &ProposalValidationContext {
                active_dialogue_npc_id: Some(other_npc_id),
                target_npc_id: npc_id,
                target_exists: true,
                current_disposition: 0,
            },
            vec![AiProposal::RelationshipAdjustment(
                RelationshipAdjustmentProposal {
                    delta: 1,
                    note: String::new(),
                },
            )],
        );

        assert!(review.accepted.is_empty());
        assert_eq!(review.rejected.len(), 1);
        assert_eq!(
            review.rejected[0].reason,
            ProposalRejectionReason::DialogueTargetMismatch
        );
    }

    #[test]
    fn no_change_is_a_valid_neutral_outcome() {
        let npc_id = NpcId(NodeIndex::new(7));
        let review = validate_proposals(
            &ConservativeProposalPolicy,
            &ProposalValidationContext {
                active_dialogue_npc_id: Some(npc_id),
                target_npc_id: npc_id,
                target_exists: true,
                current_disposition: 0,
            },
            vec![AiProposal::NoChange],
        );

        assert!(review.rejected.is_empty());
        assert_eq!(review.accepted, vec![ValidatedProposal::NoChange]);
    }

    #[test]
    fn rejects_when_target_is_missing() {
        let npc_id = NpcId(NodeIndex::new(7));
        let review = validate_proposals(
            &ConservativeProposalPolicy,
            &ProposalValidationContext {
                active_dialogue_npc_id: Some(npc_id),
                target_npc_id: npc_id,
                target_exists: false,
                current_disposition: 0,
            },
            vec![AiProposal::RelationshipAdjustment(
                RelationshipAdjustmentProposal {
                    delta: 1,
                    note: String::new(),
                },
            )],
        );

        assert!(review.accepted.is_empty());
        assert_eq!(review.rejected.len(), 1);
        assert_eq!(
            review.rejected[0].reason,
            ProposalRejectionReason::TargetMissing
        );
    }
}
