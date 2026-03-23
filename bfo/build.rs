use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

mod cco_codegen {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/build_support/cco_codegen.rs"
    ));
}

#[derive(Debug, Clone, Default)]
struct AnnotationValues {
    label: Option<String>,
    definition: Option<String>,
    spec_id: Option<String>,
    alt_labels: Vec<String>,
    examples: Vec<String>,
    scope_notes: Vec<String>,
}

#[derive(Debug, Clone)]
struct ClassDef {
    id: String,
    iri: String,
    label: String,
    definition: Option<String>,
    spec_id: Option<String>,
    alt_labels: Vec<String>,
    examples: Vec<String>,
    scope_notes: Vec<String>,
    direct_parent_ids: Vec<String>,
    subclass_constraints: Vec<RestrictionDef>,
    disjoint_ids: Vec<String>,
    equivalent_ids: Vec<String>,
    variant: String,
}

#[derive(Debug, Clone)]
struct RelationDef {
    id: String,
    iri: String,
    label: String,
    definition: Option<String>,
    spec_id: Option<String>,
    alt_labels: Vec<String>,
    examples: Vec<String>,
    scope_notes: Vec<String>,
    inverse_id: Option<String>,
    direct_parent_ids: Vec<String>,
    equivalent_ids: Vec<String>,
    disjoint_ids: Vec<String>,
    domain: Option<ClassExpr>,
    range: Option<ClassExpr>,
    symmetric: bool,
    transitive: bool,
    functional: bool,
    inverse_functional: bool,
    asymmetric: bool,
    reflexive: bool,
    irreflexive: bool,
    variant: String,
}

#[derive(Debug, Clone)]
enum ClassExpr {
    Named(String),
    Union(Vec<ClassExpr>),
    Intersection(Vec<ClassExpr>),
    Complement(Box<ClassExpr>),
    AllValuesFrom {
        relation_id: String,
        filler: Box<ClassExpr>,
    },
    SomeValuesFrom {
        relation_id: String,
        filler: Box<ClassExpr>,
    },
}

#[derive(Debug, Clone, Copy)]
enum RestrictionQuantifier {
    AllValuesFrom,
    SomeValuesFrom,
}

#[derive(Debug, Clone)]
struct RestrictionDef {
    quantifier: RestrictionQuantifier,
    relation_id: String,
    filler: ClassExpr,
}

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let ofn_path = manifest_dir.join("BFO-2020-master/21838-2/owl/bfo-core.ofn");
    println!("cargo:rerun-if-changed={}", ofn_path.display());

    let text = fs::read_to_string(&ofn_path).expect("failed to read bfo-core.ofn");
    let generated = generate(&text);
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    fs::write(out_dir.join("generated.rs"), generated).expect("failed to write generated.rs");

    if std::env::var_os("CARGO_FEATURE_CCO").is_some() {
        cco_codegen::generate_cco_files(&manifest_dir, &out_dir);
    }
}

