use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

const CCO_CURIE_PREFIX: &str = "cco:ont";
const CCO_IRI_PREFIX: &str = "https://www.commoncoreontologies.org/ont";
const CCO_MODULE_PROPERTY: &str = "cco:ont00001760";
const BFO_IRI_PREFIX: &str = "http://purl.obolibrary.org/obo/BFO_";

#[derive(Debug, Clone, Default)]
struct AnnotationValues {
    label: Option<String>,
    pref_label: Option<String>,
    definition: Option<String>,
    module_iri: Option<String>,
}

#[derive(Debug, Clone)]
struct TermDef {
    token: String,
    id: String,
    iri: String,
    label: String,
    definition: Option<String>,
    variant: String,
    cco_parent_tokens: Vec<String>,
    bfo_parent_tokens: Vec<String>,
    external_parent_tokens: Vec<String>,
    module_key: Option<String>,
    index: usize,
}

#[derive(Debug, Clone)]
struct ModuleDef {
    key: String,
    ontology_iri: String,
    enum_variant: String,
    enum_name: String,
    module_name: String,
    label: String,
    class_indices: Vec<usize>,
}

#[derive(Debug, Clone, Default)]
struct BfoIdMaps {
    class_index_by_iri: BTreeMap<String, usize>,
    relation_index_by_iri: BTreeMap<String, usize>,
}

pub fn generate_cco_files(manifest_dir: &Path, out_dir: &Path) {
    let cco_ttl_path =
        manifest_dir.join("CommonCoreOntologies-develop/src/cco-merged/CommonCoreOntologiesMerged.ttl");
    let bfo_ofn_path = manifest_dir.join("BFO-2020-master/21838-2/owl/bfo-core.ofn");

    println!("cargo:rerun-if-changed={}", cco_ttl_path.display());
    println!("cargo:rerun-if-changed={}", bfo_ofn_path.display());

    let cco_ofn = ttl2ofn::convert_file(&cco_ttl_path)
        .unwrap_or_else(|error| panic!("failed to convert CCO TTL to OFN: {error}"));
    let bfo_id_maps = load_bfo_id_maps(&bfo_ofn_path);

    let generated = generate(&cco_ofn, &bfo_id_maps);
    fs::write(out_dir.join("cco_generated.rs"), generated).expect("failed to write cco_generated.rs");
    fs::write(out_dir.join("cco.ofn"), cco_ofn).expect("failed to write cco.ofn");
}

fn load_bfo_id_maps(path: &Path) -> BfoIdMaps {
    let ofn = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let mut class_iris = parse_declarations(&ofn, "Declaration(Class(")
        .into_iter()
        .filter(|iri| is_bfo_iri(iri))
        .collect::<Vec<_>>();
    class_iris.sort_by_key(|iri| iri_to_bfo_id(iri));
    let class_index_by_iri = class_iris
        .into_iter()
        .enumerate()
        .map(|(index, iri)| (iri, index))
        .collect::<BTreeMap<_, _>>();

    let mut relation_iris = parse_declarations(&ofn, "Declaration(ObjectProperty(")
        .into_iter()
        .filter(|iri| is_bfo_iri(iri))
        .collect::<Vec<_>>();
    relation_iris.sort_by_key(|iri| iri_to_bfo_id(iri));
    let relation_index_by_iri = relation_iris
        .into_iter()
        .enumerate()
        .map(|(index, iri)| (iri, index))
        .collect::<BTreeMap<_, _>>();

    BfoIdMaps {
        class_index_by_iri,
        relation_index_by_iri,
    }
}

fn generate(ofn: &str, bfo_ids: &BfoIdMaps) -> String {
    let annotations = parse_annotations(ofn);
    let class_parent_map = parse_named_parent_axioms(ofn, "SubClassOf(");
    let relation_parent_map = parse_named_parent_axioms(ofn, "SubObjectPropertyOf(");

    let mut class_defs = parse_declarations(ofn, "Declaration(Class(")
        .into_iter()
        .filter(|token| is_cco_entity(token))
        .map(|token| {
            let parents = class_parent_map.get(&token);
            build_term_def(
                token,
                &annotations,
                parents,
                &bfo_ids.class_index_by_iri,
                true,
            )
        })
        .collect::<Vec<_>>();
    class_defs.sort_by(|left, right| left.id.cmp(&right.id));
    for (index, def) in class_defs.iter_mut().enumerate() {
        def.index = index;
    }
    let modules = build_class_modules(&class_defs);
    assign_module_variants(&mut class_defs, &modules);

    let mut relation_defs = parse_declarations(ofn, "Declaration(ObjectProperty(")
        .into_iter()
        .filter(|token| is_cco_entity(token))
        .map(|token| {
            let parents = relation_parent_map.get(&token);
            build_term_def(
                token,
                &annotations,
                parents,
                &bfo_ids.relation_index_by_iri,
                false,
            )
        })
        .collect::<Vec<_>>();
    relation_defs.sort_by(|left, right| left.id.cmp(&right.id));
    assign_unique_variants(&mut relation_defs);

    let class_index_by_token = class_defs
        .iter()
        .map(|def| (def.token.clone(), def.index))
        .collect::<BTreeMap<_, _>>();
    let relation_variant_by_token = relation_defs
        .iter()
        .map(|def| (def.token.clone(), def.variant.clone()))
        .collect::<BTreeMap<_, _>>();
    let relation_index_by_token = relation_defs
        .iter()
        .enumerate()
        .map(|(index, def)| (def.token.clone(), index))
        .collect::<BTreeMap<_, _>>();

    let mut output = String::new();
    output.push_str("// @generated by bfo CCO codegen\n");
    output.push_str("use crate::{BfoClassId, BfoRelationId};\n");
    output.push_str("use serde::{Deserialize, Serialize};\n\n");

    push_cco_module_enum(&mut output, &modules, &class_index_by_token);
    output.push('\n');
    push_cco_class_id(&mut output, &class_defs, &modules, &class_index_by_token, &bfo_ids.class_index_by_iri);
    output.push('\n');
    push_module_enums(&mut output, &modules, &class_defs);
    output.push('\n');
    push_relation_enum(
        &mut output,
        &relation_defs,
        &relation_variant_by_token,
        &relation_index_by_token,
        &bfo_ids.relation_index_by_iri,
    );

    output
}

