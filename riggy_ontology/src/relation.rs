use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RiggyRelation {
    TravelRoute,
    Contains,
    ResidentOf,
    PresentAt,
    IsAbout,
    HasOutput,
}

impl RiggyRelation {
    pub const fn label(self) -> &'static str {
        match self {
            Self::TravelRoute => "travel route",
            Self::Contains => "contains",
            Self::ResidentOf => "resident of",
            Self::PresentAt => "present at",
            Self::IsAbout => "is about",
            Self::HasOutput => "has output",
        }
    }
}