fn generate(ofn: &str) -> String {
    let class_iris = parse_declarations(ofn, "Declaration(Class(");
    let relation_iris = parse_declarations(ofn, "Declaration(ObjectProperty(");
    let annotations = parse_annotations(ofn);
    let subclass_axioms = parse_subclass_axioms(ofn);
    let class_equivalents = parse_named_group_axioms(ofn, "EquivalentClasses(");
    let class_disjoints = parse_named_group_axioms(ofn, "DisjointClasses(");
    let relation_parents = parse_named_binary_axioms(ofn, "SubObjectPropertyOf(");
    let relation_equivalents = parse_named_group_axioms(ofn, "EquivalentObjectProperties(");
    let relation_disjoints = parse_named_group_axioms(ofn, "DisjointObjectProperties(");
    let inverses = parse_inverses(ofn);
    let domains = parse_object_property_expr_map(ofn, "ObjectPropertyDomain(");
    let ranges = parse_object_property_expr_map(ofn, "ObjectPropertyRange(");
    let symmetric_relations = parse_property_flags(ofn, "SymmetricObjectProperty(");
    let transitive_relations = parse_property_flags(ofn, "TransitiveObjectProperty(");
    let functional_relations = parse_property_flags(ofn, "FunctionalObjectProperty(");
    let inverse_functional_relations =
        parse_property_flags(ofn, "InverseFunctionalObjectProperty(");
    let asymmetric_relations = parse_property_flags(ofn, "AsymmetricObjectProperty(");
    let reflexive_relations = parse_property_flags(ofn, "ReflexiveObjectProperty(");
    let irreflexive_relations = parse_property_flags(ofn, "IrreflexiveObjectProperty(");

    let mut class_defs = class_iris
        .into_iter()
        .map(|iri| {
            let id = iri_to_bfo_id(&iri);
            let annotation = annotations.get(&iri).cloned().unwrap_or_default();
            let label = annotation
                .label
                .clone()
                .unwrap_or_else(|| id_to_fallback_label(&id));
            let variant = sanitize_to_variant(&label);
            ClassDef {
                id: id.clone(),
                iri: iri.clone(),
                label,
                definition: annotation.definition,
                spec_id: annotation.spec_id,
                alt_labels: annotation.alt_labels,
                examples: annotation.examples,
                scope_notes: annotation.scope_notes,
                direct_parent_ids: direct_parent_ids(subclass_axioms.get(&id)),
                subclass_constraints: restriction_defs(subclass_axioms.get(&id)),
                disjoint_ids: class_disjoints.get(&id).cloned().unwrap_or_default(),
                equivalent_ids: class_equivalents.get(&id).cloned().unwrap_or_default(),
                variant,
            }
        })
        .collect::<Vec<_>>();
    class_defs.sort_by(|a, b| a.id.cmp(&b.id));
    assert_unique_variants(class_defs.iter().map(|def| def.variant.as_str()), "class");

    let mut relation_defs = relation_iris
        .into_iter()
        .map(|iri| {
            let id = iri_to_bfo_id(&iri);
            let annotation = annotations.get(&iri).cloned().unwrap_or_default();
            let label = annotation
                .label
                .clone()
                .unwrap_or_else(|| id_to_fallback_label(&id));
            let variant = sanitize_to_variant(&label);
            RelationDef {
                id: id.clone(),
                iri: iri.clone(),
                label,
                definition: annotation.definition,
                spec_id: annotation.spec_id,
                alt_labels: annotation.alt_labels,
                examples: annotation.examples,
                scope_notes: annotation.scope_notes,
                inverse_id: inverses.get(&id).cloned(),
                direct_parent_ids: relation_parents.get(&id).cloned().unwrap_or_default(),
                equivalent_ids: relation_equivalents.get(&id).cloned().unwrap_or_default(),
                disjoint_ids: relation_disjoints.get(&id).cloned().unwrap_or_default(),
                domain: domains.get(&id).cloned(),
                range: ranges.get(&id).cloned(),
                symmetric: symmetric_relations.contains(&id),
                transitive: transitive_relations.contains(&id),
                functional: functional_relations.contains(&id),
                inverse_functional: inverse_functional_relations.contains(&id),
                asymmetric: asymmetric_relations.contains(&id),
                reflexive: reflexive_relations.contains(&id),
                irreflexive: irreflexive_relations.contains(&id),
                variant,
            }
        })
        .collect::<Vec<_>>();
    relation_defs.sort_by(|a, b| a.id.cmp(&b.id));
    assert_unique_variants(
        relation_defs.iter().map(|def| def.variant.as_str()),
        "relation",
    );

    let class_variant_by_id = class_defs
        .iter()
        .map(|def| (def.id.clone(), def.variant.clone()))
        .collect::<BTreeMap<_, _>>();
    let class_label_by_id = class_defs
        .iter()
        .map(|def| (def.id.clone(), def.label.clone()))
        .collect::<BTreeMap<_, _>>();
    let class_parent_ids_by_id = class_defs
        .iter()
        .map(|def| (def.id.clone(), def.direct_parent_ids.clone()))
        .collect::<BTreeMap<_, _>>();
    let relation_variant_by_id = relation_defs
        .iter()
        .map(|def| (def.id.clone(), def.variant.clone()))
        .collect::<BTreeMap<_, _>>();
    let class_index_by_id = class_defs
        .iter()
        .enumerate()
        .map(|(index, def)| (def.id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let relation_index_by_id = relation_defs
        .iter()
        .enumerate()
        .map(|(index, def)| (def.id.clone(), index))
        .collect::<BTreeMap<_, _>>();

    let mut output = String::new();
    output.push_str("// @generated by build.rs\n");
    output.push_str("use serde::{Deserialize, Serialize};\n\n");

    push_doc_lines(
        &mut output,
        &["Generated representation of a quantified subclass restriction axiom.".to_string()],
    );
    output.push_str("#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]\n");
    output.push_str("pub enum ClassConstraint {\n");
    output.push_str("    AllValuesFrom {\n");
    output.push_str("        relation: RelationKind,\n");
    output.push_str("        filler_ofn: &'static str,\n");
    output.push_str("    },\n");
    output.push_str("    SomeValuesFrom {\n");
    output.push_str("        relation: RelationKind,\n");
    output.push_str("        filler_ofn: &'static str,\n");
    output.push_str("    },\n");
    output.push_str("}\n\n");

    push_doc_lines(
        &mut output,
        &["Stable generated identifier for a BFO class.".to_string()],
    );
    output.push_str("#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]\n");
    output.push_str("pub struct BfoClassId(usize);\n\n");

    push_id_slice_tables(
        &mut output,
        "BFO_CLASS_ID_DIRECT_PARENTS",
        "BfoClassId",
        &class_defs
            .iter()
            .map(|def| resolve_id_indices(&def.direct_parent_ids, &class_index_by_id, "BFO class"))
            .collect::<Vec<_>>(),
        |index| format!("BfoClassId({index})"),
    );
    output.push('\n');

    output.push_str("impl BfoClassId {\n");
    output.push_str("    pub const fn new(index: usize) -> Self {\n");
    output.push_str("        Self(index)\n");
    output.push_str("    }\n\n");

    push_doc_lines(
        &mut output,
        &["All generated BFO class identifiers ordered by BFO identifier.".to_string()],
    );
    output.push_str("    pub const ALL: &'static [Self] = &[\n");
    for (index, _) in class_defs.iter().enumerate() {
        output.push_str(&format!("        Self::new({index}),\n"));
    }
    output.push_str("    ];\n\n");

    output.push_str("    pub const fn index(self) -> usize {\n");
    output.push_str("        self.0\n");
    output.push_str("    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns the canonical OBO identifier for this class.".to_string()],
    );
    output.push_str("    pub const fn id(self) -> &'static str {\n        match self.0 {\n");
    for (index, def) in class_defs.iter().enumerate() {
        output.push_str(&format!("            {index} => \"{}\",\n", def.id));
    }
    output.push_str("            _ => \"\",\n        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Looks up a class identifier by canonical OBO identifier.".to_string()],
    );
    output.push_str("    pub fn from_obo_id(id: &str) -> Option<Self> {\n        match id {\n");
    for (index, def) in class_defs.iter().enumerate() {
        output.push_str(&format!(
            "            \"{}\" => Some(Self::new({index})),\n",
            def.id
        ));
    }
    output.push_str("            _ => None,\n        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns the canonical BFO IRI for this class.".to_string()],
    );
    output.push_str("    pub const fn iri(self) -> &'static str {\n        match self.0 {\n");
    for (index, def) in class_defs.iter().enumerate() {
        output.push_str(&format!("            {index} => \"{}\",\n", def.iri));
    }
    output.push_str("            _ => \"\",\n        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Looks up a class identifier by canonical IRI.".to_string()],
    );
    output.push_str("    pub fn from_iri(iri: &str) -> Option<Self> {\n        match iri {\n");
    for (index, def) in class_defs.iter().enumerate() {
        output.push_str(&format!(
            "            \"{}\" => Some(Self::new({index})),\n",
            def.iri
        ));
    }
    output.push_str("            _ => None,\n        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns the source `rdfs:label` for this class.".to_string()],
    );
    output.push_str("    pub const fn label(self) -> &'static str {\n        match self.0 {\n");
    for (index, def) in class_defs.iter().enumerate() {
        output.push_str(&format!(
            "            {index} => \"{}\",\n",
            escape_string(&def.label)
        ));
    }
    output.push_str("            _ => \"\",\n        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &[
            "Returns direct named superclasses declared by simple `SubClassOf(...)` axioms."
                .to_string(),
        ],
    );
    output.push_str(
        "    pub const fn direct_parents(self) -> &'static [Self] {\n        match self.0 {\n",
    );
    for (index, _) in class_defs.iter().enumerate() {
        output.push_str(&format!(
            "            {index} => {},\n",
            render_slice_table_ref("BFO_CLASS_ID_DIRECT_PARENTS", index)
        ));
    }
    output.push_str("            _ => &[],\n        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns `true` when `self` is equal to or a subclass of `other` in the generated BFO class hierarchy.".to_string()],
    );
    output.push_str("    pub fn is_a(self, other: Self) -> bool {\n        match self.0 {\n");
    for def in &class_defs {
        let index = class_index_by_id[&def.id];
        let ancestors = class_ancestor_ids(def, &class_parent_ids_by_id);
        output.push_str(&format!("            {index} => match other.0 {{\n"));
        for ancestor in ancestors {
            let ancestor_index = class_index_by_id[&ancestor];
            output.push_str(&format!("                {ancestor_index} => true,\n"));
        }
        output.push_str("                _ => false,\n");
        output.push_str("            },\n");
    }
    output.push_str("            _ => false,\n        }\n    }\n}\n\n");

    push_doc_lines(
        &mut output,
        &[
            "BFO 2020 classes generated from `bfo-core.ofn`.".to_string(),
            "Each variant preserves the source ontology term label, identifiers, IRI, and hierarchy."
                .to_string(),
        ],
    );
    output.push_str("#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]\n");
    output.push_str("pub enum BfoClass {\n");
    for def in &class_defs {
        push_doc_lines(&mut output, &class_doc_lines(def, &class_variant_by_id));
        output.push_str(&format!("    {},\n", def.variant));
    }
    output.push_str("}\n\n");

    output.push_str("impl BfoClass {\n");
    output.push_str("    pub const fn class_id(self) -> BfoClassId {\n        match self {\n");
    for (index, def) in class_defs.iter().enumerate() {
        output.push_str(&format!(
            "            Self::{} => BfoClassId::new({index}),\n",
            def.variant
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["All BFO classes declared in `bfo-core.ofn`, ordered by BFO identifier.".to_string()],
    );
    output.push_str("    pub const ALL: &'static [Self] = &[\n");
    for def in &class_defs {
        output.push_str(&format!("        Self::{},\n", def.variant));
    }
    output.push_str("    ];\n\n");

    push_doc_lines(
        &mut output,
        &["Returns the canonical OBO identifier for this class.".to_string()],
    );
    output.push_str("    pub const fn id(self) -> &'static str {\n        match self {\n");
    for def in &class_defs {
        output.push_str(&format!(
            "            Self::{} => \"{}\",\n",
            def.variant, def.id
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Looks up a class by canonical OBO identifier.".to_string()],
    );
    output.push_str("    pub fn from_obo_id(id: &str) -> Option<Self> {\n        match id {\n");
    for def in &class_defs {
        output.push_str(&format!(
            "            \"{}\" => Some(Self::{}),\n",
            def.id, def.variant
        ));
    }
    output.push_str("            _ => None,\n        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns the canonical BFO IRI for this class.".to_string()],
    );
    output.push_str("    pub const fn iri(self) -> &'static str {\n        match self {\n");
    for def in &class_defs {
        output.push_str(&format!(
            "            Self::{} => \"{}\",\n",
            def.variant, def.iri
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Looks up a class by canonical IRI.".to_string()],
    );
    output.push_str("    pub fn from_iri(iri: &str) -> Option<Self> {\n        match iri {\n");
    for def in &class_defs {
        output.push_str(&format!(
            "            \"{}\" => Some(Self::{}),\n",
            def.iri, def.variant
        ));
    }
    output.push_str("            _ => None,\n        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &[
            "Returns the BFO specification identifier (`dc11:identifier`) when present."
                .to_string(),
        ],
    );
    output.push_str(
        "    pub const fn spec_id(self) -> Option<&'static str> {\n        match self {\n",
    );
    for def in &class_defs {
        match &def.spec_id {
            Some(spec_id) => output.push_str(&format!(
                "            Self::{} => Some(\"{}\"),\n",
                def.variant,
                escape_string(spec_id)
            )),
            None => output.push_str(&format!("            Self::{} => None,\n", def.variant)),
        }
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Looks up a class by BFO specification identifier (`dc11:identifier`).".to_string()],
    );
    output.push_str(
        "    pub fn from_spec_id(spec_id: &str) -> Option<Self> {\n        match spec_id {\n",
    );
    for def in &class_defs {
        if let Some(spec_id) = &def.spec_id {
            output.push_str(&format!(
                "            \"{}\" => Some(Self::{}),\n",
                escape_string(spec_id),
                def.variant
            ));
        }
    }
    output.push_str("            _ => None,\n        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns the source `rdfs:label` for this class.".to_string()],
    );
    output.push_str("    pub const fn label(self) -> &'static str {\n        match self {\n");
    for def in &class_defs {
        output.push_str(&format!(
            "            Self::{} => \"{}\",\n",
            def.variant,
            escape_string(&def.label)
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &[
            "Returns the source `skos:definition` for this class when BFO provides one."
                .to_string(),
        ],
    );
    output.push_str(
        "    pub const fn definition(self) -> Option<&'static str> {\n        match self {\n",
    );
    for def in &class_defs {
        match &def.definition {
            Some(definition) => output.push_str(&format!(
                "            Self::{} => Some(\"{}\"),\n",
                def.variant,
                escape_string(definition)
            )),
            None => output.push_str(&format!("            Self::{} => None,\n", def.variant)),
        }
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns source `skos:altLabel` values for this class.".to_string()],
    );
    output.push_str(
        "    pub fn alt_labels(self) -> &'static [&'static str] {\n        match self {\n",
    );
    for def in &class_defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant,
            render_string_slice_expr(&def.alt_labels)
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns source `skos:example` values for this class.".to_string()],
    );
    output
        .push_str("    pub fn examples(self) -> &'static [&'static str] {\n        match self {\n");
    for def in &class_defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant,
            render_string_slice_expr(&def.examples)
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns source `skos:scopeNote` values for this class.".to_string()],
    );
    output.push_str(
        "    pub fn scope_notes(self) -> &'static [&'static str] {\n        match self {\n",
    );
    for def in &class_defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant,
            render_string_slice_expr(&def.scope_notes)
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &[
            "Returns one direct superclass declared by a simple `SubClassOf(...)` axiom, if any."
                .to_string(),
        ],
    );
    output.push_str("    pub const fn parent(self) -> Option<Self> {\n        match self {\n");
    for def in &class_defs {
        match def.direct_parent_ids.first() {
            Some(parent_id) => output.push_str(&format!(
                "            Self::{} => Some(Self::{}),\n",
                def.variant,
                class_variant_by_id
                    .get(parent_id)
                    .unwrap_or_else(|| panic!("missing parent variant for {}", parent_id))
            )),
            None => output.push_str(&format!("            Self::{} => None,\n", def.variant)),
        }
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &[
            "Returns direct named superclasses declared by simple `SubClassOf(...)` axioms."
                .to_string(),
        ],
    );
    output.push_str("    pub fn direct_parents(self) -> &'static [Self] {\n        match self {\n");
    for def in &class_defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant,
            render_self_slice_expr(&def.direct_parent_ids, &class_variant_by_id)
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns classes declared disjoint with this class.".to_string()],
    );
    output.push_str("    pub fn disjoint_with(self) -> &'static [Self] {\n        match self {\n");
    for def in &class_defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant,
            render_self_slice_expr(&def.disjoint_ids, &class_variant_by_id)
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns classes declared equivalent to this class.".to_string()],
    );
    output.push_str("    pub fn equivalent_to(self) -> &'static [Self] {\n        match self {\n");
    for def in &class_defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant,
            render_self_slice_expr(&def.equivalent_ids, &class_variant_by_id)
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &[
            "Returns quantified subclass constraints preserved from `SubClassOf(...)` axioms."
                .to_string(),
        ],
    );
    output.push_str(
        "    pub fn subclass_constraints(self) -> &'static [ClassConstraint] {\n        match self {\n",
    );
    for def in &class_defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant,
            render_constraint_slice_expr(&def.subclass_constraints, &relation_variant_by_id,)
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns `true` when `self` is equal to or a subclass of `other` in the generated BFO class hierarchy.".to_string()],
    );
    output.push_str("    pub fn is_a(self, other: Self) -> bool {\n        match self {\n");
    for def in &class_defs {
        let ancestors = class_ancestor_variants(def, &class_parent_ids_by_id, &class_variant_by_id);
        output.push_str(&format!(
            "            Self::{} => match other {{\n",
            def.variant
        ));
        for ancestor in ancestors {
            output.push_str(&format!("                Self::{} => true,\n", ancestor));
        }
        output.push_str("                _ => false,\n");
        output.push_str("            },\n");
    }
    output.push_str("        }\n    }\n}\n\n");

    output.push_str("impl From<BfoClass> for BfoClassId {\n");
    output.push_str("    fn from(value: BfoClass) -> Self {\n");
    output.push_str("        value.class_id()\n");
    output.push_str("    }\n");
    output.push_str("}\n\n");

    output.push_str("impl TryFrom<BfoClassId> for BfoClass {\n");
    output.push_str("    type Error = ();\n\n");
    output.push_str("    fn try_from(value: BfoClassId) -> Result<Self, Self::Error> {\n");
    output.push_str("        match value.0 {\n");
    for (index, def) in class_defs.iter().enumerate() {
        output.push_str(&format!(
            "            {index} => Ok(Self::{}),\n",
            def.variant
        ));
    }
    output.push_str("            _ => Err(()),\n");
    output.push_str("        }\n");
    output.push_str("    }\n");
    output.push_str("}\n\n");

    push_doc_lines(
        &mut output,
        &["Stable generated identifier for a BFO object property.".to_string()],
    );
    output.push_str("#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]\n");
    output.push_str("pub struct BfoRelationId(usize);\n\n");

    push_id_slice_tables(
        &mut output,
        "BFO_RELATION_ID_DIRECT_PARENTS",
        "BfoRelationId",
        &relation_defs
            .iter()
            .map(|def| {
                resolve_id_indices(
                    &def.direct_parent_ids,
                    &relation_index_by_id,
                    "BFO relation",
                )
            })
            .collect::<Vec<_>>(),
        |index| format!("BfoRelationId({index})"),
    );
    output.push('\n');

    output.push_str("impl BfoRelationId {\n");
    output.push_str("    pub const fn new(index: usize) -> Self {\n");
    output.push_str("        Self(index)\n");
    output.push_str("    }\n\n");

    push_doc_lines(
        &mut output,
        &["All generated BFO relation identifiers ordered by BFO identifier.".to_string()],
    );
    output.push_str("    pub const ALL: &'static [Self] = &[\n");
    for (index, _) in relation_defs.iter().enumerate() {
        output.push_str(&format!("        Self::new({index}),\n"));
    }
    output.push_str("    ];\n\n");

    output.push_str("    pub const fn index(self) -> usize {\n");
    output.push_str("        self.0\n");
    output.push_str("    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns the canonical OBO identifier for this relation.".to_string()],
    );
    output.push_str("    pub const fn id(self) -> &'static str {\n        match self.0 {\n");
    for (index, def) in relation_defs.iter().enumerate() {
        output.push_str(&format!("            {index} => \"{}\",\n", def.id));
    }
    output.push_str("            _ => \"\",\n        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Looks up a relation identifier by canonical OBO identifier.".to_string()],
    );
    output.push_str("    pub fn from_obo_id(id: &str) -> Option<Self> {\n        match id {\n");
    for (index, def) in relation_defs.iter().enumerate() {
        output.push_str(&format!(
            "            \"{}\" => Some(Self::new({index})),\n",
            def.id
        ));
    }
    output.push_str("            _ => None,\n        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns the canonical BFO IRI for this relation.".to_string()],
    );
    output.push_str("    pub const fn iri(self) -> &'static str {\n        match self.0 {\n");
    for (index, def) in relation_defs.iter().enumerate() {
        output.push_str(&format!("            {index} => \"{}\",\n", def.iri));
    }
    output.push_str("            _ => \"\",\n        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Looks up a relation identifier by canonical IRI.".to_string()],
    );
    output.push_str("    pub fn from_iri(iri: &str) -> Option<Self> {\n        match iri {\n");
    for (index, def) in relation_defs.iter().enumerate() {
        output.push_str(&format!(
            "            \"{}\" => Some(Self::new({index})),\n",
            def.iri
        ));
    }
    output.push_str("            _ => None,\n        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns the source `rdfs:label` for this relation.".to_string()],
    );
    output.push_str("    pub const fn label(self) -> &'static str {\n        match self.0 {\n");
    for (index, def) in relation_defs.iter().enumerate() {
        output.push_str(&format!(
            "            {index} => \"{}\",\n",
            escape_string(&def.label)
        ));
    }
    output.push_str("            _ => \"\",\n        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns direct parent relations declared by `SubObjectPropertyOf(...)`.".to_string()],
    );
    output.push_str(
        "    pub const fn direct_parents(self) -> &'static [Self] {\n        match self.0 {\n",
    );
    for (index, _) in relation_defs.iter().enumerate() {
        output.push_str(&format!(
            "            {index} => {},\n",
            render_slice_table_ref("BFO_RELATION_ID_DIRECT_PARENTS", index)
        ));
    }
    output.push_str("            _ => &[],\n        }\n    }\n}\n\n");

    push_doc_lines(
        &mut output,
        &[
            "BFO 2020 object properties generated from `bfo-core.ofn`.".to_string(),
            "Each variant preserves the source ontology term label, identifiers, IRI, domain, range, inverse, and property characteristics.".to_string(),
        ],
    );
    output.push_str("#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]\n");
    output.push_str("pub enum RelationKind {\n");
    for def in &relation_defs {
        push_doc_lines(
            &mut output,
            &relation_doc_lines(def, &class_label_by_id, &relation_variant_by_id),
        );
        output.push_str(&format!("    {},\n", def.variant));
    }
    output.push_str("}\n\n");

    output.push_str("impl RelationKind {\n");
    output
        .push_str("    pub const fn relation_id(self) -> BfoRelationId {\n        match self {\n");
    for (index, def) in relation_defs.iter().enumerate() {
        output.push_str(&format!(
            "            Self::{} => BfoRelationId::new({index}),\n",
            def.variant
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &[
            "All BFO object properties declared in `bfo-core.ofn`, ordered by BFO identifier."
                .to_string(),
        ],
    );
    output.push_str("    pub const ALL: &'static [Self] = &[\n");
    for def in &relation_defs {
        output.push_str(&format!("        Self::{},\n", def.variant));
    }
    output.push_str("    ];\n\n");

    push_doc_lines(
        &mut output,
        &["Returns the canonical OBO identifier for this relation.".to_string()],
    );
    output.push_str("    pub const fn id(self) -> &'static str {\n        match self {\n");
    for def in &relation_defs {
        output.push_str(&format!(
            "            Self::{} => \"{}\",\n",
            def.variant, def.id
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Looks up a relation by canonical OBO identifier.".to_string()],
    );
    output.push_str("    pub fn from_obo_id(id: &str) -> Option<Self> {\n        match id {\n");
    for def in &relation_defs {
        output.push_str(&format!(
            "            \"{}\" => Some(Self::{}),\n",
            def.id, def.variant
        ));
    }
    output.push_str("            _ => None,\n        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns the canonical BFO IRI for this relation.".to_string()],
    );
    output.push_str("    pub const fn iri(self) -> &'static str {\n        match self {\n");
    for def in &relation_defs {
        output.push_str(&format!(
            "            Self::{} => \"{}\",\n",
            def.variant, def.iri
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Looks up a relation by canonical IRI.".to_string()],
    );
    output.push_str("    pub fn from_iri(iri: &str) -> Option<Self> {\n        match iri {\n");
    for def in &relation_defs {
        output.push_str(&format!(
            "            \"{}\" => Some(Self::{}),\n",
            def.iri, def.variant
        ));
    }
    output.push_str("            _ => None,\n        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &[
            "Returns the BFO specification identifier (`dc11:identifier`) when present."
                .to_string(),
        ],
    );
    output.push_str(
        "    pub const fn spec_id(self) -> Option<&'static str> {\n        match self {\n",
    );
    for def in &relation_defs {
        match &def.spec_id {
            Some(spec_id) => output.push_str(&format!(
                "            Self::{} => Some(\"{}\"),\n",
                def.variant,
                escape_string(spec_id)
            )),
            None => output.push_str(&format!("            Self::{} => None,\n", def.variant)),
        }
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Looks up a relation by BFO specification identifier (`dc11:identifier`).".to_string()],
    );
    output.push_str(
        "    pub fn from_spec_id(spec_id: &str) -> Option<Self> {\n        match spec_id {\n",
    );
    for def in &relation_defs {
        if let Some(spec_id) = &def.spec_id {
            output.push_str(&format!(
                "            \"{}\" => Some(Self::{}),\n",
                escape_string(spec_id),
                def.variant
            ));
        }
    }
    output.push_str("            _ => None,\n        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns the source `rdfs:label` for this relation.".to_string()],
    );
    output.push_str("    pub const fn label(self) -> &'static str {\n        match self {\n");
    for def in &relation_defs {
        output.push_str(&format!(
            "            Self::{} => \"{}\",\n",
            def.variant,
            escape_string(&def.label)
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &[
            "Returns the source `skos:definition` for this relation when BFO provides one."
                .to_string(),
        ],
    );
    output.push_str(
        "    pub const fn definition(self) -> Option<&'static str> {\n        match self {\n",
    );
    for def in &relation_defs {
        match &def.definition {
            Some(definition) => output.push_str(&format!(
                "            Self::{} => Some(\"{}\"),\n",
                def.variant,
                escape_string(definition)
            )),
            None => output.push_str(&format!("            Self::{} => None,\n", def.variant)),
        }
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns source `skos:altLabel` values for this relation.".to_string()],
    );
    output.push_str(
        "    pub fn alt_labels(self) -> &'static [&'static str] {\n        match self {\n",
    );
    for def in &relation_defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant,
            render_string_slice_expr(&def.alt_labels)
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns source `skos:example` values for this relation.".to_string()],
    );
    output
        .push_str("    pub fn examples(self) -> &'static [&'static str] {\n        match self {\n");
    for def in &relation_defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant,
            render_string_slice_expr(&def.examples)
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns source `skos:scopeNote` values for this relation.".to_string()],
    );
    output.push_str(
        "    pub fn scope_notes(self) -> &'static [&'static str] {\n        match self {\n",
    );
    for def in &relation_defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant,
            render_string_slice_expr(&def.scope_notes)
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns direct parent relations declared by `SubObjectPropertyOf(...)`.".to_string()],
    );
    output.push_str("    pub fn direct_parents(self) -> &'static [Self] {\n        match self {\n");
    for def in &relation_defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant,
            render_self_slice_expr(&def.direct_parent_ids, &relation_variant_by_id)
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns relations declared equivalent to this relation.".to_string()],
    );
    output.push_str("    pub fn equivalent_to(self) -> &'static [Self] {\n        match self {\n");
    for def in &relation_defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant,
            render_self_slice_expr(&def.equivalent_ids, &relation_variant_by_id)
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &["Returns relations declared disjoint with this relation.".to_string()],
    );
    output.push_str("    pub fn disjoint_with(self) -> &'static [Self] {\n        match self {\n");
    for def in &relation_defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant,
            render_self_slice_expr(&def.disjoint_ids, &relation_variant_by_id)
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &[
            "Returns `true` when the given class satisfies the source-generated relation domain."
                .to_string(),
        ],
    );
    output.push_str(
        "    pub fn domain_allows(self, class: BfoClass) -> bool {\n        match self {\n",
    );
    for def in &relation_defs {
        let body = def
            .domain
            .as_ref()
            .map(|expr| render_class_expr(expr, &class_variant_by_id))
            .unwrap_or_else(|| "true".to_string());
        output.push_str(&format!("            Self::{} => {},\n", def.variant, body));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &[
            "Returns `true` when the given class satisfies the source-generated relation range."
                .to_string(),
        ],
    );
    output.push_str(
        "    pub fn range_allows(self, class: BfoClass) -> bool {\n        match self {\n",
    );
    for def in &relation_defs {
        let body = def
            .range
            .as_ref()
            .map(|expr| render_class_expr(expr, &class_variant_by_id))
            .unwrap_or_else(|| "true".to_string());
        output.push_str(&format!("            Self::{} => {},\n", def.variant, body));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &[
            "Returns whether BFO declares this relation as a `SymmetricObjectProperty`."
                .to_string(),
        ],
    );
    output.push_str("    pub const fn is_symmetric(self) -> bool {\n        match self {\n");
    for def in &relation_defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant, def.symmetric
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &[
            "Returns whether BFO declares this relation as a `TransitiveObjectProperty`."
                .to_string(),
        ],
    );
    output.push_str("    pub const fn is_transitive(self) -> bool {\n        match self {\n");
    for def in &relation_defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant, def.transitive
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &[
            "Returns whether BFO declares this relation as a `FunctionalObjectProperty`."
                .to_string(),
        ],
    );
    output.push_str("    pub const fn is_functional(self) -> bool {\n        match self {\n");
    for def in &relation_defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant, def.functional
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &[
            "Returns whether BFO declares this relation as an `InverseFunctionalObjectProperty`."
                .to_string(),
        ],
    );
    output
        .push_str("    pub const fn is_inverse_functional(self) -> bool {\n        match self {\n");
    for def in &relation_defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant, def.inverse_functional
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &[
            "Returns whether BFO declares this relation as an `AsymmetricObjectProperty`."
                .to_string(),
        ],
    );
    output.push_str("    pub const fn is_asymmetric(self) -> bool {\n        match self {\n");
    for def in &relation_defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant, def.asymmetric
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &[
            "Returns whether BFO declares this relation as a `ReflexiveObjectProperty`."
                .to_string(),
        ],
    );
    output.push_str("    pub const fn is_reflexive(self) -> bool {\n        match self {\n");
    for def in &relation_defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant, def.reflexive
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &[
            "Returns whether BFO declares this relation as an `IrreflexiveObjectProperty`."
                .to_string(),
        ],
    );
    output.push_str("    pub const fn is_irreflexive(self) -> bool {\n        match self {\n");
    for def in &relation_defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant, def.irreflexive
        ));
    }
    output.push_str("        }\n    }\n\n");

    push_doc_lines(
        &mut output,
        &[
            "Returns the inverse relation declared by `InverseObjectProperties(...)`, if any."
                .to_string(),
        ],
    );
    output.push_str("    pub const fn inverse(self) -> Option<Self> {\n        match self {\n");
    for def in &relation_defs {
        match &def.inverse_id {
            Some(inverse_id) => output.push_str(&format!(
                "            Self::{} => Some(Self::{}),\n",
                def.variant,
                relation_variant_by_id
                    .get(inverse_id)
                    .unwrap_or_else(|| panic!("missing inverse variant for {}", inverse_id))
            )),
            None => output.push_str(&format!("            Self::{} => None,\n", def.variant)),
        }
    }
    output.push_str("        }\n    }\n}\n");

    output.push_str("\nimpl From<RelationKind> for BfoRelationId {\n");
    output.push_str("    fn from(value: RelationKind) -> Self {\n");
    output.push_str("        value.relation_id()\n");
    output.push_str("    }\n");
    output.push_str("}\n\n");

    output.push_str("impl TryFrom<BfoRelationId> for RelationKind {\n");
    output.push_str("    type Error = ();\n\n");
    output.push_str("    fn try_from(value: BfoRelationId) -> Result<Self, Self::Error> {\n");
    output.push_str("        match value.0 {\n");
    for (index, def) in relation_defs.iter().enumerate() {
        output.push_str(&format!(
            "            {index} => Ok(Self::{}),\n",
            def.variant
        ));
    }
    output.push_str("            _ => Err(()),\n");
    output.push_str("        }\n");
    output.push_str("    }\n");
    output.push_str("}\n");

    output
}

