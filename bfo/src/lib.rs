include!(concat!(env!("OUT_DIR"), "/generated.rs"));

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;

    use super::{BfoClass, ClassConstraint, RelationKind};

    fn ofn_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("BFO-2020-master/21838-2/owl/bfo-core.ofn")
    }

    fn count_declarations(prefix: &str) -> usize {
        fs::read_to_string(ofn_path())
            .expect("failed to read bfo-core.ofn")
            .lines()
            .filter(|line| line.trim().starts_with(prefix))
            .count()
    }

    fn declared_ids(prefix: &str) -> BTreeSet<String> {
        fs::read_to_string(ofn_path())
            .expect("failed to read bfo-core.ofn")
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if !trimmed.starts_with(prefix) {
                    return None;
                }
                let start = trimmed.find('<')?;
                let end = trimmed[start + 1..].find('>')? + start + 1;
                let iri = &trimmed[start + 1..end];
                Some(
                    iri.rsplit('/')
                        .next()
                        .expect("iri should have suffix")
                        .replace('_', ":"),
                )
            })
            .collect()
    }

    fn count_subclass_restrictions() -> usize {
        fs::read_to_string(ofn_path())
            .expect("failed to read bfo-core.ofn")
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                trimmed.starts_with("SubClassOf(")
                    && (trimmed.contains("ObjectAllValuesFrom(")
                        || trimmed.contains("ObjectSomeValuesFrom("))
            })
            .count()
    }

    #[test]
    fn generated_class_inventory_matches_ofn_declarations() {
        assert_eq!(
            BfoClass::ALL.len(),
            count_declarations("Declaration(Class(")
        );
        let generated = BfoClass::ALL
            .iter()
            .map(|class| class.id().to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(generated, declared_ids("Declaration(Class("));
    }

    #[test]
    fn generated_relation_inventory_matches_ofn_declarations() {
        assert_eq!(
            RelationKind::ALL.len(),
            count_declarations("Declaration(ObjectProperty(")
        );
        let generated = RelationKind::ALL
            .iter()
            .map(|relation| relation.id().to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(generated, declared_ids("Declaration(ObjectProperty("));
    }

    #[test]
    fn object_is_a_material_entity() {
        assert!(BfoClass::Object.is_a(BfoClass::MaterialEntity));
        assert!(BfoClass::Object.is_a(BfoClass::Continuant));
        assert!(!BfoClass::Site.is_a(BfoClass::MaterialEntity));
    }

    #[test]
    fn generated_relation_domain_and_range_are_usable() {
        assert!(RelationKind::HasParticipant.domain_allows(BfoClass::Process));
        assert!(RelationKind::HasParticipant.range_allows(BfoClass::Object));
        assert!(RelationKind::OccursIn.range_allows(BfoClass::Site));
        assert!(!RelationKind::InheresIn.domain_allows(BfoClass::Process));
    }

    #[test]
    fn generated_lookup_apis_round_trip_terms() {
        assert_eq!(BfoClass::from_obo_id("BFO:0000030"), Some(BfoClass::Object));
        assert_eq!(
            BfoClass::from_iri("http://purl.obolibrary.org/obo/BFO_0000030"),
            Some(BfoClass::Object)
        );
        assert_eq!(BfoClass::from_spec_id("024-BFO"), Some(BfoClass::Object));

        assert_eq!(
            RelationKind::from_obo_id("BFO:0000197"),
            Some(RelationKind::InheresIn)
        );
        assert_eq!(
            RelationKind::from_iri("http://purl.obolibrary.org/obo/BFO_0000197"),
            Some(RelationKind::InheresIn)
        );
        assert_eq!(
            RelationKind::from_spec_id("051-BFO"),
            Some(RelationKind::InheresIn)
        );
    }

    #[test]
    fn generated_annotation_accessors_preserve_source_data() {
        assert_eq!(BfoClass::Object.spec_id(), Some("024-BFO"));
        assert!(!BfoClass::Object.examples().is_empty());
        assert!(BfoClass::Object.examples()[0].contains("organism"));

        assert_eq!(
            RelationKind::SpecificallyDependsOn.alt_labels(),
            &["s-depends on"]
        );
        assert_eq!(
            RelationKind::SpecificallyDependsOn.spec_id(),
            Some("012-BFO")
        );
        assert!(RelationKind::SpecificallyDependsOn.examples()[0].contains("shape"));
        assert!(RelationKind::SpecificallyDependsOn.scope_notes()[0].contains("has participant"));
    }

    #[test]
    fn generated_class_disjointness_is_available() {
        let disjoint = BfoClass::IndependentContinuant.disjoint_with();
        assert!(disjoint.contains(&BfoClass::SpecificallyDependentContinuant));
        assert!(disjoint.contains(&BfoClass::GenericallyDependentContinuant));
        assert!(!disjoint.contains(&BfoClass::MaterialEntity));
    }

    #[test]
    fn generated_relation_subproperty_parents_are_available() {
        assert_eq!(
            RelationKind::HasMemberPart.direct_parents(),
            &[RelationKind::HasContinuantPart]
        );
        assert_eq!(
            RelationKind::InheresIn.direct_parents(),
            &[RelationKind::SpecificallyDependsOn]
        );
        assert_eq!(
            RelationKind::BearerOf.direct_parents(),
            &[RelationKind::SpecificallyDependedOnBy]
        );
    }

    #[test]
    fn generated_subclass_constraints_preserve_restriction_axioms() {
        let generated_count = BfoClass::ALL
            .iter()
            .map(|class| class.subclass_constraints().len())
            .sum::<usize>();
        assert_eq!(generated_count, count_subclass_restrictions());

        assert_eq!(
            BfoClass::Continuant.subclass_constraints(),
            &[ClassConstraint::AllValuesFrom {
                relation: RelationKind::ContinuantPartOf,
                filler_ofn: "BFO:0000002",
            }]
        );

        assert!(
            BfoClass::Site
                .subclass_constraints()
                .contains(&ClassConstraint::AllValuesFrom {
                    relation: RelationKind::OccupiesSpatialRegion,
                    filler_ofn: "BFO:0000028",
                })
        );
    }
}