fn build_term_def(
    token: String,
    annotations: &BTreeMap<String, AnnotationValues>,
    parents: Option<&Vec<String>>,
    bfo_ids: &BTreeMap<String, usize>,
    require_module: bool,
) -> TermDef {
    let annotation = annotations.get(&token).cloned().unwrap_or_default();
    let id = cco_id(&token).unwrap_or_else(|| panic!("unsupported CCO token: {token}"));
    let iri = cco_iri(&token).unwrap_or_else(|| panic!("unsupported CCO token: {token}"));
    let label = annotation
        .label
        .clone()
        .or(annotation.pref_label.clone())
        .unwrap_or_else(|| id.clone());

    let mut cco_parent_tokens = Vec::new();
    let mut bfo_parent_tokens = Vec::new();
    let mut external_parent_tokens = Vec::new();
    for parent in parents.into_iter().flatten() {
        if is_cco_entity(parent) {
            cco_parent_tokens.push(parent.clone());
        } else if is_bfo_iri(parent) && bfo_ids.contains_key(parent) {
            bfo_parent_tokens.push(parent.clone());
        } else {
            external_parent_tokens.push(parent.clone());
        }
    }
    sort_dedup(&mut cco_parent_tokens);
    sort_dedup(&mut bfo_parent_tokens);
    sort_dedup(&mut external_parent_tokens);

    let module_key = annotation.module_iri.as_deref().map(module_key_from_ontology_iri);
    assert!(
        !require_module || module_key.is_some(),
        "missing module annotation for {token}"
    );

    TermDef {
        token,
        id,
        iri,
        label,
        definition: annotation.definition,
        variant: String::new(),
        cco_parent_tokens,
        bfo_parent_tokens,
        external_parent_tokens,
        module_key,
        index: 0,
    }
}

fn build_class_modules(class_defs: &[TermDef]) -> Vec<ModuleDef> {
    let mut module_map = BTreeMap::<String, ModuleDef>::new();
    for class in class_defs {
        let key = class
            .module_key
            .as_ref()
            .unwrap_or_else(|| panic!("missing module for {}", class.id));
        let entry = module_map
            .entry(key.clone())
            .or_insert_with(|| module_def_from_key(key));
        entry.class_indices.push(class.index);
    }
    module_map.into_values().collect()
}

fn module_def_from_key(key: &str) -> ModuleDef {
    let ontology_iri = format!("https://www.commoncoreontologies.org/{key}Ontology");
    let enum_variant = sanitize_to_variant(key);
    let enum_name = format!("{enum_variant}Class");
    let module_name = to_snake_case(key);
    let label = format!("{} Ontology", split_camel_case(key));
    ModuleDef {
        key: key.to_string(),
        ontology_iri,
        enum_variant,
        enum_name,
        module_name,
        label,
        class_indices: Vec::new(),
    }
}

fn assign_module_variants(class_defs: &mut [TermDef], modules: &[ModuleDef]) {
    for module in modules {
        let mut used = BTreeSet::new();
        for &index in &module.class_indices {
            let def = &mut class_defs[index];
            let base = sanitize_to_variant(&def.label);
            let mut variant = base.clone();
            if !used.insert(variant.clone()) {
                variant = format!("{base}{}", sanitize_to_variant(&def.id));
                while !used.insert(variant.clone()) {
                    variant.push('X');
                }
            }
            def.variant = variant;
        }
    }
}