fn parse_declarations(ofn: &str, prefix: &str) -> Vec<String> {
    let mut values = Vec::new();
    for line in ofn.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            if let Some(iri) = extract_iri(rest) {
                values.push(iri.to_string());
            }
        }
    }
    values
}

fn parse_annotations(ofn: &str) -> BTreeMap<String, AnnotationValues> {
    let mut values = BTreeMap::new();
    for line in ofn.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("AnnotationAssertion(") else {
            continue;
        };
        let Some(property_end) = rest.find(' ') else {
            continue;
        };
        let property = &rest[..property_end];
        let rest = &rest[property_end + 1..];
        let Some(iri) = extract_iri(rest) else {
            continue;
        };
        let Some(value) = extract_quoted_text(rest) else {
            continue;
        };
        let entry = values
            .entry(iri.to_string())
            .or_insert_with(AnnotationValues::default);
        match property {
            "rdfs:label" => entry.label = Some(value),
            "skos:definition" => entry.definition = Some(value),
            "dc11:identifier" => entry.spec_id = Some(value),
            "skos:altLabel" => entry.alt_labels.push(value),
            "skos:example" => entry.examples.push(value),
            "skos:scopeNote" => entry.scope_notes.push(value),
            _ => {}
        }
    }
    values
}

fn parse_subclass_axioms(ofn: &str) -> BTreeMap<String, Vec<ClassExpr>> {
    let mut axioms = BTreeMap::new();
    for line in ofn.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("SubClassOf(") else {
            continue;
        };
        let Some(end) = rest.find('>') else {
            panic!("unsupported SubClassOf axiom: {trimmed}");
        };
        let child_iri = &rest[1..end];
        let expr_text = rest[end + 1..]
            .trim()
            .strip_suffix(')')
            .unwrap_or_else(|| panic!("SubClassOf should close: {trimmed}"));
        let child = iri_to_bfo_id(child_iri);
        axioms
            .entry(child)
            .or_insert_with(Vec::new)
            .push(parse_class_expr(expr_text));
    }
    axioms
}

