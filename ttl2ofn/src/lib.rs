use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use oxrdf::{Graph, LiteralRef, NamedNodeRef, NamedOrBlankNodeRef, TermRef, Triple, TripleRef};
use oxttl::TurtleParser;

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";

const OWL_ANNOTATION_PROPERTY: &str = "http://www.w3.org/2002/07/owl#AnnotationProperty";
const OWL_ALL_DISJOINT_CLASSES: &str = "http://www.w3.org/2002/07/owl#AllDisjointClasses";
const OWL_ALL_VALUES_FROM: &str = "http://www.w3.org/2002/07/owl#allValuesFrom";
const OWL_CLASS: &str = "http://www.w3.org/2002/07/owl#Class";
const OWL_COMPLEMENT_OF: &str = "http://www.w3.org/2002/07/owl#complementOf";
const OWL_DISJOINT_WITH: &str = "http://www.w3.org/2002/07/owl#disjointWith";
const OWL_EQUIVALENT_CLASS: &str = "http://www.w3.org/2002/07/owl#equivalentClass";
const OWL_FUNCTIONAL_PROPERTY: &str = "http://www.w3.org/2002/07/owl#FunctionalProperty";
const OWL_INTERSECTION_OF: &str = "http://www.w3.org/2002/07/owl#intersectionOf";
const OWL_INVERSE_FUNCTIONAL_PROPERTY: &str =
    "http://www.w3.org/2002/07/owl#InverseFunctionalProperty";
const OWL_INVERSE_OF: &str = "http://www.w3.org/2002/07/owl#inverseOf";
const OWL_IRREFLEXIVE_PROPERTY: &str = "http://www.w3.org/2002/07/owl#IrreflexiveProperty";
const OWL_MEMBERS: &str = "http://www.w3.org/2002/07/owl#members";
const OWL_OBJECT_PROPERTY: &str = "http://www.w3.org/2002/07/owl#ObjectProperty";
const OWL_ON_PROPERTY: &str = "http://www.w3.org/2002/07/owl#onProperty";
const OWL_ONTOLOGY: &str = "http://www.w3.org/2002/07/owl#Ontology";
const OWL_REFLEXIVE_PROPERTY: &str = "http://www.w3.org/2002/07/owl#ReflexiveProperty";
const OWL_SOME_VALUES_FROM: &str = "http://www.w3.org/2002/07/owl#someValuesFrom";
const OWL_SYMMETRIC_PROPERTY: &str = "http://www.w3.org/2002/07/owl#SymmetricProperty";
const OWL_TRANSITIVE_PROPERTY: &str = "http://www.w3.org/2002/07/owl#TransitiveProperty";
const OWL_UNION_OF: &str = "http://www.w3.org/2002/07/owl#unionOf";
const OWL_VERSION_IRI: &str = "http://www.w3.org/2002/07/owl#versionIRI";

const RDFS_COMMENT: &str = "http://www.w3.org/2000/01/rdf-schema#comment";
const RDFS_DOMAIN: &str = "http://www.w3.org/2000/01/rdf-schema#domain";
const RDFS_RANGE: &str = "http://www.w3.org/2000/01/rdf-schema#range";
const RDFS_SUBCLASS_OF: &str = "http://www.w3.org/2000/01/rdf-schema#subClassOf";
const RDFS_SUBPROPERTY_OF: &str = "http://www.w3.org/2000/01/rdf-schema#subPropertyOf";

const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";
const BFO_NAMESPACE: &str = "http://purl.obolibrary.org/obo/BFO_";

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
enum ClassExpr {
    Named(String),
    Union(Vec<ClassExpr>),
    Intersection(Vec<ClassExpr>),
    Complement(Box<ClassExpr>),
    AllValuesFrom {
        property: String,
        filler: Box<ClassExpr>,
    },
    SomeValuesFrom {
        property: String,
        filler: Box<ClassExpr>,
    },
}

#[derive(Debug, Clone)]
struct Prefix {
    name: String,
    iri: String,
}

#[derive(Debug, Clone)]
struct RenderConfig {
    prefixes: Vec<Prefix>,
}

pub fn convert_file(input: &Path) -> Result<String> {
    let source =
        fs::read_to_string(input).with_context(|| format!("failed to read {}", input.display()))?;
    let prefixes = parse_prefixes(&source);
    let graph = load_graph(source.as_bytes())?;
    convert_graph(&graph, &RenderConfig { prefixes })
}

