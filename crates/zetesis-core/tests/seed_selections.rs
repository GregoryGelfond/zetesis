//! Shared and owned seed views denote the same instance-bound true set.

use std::sync::Arc;
use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, Predicate, Program, SeedSelection, SeedSelectionError,
    Sign, Template, Term, Value, ValueLimits, ValueNode,
};

fn program(atoms: &[Arc<Atom>]) -> Program {
    Program::new(
        atoms
            .iter()
            .map(|atom| {
                let head = AtomPattern::new(
                    atom.predicate().clone(),
                    atom.values().iter().cloned().map(Term::Constant).collect(),
                )
                .unwrap();
                Template::new(Some(head.clone()), vec![], vec![head], vec![], vec![])
            })
            .collect(),
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn atoms() -> Vec<Arc<Atom>> {
    [Sign::Positive, Sign::Negative]
        .into_iter()
        .flat_map(|sign| {
            [
                Value::Number(1),
                Value::String("1".repeat(256)),
                Value::Symbol("1".repeat(256)),
                Value::from_nodes(
                    vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(1)],
                    ValueLimits::default(),
                )
                .unwrap(),
            ]
            .into_iter()
            .map(move |value| {
                Arc::new(
                    Atom::new(Predicate::with_sign("p", 1, sign).unwrap(), vec![value]).unwrap(),
                )
            })
        })
        .collect()
}

#[test]
fn canonicalization_moves_shared_handles_without_copying_payloads() {
    let atoms = atoms();
    let source = program(&atoms);
    let selection = SeedSelection::new(&source, atoms.iter().rev().chain(&atoms).cloned()).unwrap();
    let true_atoms: Vec<_> = selection.view().atoms().collect();
    assert_eq!(true_atoms.len(), atoms.len());
    assert!(true_atoms.windows(2).all(|pair| pair[0] < pair[1]));
    for atom in &atoms {
        let stored = true_atoms
            .iter()
            .find(|stored| ***stored == **atom)
            .unwrap();
        assert!(std::ptr::eq(*stored, atom.as_ref()));
        assert_eq!(stored.values().as_ptr(), atom.values().as_ptr());
    }
}

#[test]
fn materialization_and_views_preserve_complete_typed_membership() {
    let atoms = atoms();
    let source = program(&atoms);
    let selection = SeedSelection::new(&source, atoms.iter().take(5).cloned()).unwrap();
    let owned = selection.to_seed();
    assert!(owned.program().same_instance(&source));
    assert!(selection.view().program().same_instance(&source));
    assert!(owned.view().atoms().eq(selection.view().atoms()));
    for (index, atom) in atoms.iter().enumerate() {
        let pattern = AtomPattern::new(atom.predicate().clone(), vec![Term::Variable(0)]).unwrap();
        let key = pattern.key(atom.values()).unwrap();
        for view in [owned.view(), selection.view()] {
            assert_eq!(view.contains(atom), index < 5);
            assert_eq!(view.contains_key(&key), index < 5);
        }
    }
    let nullary = AtomPattern::new(Predicate::new("p", 0).unwrap(), vec![]).unwrap();
    let key = nullary.key(&[] as &[Value]).unwrap();
    assert!(!selection.view().contains_key(&key));
    assert!(!owned.view().contains_key(&key));
    let mut forward = selection.view().atoms();
    let first = forward.next().unwrap();
    assert_eq!(forward.len(), 4);
    let last = forward.next_back().unwrap();
    assert!(first < last);
    assert_eq!(forward.len(), 3);
}

#[test]
fn selected_payloads_outlive_their_supplied_handles() {
    let atoms = atoms();
    let source = program(&atoms);
    let pointer = Arc::as_ptr(&atoms[0]);
    let selection = SeedSelection::new(&source, [Arc::clone(&atoms[0])]).unwrap();
    drop(atoms);
    let copied = selection.clone();
    drop(selection);
    assert!(std::ptr::eq(copied.view().atoms().next().unwrap(), pointer));
}

#[test]
fn equal_syntax_keeps_distinct_selection_instance_identity() {
    let atoms = atoms();
    let first = program(&atoms);
    let second = program(&atoms);
    let left = SeedSelection::new(&first, atoms.iter().cloned()).unwrap();
    let right = SeedSelection::new(&second, atoms.iter().cloned()).unwrap();
    assert!(left.view().atoms().eq(right.view().atoms()));
    assert!(!left.view().program().same_instance(right.view().program()));
    assert!(
        !left
            .to_seed()
            .program()
            .same_instance(right.to_seed().program())
    );
}

#[test]
fn outside_carrier_selection_is_a_typed_construction_refusal() {
    let atoms = atoms();
    let source = program(&atoms);
    let foreign = Arc::new(Atom::new(Predicate::new("absent", 0).unwrap(), vec![]).unwrap());
    let failed = SeedSelection::new(&source, [Arc::clone(&foreign)]).unwrap_err();
    let SeedSelectionError::OutsideCarrier { atom } = failed else {
        panic!("wrong construction refusal")
    };
    assert!(Arc::ptr_eq(&atom, &foreign));
}