fn parse_named_binary_axioms(ofn: &str, prefix: &str) -> BTreeMap<String, Vec<String>> {
    let mut values = BTreeMap::new();
    for line in ofn.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix(prefix) else {
            continue;
        };
        let iris = extract_all_iris(rest);
        if iris.len() != 2 {
            panic!("unsupported binary axiom for {prefix}: {trimmed}");
        }
        let left = iri_to_bfo_id(&iris[0]);
        let right = iri_to_bfo_id(&iris[1]);
        push_unique_map_value(&mut values, left, right);
    }
    sort_and_dedup_map_values(&mut values);
    values
}

fn parse_named_group_axioms(ofn: &str, prefix: &str) -> BTreeMap<String, Vec<String>> {
    let mut values = BTreeMap::new();
    for line in ofn.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix(prefix) else {
            continue;
        };
        let iris = extract_all_iris(rest);
        if iris.len() < 2 {
            panic!("unsupported group axiom for {prefix}: {trimmed}");
        }
        let ids = iris
            .into_iter()
            .map(|iri| iri_to_bfo_id(&iri))
            .collect::<Vec<_>>();
        for (index, id) in ids.iter().enumerate() {
            for (other_index, other_id) in ids.iter().enumerate() {
                if index != other_index {
                    push_unique_map_value(&mut values, id.clone(), other_id.clone());
                }
            }
        }
    }
    sort_and_dedup_map_values(&mut values);
    values
}

