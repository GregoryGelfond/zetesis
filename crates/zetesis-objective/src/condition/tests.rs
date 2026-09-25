use std::convert::Infallible;

use super::*;
use crate::{ObjectiveProgram, ObjectiveTemplate};
use zetesis_core::{Predicate, Term, Value};

fn atom() -> Atom {
    Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(7)]).unwrap()
}
fn catalog() -> AtomCatalog {
    AtomCatalog::new(vec![atom()]).unwrap()
}
fn construct(
    atoms: AtomCatalog,
    nodes: Vec<ConditionIndex>,
    bytes: usize,
) -> Result<Condition, ConditionError> {
    match Condition::from_catalog_with(atoms, nodes, bytes, || Ok::<_, Infallible>(())) {
        Ok(condition) => Ok(condition),
        Err(ConditionFailure::Condition(error)) => Err(error),
        Err(ConditionFailure::Stopped(never)) => match never {},
    }
}
fn template(condition: Condition) -> ObjectiveTemplate {
    ObjectiveTemplate::new(Term::Constant(Value::Number(1)), 0, vec![], vec![], vec![])
        .with_condition(condition)
}

#[test]
fn objective_admission_retains_no_owned_condition_nodes() {
    let program = ObjectiveProgram::new(
        vec![template(Condition::new(vec![
            ConditionNode::Atom(atom()),
            ConditionNode::Atom(atom()),
            ConditionNode::And(0, 1),
        ]))],
        AdmissionLimits::default(),
    )
    .unwrap();
    let Source::Canonical(data) = &program.templates().at(0).unwrap().condition().0 else {
        panic!("an admitted condition must contain only canonical references");
    };
    assert_eq!(data.atoms.storage().atoms, 1);
    assert_eq!(
        data.nodes,
        [
            ConditionNode::Atom(0),
            ConditionNode::Atom(0),
            ConditionNode::And(0, 1)
        ]
    );
}

#[test]
fn catalog_backed_admission_preserves_the_original_authority() {
    let atoms = catalog();
    let condition = construct(atoms.clone(), vec![ConditionNode::Atom(0)], usize::MAX).unwrap();
    let program =
        ObjectiveProgram::new(vec![template(condition)], AdmissionLimits::default()).unwrap();
    let Source::Canonical(data) = &program.templates().at(0).unwrap().condition().0 else {
        panic!("canonical condition");
    };
    assert!(data.atoms.same_owner(&atoms));
    drop(atoms);
    assert_eq!(
        program.templates().at(0).unwrap().condition().nodes().at(0),
        Some(ConditionNode::Atom(AtomRef::from(&atom())))
    );
}

#[test]
fn catalog_coordinates_are_checked_before_publication() {
    assert_eq!(
        construct(catalog(), vec![ConditionNode::Atom(1)], usize::MAX).unwrap_err(),
        ConditionError::AtomReference {
            node: 0,
            atom: 1,
            atoms: 1
        }
    );
}

#[test]
fn catalog_backed_forward_references_are_refused() {
    assert_eq!(
        construct(catalog(), vec![ConditionNode::Not(0)], usize::MAX).unwrap_err(),
        ConditionError::Reference {
            node: 0,
            operand: 0
        }
    );
}

#[test]
fn retained_condition_storage_is_an_inclusive_bound() {
    let atoms = catalog();
    let nodes = vec![ConditionNode::Atom(0)];
    let admitted = construct(atoms.clone(), nodes.clone(), usize::MAX).unwrap();
    let Source::Canonical(data) = &admitted.0 else {
        panic!("canonical condition");
    };
    let bytes =
        usize::try_from(data.atoms.storage().bytes + node_bytes(data.nodes.capacity())).unwrap();
    assert!(construct(atoms.clone(), nodes.clone(), bytes).is_ok());
    assert_eq!(
        construct(atoms, nodes, bytes - 1).unwrap_err(),
        ConditionError::Storage(CatalogError::Storage {
            required: bytes as u128,
            limit: bytes - 1
        })
    );
}