fn push_cco_module_enum(
    output: &mut String,
    modules: &[ModuleDef],
    class_index_by_token: &BTreeMap<String, usize>,
) {
    push_doc_lines(
        output,
        &["Top-level CCO source modules used to group generated class enums.".to_string()],
    );
    output.push_str(
        "#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]\n",
    );
    output.push_str("pub enum CcoModule {\n");
    for module in modules {
        push_doc_lines(
            output,
            &[
                format!("CCO module `{}`.", module.label),
                format!("Ontology IRI: `{}`.", module.ontology_iri),
            ],
        );
        output.push_str(&format!("    {},\n", module.enum_variant));
    }
    output.push_str("}\n\n");

    push_id_slice_tables(
        output,
        "CCO_MODULE_CLASSES",
        "CcoClassId",
        &modules
            .iter()
            .map(|module| module.class_indices.clone())
            .collect::<Vec<_>>(),
        |index| format!("CcoClassId({index})"),
    );
    output.push('\n');

    output.push_str("impl CcoModule {\n");
    output.push_str("    pub const ALL: &'static [Self] = &[\n");
    for module in modules {
        output.push_str(&format!("        Self::{},\n", module.enum_variant));
    }
    output.push_str("    ];\n\n");

    output.push_str("    pub const fn label(self) -> &'static str {\n");
    output.push_str("        match self {\n");
    for module in modules {
        output.push_str(&format!(
            "            Self::{} => \"{}\",\n",
            module.enum_variant,
            escape_string(&module.label)
        ));
    }
    output.push_str("        }\n");
    output.push_str("    }\n\n");

    output.push_str("    pub const fn ontology_iri(self) -> &'static str {\n");
    output.push_str("        match self {\n");
    for module in modules {
        output.push_str(&format!(
            "            Self::{} => \"{}\",\n",
            module.enum_variant,
            module.ontology_iri
        ));
    }
    output.push_str("        }\n");
    output.push_str("    }\n\n");

    output.push_str("    pub fn classes(self) -> &'static [CcoClassId] {\n");
    output.push_str("        match self {\n");
    for module in modules {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            module.enum_variant,
            render_slice_table_ref("CCO_MODULE_CLASSES", index_for_module(modules, &module.key))
        ));
    }
    output.push_str("        }\n");
    output.push_str("    }\n");
    output.push_str("}\n");

    let _ = class_index_by_token;
}

fn push_cco_class_id(
    output: &mut String,
    class_defs: &[TermDef],
    modules: &[ModuleDef],
    class_index_by_token: &BTreeMap<String, usize>,
    bfo_index_by_iri: &BTreeMap<String, usize>,
) {
    push_doc_lines(
        output,
        &["Stable identifier for any generated CCO class.".to_string()],
    );
    output.push_str(
        "#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]\n",
    );
    output.push_str("pub struct CcoClassId(usize);\n\n");

    push_id_slice_tables(
        output,
        "CCO_CLASS_DIRECT_CCO_PARENTS",
        "CcoClassId",
        &class_defs
            .iter()
            .map(|class| resolve_token_indices(&class.cco_parent_tokens, class_index_by_token, "CCO class"))
            .collect::<Vec<_>>(),
        |index| format!("CcoClassId({index})"),
    );
    output.push('\n');
    push_id_slice_tables(
        output,
        "CCO_CLASS_DIRECT_BFO_PARENTS",
        "BfoClassId",
        &class_defs
            .iter()
            .map(|class| resolve_iri_indices(&class.bfo_parent_tokens, bfo_index_by_iri, "BFO class"))
            .collect::<Vec<_>>(),
        |index| format!("BfoClassId::new({index})"),
    );
    output.push('\n');

    output.push_str("impl CcoClassId {\n");
    output.push_str("    const fn new(index: usize) -> Self {\n");
    output.push_str("        Self(index)\n");
    output.push_str("    }\n\n");

    output.push_str("    pub const ALL: &'static [Self] = &[\n");
    for class in class_defs {
        output.push_str(&format!("        Self::new({}),\n", class.index));
    }
    output.push_str("    ];\n\n");

    push_class_id_match_method(output, class_defs, "id", "&'static str", |class| {
        format!("\"{}\"", class.id)
    });
    push_class_id_match_method(output, class_defs, "curie", "&'static str", |class| {
        format!("\"cco:{}\"", class.id)
    });
    push_class_id_match_method(output, class_defs, "iri", "&'static str", |class| {
        format!("\"{}\"", class.iri)
    });
    push_class_id_match_method(output, class_defs, "label", "&'static str", |class| {
        format!("\"{}\"", escape_string(&class.label))
    });

    output.push_str("    pub const fn definition(self) -> Option<&'static str> {\n");
    output.push_str("        match self.0 {\n");
    for class in class_defs {
        match &class.definition {
            Some(definition) => output.push_str(&format!(
                "            {} => Some(\"{}\"),\n",
                class.index,
                escape_string(definition)
            )),
            None => output.push_str(&format!("            {} => None,\n", class.index)),
        }
    }
    output.push_str("            _ => None,\n");
    output.push_str("        }\n");
    output.push_str("    }\n\n");

    output.push_str("    pub const fn module(self) -> CcoModule {\n");
    output.push_str("        match self.0 {\n");
    for class in class_defs {
        let module = class
            .module_key
            .as_ref()
            .unwrap_or_else(|| panic!("missing module for {}", class.id));
        let module_variant = modules
            .iter()
            .find(|candidate| &candidate.key == module)
            .unwrap_or_else(|| panic!("missing module def for {}", module))
            .enum_variant
            .clone();
        output.push_str(&format!(
            "            {} => CcoModule::{},\n",
            class.index, module_variant
        ));
    }
    output.push_str(&format!(
        "            _ => CcoModule::{},\n",
        modules
            .first()
            .expect("at least one CCO module should exist")
            .enum_variant
    ));
    output.push_str("        }\n");
    output.push_str("    }\n\n");

    output.push_str("    pub const fn direct_cco_parents(self) -> &'static [Self] {\n");
    output.push_str("        match self.0 {\n");
    for class in class_defs {
        output.push_str(&format!(
            "            {} => {},\n",
            class.index,
            render_slice_table_ref("CCO_CLASS_DIRECT_CCO_PARENTS", class.index)
        ));
    }
    output.push_str("            _ => &[],\n");
    output.push_str("        }\n");
    output.push_str("    }\n\n");

    output.push_str("    pub const fn direct_bfo_parents(self) -> &'static [BfoClassId] {\n");
    output.push_str("        match self.0 {\n");
    for class in class_defs {
        output.push_str(&format!(
            "            {} => {},\n",
            class.index,
            render_slice_table_ref("CCO_CLASS_DIRECT_BFO_PARENTS", class.index)
        ));
    }
    output.push_str("            _ => &[],\n");
    output.push_str("        }\n");
    output.push_str("    }\n\n");

    output.push_str("    pub fn direct_external_parents(self) -> &'static [&'static str] {\n");
    output.push_str("        match self.0 {\n");
    for class in class_defs {
        let rendered = render_string_slice_expr(&class.external_parent_tokens);
        output.push_str(&format!("            {} => {},\n", class.index, rendered));
    }
    output.push_str("            _ => &[],\n");
    output.push_str("        }\n");
    output.push_str("    }\n\n");

    push_class_lookup_method(output, class_defs, "from_id", |class| class.id.clone());
    push_class_lookup_method(output, class_defs, "from_curie", |class| format!("cco:{}", class.id));
    push_class_lookup_method(output, class_defs, "from_iri", |class| class.iri.clone());

    output.push_str("}\n");
}