fn parse_inverses(ofn: &str) -> BTreeMap<String, String> {
    let mut inverses = BTreeMap::new();
    for line in ofn.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("InverseObjectProperties(") else {
            continue;
        };
        let iris = extract_all_iris(rest);
        if iris.len() != 2 {
            panic!("unsupported inverse axiom: {trimmed}");
        }
        let left = iri_to_bfo_id(&iris[0]);
        let right = iri_to_bfo_id(&iris[1]);
        inverses.insert(left.clone(), right.clone());
        inverses.insert(right, left);
    }
    inverses
}

fn parse_object_property_expr_map(ofn: &str, prefix: &str) -> BTreeMap<String, ClassExpr> {
    let mut values = BTreeMap::new();
    for line in ofn.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix(prefix) else {
            continue;
        };
        let Some(iri) = extract_iri(rest) else {
            continue;
        };
        let Some(end) = rest.find('>') else {
            continue;
        };
        let expr_text = rest[end + 1..].trim();
        let Some(expr_text) = expr_text.strip_suffix(')') else {
            continue;
        };
        values.insert(iri_to_bfo_id(iri), parse_class_expr(expr_text));
    }
    values
}

fn parse_property_flags(ofn: &str, prefix: &str) -> BTreeSet<String> {
    let mut values = BTreeSet::new();
    for line in ofn.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix(prefix) else {
            continue;
        };
        let Some(iri) = extract_iri(rest) else {
            continue;
        };
        values.insert(iri_to_bfo_id(iri));
    }
    values
}

