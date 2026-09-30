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
fn canonicalization_retains_only_program_bound_coordinates() {
    let atoms = atoms();
    let source = program(&atoms);
    let selection = SeedSelection::new(&source, atoms.iter().rev().chain(&atoms).cloned()).unwrap();
    let true_atoms: Vec<_> = selection.view().atoms().collect();
    assert_eq!(true_atoms.len(), atoms.len());
    assert!(true_atoms.windows(2).all(|pair| pair[0] < pair[1]));
    for atom in &atoms {
        let stored = true_atoms
            .iter()
            .copied()
            .find(|stored| *stored == *atom.as_ref())
            .unwrap();
        assert_eq!(stored, *atom.as_ref());
        let value = stored.values().at(0).unwrap();
        let rank = source.domain().binary_search(value).unwrap();
        assert_eq!(value, source.domain().at(rank).unwrap());
    }
    let weak: Vec<_> = atoms.iter().map(Arc::downgrade).collect();
    drop(true_atoms);
    drop(atoms);
    assert!(weak.into_iter().all(|atom| atom.upgrade().is_none()));
    assert_eq!(selection.view().atoms().len(), 8);
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
            assert_eq!(view.contains(atom.as_ref()), index < 5);
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
fn canonical_queries_preserve_seed_membership() {
    let atoms = atoms();
    let source = program(&atoms);
    let selection = SeedSelection::new(&source, atoms.iter().take(5).cloned()).unwrap();
    let owned = selection.to_seed();
    // Reverse insertion ensures foreign catalog positions have no relation to
    // seed membership or the selected atoms' semantic order.
    let catalog = zetesis_core::AtomCatalog::new(
        atoms
            .iter()
            .rev()
            .map(|atom| atom.as_ref().clone())
            .collect(),
    )
    .unwrap();
    for atom in catalog.atoms() {
        let expected = atoms
            .iter()
            .take(5)
            .any(|selected| atom == *selected.as_ref());
        assert_eq!(owned.contains(atom), expected);
        assert_eq!(owned.view().contains(atom), expected);
        assert_eq!(selection.view().contains(atom), expected);
    }
}

#[test]
fn selected_payloads_outlive_their_supplied_handles() {
    let atoms = atoms();
    let source = program(&atoms);
    let weak = Arc::downgrade(&atoms[0]);
    let expected = atoms[0].as_ref().clone();
    let selection = SeedSelection::new(&source, [Arc::clone(&atoms[0])]).unwrap();
    drop(atoms);
    let copied = selection.clone();
    drop(selection);
    assert!(weak.upgrade().is_none());
    assert_eq!(copied.view().atoms().next().unwrap(), expected);
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

#[test]
fn checked_seed_membership_retains_each_refusal() {
    let atoms = atoms();
    let source = program(&atoms);
    let selection = SeedSelection::new(&source, atoms.iter().cloned()).unwrap();
    let pattern = AtomPattern::new(atoms[2].predicate().clone(), vec![Term::Variable(0)]).unwrap();
    let key = pattern.key(atoms[2].values()).unwrap();
    let mut calls = 0;
    assert!(
        selection
            .view()
            .contains_key_with(&key, || {
                calls += 1;
                Ok::<(), usize>(())
            })
            .unwrap()
    );
    for cutoff in 0..calls {
        let mut admitted = 0;
        let result = selection.view().contains_key_with(&key, || {
            if admitted == cutoff {
                return Err(cutoff);
            }
            admitted += 1;
            Ok(())
        });
        assert_eq!(result, Err(cutoff));
        assert_eq!(admitted, cutoff);
    }
}