fn push_class_id_match_method(
    output: &mut String,
    class_defs: &[TermDef],
    method_name: &str,
    return_type: &str,
    render_expr: impl Fn(&TermDef) -> String,
) {
    output.push_str(&format!(
        "    pub const fn {method_name}(self) -> {return_type} {{\n"
    ));
    output.push_str("        match self.0 {\n");
    for class in class_defs {
        output.push_str(&format!(
            "            {} => {},\n",
            class.index,
            render_expr(class)
        ));
    }
    output.push_str("            _ => \"\",\n");
    output.push_str("        }\n");
    output.push_str("    }\n\n");
}

fn push_class_lookup_method(
    output: &mut String,
    class_defs: &[TermDef],
    method_name: &str,
    render_key: impl Fn(&TermDef) -> String,
) {
    output.push_str(&format!(
        "    pub fn {method_name}(value: &str) -> Option<Self> {{\n"
    ));
    output.push_str("        match value {\n");
    for class in class_defs {
        output.push_str(&format!(
            "            \"{}\" => Some(Self::new({})),\n",
            escape_string(&render_key(class)),
            class.index
        ));
    }
    output.push_str("            _ => None,\n");
    output.push_str("        }\n");
    output.push_str("    }\n\n");
}

fn push_module_enums(output: &mut String, modules: &[ModuleDef], class_defs: &[TermDef]) {
    for module in modules {
        push_doc_lines(
            output,
            &[format!("CCO classes curated in the {}.", module.label)],
        );
        output.push_str(&format!("pub mod {} {{\n", module.module_name));
        output.push_str("    use super::{CcoClassId, CcoModule};\n");
        output.push_str("    use serde::{Deserialize, Serialize};\n\n");

        output.push_str(
            "    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]\n",
        );
        output.push_str(&format!("    pub enum {} {{\n", module.enum_name));
        for &index in &module.class_indices {
            let class = &class_defs[index];
            push_indented_doc_lines(
                output,
                1,
                &[
                    format!("CCO term `{}`.", class.label),
                    format!("ID: `{}`.", class.id),
                    format!("IRI: `{}`.", class.iri),
                ],
            );
            output.push_str(&format!("        {},\n", class.variant));
        }
        output.push_str("    }\n\n");

        output.push_str(&format!("    impl {} {{\n", module.enum_name));
        output.push_str("        pub const ALL: &'static [Self] = &[\n");
        for &index in &module.class_indices {
            let class = &class_defs[index];
            output.push_str(&format!("            Self::{},\n", class.variant));
        }
        output.push_str("        ];\n\n");

        output.push_str("        pub const fn class(self) -> CcoClassId {\n");
        output.push_str("            match self {\n");
        for &index in &module.class_indices {
            let class = &class_defs[index];
            output.push_str(&format!(
                "                Self::{} => CcoClassId::new({}),\n",
                class.variant, class.index
            ));
        }
        output.push_str("            }\n");
        output.push_str("        }\n\n");

        output.push_str("        pub const fn id(self) -> &'static str {\n");
        output.push_str("            self.class().id()\n");
        output.push_str("        }\n\n");

        output.push_str("        pub const fn label(self) -> &'static str {\n");
        output.push_str("            self.class().label()\n");
        output.push_str("        }\n\n");

        output.push_str("        pub const fn module(self) -> CcoModule {\n");
        output.push_str("            CcoModule::");
        output.push_str(&module.enum_variant);
        output.push('\n');
        output.push_str("        }\n\n");

        output.push_str("        pub fn from_id(value: &str) -> Option<Self> {\n");
        output.push_str("            match value {\n");
        for &index in &module.class_indices {
            let class = &class_defs[index];
            output.push_str(&format!(
                "                \"{}\" => Some(Self::{}),\n",
                class.id, class.variant
            ));
        }
        output.push_str("                _ => None,\n");
        output.push_str("            }\n");
        output.push_str("        }\n");
        output.push_str("    }\n");
        output.push_str("}\n\n");

        output.push_str(&format!(
            "impl From<{}::{}> for CcoClassId {{\n",
            module.module_name, module.enum_name
        ));
        output.push_str(&format!(
            "    fn from(value: {}::{}) -> Self {{\n",
            module.module_name, module.enum_name
        ));
        output.push_str("        value.class()\n");
        output.push_str("    }\n");
        output.push_str("}\n\n");

        output.push_str(&format!(
            "impl TryFrom<CcoClassId> for {}::{} {{\n",
            module.module_name, module.enum_name
        ));
        output.push_str("    type Error = ();\n\n");
        output.push_str("    fn try_from(value: CcoClassId) -> Result<Self, Self::Error> {\n");
        output.push_str("        match value.0 {\n");
        for &index in &module.class_indices {
            let class = &class_defs[index];
            output.push_str(&format!(
                "            {} => Ok({}::{}::{}),\n",
                class.index, module.module_name, module.enum_name, class.variant
            ));
        }
        output.push_str("            _ => Err(()),\n");
        output.push_str("        }\n");
        output.push_str("    }\n");
        output.push_str("}\n\n");
    }
}