fn parse_class_expr(text: &str) -> ClassExpr {
    let (expr, rest) = parse_class_expr_inner(text.trim());
    if !rest.trim().is_empty() {
        panic!("unexpected trailing class expression tokens: {rest}");
    }
    expr
}

fn parse_class_expr_inner(text: &str) -> (ClassExpr, &str) {
    let text = text.trim_start();
    if let Some(rest) = text.strip_prefix("ObjectUnionOf(") {
        let (items, rest) = parse_class_expr_list(rest);
        return (ClassExpr::Union(items), rest);
    }
    if let Some(rest) = text.strip_prefix("ObjectIntersectionOf(") {
        let (items, rest) = parse_class_expr_list(rest);
        return (ClassExpr::Intersection(items), rest);
    }
    if let Some(rest) = text.strip_prefix("ObjectComplementOf(") {
        let (expr, rest) = parse_class_expr_inner(rest);
        let rest = rest
            .trim_start()
            .strip_prefix(')')
            .expect("ObjectComplementOf should close");
        return (ClassExpr::Complement(Box::new(expr)), rest);
    }
    if let Some(rest) = text.strip_prefix("ObjectAllValuesFrom(") {
        let (relation_id, rest) = parse_object_property_ref(rest);
        let (filler, rest) = parse_class_expr_inner(rest);
        let rest = rest
            .trim_start()
            .strip_prefix(')')
            .expect("ObjectAllValuesFrom should close");
        return (
            ClassExpr::AllValuesFrom {
                relation_id,
                filler: Box::new(filler),
            },
            rest,
        );
    }
    if let Some(rest) = text.strip_prefix("ObjectSomeValuesFrom(") {
        let (relation_id, rest) = parse_object_property_ref(rest);
        let (filler, rest) = parse_class_expr_inner(rest);
        let rest = rest
            .trim_start()
            .strip_prefix(')')
            .expect("ObjectSomeValuesFrom should close");
        return (
            ClassExpr::SomeValuesFrom {
                relation_id,
                filler: Box::new(filler),
            },
            rest,
        );
    }
    let end = text
        .find('>')
        .expect("named class expression should contain closing '>'");
    let iri = &text[1..end];
    (ClassExpr::Named(iri_to_bfo_id(iri)), &text[end + 1..])
}

fn parse_object_property_ref(text: &str) -> (String, &str) {
    let text = text.trim_start();
    let end = text
        .find('>')
        .expect("object property reference should contain closing '>'");
    let iri = &text[1..end];
    (iri_to_bfo_id(iri), &text[end + 1..])
}

fn parse_class_expr_list(mut text: &str) -> (Vec<ClassExpr>, &str) {
    let mut items = Vec::new();
    loop {
        text = text.trim_start();
        if let Some(rest) = text.strip_prefix(')') {
            return (items, rest);
        }
        let (expr, rest) = parse_class_expr_inner(text);
        items.push(expr);
        text = rest;
    }
}

fn render_class_expr(expr: &ClassExpr, class_variant_by_id: &BTreeMap<String, String>) -> String {
    render_class_expr_with_parens(expr, class_variant_by_id, false)
}

fn class_ancestor_variants(
    def: &ClassDef,
    parent_ids_by_id: &BTreeMap<String, Vec<String>>,
    class_variant_by_id: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    collect_class_ancestor_variants(
        &def.id,
        parent_ids_by_id,
        class_variant_by_id,
        &mut seen,
        &mut out,
    );
    out
}

