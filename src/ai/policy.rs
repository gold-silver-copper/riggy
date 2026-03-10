use crate::ai::proposals::RelationshipAdjustmentProposal;

pub const MAX_RELATIONSHIP_DELTA: i32 = 2;
pub const MAX_RELATIONSHIP_NOTE_LEN: usize = 160;
const MIN_RELATIONSHIP_DISPOSITION: i32 = -10;
const MAX_RELATIONSHIP_DISPOSITION: i32 = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovedRelationshipAdjustment {
    pub delta: i32,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyRejection {
    NoMeaningfulChange,
    DeltaOutOfRange { delta: i32 },
    NoteTooLong { len: usize, max: usize },
}

pub trait ProposalPolicy {
    fn approve_relationship_adjustment(
        &self,
        current_disposition: i32,
        proposal: &RelationshipAdjustmentProposal,
    ) -> Result<ApprovedRelationshipAdjustment, PolicyRejection>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ConservativeProposalPolicy;

impl ProposalPolicy for ConservativeProposalPolicy {
    fn approve_relationship_adjustment(
        &self,
        current_disposition: i32,
        proposal: &RelationshipAdjustmentProposal,
    ) -> Result<ApprovedRelationshipAdjustment, PolicyRejection> {
        if !(-MAX_RELATIONSHIP_DELTA..=MAX_RELATIONSHIP_DELTA).contains(&proposal.delta) {
            return Err(PolicyRejection::DeltaOutOfRange {
                delta: proposal.delta,
            });
        }

        let trimmed_note = proposal.note.trim();
        if trimmed_note.len() > MAX_RELATIONSHIP_NOTE_LEN {
            return Err(PolicyRejection::NoteTooLong {
                len: trimmed_note.len(),
                max: MAX_RELATIONSHIP_NOTE_LEN,
            });
        }

        let resulting_disposition = (current_disposition + proposal.delta)
            .clamp(MIN_RELATIONSHIP_DISPOSITION, MAX_RELATIONSHIP_DISPOSITION);
        if resulting_disposition == current_disposition {
            return Err(PolicyRejection::NoMeaningfulChange);
        }

        Ok(ApprovedRelationshipAdjustment {
            delta: proposal.delta,
            note: (!trimmed_note.is_empty()).then(|| trimmed_note.to_string()),
        })
    }
}