fn load_graph(input: &[u8]) -> Result<Graph> {
    let mut graph = Graph::new();
    for triple in TurtleParser::new().for_reader(input) {
        let triple = Triple::from(triple?);
        graph.insert(triple.as_ref());
    }
    Ok(graph)
}

fn convert_graph(graph: &Graph, render: &RenderConfig) -> Result<String> {
    let annotation_properties = annotation_properties(graph);
    let ontology_iri = ontology_iri(graph)?;
    let version_iri = graph
        .object_for_subject_predicate(nnob(&ontology_iri), nn(OWL_VERSION_IRI))
        .and_then(named_node_term_iri)
        .map(|iri| rewrite_version_iri(&iri));

    let ontology_annotations = annotation_assertions_for_subject(
        graph,
        render,
        &annotation_properties,
        nnob(&ontology_iri),
        true,
    )?;
    let declarations = collect_declarations(graph, render)?;
    let annotation_assertions = collect_annotation_assertions(
        graph,
        render,
        &annotation_properties,
        ontology_iri.as_str(),
    )?;
    let object_property_axioms = collect_object_property_axioms(graph, render)?;
    let class_axioms = collect_class_axioms(graph, render)?;
    let general_axioms = collect_general_axioms(graph, render)?;

    let mut output = String::new();
    for prefix in &render.prefixes {
        output.push_str("Prefix(");
        output.push_str(&prefix.name);
        output.push_str("=<");
        output.push_str(&prefix.iri);
        output.push_str(">)\n");
    }
    if !render.prefixes.is_empty() {
        output.push('\n');
    }

    output.push_str("Ontology(");
    output.push_str(&render_full_iri(ontology_iri.as_str()));
    output.push('\n');
    if let Some(version_iri) = version_iri {
        output.push_str(&render_full_iri(&version_iri));
        output.push('\n');
    }
    for line in ontology_annotations {
        output.push_str(&line);
        output.push('\n');
    }
    if !declarations.is_empty()
        || !annotation_assertions.is_empty()
        || !object_property_axioms.is_empty()
        || !class_axioms.is_empty()
        || !general_axioms.is_empty()
    {
        output.push('\n');
    }
    for line in declarations {
        output.push_str(&line);
        output.push('\n');
    }
    for line in annotation_assertions {
        output.push_str(&line);
        output.push('\n');
    }
    for line in object_property_axioms {
        output.push_str(&line);
        output.push('\n');
    }
    for line in class_axioms {
        output.push_str(&line);
        output.push('\n');
    }
    for line in general_axioms {
        output.push_str(&line);
        output.push('\n');
    }
    output.push(')');
    output.push('\n');
    Ok(output)
}

fn collect_declarations(graph: &Graph, render: &RenderConfig) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    for iri in declared_named_subjects(graph, OWL_CLASS) {
        out.insert(format!(
            "Declaration(Class({}))",
            render_entity_iri(render, &iri)
        ));
    }
    for iri in declared_named_subjects(graph, OWL_OBJECT_PROPERTY) {
        out.insert(format!(
            "Declaration(ObjectProperty({}))",
            render_entity_iri(render, &iri)
        ));
    }
    for iri in declared_named_subjects(graph, OWL_ANNOTATION_PROPERTY) {
        out.insert(format!(
            "Declaration(AnnotationProperty({}))",
            render_compactable_iri(render, &iri)
        ));
    }
    Ok(out)
}

fn collect_annotation_assertions(
    graph: &Graph,
    render: &RenderConfig,
    annotation_properties: &BTreeSet<String>,
    ontology_iri: &str,
) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    for triple in graph.iter() {
        let Some(subject_iri) = named_subject_iri(triple.subject) else {
            continue;
        };
        if subject_iri == ontology_iri {
            continue;
        }
        let predicate_iri = triple.predicate.as_str();
        if !annotation_properties.contains(predicate_iri) {
            continue;
        }
        out.insert(render_annotation_assertion(
            render,
            predicate_iri,
            &subject_iri,
            triple.object,
        )?);
    }
    Ok(out)
}