fn push_relation_enum(
    output: &mut String,
    relation_defs: &[TermDef],
    relation_variant_by_token: &BTreeMap<String, String>,
    relation_index_by_token: &BTreeMap<String, usize>,
    bfo_index_by_iri: &BTreeMap<String, usize>,
) {
    push_doc_lines(
        output,
        &["CCO object properties declared in `CommonCoreOntologiesMerged.ttl`.".to_string()],
    );
    output.push_str(
        "#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]\n",
    );
    output.push_str("pub struct CcoRelationId(usize);\n\n");

    output.push_str(
        "#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]\n",
    );
    output.push_str("pub enum CcoRelation {\n");
    for def in relation_defs {
        push_doc_lines(
            output,
            &[
                format!("CCO relation `{}`.", def.label),
                format!("ID: `{}`.", def.id),
                format!("IRI: `{}`.", def.iri),
            ],
        );
        output.push_str(&format!("    {},\n", def.variant));
    }
    output.push_str("}\n\n");

    push_id_slice_tables(
        output,
        "CCO_RELATION_DIRECT_CCO_PARENTS",
        "CcoRelationId",
        &relation_defs
            .iter()
            .map(|def| resolve_token_indices(&def.cco_parent_tokens, relation_index_by_token, "CCO relation"))
            .collect::<Vec<_>>(),
        |index| format!("CcoRelationId({index})"),
    );
    output.push('\n');
    push_id_slice_tables(
        output,
        "CCO_RELATION_DIRECT_BFO_PARENTS",
        "BfoRelationId",
        &relation_defs
            .iter()
            .map(|def| resolve_iri_indices(&def.bfo_parent_tokens, bfo_index_by_iri, "BFO relation"))
            .collect::<Vec<_>>(),
        |index| format!("BfoRelationId::new({index})"),
    );
    output.push('\n');

    output.push_str("impl CcoRelationId {\n");
    output.push_str("    const fn new(index: usize) -> Self {\n");
    output.push_str("        Self(index)\n");
    output.push_str("    }\n\n");
    output.push_str("    pub const ALL: &'static [Self] = &[\n");
    for (index, _) in relation_defs.iter().enumerate() {
        output.push_str(&format!("        Self::new({index}),\n"));
    }
    output.push_str("    ];\n\n");

    push_relation_id_match_method(output, relation_defs, "id", "&'static str", |def| {
        format!("\"{}\"", def.id)
    });
    push_relation_id_match_method(output, relation_defs, "curie", "&'static str", |def| {
        format!("\"cco:{}\"", def.id)
    });
    push_relation_id_match_method(output, relation_defs, "iri", "&'static str", |def| {
        format!("\"{}\"", def.iri)
    });
    push_relation_id_match_method(output, relation_defs, "label", "&'static str", |def| {
        format!("\"{}\"", escape_string(&def.label))
    });

    output.push_str("    pub const fn definition(self) -> Option<&'static str> {\n");
    output.push_str("        match self.0 {\n");
    for (index, def) in relation_defs.iter().enumerate() {
        match &def.definition {
            Some(definition) => output.push_str(&format!(
                "            {index} => Some(\"{}\"),\n",
                escape_string(definition)
            )),
            None => output.push_str(&format!("            {index} => None,\n")),
        }
    }
    output.push_str("            _ => None,\n");
    output.push_str("        }\n");
    output.push_str("    }\n\n");

    output.push_str("    pub const fn direct_cco_parents(self) -> &'static [Self] {\n");
    output.push_str("        match self.0 {\n");
    for (index, _) in relation_defs.iter().enumerate() {
        output.push_str(&format!(
            "            {index} => {},\n",
            render_slice_table_ref("CCO_RELATION_DIRECT_CCO_PARENTS", index)
        ));
    }
    output.push_str("            _ => &[],\n");
    output.push_str("        }\n");
    output.push_str("    }\n\n");

    output.push_str("    pub const fn direct_bfo_parents(self) -> &'static [BfoRelationId] {\n");
    output.push_str("        match self.0 {\n");
    for (index, _) in relation_defs.iter().enumerate() {
        output.push_str(&format!(
            "            {index} => {},\n",
            render_slice_table_ref("CCO_RELATION_DIRECT_BFO_PARENTS", index)
        ));
    }
    output.push_str("            _ => &[],\n");
    output.push_str("        }\n");
    output.push_str("    }\n\n");

    output.push_str("    pub fn direct_external_parents(self) -> &'static [&'static str] {\n");
    output.push_str("        match self.0 {\n");
    for (index, def) in relation_defs.iter().enumerate() {
        let rendered = render_string_slice_expr(&def.external_parent_tokens);
        output.push_str(&format!("            {index} => {rendered},\n"));
    }
    output.push_str("            _ => &[],\n");
    output.push_str("        }\n");
    output.push_str("    }\n\n");

    push_relation_id_lookup_method(output, relation_defs, "from_id", |def| def.id.clone());
    push_relation_id_lookup_method(output, relation_defs, "from_curie", |def| {
        format!("cco:{}", def.id)
    });
    push_relation_id_lookup_method(output, relation_defs, "from_iri", |def| def.iri.clone());

    output.push_str("}\n\n");

    output.push_str("impl CcoRelation {\n");
    output.push_str("    pub const fn relation_id(self) -> CcoRelationId {\n");
    output.push_str("        match self {\n");
    for (index, def) in relation_defs.iter().enumerate() {
        output.push_str(&format!(
            "            Self::{} => CcoRelationId::new({index}),\n",
            def.variant
        ));
    }
    output.push_str("        }\n");
    output.push_str("    }\n\n");

    output.push_str("    pub const ALL: &'static [Self] = &[\n");
    for def in relation_defs {
        output.push_str(&format!("        Self::{},\n", def.variant));
    }
    output.push_str("    ];\n\n");

    push_relation_match_method(output, relation_defs, "id", "&'static str", |def| {
        format!("\"{}\"", def.id)
    });
    push_relation_match_method(output, relation_defs, "curie", "&'static str", |def| {
        format!("\"cco:{}\"", def.id)
    });
    push_relation_match_method(output, relation_defs, "iri", "&'static str", |def| {
        format!("\"{}\"", def.iri)
    });
    push_relation_match_method(output, relation_defs, "label", "&'static str", |def| {
        format!("\"{}\"", escape_string(&def.label))
    });

    output.push_str("    pub const fn definition(self) -> Option<&'static str> {\n");
    output.push_str("        match self {\n");
    for def in relation_defs {
        match &def.definition {
            Some(definition) => output.push_str(&format!(
                "            Self::{} => Some(\"{}\"),\n",
                def.variant,
                escape_string(definition)
            )),
            None => output.push_str(&format!("            Self::{} => None,\n", def.variant)),
        }
    }
    output.push_str("        }\n");
    output.push_str("    }\n\n");

    push_relation_runtime_match_method(
        output,
        relation_defs,
        "direct_cco_parents",
        "&'static [Self]",
        |def| render_relation_slice_expr(&def.cco_parent_tokens, relation_variant_by_token),
    );
    push_relation_runtime_match_method(
        output,
        relation_defs,
        "direct_bfo_parents",
        "&'static [BfoRelationId]",
        |def| {
            let index = relation_index_by_token
                .get(&def.token)
                .unwrap_or_else(|| panic!("missing CCO relation index for {}", def.token));
            render_slice_table_ref("CCO_RELATION_DIRECT_BFO_PARENTS", *index)
        },
    );
    push_relation_runtime_match_method(
        output,
        relation_defs,
        "direct_external_parents",
        "&'static [&'static str]",
        |def| render_string_slice_expr(&def.external_parent_tokens),
    );

    push_relation_lookup_method(output, relation_defs, "from_id", |def| def.id.clone());
    push_relation_lookup_method(output, relation_defs, "from_curie", |def| format!("cco:{}", def.id));
    push_relation_lookup_method(output, relation_defs, "from_iri", |def| def.iri.clone());

    output.push_str("}\n");

    output.push_str("\nimpl From<CcoRelation> for CcoRelationId {\n");
    output.push_str("    fn from(value: CcoRelation) -> Self {\n");
    output.push_str("        value.relation_id()\n");
    output.push_str("    }\n");
    output.push_str("}\n\n");

    output.push_str("impl TryFrom<CcoRelationId> for CcoRelation {\n");
    output.push_str("    type Error = ();\n\n");
    output.push_str("    fn try_from(value: CcoRelationId) -> Result<Self, Self::Error> {\n");
    output.push_str("        match value.0 {\n");
    for (index, def) in relation_defs.iter().enumerate() {
        output.push_str(&format!("            {index} => Ok(Self::{}),\n", def.variant));
    }
    output.push_str("            _ => Err(()),\n");
    output.push_str("        }\n");
    output.push_str("    }\n");
    output.push_str("}\n");
}

