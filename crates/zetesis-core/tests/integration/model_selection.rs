//! Owned models and borrowed views preserve the complete typed selected set.

use std::convert::Infallible;
use std::sync::Arc;
use zetesis_core::{
    Atom, AtomPattern, Model, Predicate, Sign, Term, Value, ValueLimits, ValueNode,
};

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
fn model_canonicalization_releases_supplied_handles() {
    let atoms = atoms();
    let selection = Model::new(
        atoms
            .iter()
            .rev()
            .chain(&atoms)
            .map(|atom| atom.as_ref().clone()),
    )
    .unwrap();
    let true_atoms: Vec<_> = selection.atoms().iter().collect();
    assert_eq!(true_atoms.len(), atoms.len());
    assert!(true_atoms.windows(2).all(|pair| pair[0] < pair[1]));
    for atom in &atoms {
        let stored = true_atoms
            .iter()
            .copied()
            .find(|stored| *stored == *atom.as_ref())
            .unwrap();
        assert_eq!(stored, *atom.as_ref());
    }
    let weak: Vec<_> = atoms.iter().map(Arc::downgrade).collect();
    drop(true_atoms);
    drop(atoms);
    assert!(weak.into_iter().all(|atom| atom.upgrade().is_none()));
    assert_eq!(selection.atoms().len(), 8);
}

#[test]
fn owned_and_borrowed_models_preserve_complete_typed_membership() {
    let atoms = atoms();
    let selection = Model::new(atoms.iter().take(5).map(|atom| atom.as_ref().clone())).unwrap();
    let owned = selection.clone();
    assert!(owned.atoms().iter().eq(selection.atoms()));
    for (index, atom) in atoms.iter().enumerate() {
        let pattern = AtomPattern::new(atom.predicate().clone(), vec![Term::Variable(0)]).unwrap();
        let key = pattern.key(atom.values()).unwrap();
        for view in [owned.atoms(), selection.atoms()] {
            assert_eq!(view.contains(atom.as_ref()), index < 5);
            assert_eq!(
                view.lookup()
                    .get_key_with(&key, || Ok::<_, Infallible>(()))
                    .unwrap()
                    .is_some(),
                index < 5
            );
        }
    }
    let nullary = AtomPattern::new(Predicate::new("p", 0).unwrap(), vec![]).unwrap();
    let key = nullary.key(&[] as &[Value]).unwrap();
    assert!(
        selection
            .lookup()
            .get_key_with(&key, || Ok::<_, Infallible>(()))
            .unwrap()
            .is_none()
    );
    assert!(
        owned
            .lookup()
            .get_key_with(&key, || Ok::<_, Infallible>(()))
            .unwrap()
            .is_none()
    );
    let mut forward = selection.atoms().iter();
    let first = forward.next().unwrap();
    assert_eq!(forward.len(), 4);
    let last = forward.next_back().unwrap();
    assert!(first < last);
    assert_eq!(forward.len(), 3);
}

#[test]
fn canonical_queries_preserve_model_membership() {
    let atoms = atoms();
    let selection = Model::new(atoms.iter().take(5).map(|atom| atom.as_ref().clone())).unwrap();
    let owned = selection.clone();
    // Foreign insertion positions cannot stand in for semantic membership.
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
        assert_eq!(owned.atoms().contains(atom), expected);
        assert_eq!(selection.atoms().contains(atom), expected);
    }
}

#[test]
fn selected_payloads_outlive_their_supplied_handles() {
    let atoms = atoms();
    let weak = Arc::downgrade(&atoms[0]);
    let expected = atoms[0].as_ref().clone();
    let selection = Model::new([atoms[0].as_ref().clone()]).unwrap();
    drop(atoms);
    let copied = selection.clone();
    drop(selection);
    assert!(weak.upgrade().is_none());
    assert_eq!(copied.atoms().first().unwrap(), expected);
}

#[test]
fn checked_model_membership_retains_each_refusal() {
    let atoms = atoms();
    let selection = Model::new(atoms.iter().map(|atom| atom.as_ref().clone())).unwrap();
    let pattern = AtomPattern::new(atoms[2].predicate().clone(), vec![Term::Variable(0)]).unwrap();
    let key = pattern.key(atoms[2].values()).unwrap();
    let mut calls = 0;
    assert!(
        selection
            .lookup()
            .get_key_with(&key, || {
                calls += 1;
                Ok::<(), usize>(())
            })
            .unwrap()
            .is_some()
    );
    assert!(calls > 0);
    for cutoff in 0..calls {
        let mut admitted = 0;
        let result = selection.lookup().get_key_with(&key, || {
            if admitted == cutoff {
                return Err(cutoff);
            }
            admitted += 1;
            Ok(())
        });
        assert_eq!(result.unwrap_err(), cutoff);
        assert_eq!(admitted, cutoff);
    }
}