fn annotation_assertions_for_subject(
    graph: &Graph,
    render: &RenderConfig,
    annotation_properties: &BTreeSet<String>,
    subject: NamedOrBlankNodeRef<'_>,
    ontology_header: bool,
) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    for triple in graph.triples_for_subject(subject) {
        let predicate_iri = triple.predicate.as_str();
        if !annotation_properties.contains(predicate_iri) {
            continue;
        }
        let rendered = if ontology_header {
            format!(
                "Annotation({} {})",
                render_compactable_iri(render, predicate_iri),
                render_annotation_value(render, triple.object)?
            )
        } else {
            let subject_iri = named_subject_iri(triple.subject)
                .ok_or_else(|| anyhow!("annotation assertion subject should be a named node"))?;
            render_annotation_assertion(render, predicate_iri, &subject_iri, triple.object)?
        };
        out.insert(rendered);
    }
    Ok(out)
}

fn collect_object_property_axioms(
    graph: &Graph,
    render: &RenderConfig,
) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    let properties = declared_named_subjects(graph, OWL_OBJECT_PROPERTY);
    for property in properties {
        let rendered_property = render_entity_iri(render, &property);
        for object in graph.objects_for_subject_predicate(nnob(&property), nn(OWL_INVERSE_OF)) {
            let inverse = named_node_term_iri(object)
                .ok_or_else(|| anyhow!("owl:inverseOf must point to a named property"))?;
            out.insert(format!(
                "InverseObjectProperties({} {})",
                rendered_property,
                render_entity_iri(render, &inverse)
            ));
        }
        for object in graph.objects_for_subject_predicate(nnob(&property), nn(RDFS_DOMAIN)) {
            out.insert(format!(
                "ObjectPropertyDomain({} {})",
                rendered_property,
                render_class_expr(render, &parse_class_expr(graph, object)?)?
            ));
        }
        for object in graph.objects_for_subject_predicate(nnob(&property), nn(RDFS_RANGE)) {
            out.insert(format!(
                "ObjectPropertyRange({} {})",
                rendered_property,
                render_class_expr(render, &parse_class_expr(graph, object)?)?
            ));
        }
        for object in graph.objects_for_subject_predicate(nnob(&property), nn(RDFS_SUBPROPERTY_OF))
        {
            let parent = named_node_term_iri(object)
                .ok_or_else(|| anyhow!("rdfs:subPropertyOf must point to a named property"))?;
            out.insert(format!(
                "SubObjectPropertyOf({} {})",
                rendered_property,
                render_entity_iri(render, &parent)
            ));
        }
        if has_type(graph, &property, OWL_TRANSITIVE_PROPERTY) {
            out.insert(format!("TransitiveObjectProperty({})", rendered_property));
        }
        if has_type(graph, &property, OWL_SYMMETRIC_PROPERTY) {
            out.insert(format!("SymmetricObjectProperty({})", rendered_property));
        }
        if has_type(graph, &property, OWL_FUNCTIONAL_PROPERTY) {
            out.insert(format!("FunctionalObjectProperty({})", rendered_property));
        }
        if has_type(graph, &property, OWL_INVERSE_FUNCTIONAL_PROPERTY) {
            out.insert(format!(
                "InverseFunctionalObjectProperty({})",
                rendered_property
            ));
        }
        if has_type(graph, &property, OWL_REFLEXIVE_PROPERTY) {
            out.insert(format!("ReflexiveObjectProperty({})", rendered_property));
        }
        if has_type(graph, &property, OWL_IRREFLEXIVE_PROPERTY) {
            out.insert(format!("IrreflexiveObjectProperty({})", rendered_property));
        }
    }
    Ok(out)
}

fn collect_class_axioms(graph: &Graph, render: &RenderConfig) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    let classes = declared_named_subjects(graph, OWL_CLASS);
    for class in classes {
        let rendered_class = render_entity_iri(render, &class);
        for object in graph.objects_for_subject_predicate(nnob(&class), nn(RDFS_SUBCLASS_OF)) {
            out.insert(format!(
                "SubClassOf({} {})",
                rendered_class,
                render_class_expr(render, &parse_class_expr(graph, object)?)?
            ));
        }
        for object in graph.objects_for_subject_predicate(nnob(&class), nn(OWL_EQUIVALENT_CLASS)) {
            out.insert(format!(
                "EquivalentClasses({} {})",
                rendered_class,
                render_class_expr(render, &parse_class_expr(graph, object)?)?
            ));
        }
        for object in graph.objects_for_subject_predicate(nnob(&class), nn(OWL_DISJOINT_WITH)) {
            out.insert(format!(
                "DisjointClasses({} {})",
                rendered_class,
                render_class_expr(render, &parse_class_expr(graph, object)?)?
            ));
        }
    }
    Ok(out)
}