fn class_ancestor_ids(
    def: &ClassDef,
    parent_ids_by_id: &BTreeMap<String, Vec<String>>,
) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    collect_class_ancestor_ids(&def.id, parent_ids_by_id, &mut seen, &mut out);
    out
}

fn collect_class_ancestor_variants(
    id: &str,
    parent_ids_by_id: &BTreeMap<String, Vec<String>>,
    class_variant_by_id: &BTreeMap<String, String>,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<String>,
) {
    if !seen.insert(id.to_string()) {
        return;
    }
    let variant = class_variant_by_id
        .get(id)
        .unwrap_or_else(|| panic!("missing ancestor variant for {}", id));
    out.push(variant.clone());
    if let Some(parent_ids) = parent_ids_by_id.get(id) {
        for parent_id in parent_ids {
            collect_class_ancestor_variants(
                parent_id,
                parent_ids_by_id,
                class_variant_by_id,
                seen,
                out,
            );
        }
    }
}

fn collect_class_ancestor_ids(
    id: &str,
    parent_ids_by_id: &BTreeMap<String, Vec<String>>,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<String>,
) {
    if !seen.insert(id.to_string()) {
        return;
    }
    out.push(id.to_string());
    if let Some(parent_ids) = parent_ids_by_id.get(id) {
        for parent_id in parent_ids {
            collect_class_ancestor_ids(parent_id, parent_ids_by_id, seen, out);
        }
    }
}

fn push_doc_lines(output: &mut String, lines: &[String]) {
    for line in lines {
        output.push_str(&format!("#[doc = \"{}\"]\n", escape_string(line)));
    }
}

fn class_doc_lines(def: &ClassDef, class_variant_by_id: &BTreeMap<String, String>) -> Vec<String> {
    let mut lines = vec![
        format!("BFO class `{}`.", def.label),
        format!("ID: `{}`.", def.id),
        format!("IRI: `{}`.", def.iri),
    ];
    if let Some(spec_id) = &def.spec_id {
        lines.push(format!("Spec ID: `{}`.", spec_id));
    }
    if let Some(definition) = &def.definition {
        lines.push(format!("Definition: {}", definition));
    }
    if let Some(parent_id) = def.direct_parent_ids.first() {
        let parent_variant = class_variant_by_id
            .get(parent_id)
            .unwrap_or_else(|| panic!("missing parent doc variant for {}", parent_id));
        lines.push(format!(
            "Direct parent: [`BfoClass::{}`](enum.BfoClass.html#variant.{}).",
            parent_variant, parent_variant
        ));
    }
    lines
}

fn relation_doc_lines(
    def: &RelationDef,
    class_label_by_id: &BTreeMap<String, String>,
    relation_variant_by_id: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut lines = vec![
        format!("BFO relation `{}`.", def.label),
        format!("ID: `{}`.", def.id),
        format!("IRI: `{}`.", def.iri),
    ];
    if let Some(spec_id) = &def.spec_id {
        lines.push(format!("Spec ID: `{}`.", spec_id));
    }
    if let Some(definition) = &def.definition {
        lines.push(format!("Definition: {}", definition));
    }
    if !def.direct_parent_ids.is_empty() {
        let parents = def
            .direct_parent_ids
            .iter()
            .map(|parent_id| {
                let parent_variant = relation_variant_by_id
                    .get(parent_id)
                    .unwrap_or_else(|| panic!("missing relation parent variant for {}", parent_id));
                format!(
                    "[`RelationKind::{}`](enum.RelationKind.html#variant.{})",
                    parent_variant, parent_variant
                )
            })
            .collect::<Vec<_>>();
        lines.push(format!("Direct parents: {}.", parents.join(", ")));
    }
    if let Some(domain) = &def.domain {
        lines.push(format!(
            "Domain: `{}`.",
            render_class_expr_doc(domain, class_label_by_id)
        ));
    }
    if let Some(range) = &def.range {
        lines.push(format!(
            "Range: `{}`.",
            render_class_expr_doc(range, class_label_by_id)
        ));
    }
    if let Some(inverse_id) = &def.inverse_id {
        let inverse_variant = relation_variant_by_id
            .get(inverse_id)
            .unwrap_or_else(|| panic!("missing inverse doc variant for {}", inverse_id));
        lines.push(format!(
            "Inverse: [`RelationKind::{}`](enum.RelationKind.html#variant.{}).",
            inverse_variant, inverse_variant
        ));
    }
    let mut characteristics = Vec::new();
    if def.symmetric {
        characteristics.push("symmetric");
    }
    if def.transitive {
        characteristics.push("transitive");
    }
    if def.functional {
        characteristics.push("functional");
    }
    if def.inverse_functional {
        characteristics.push("inverse functional");
    }
    if def.asymmetric {
        characteristics.push("asymmetric");
    }
    if def.reflexive {
        characteristics.push("reflexive");
    }
    if def.irreflexive {
        characteristics.push("irreflexive");
    }
    if !characteristics.is_empty() {
        lines.push(format!("Characteristics: {}.", characteristics.join(", ")));
    }
    lines
}

fn render_class_expr_doc(expr: &ClassExpr, class_label_by_id: &BTreeMap<String, String>) -> String {
    match expr {
        ClassExpr::Named(id) => class_label_by_id
            .get(id)
            .cloned()
            .unwrap_or_else(|| id.clone()),
        ClassExpr::Union(items) => items
            .iter()
            .map(|item| render_class_expr_doc(item, class_label_by_id))
            .collect::<Vec<_>>()
            .join(" or "),
        ClassExpr::Intersection(items) => items
            .iter()
            .map(|item| render_class_expr_doc(item, class_label_by_id))
            .collect::<Vec<_>>()
            .join(" and "),
        ClassExpr::Complement(item) => {
            format!("not ({})", render_class_expr_doc(item, class_label_by_id))
        }
        ClassExpr::AllValuesFrom {
            relation_id,
            filler,
        } => format!(
            "all `{relation_id}` values in ({})",
            render_class_expr_doc(filler, class_label_by_id)
        ),
        ClassExpr::SomeValuesFrom {
            relation_id,
            filler,
        } => format!(
            "some `{relation_id}` values in ({})",
            render_class_expr_doc(filler, class_label_by_id)
        ),
    }
}

fn render_class_expr_with_parens(
    expr: &ClassExpr,
    class_variant_by_id: &BTreeMap<String, String>,
    wrap: bool,
) -> String {
    match expr {
        ClassExpr::Named(id) => format!(
            "class.is_a(BfoClass::{})",
            class_variant_by_id
                .get(id)
                .unwrap_or_else(|| panic!("missing class variant for {}", id))
        ),
        ClassExpr::Union(items) => {
            let parts = items
                .iter()
                .map(|item| render_class_expr_with_parens(item, class_variant_by_id, true))
                .collect::<Vec<_>>();
            let body = parts.join(" || ");
            if wrap { format!("({body})") } else { body }
        }
        ClassExpr::Intersection(items) => {
            let parts = items
                .iter()
                .map(|item| render_class_expr_with_parens(item, class_variant_by_id, true))
                .collect::<Vec<_>>();
            let body = parts.join(" && ");
            if wrap { format!("({body})") } else { body }
        }
        ClassExpr::Complement(item) => {
            format!(
                "!({})",
                render_class_expr_with_parens(item, class_variant_by_id, false)
            )
        }
        ClassExpr::AllValuesFrom { .. } | ClassExpr::SomeValuesFrom { .. } => {
            panic!("quantified class expressions are not yet evaluable in domain/range helpers")
        }
    }
}