fn push_relation_match_method(
    output: &mut String,
    defs: &[TermDef],
    method_name: &str,
    return_type: &str,
    render_expr: impl Fn(&TermDef) -> String,
) {
    output.push_str(&format!(
        "    pub const fn {method_name}(self) -> {return_type} {{\n"
    ));
    output.push_str("        match self {\n");
    for def in defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant,
            render_expr(def)
        ));
    }
    output.push_str("        }\n");
    output.push_str("    }\n\n");
}

fn push_relation_runtime_match_method(
    output: &mut String,
    defs: &[TermDef],
    method_name: &str,
    return_type: &str,
    render_expr: impl Fn(&TermDef) -> String,
) {
    output.push_str(&format!("    pub fn {method_name}(self) -> {return_type} {{\n"));
    output.push_str("        match self {\n");
    for def in defs {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            def.variant,
            render_expr(def)
        ));
    }
    output.push_str("        }\n");
    output.push_str("    }\n\n");
}

fn push_relation_id_match_method(
    output: &mut String,
    defs: &[TermDef],
    method_name: &str,
    return_type: &str,
    render_expr: impl Fn(&TermDef) -> String,
) {
    output.push_str(&format!(
        "    pub const fn {method_name}(self) -> {return_type} {{\n"
    ));
    output.push_str("        match self.0 {\n");
    for (index, def) in defs.iter().enumerate() {
        output.push_str(&format!("            {index} => {},\n", render_expr(def)));
    }
    output.push_str("            _ => \"\",\n");
    output.push_str("        }\n");
    output.push_str("    }\n\n");
}