#[test]
fn catalog_construction_preserves_every_typed_stop() {
    let atoms = catalog();
    let nodes = vec![ConditionNode::Atom(0), ConditionNode::Not(0)];
    let mut total = 0;
    Condition::from_catalog_with(atoms.clone(), nodes.clone(), usize::MAX, || {
        total += 1;
        Ok::<_, Infallible>(())
    })
    .unwrap();
    for cutoff in 0..total {
        let cause = ("condition", cutoff);
        let mut accepted = 0;
        let result = Condition::from_catalog_with(atoms.clone(), nodes.clone(), usize::MAX, || {
            if accepted == cutoff {
                Err(&cause)
            } else {
                accepted += 1;
                Ok(())
            }
        });
        assert!(
            matches!(result, Err(ConditionFailure::Stopped(actual)) if std::ptr::eq(actual, &raw const cause))
        );
        assert_eq!(accepted, cutoff);
    }
}

#[test]
fn storage_refusal_keeps_its_template_index() {
    let error = ObjectiveProgram::new(
        vec![template(Condition::new(vec![ConditionNode::Atom(atom())]))],
        AdmissionLimits {
            max_condition_node_bytes: 0,
            ..AdmissionLimits::default()
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        AdmissionError::ConditionStorage {
            template: 0,
            error: ConditionError::Storage(CatalogError::Storage { limit: 0, .. })
        }
    ));
}

#[test]
fn admitted_conditions_keep_already_valid_deep_values() {
    let depth = 300;
    let mut nodes = vec![zetesis_core::ValueNode::Tuple { arity: 1 }; depth];
    nodes.push(zetesis_core::ValueNode::Number(7));
    let value = Value::from_nodes(
        nodes,
        zetesis_core::ValueLimits {
            max_nodes: depth + 1,
            max_depth: depth + 1,
            max_bytes: 1_048_576,
        },
    )
    .unwrap();
    let input = Atom::new(Predicate::new("p", 1).unwrap(), vec![value]).unwrap();
    let program = ObjectiveProgram::new(
        vec![template(Condition::new(vec![ConditionNode::Atom(input)]))],
        AdmissionLimits::default(),
    )
    .unwrap();
    let ConditionNode::Atom(atom) = program
        .templates()
        .at(0)
        .unwrap()
        .condition()
        .nodes()
        .at(0)
        .unwrap()
    else {
        panic!("atom condition");
    };
    assert_eq!(atom.values().at(0).unwrap().depth(), depth + 1);
}

#[test]
fn owned_conditions_publish_one_shared_atom_catalog() {
    let program = ObjectiveProgram::new(
        vec![
            template(Condition::new(vec![ConditionNode::Atom(atom())])),
            template(Condition::new(vec![ConditionNode::Atom(atom())])),
        ],
        AdmissionLimits::default(),
    )
    .unwrap();
    let Source::Canonical(left) = &program.templates().at(0).unwrap().condition().0 else {
        panic!("canonical condition");
    };
    let Source::Canonical(right) = &program.templates().at(1).unwrap().condition().0 else {
        panic!("canonical condition");
    };
    assert!(left.atoms.same_owner(&right.atoms));
    assert_eq!(left.atoms.storage().atoms, 1);
}

#[test]
fn condition_and_template_constants_share_text_payload() {
    let text = "shared objective constant".repeat(32);
    let input = Atom::new(
        Predicate::new("p", 1).unwrap(),
        vec![Value::String(text.clone())],
    )
    .unwrap();
    let row = ObjectiveTemplate::new(
        Term::Constant(Value::Number(1)),
        0,
        vec![Term::Constant(Value::String(text))],
        vec![],
        vec![],
    )
    .with_condition(Condition::new(vec![ConditionNode::Atom(input)]));
    let program = ObjectiveProgram::new(vec![row], AdmissionLimits::default()).unwrap();
    let row = program.templates().at(0).unwrap();
    let zetesis_core::TemplateTerm::Constant(value) = row.tuple().at(0).unwrap() else {
        panic!("constant tuple");
    };
    let ConditionNode::Atom(atom) = row.condition().nodes().at(0).unwrap() else {
        panic!("atom condition");
    };
    let zetesis_core::ValueNodeRef::String(left) = value.descriptor() else {
        panic!("string constant");
    };
    let zetesis_core::ValueNodeRef::String(right) = atom.values().at(0).unwrap().descriptor()
    else {
        panic!("string argument");
    };
    assert!(std::ptr::eq(left, right));
}
