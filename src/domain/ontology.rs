use crate::graph_ecs::{BfoClass, RelationKind};

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
    use super::{bfo_class_allowed, relation_spec};
    use crate::graph_ecs::{BfoClass, RelationKind};

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