fn render_class_expr_ofn(expr: &ClassExpr) -> String {
    match expr {
        ClassExpr::Named(id) => id.clone(),
        ClassExpr::Union(items) => format!(
            "ObjectUnionOf({})",
            items
                .iter()
                .map(render_class_expr_ofn)
                .collect::<Vec<_>>()
                .join(" ")
        ),
        ClassExpr::Intersection(items) => format!(
            "ObjectIntersectionOf({})",
            items
                .iter()
                .map(render_class_expr_ofn)
                .collect::<Vec<_>>()
                .join(" ")
        ),
        ClassExpr::Complement(item) => {
            format!("ObjectComplementOf({})", render_class_expr_ofn(item))
        }
        ClassExpr::AllValuesFrom {
            relation_id,
            filler,
        } => format!(
            "ObjectAllValuesFrom({} {})",
            relation_id,
            render_class_expr_ofn(filler)
        ),
        ClassExpr::SomeValuesFrom {
            relation_id,
            filler,
        } => format!(
            "ObjectSomeValuesFrom({} {})",
            relation_id,
            render_class_expr_ofn(filler)
        ),
    }
}

fn render_constraint_slice_expr(
    constraints: &[RestrictionDef],
    relation_variant_by_id: &BTreeMap<String, String>,
) -> String {
    if constraints.is_empty() {
        return "&[]".to_string();
    }
    let items = constraints
        .iter()
        .map(|constraint| {
            let relation_variant = relation_variant_by_id
                .get(&constraint.relation_id)
                .unwrap_or_else(|| panic!("missing relation variant for {}", constraint.relation_id));
            let filler = escape_string(&render_class_expr_ofn(&constraint.filler));
            match constraint.quantifier {
                RestrictionQuantifier::AllValuesFrom => format!(
                    "ClassConstraint::AllValuesFrom {{ relation: RelationKind::{relation_variant}, filler_ofn: \"{filler}\" }}"
                ),
                RestrictionQuantifier::SomeValuesFrom => format!(
                    "ClassConstraint::SomeValuesFrom {{ relation: RelationKind::{relation_variant}, filler_ofn: \"{filler}\" }}"
                ),
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("&[{items}]")
}

fn direct_parent_ids(exprs: Option<&Vec<ClassExpr>>) -> Vec<String> {
    let mut parents = exprs
        .into_iter()
        .flat_map(|exprs| exprs.iter())
        .filter_map(|expr| match expr {
            ClassExpr::Named(id) => Some(id.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    parents.sort();
    parents.dedup();
    parents
}

fn restriction_defs(exprs: Option<&Vec<ClassExpr>>) -> Vec<RestrictionDef> {
    let mut restrictions = exprs
        .into_iter()
        .flat_map(|exprs| exprs.iter())
        .filter_map(|expr| match expr {
            ClassExpr::AllValuesFrom {
                relation_id,
                filler,
            } => Some(RestrictionDef {
                quantifier: RestrictionQuantifier::AllValuesFrom,
                relation_id: relation_id.clone(),
                filler: (**filler).clone(),
            }),
            ClassExpr::SomeValuesFrom {
                relation_id,
                filler,
            } => Some(RestrictionDef {
                quantifier: RestrictionQuantifier::SomeValuesFrom,
                relation_id: relation_id.clone(),
                filler: (**filler).clone(),
            }),
            ClassExpr::Named(_) => None,
            other => panic!(
                "unsupported top-level subclass expression: {}",
                render_class_expr_ofn(other)
            ),
        })
        .collect::<Vec<_>>();
    restrictions.sort_by(|a, b| {
        a.relation_id
            .cmp(&b.relation_id)
            .then_with(|| render_class_expr_ofn(&a.filler).cmp(&render_class_expr_ofn(&b.filler)))
    });
    restrictions
}

fn render_string_slice_expr(values: &[String]) -> String {
    if values.is_empty() {
        "&[]".to_string()
    } else {
        let items = values
            .iter()
            .map(|value| format!("\"{}\"", escape_string(value)))
            .collect::<Vec<_>>()
            .join(", ");
        format!("&[{items}]")
    }
}

fn render_self_slice_expr(ids: &[String], variant_by_id: &BTreeMap<String, String>) -> String {
    if ids.is_empty() {
        "&[]".to_string()
    } else {
        let items = ids
            .iter()
            .map(|id| {
                let variant = variant_by_id
                    .get(id)
                    .unwrap_or_else(|| panic!("missing variant for {}", id));
                format!("Self::{variant}")
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("&[{items}]")
    }
}

fn push_id_slice_tables(
    output: &mut String,
    prefix: &str,
    type_name: &str,
    slices: &[Vec<usize>],
    render_value: impl Fn(usize) -> String,
) {
    for (index, slice) in slices.iter().enumerate() {
        output.push_str(&format!(
            "const {}: &[{type_name}] = ",
            render_slice_table_ref(prefix, index)
        ));
        if slice.is_empty() {
            output.push_str("&[];\n");
            continue;
        }

        let items = slice
            .iter()
            .map(|value| render_value(*value))
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!("&[{items}];\n"));
    }
}

fn resolve_id_indices(
    ids: &[String],
    index_by_id: &BTreeMap<String, usize>,
    entity_label: &str,
) -> Vec<usize> {
    ids.iter()
        .map(|id| {
            *index_by_id
                .get(id)
                .unwrap_or_else(|| panic!("missing index for {entity_label} {id}"))
        })
        .collect()
}

fn render_slice_table_ref(prefix: &str, index: usize) -> String {
    format!("{prefix}_{index}")
}

fn extract_iri(text: &str) -> Option<&str> {
    let start = text.find('<')?;
    let end = text[start + 1..].find('>')? + start + 1;
    Some(&text[start + 1..end])
}

fn extract_all_iris(text: &str) -> Vec<String> {
    let mut iris = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('>') else {
            break;
        };
        iris.push(after[..end].to_string());
        rest = &after[end + 1..];
    }
    iris
}

fn extract_quoted_text(text: &str) -> Option<String> {
    let start = text.find('"')?;
    let mut out = String::new();
    let mut escaped = false;
    for ch in text[start + 1..].chars() {
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => return Some(out),
            _ => out.push(ch),
        }
    }
    None
}

fn iri_to_bfo_id(iri: &str) -> String {
    let suffix = iri
        .rsplit('/')
        .next()
        .expect("iri should have trailing component");
    suffix.replace('_', ":")
}

fn id_to_fallback_label(id: &str) -> String {
    id.to_string()
}

fn sanitize_to_variant(label: &str) -> String {
    let mut out = String::new();
    for part in label
        .chars()
        .map(|ch| if ch.is_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
    {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    if out.is_empty() {
        return "Term".to_string();
    }
    if out.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        format!("Term{out}")
    } else {
        out
    }
}

fn escape_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn push_unique_map_value(map: &mut BTreeMap<String, Vec<String>>, key: String, value: String) {
    map.entry(key).or_default().push(value);
}

fn sort_and_dedup_map_values(map: &mut BTreeMap<String, Vec<String>>) {
    for values in map.values_mut() {
        values.sort();
        values.dedup();
    }
}

fn assert_unique_variants<'a>(variants: impl Iterator<Item = &'a str>, kind: &str) {
    let mut seen = BTreeSet::new();
    for variant in variants {
        if !seen.insert(variant.to_string()) {
            panic!("duplicate {kind} variant: {variant}");
        }
    }
}