fn push_relation_lookup_method(
    output: &mut String,
    defs: &[TermDef],
    method_name: &str,
    render_key: impl Fn(&TermDef) -> String,
) {
    output.push_str(&format!(
        "    pub fn {method_name}(value: &str) -> Option<Self> {{\n"
    ));
    output.push_str("        match value {\n");
    for def in defs {
        output.push_str(&format!(
            "            \"{}\" => Some(Self::{}),\n",
            escape_string(&render_key(def)),
            def.variant
        ));
    }
    output.push_str("            _ => None,\n");
    output.push_str("        }\n");
    output.push_str("    }\n\n");
}

fn push_relation_id_lookup_method(
    output: &mut String,
    defs: &[TermDef],
    method_name: &str,
    render_key: impl Fn(&TermDef) -> String,
) {
    output.push_str(&format!(
        "    pub fn {method_name}(value: &str) -> Option<Self> {{\n"
    ));
    output.push_str("        match value {\n");
    for (index, def) in defs.iter().enumerate() {
        output.push_str(&format!(
            "            \"{}\" => Some(Self::new({index})),\n",
            escape_string(&render_key(def))
        ));
    }
    output.push_str("            _ => None,\n");
    output.push_str("        }\n");
    output.push_str("    }\n\n");
}

fn parse_declarations(ofn: &str, prefix: &str) -> Vec<String> {
    let mut values = Vec::new();
    for line in ofn.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix(prefix) else {
            continue;
        };
        if let Some(entity) = take_entity_ref(rest) {
            values.push(entity);
        }
    }
    values
}

fn parse_annotations(ofn: &str) -> BTreeMap<String, AnnotationValues> {
    let mut values: BTreeMap<String, AnnotationValues> = BTreeMap::new();
    for line in ofn.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("AnnotationAssertion(") else {
            continue;
        };
        let Some(body) = rest.strip_suffix(')') else {
            continue;
        };
        let args = split_top_level_arguments(body);
        if args.len() != 3 {
            continue;
        }
        let property = args[0].as_str();
        let Some(subject) = take_entity_ref(&args[1]) else {
            continue;
        };
        let Some(value) = extract_quoted_text(&args[2]) else {
            continue;
        };
        let entry = values.entry(subject).or_default();
        match property {
            "rdfs:label" => entry.label = Some(value),
            "skos:prefLabel" => entry.pref_label = Some(value),
            "skos:definition" => entry.definition = Some(value),
            CCO_MODULE_PROPERTY => entry.module_iri = Some(value),
            _ => {}
        }
    }
    values
}

fn parse_named_parent_axioms(ofn: &str, prefix: &str) -> BTreeMap<String, Vec<String>> {
    let mut values: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in ofn.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix(prefix) else {
            continue;
        };
        let Some(body) = rest.strip_suffix(')') else {
            continue;
        };
        let args = split_top_level_arguments(body);
        if args.len() != 2 || !is_named_ref(&args[0]) || !is_named_ref(&args[1]) {
            continue;
        }
        let Some(child) = take_entity_ref(&args[0]) else {
            continue;
        };
        let Some(parent) = take_entity_ref(&args[1]) else {
            continue;
        };
        values.entry(child).or_default().push(parent);
    }
    for parents in values.values_mut() {
        sort_dedup(parents);
    }
    values
}

fn assign_unique_variants(defs: &mut [TermDef]) {
    let mut used = BTreeSet::new();
    for def in defs {
        let base = sanitize_to_variant(&def.label);
        let mut variant = base.clone();
        if !used.insert(variant.clone()) {
            variant = format!("{base}{}", sanitize_to_variant(&def.id));
            while !used.insert(variant.clone()) {
                variant.push('X');
            }
        }
        def.variant = variant;
    }
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
        out = "Term".to_string();
    }
    if out.chars().next().is_some_and(|ch| ch.is_ascii_digit()) || is_rust_keyword(&out) {
        out = format!("Term{out}");
    }
    out
}

fn is_rust_keyword(value: &str) -> bool {
    matches!(
        value,
        "As"
            | "Break"
            | "Const"
            | "Continue"
            | "Crate"
            | "Else"
            | "Enum"
            | "Extern"
            | "False"
            | "Fn"
            | "For"
            | "If"
            | "Impl"
            | "In"
            | "Let"
            | "Loop"
            | "Match"
            | "Mod"
            | "Move"
            | "Mut"
            | "Pub"
            | "Ref"
            | "Return"
            | "Self"
            | "SelfType"
            | "Static"
            | "Struct"
            | "Super"
            | "Trait"
            | "True"
            | "Type"
            | "Unsafe"
            | "Use"
            | "Where"
            | "While"
    )
}

