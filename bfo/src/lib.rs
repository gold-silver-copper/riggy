include!(concat!(env!("OUT_DIR"), "/generated.rs"));

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;

    use super::{BfoClass, RelationKind};

    fn ofn_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("BFO-2020-master/21838-2/owl/bfo-core.ofn")
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

    #[test]
    fn generated_class_inventory_matches_ofn_declarations() {
        assert_eq!(BfoClass::ALL.len(), count_declarations("Declaration(Class("));
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
}
