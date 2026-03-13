use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BfoClass {
    Entity,
    Continuant,
    IndependentContinuant,
    MaterialEntity,
    Object,
    ImmaterialEntity,
    Site,
    SpecificallyDependentContinuant,
    Role,
    Disposition,
    Function,
    Quality,
    GenericallyDependentContinuant,
    InformationContentEntity,
    Occurrent,
    Process,
    History,
    TemporalRegion,
}

impl BfoClass {
    pub const fn parent(self) -> Option<Self> {
        match self {
            Self::Entity => None,
            Self::Continuant | Self::Occurrent => Some(Self::Entity),
            Self::IndependentContinuant
            | Self::SpecificallyDependentContinuant
            | Self::GenericallyDependentContinuant => Some(Self::Continuant),
            Self::MaterialEntity | Self::ImmaterialEntity => Some(Self::IndependentContinuant),
            Self::Object => Some(Self::MaterialEntity),
            Self::Site => Some(Self::ImmaterialEntity),
            Self::Role | Self::Disposition | Self::Function | Self::Quality => {
                Some(Self::SpecificallyDependentContinuant)
            }
            Self::InformationContentEntity => Some(Self::GenericallyDependentContinuant),
            Self::Process | Self::History | Self::TemporalRegion => Some(Self::Occurrent),
        }
    }

    pub fn is_a(self, other: Self) -> bool {
        let mut current = Some(self);
        while let Some(class) = current {
            if class == other {
                return true;
            }
            current = class.parent();
        }
        false
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Entity => "entity",
            Self::Continuant => "continuant",
            Self::IndependentContinuant => "independent continuant",
            Self::MaterialEntity => "material entity",
            Self::Object => "object",
            Self::ImmaterialEntity => "immaterial entity",
            Self::Site => "site",
            Self::SpecificallyDependentContinuant => "specifically dependent continuant",
            Self::Role => "role",
            Self::Disposition => "disposition",
            Self::Function => "function",
            Self::Quality => "quality",
            Self::GenericallyDependentContinuant => "generically dependent continuant",
            Self::InformationContentEntity => "information content entity",
            Self::Occurrent => "occurrent",
            Self::Process => "process",
            Self::History => "history",
            Self::TemporalRegion => "temporal region",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelationKind {
    Contains,
    Occupies,
    ResidentOf,
    ConnectedTo,
    SpecificallyDependsOn,
    InheresIn,
    IsAbout,
    HasParticipant,
    OccursIn,
    HasOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelationSpec {
    pub kind: RelationKind,
    pub source: &'static [BfoClass],
    pub target: &'static [BfoClass],
    pub target_max_incoming: Option<usize>,
    pub symmetric: bool,
}

const SITE: [BfoClass; 1] = [BfoClass::Site];
const MATERIAL_ENTITY: [BfoClass; 1] = [BfoClass::MaterialEntity];
const SITE_OR_MATERIAL_ENTITY: [BfoClass; 2] = [BfoClass::Site, BfoClass::MaterialEntity];
const INDEPENDENT_CONTINUANT: [BfoClass; 1] = [BfoClass::IndependentContinuant];
const SPECIFICALLY_DEPENDENT_CONTINUANT: [BfoClass; 1] =
    [BfoClass::SpecificallyDependentContinuant];
const INFORMATION_CONTENT_ENTITY: [BfoClass; 1] = [BfoClass::InformationContentEntity];
const PROCESS: [BfoClass; 1] = [BfoClass::Process];
const ENTITY: [BfoClass; 1] = [BfoClass::Entity];

const RELATION_SPECS: [RelationSpec; 10] = [
    RelationSpec {
        kind: RelationKind::Contains,
        source: &SITE_OR_MATERIAL_ENTITY,
        target: &SITE_OR_MATERIAL_ENTITY,
        target_max_incoming: Some(1),
        symmetric: false,
    },
    RelationSpec {
        kind: RelationKind::Occupies,
        source: &SITE,
        target: &MATERIAL_ENTITY,
        target_max_incoming: Some(1),
        symmetric: false,
    },
    RelationSpec {
        kind: RelationKind::ResidentOf,
        source: &SITE,
        target: &MATERIAL_ENTITY,
        target_max_incoming: Some(1),
        symmetric: false,
    },
    RelationSpec {
        kind: RelationKind::ConnectedTo,
        source: &SITE,
        target: &SITE,
        target_max_incoming: None,
        symmetric: true,
    },
    RelationSpec {
        kind: RelationKind::SpecificallyDependsOn,
        source: &SPECIFICALLY_DEPENDENT_CONTINUANT,
        target: &INDEPENDENT_CONTINUANT,
        target_max_incoming: None,
        symmetric: false,
    },
    RelationSpec {
        kind: RelationKind::InheresIn,
        source: &SPECIFICALLY_DEPENDENT_CONTINUANT,
        target: &MATERIAL_ENTITY,
        target_max_incoming: None,
        symmetric: false,
    },
    RelationSpec {
        kind: RelationKind::IsAbout,
        source: &INFORMATION_CONTENT_ENTITY,
        target: &ENTITY,
        target_max_incoming: None,
        symmetric: false,
    },
    RelationSpec {
        kind: RelationKind::HasParticipant,
        source: &PROCESS,
        target: &MATERIAL_ENTITY,
        target_max_incoming: None,
        symmetric: false,
    },
    RelationSpec {
        kind: RelationKind::OccursIn,
        source: &PROCESS,
        target: &SITE,
        target_max_incoming: None,
        symmetric: false,
    },
    RelationSpec {
        kind: RelationKind::HasOutput,
        source: &PROCESS,
        target: &INFORMATION_CONTENT_ENTITY,
        target_max_incoming: None,
        symmetric: false,
    },
];

pub const fn relation_specs() -> &'static [RelationSpec] {
    &RELATION_SPECS
}

pub fn relation_spec(kind: RelationKind) -> &'static RelationSpec {
    RELATION_SPECS
        .iter()
        .find(|spec| spec.kind == kind)
        .expect("relation spec should exist")
}

pub fn bfo_class_allowed(class: BfoClass, allowed: &[BfoClass]) -> bool {
    allowed
        .iter()
        .copied()
        .any(|candidate| class.is_a(candidate))
}

#[cfg(test)]
mod tests {
    use super::{BfoClass, RelationKind, bfo_class_allowed, relation_spec};

    #[test]
    fn object_is_a_material_entity() {
        assert!(BfoClass::Object.is_a(BfoClass::MaterialEntity));
        assert!(BfoClass::Object.is_a(BfoClass::Continuant));
        assert!(!BfoClass::Site.is_a(BfoClass::MaterialEntity));
    }

    #[test]
    fn relation_specs_accept_subclasses() {
        let spec = relation_spec(RelationKind::Occupies);
        assert!(bfo_class_allowed(BfoClass::Object, spec.target));
        assert!(!bfo_class_allowed(BfoClass::Site, spec.target));
    }
}