fn collect_general_axioms(graph: &Graph, render: &RenderConfig) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    for subject in
        graph.subjects_for_predicate_object(nn(RDF_TYPE), nn_term(OWL_ALL_DISJOINT_CLASSES))
    {
        let NamedOrBlankNodeRef::BlankNode(blank) = subject else {
            continue;
        };
        let members = graph
            .object_for_subject_predicate(blank, nn(OWL_MEMBERS))
            .ok_or_else(|| anyhow!("owl:AllDisjointClasses without owl:members"))?;
        let mut rendered = Vec::new();
        for member in parse_list(graph, members)? {
            rendered.push(render_class_expr(
                render,
                &parse_class_expr(graph, member)?,
            )?);
        }
        out.insert(format!("DisjointClasses({})", rendered.join(" ")));
    }
    Ok(out)
}

fn parse_class_expr(graph: &Graph, term: TermRef<'_>) -> Result<ClassExpr> {
    match term {
        TermRef::NamedNode(node) => Ok(ClassExpr::Named(node.as_str().to_owned())),
        TermRef::BlankNode(node) => {
            if let Some(filler) = graph.object_for_subject_predicate(node, nn(OWL_ALL_VALUES_FROM))
            {
                let property = graph
                    .object_for_subject_predicate(node, nn(OWL_ON_PROPERTY))
                    .and_then(named_node_term_iri)
                    .ok_or_else(|| {
                        anyhow!("owl:allValuesFrom restriction without owl:onProperty")
                    })?;
                return Ok(ClassExpr::AllValuesFrom {
                    property,
                    filler: Box::new(parse_class_expr(graph, filler)?),
                });
            }
            if let Some(filler) = graph.object_for_subject_predicate(node, nn(OWL_SOME_VALUES_FROM))
            {
                let property = graph
                    .object_for_subject_predicate(node, nn(OWL_ON_PROPERTY))
                    .and_then(named_node_term_iri)
                    .ok_or_else(|| {
                        anyhow!("owl:someValuesFrom restriction without owl:onProperty")
                    })?;
                return Ok(ClassExpr::SomeValuesFrom {
                    property,
                    filler: Box::new(parse_class_expr(graph, filler)?),
                });
            }
            if let Some(value) = graph.object_for_subject_predicate(node, nn(OWL_UNION_OF)) {
                let parts = parse_list(graph, value)?
                    .into_iter()
                    .map(|item| parse_class_expr(graph, item))
                    .collect::<Result<Vec<_>>>()?;
                return Ok(ClassExpr::Union(parts));
            }
            if let Some(value) = graph.object_for_subject_predicate(node, nn(OWL_INTERSECTION_OF)) {
                let parts = parse_list(graph, value)?
                    .into_iter()
                    .map(|item| parse_class_expr(graph, item))
                    .collect::<Result<Vec<_>>>()?;
                return Ok(ClassExpr::Intersection(parts));
            }
            if let Some(value) = graph.object_for_subject_predicate(node, nn(OWL_COMPLEMENT_OF)) {
                return Ok(ClassExpr::Complement(Box::new(parse_class_expr(
                    graph, value,
                )?)));
            }
            bail!(
                "unsupported blank-node class expression: _:{}",
                node.as_str()
            );
        }
        TermRef::Literal(_) => bail!("literals are not valid class expressions"),
    }
}

fn parse_list<'a>(graph: &'a Graph, head: TermRef<'a>) -> Result<Vec<TermRef<'a>>> {
    let mut out = Vec::new();
    let mut cursor = head;
    loop {
        match cursor {
            TermRef::NamedNode(node) if node.as_str() == RDF_NIL => return Ok(out),
            TermRef::BlankNode(node) => {
                let first = graph
                    .object_for_subject_predicate(node, nn(RDF_FIRST))
                    .ok_or_else(|| anyhow!("rdf:list node missing rdf:first"))?;
                let rest = graph
                    .object_for_subject_predicate(node, nn(RDF_REST))
                    .ok_or_else(|| anyhow!("rdf:list node missing rdf:rest"))?;
                out.push(first);
                cursor = rest;
            }
            _ => bail!("malformed rdf:list"),
        }
    }
}

