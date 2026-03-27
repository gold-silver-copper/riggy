use std::collections::BTreeSet;

use bfo::BfoClassId;
use bfo::cco::{agent, artifact, event, CcoClassId, CcoRelation};

fn main() {
    println!("BFO + CCO demo");
    println!();
    println!("This example walks a few CCO terms, shows their CCO module,");
    println!("their direct CCO parents, and the BFO classes they ultimately");
    println!("connect to through direct inheritance.");
    println!();

    let classes = [
        artifact::ArtifactClass::DeflectingPrism.into(),
        event::EventClass::Change.into(),
        agent::AgentClass::Disease.into(),
    ];

    for class in classes {
        print_class(class);
    }

    print_relation(CcoRelation::HasProcessPart);
}

fn print_class(class: CcoClassId) {
    println!("CCO class: {} ({})", class.label(), class.curie());
    println!("  IRI: {}", class.iri());
    println!("  Module: {}", class.module().label());

    let direct_cco_parents = class.direct_cco_parents();
    if direct_cco_parents.is_empty() {
        println!("  Direct CCO parents: none");
    } else {
        println!("  Direct CCO parents:");
        for parent in direct_cco_parents {
            println!("    - {} ({})", parent.label(), parent.curie());
        }
    }

    let direct_bfo_parents = class.direct_bfo_parents();
    if direct_bfo_parents.is_empty() {
        println!("  Direct BFO parents: none");
    } else {
        println!("  Direct BFO parents:");
        for parent in direct_bfo_parents {
            println!("    - {} ({})", parent.label(), parent.id());
        }
    }

    let inferred_bfo = inferred_bfo_ancestors(class);
    println!("  Inferred BFO ancestry:");
    for ancestor in inferred_bfo {
        println!("    - {} ({})", ancestor.label(), ancestor.id());
    }

    println!();
}

fn print_relation(relation: CcoRelation) {
    println!("CCO relation: {} ({})", relation.label(), relation.curie());
    println!("  IRI: {}", relation.iri());

    let direct_bfo_parents = relation.direct_bfo_parents();
    if direct_bfo_parents.is_empty() {
        println!("  Direct BFO parents: none");
    } else {
        println!("  Direct BFO parents:");
        for parent in direct_bfo_parents {
            println!("    - {} ({})", parent.label(), parent.id());
        }
    }

    let direct_external_parents = relation.direct_external_parents();
    if !direct_external_parents.is_empty() {
        println!("  External parents:");
        for parent in direct_external_parents {
            println!("    - {}", parent);
        }
    }

    println!();
}

fn inferred_bfo_ancestors(class: CcoClassId) -> Vec<BfoClassId> {
    let mut seen_cco = BTreeSet::new();
    let mut seen_bfo = BTreeSet::new();
    collect_bfo_ancestors(class, &mut seen_cco, &mut seen_bfo);
    seen_bfo.into_iter().collect()
}

fn collect_bfo_ancestors(
    class: CcoClassId,
    seen_cco: &mut BTreeSet<CcoClassId>,
    seen_bfo: &mut BTreeSet<BfoClassId>,
) {
    if !seen_cco.insert(class) {
        return;
    }

    for parent in class.direct_bfo_parents() {
        for ancestor in bfo_ancestor_chain(*parent) {
            seen_bfo.insert(ancestor);
        }
    }

    for parent in class.direct_cco_parents() {
        collect_bfo_ancestors(*parent, seen_cco, seen_bfo);
    }
}

fn bfo_ancestor_chain(class: BfoClassId) -> Vec<BfoClassId> {
    BfoClassId::ALL
        .iter()
        .copied()
        .filter(|candidate| class.is_a(*candidate))
        .collect()
}