fn module_key_from_ontology_iri(iri: &str) -> String {
    let suffix = iri
        .rsplit('/')
        .next()
        .unwrap_or_else(|| panic!("ontology IRI should have suffix: {iri}"));
    suffix.strip_suffix("Ontology").unwrap_or(suffix).to_string()
}

fn split_camel_case(input: &str) -> String {
    let mut out = String::new();
    for (index, ch) in input.chars().enumerate() {
        if index > 0 && ch.is_uppercase() {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

fn to_snake_case(input: &str) -> String {
    let mut out = String::new();
    for (index, ch) in input.chars().enumerate() {
        if ch.is_uppercase() {
            if index > 0 {
                out.push('_');
            }
            for lower in ch.to_lowercase() {
                out.push(lower);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn push_doc_lines(output: &mut String, lines: &[String]) {
    for line in lines {
        output.push_str(&format!("#[doc = \"{}\"]\n", escape_string(line)));
    }
}

fn push_indented_doc_lines(output: &mut String, indent: usize, lines: &[String]) {
    let prefix = "    ".repeat(indent);
    for line in lines {
        output.push_str(&format!(
            "{}#[doc = \"{}\"]\n",
            prefix,
            escape_string(line)
        ));
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

fn resolve_token_indices(
    tokens: &[String],
    index_by_token: &BTreeMap<String, usize>,
    entity_label: &str,
) -> Vec<usize> {
    tokens
        .iter()
        .map(|token| {
            *index_by_token
                .get(token)
                .unwrap_or_else(|| panic!("missing {entity_label} index for {token}"))
        })
        .collect()
}

fn render_relation_slice_expr(
    tokens: &[String],
    variant_by_token: &BTreeMap<String, String>,
) -> String {
    if tokens.is_empty() {
        return "&[]".to_string();
    }
    let items = tokens
        .iter()
        .map(|token| {
            let variant = variant_by_token
                .get(token)
                .unwrap_or_else(|| panic!("missing CCO relation variant for {token}"));
            format!("Self::{variant}")
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("&[{items}]")
}

fn resolve_iri_indices(
    iris: &[String],
    index_by_iri: &BTreeMap<String, usize>,
    entity_label: &str,
) -> Vec<usize> {
    iris.iter()
        .map(|iri| {
            *index_by_iri
                .get(iri)
                .unwrap_or_else(|| panic!("missing {entity_label} index for {iri}"))
        })
        .collect()
}

fn render_slice_table_ref(prefix: &str, index: usize) -> String {
    format!("{prefix}_{index}")
}

fn index_for_module(modules: &[ModuleDef], key: &str) -> usize {
    modules
        .iter()
        .position(|module| module.key == key)
        .unwrap_or_else(|| panic!("missing module definition for {key}"))
}

fn render_string_slice_expr(values: &[String]) -> String {
    if values.is_empty() {
        return "&[]".to_string();
    }
    let items = values
        .iter()
        .map(|value| format!("\"{}\"", escape_string(value)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("&[{items}]")
}

fn split_top_level_arguments(text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    let mut in_angle = false;
    let mut in_string = false;
    let mut escaped = false;

    for ch in text.chars() {
        if in_string {
            current.push(ch);
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => {
                in_string = true;
                current.push(ch);
            }
            '<' => {
                in_angle = true;
                current.push(ch);
            }
            '>' => {
                in_angle = false;
                current.push(ch);
            }
            '(' if !in_angle => {
                depth += 1;
                current.push(ch);
            }
            ')' if !in_angle && depth > 0 => {
                depth -= 1;
                current.push(ch);
            }
            ch if ch.is_whitespace() && depth == 0 && !in_angle => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        args.push(current);
    }
    args
}

fn take_entity_ref(text: &str) -> Option<String> {
    let text = text.trim_start();
    if let Some(rest) = text.strip_prefix('<') {
        let end = rest.find('>')?;
        return Some(rest[..end].to_string());
    }
    let end = text
        .find(|ch: char| ch.is_whitespace() || ch == ')')
        .unwrap_or(text.len());
    if end == 0 {
        None
    } else {
        Some(text[..end].to_string())
    }
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

fn is_named_ref(text: &str) -> bool {
    !text.is_empty() && !text.contains('(') && !text.starts_with('"')
}

fn is_cco_entity(token: &str) -> bool {
    token.starts_with(CCO_CURIE_PREFIX) || token.starts_with(CCO_IRI_PREFIX)
}

fn is_bfo_iri(token: &str) -> bool {
    token.starts_with(BFO_IRI_PREFIX)
}

fn cco_id(token: &str) -> Option<String> {
    if let Some(id) = token.strip_prefix("cco:") {
        return Some(id.to_string());
    }
    token
        .strip_prefix("https://www.commoncoreontologies.org/")
        .map(|id| id.to_string())
}

fn cco_iri(token: &str) -> Option<String> {
    if let Some(id) = token.strip_prefix("cco:") {
        return Some(format!("https://www.commoncoreontologies.org/{id}"));
    }
    if token.starts_with("https://www.commoncoreontologies.org/") {
        return Some(token.to_string());
    }
    None
}

fn iri_to_bfo_id(iri: &str) -> String {
    let suffix = iri
        .rsplit('/')
        .next()
        .unwrap_or_else(|| panic!("BFO IRI should have trailing component: {iri}"));
    suffix.replace('_', ":")
}

fn sort_dedup(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

fn escape_string(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}