fn render_annotation_assertion(
    render: &RenderConfig,
    predicate_iri: &str,
    subject_iri: &str,
    object: TermRef<'_>,
) -> Result<String> {
    Ok(format!(
        "AnnotationAssertion({} {} {})",
        render_compactable_iri(render, predicate_iri),
        render_entity_iri(render, subject_iri),
        render_annotation_value(render, object)?
    ))
}

fn render_annotation_value(render: &RenderConfig, object: TermRef<'_>) -> Result<String> {
    match object {
        TermRef::NamedNode(node) => Ok(render_full_iri(node.as_str())),
        TermRef::BlankNode(node) => bail!(
            "blank node annotation values are not supported: _:{}",
            node.as_str()
        ),
        TermRef::Literal(literal) => Ok(render_literal(render, literal)),
    }
}

fn render_class_expr(render: &RenderConfig, expr: &ClassExpr) -> Result<String> {
    Ok(match expr {
        ClassExpr::Named(iri) => render_entity_iri(render, iri),
        ClassExpr::Union(parts) => format!(
            "ObjectUnionOf({})",
            parts
                .iter()
                .map(|part| render_class_expr(render, part))
                .collect::<Result<Vec<_>>>()?
                .join(" ")
        ),
        ClassExpr::Intersection(parts) => format!(
            "ObjectIntersectionOf({})",
            parts
                .iter()
                .map(|part| render_class_expr(render, part))
                .collect::<Result<Vec<_>>>()?
                .join(" ")
        ),
        ClassExpr::Complement(inner) => {
            format!("ObjectComplementOf({})", render_class_expr(render, inner)?)
        }
        ClassExpr::AllValuesFrom { property, filler } => format!(
            "ObjectAllValuesFrom({} {})",
            render_entity_iri(render, property),
            render_class_expr(render, filler)?
        ),
        ClassExpr::SomeValuesFrom { property, filler } => format!(
            "ObjectSomeValuesFrom({} {})",
            render_entity_iri(render, property),
            render_class_expr(render, filler)?
        ),
    })
}

fn render_literal(render: &RenderConfig, literal: LiteralRef<'_>) -> String {
    let escaped = escape_string(literal.value());
    if let Some(language) = literal.language() {
        return format!("\"{}\"@{}", escaped, language);
    }
    let datatype = literal.datatype().as_str();
    if datatype == XSD_STRING {
        return format!("\"{}\"", escaped);
    }
    format!(
        "\"{}\"^^{}",
        escaped,
        render_compactable_iri(render, datatype)
    )
}

fn escape_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

fn render_entity_iri(render: &RenderConfig, iri: &str) -> String {
    if iri.starts_with(BFO_NAMESPACE) {
        render_full_iri(iri)
    } else {
        render_compactable_iri(render, iri)
    }
}

fn render_compactable_iri(render: &RenderConfig, iri: &str) -> String {
    if iri.starts_with(BFO_NAMESPACE) {
        return render_full_iri(iri);
    }
    for prefix in &render.prefixes {
        if iri.starts_with(&prefix.iri) {
            let local = &iri[prefix.iri.len()..];
            if is_qname_local(local) {
                return format!("{}:{}", prefix.name, local);
            }
        }
    }
    render_full_iri(iri)
}

fn render_full_iri(iri: &str) -> String {
    format!("<{}>", iri)
}

fn is_qname_local(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

fn declared_named_subjects(graph: &Graph, class_iri: &str) -> BTreeSet<String> {
    graph
        .subjects_for_predicate_object(nn(RDF_TYPE), term_named_node(class_iri))
        .filter_map(named_subject_iri)
        .collect()
}

fn ontology_iri(graph: &Graph) -> Result<String> {
    let mut subjects = graph
        .subjects_for_predicate_object(nn(RDF_TYPE), nn_term(OWL_ONTOLOGY))
        .filter_map(named_subject_iri);
    let ontology = subjects
        .next()
        .ok_or_else(|| anyhow!("missing owl:Ontology declaration"))?;
    if subjects.next().is_some() {
        bail!("multiple owl:Ontology declarations are not supported");
    }
    Ok(ontology)
}

fn has_type(graph: &Graph, subject_iri: &str, type_iri: &str) -> bool {
    graph.contains(TripleRef::new(
        nnob(subject_iri),
        nn(RDF_TYPE),
        term_named_node(type_iri),
    ))
}

fn parse_prefixes(source: &str) -> Vec<Prefix> {
    let mut prefixes = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("@prefix ") {
            if let Some((name_part, iri_part)) = rest.split_once('<') {
                let raw_name = name_part.trim();
                let name = if raw_name == ":" {
                    ":".to_owned()
                } else {
                    raw_name.trim_end_matches(':').to_owned()
                };
                let iri = iri_part
                    .split('>')
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                prefixes.push(Prefix { name, iri });
            }
        }
    }
    prefixes
}

fn annotation_properties(graph: &Graph) -> BTreeSet<String> {
    let mut properties = declared_named_subjects(graph, OWL_ANNOTATION_PROPERTY);
    properties.insert(RDFS_COMMENT.to_owned());
    properties.insert("http://www.w3.org/2000/01/rdf-schema#label".to_owned());
    properties
}

fn rewrite_version_iri(iri: &str) -> String {
    if let Some(prefix) = iri.strip_suffix(".ttl") {
        return format!("{prefix}.ofn");
    }
    iri.to_owned()
}

fn named_subject_iri(subject: NamedOrBlankNodeRef<'_>) -> Option<String> {
    match subject {
        NamedOrBlankNodeRef::NamedNode(node) => Some(node.as_str().to_owned()),
        NamedOrBlankNodeRef::BlankNode(_) => None,
    }
}

fn named_node_term_iri(term: TermRef<'_>) -> Option<String> {
    match term {
        TermRef::NamedNode(node) => Some(node.as_str().to_owned()),
        _ => None,
    }
}

fn nn(iri: &'static str) -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(iri)
}

fn nn_term(iri: &'static str) -> TermRef<'static> {
    TermRef::NamedNode(nn(iri))
}

fn nnob(iri: &str) -> NamedOrBlankNodeRef<'_> {
    NamedOrBlankNodeRef::NamedNode(NamedNodeRef::new_unchecked(iri))
}

fn term_named_node<'a>(iri: &'a str) -> TermRef<'a> {
    TermRef::NamedNode(NamedNodeRef::new_unchecked(iri))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::convert_file;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("ttl2ofn should be in the workspace root")
            .to_path_buf()
    }

    fn bfo_ttl() -> PathBuf {
        repo_root().join("bfo/BFO-2020-master/21838-2/owl/bfo-core.ttl")
    }

    fn bfo_ofn() -> PathBuf {
        repo_root().join("bfo/BFO-2020-master/21838-2/owl/bfo-core.ofn")
    }

    fn supported_line(line: &str) -> bool {
        [
            "Declaration(",
            "AnnotationAssertion(",
            "SubClassOf(",
            "DisjointClasses(",
            "EquivalentClasses(",
            "SubObjectPropertyOf(",
            "InverseObjectProperties(",
            "ObjectPropertyDomain(",
            "ObjectPropertyRange(",
            "SymmetricObjectProperty(",
            "TransitiveObjectProperty(",
            "FunctionalObjectProperty(",
            "InverseFunctionalObjectProperty(",
            "AsymmetricObjectProperty(",
            "ReflexiveObjectProperty(",
            "IrreflexiveObjectProperty(",
        ]
        .iter()
        .any(|prefix| line.starts_with(prefix))
    }

    fn normalized_supported_lines(text: &str) -> Vec<String> {
        let mut lines = text
            .lines()
            .map(str::trim)
            .filter(|line| supported_line(line))
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        lines.sort();
        lines
    }

    #[test]
    fn generated_bfo_axioms_match_official_ofn_for_supported_axioms() {
        let generated = convert_file(&bfo_ttl()).expect("BFO TTL should convert");
        let official = fs::read_to_string(bfo_ofn()).expect("official BFO OFN should be readable");
        assert_eq!(
            normalized_supported_lines(&generated),
            normalized_supported_lines(&official)
        );
        assert!(generated.contains("<http://purl.obolibrary.org/obo/bfo/2020/bfo-core.ofn>"));
    }
}
