//! Parent permits precede the shared dictionary resolver's actual work.

use super::{atoms, predicate};
use crate::Value;
use crate::catalog::TermRef;
use crate::relation::{Catalog, Failure, Limits, QueryFailure, Relation, Resource};

fn refused_prefixes(relation: &Relation<'_>) {
    let value = Value::String("common-prefix-16".into());
    let missing = Value::Number(16);
    let keys = [(0, TermRef::from(&value)), (0, TermRef::from(&missing))];
    let complete = relation.query_attempt(&keys, Limits::default());
    assert!(complete.work > 4);
    let query = complete.result.unwrap();
    for maximum in 0..=complete.work {
        let mut accepted = 0;
        let mut calls = 0;
        let attempt = relation.query_attempt_with(&keys, Limits::default(), || {
            calls += 1;
            if accepted == maximum {
                return Err("parent allowance");
            }
            accepted += 1;
            Ok(())
        });
        assert_eq!(attempt.work, accepted);
        assert_eq!(accepted, maximum);
        assert_eq!(attempt.peak_bytes, complete.peak_bytes);
        if maximum == complete.work {
            assert_eq!(calls, maximum);
            assert_eq!(attempt.result.unwrap().equalities(), query.equalities());
        } else {
            assert_eq!(calls, maximum + 1, "no callback after the refusal");
            assert!(matches!(
                attempt.result,
                Err(QueryFailure::Stopped("parent allowance"))
            ));
        }
    }
}

#[test]
fn caller_refusals_bound_sorted_dictionary_work() {
    let predicate = predicate(1);
    let atoms = atoms(
        &predicate,
        (0..17)
            .map(|row| vec![Value::String(format!("common-prefix-{row:02}"))])
            .collect(),
    );
    let relation = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
    refused_prefixes(&relation);
}

#[test]
fn caller_refusals_bound_append_dictionary_work() {
    use crate::atom_interner::{AtomInterner, Limits as AtomLimits};
    let predicate = predicate(1);
    let mut authority = AtomInterner::default();
    let atom_limits = AtomLimits::for_atoms(32, 1024 * 1024);
    let declared = authority
        .declare_predicate_with(&predicate, atom_limits, || Ok::<_, ()>(()))
        .unwrap();
    let mut catalog = Catalog::new(authority.read(), declared, Limits::default()).unwrap();
    for atom in atoms(
        &predicate,
        (0..17)
            .map(|row| vec![Value::String(format!("common-prefix-{row:02}"))])
            .collect(),
    ) {
        let canonical = authority
            .entry_atom_with(&atom, atom_limits, || Ok::<_, ()>(()))
            .unwrap()
            .insert_ref_with(atom_limits, || Ok::<_, ()>(()))
            .unwrap();
        catalog.insert(canonical, Limits::default()).unwrap();
    }
    refused_prefixes(&catalog.view(authority.read()).unwrap());
}

#[test]
fn operation_limit_precedes_parent_admission() {
    let predicate = predicate(1);
    let atoms = atoms(&predicate, vec![vec![Value::Number(7)]]);
    let relation = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
    let value = Value::Number(7);
    let mut calls = 0;
    let attempt = relation.query_attempt_with(
        &[(0, TermRef::from(&value))],
        Limits {
            max_work: 0,
            ..Limits::default()
        },
        || {
            calls += 1;
            Ok::<(), ()>(())
        },
    );
    assert!(matches!(
        attempt.result,
        Err(QueryFailure::Relation(Failure::Limit {
            resource: Resource::Work,
            observed: 1,
            limit: 0,
        }))
    ));
    assert_eq!(attempt.work, 0);
    assert_eq!(calls, 0);
}

#[test]
fn admitted_callbacks_preserve_local_query_results() {
    let predicate = predicate(1);
    let atoms = atoms(
        &predicate,
        vec![vec![Value::Number(7)], vec![Value::String("7".into())]],
    );
    let relation = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
    let value = Value::String("7".into());
    let keys = [(0, TermRef::from(&value)), (0, TermRef::from(&value))];
    let local = relation.query_attempt(&keys, Limits::default());
    let mut accepted = 0;
    let metered = relation.query_attempt_with(&keys, Limits::default(), || {
        accepted += 1;
        Ok::<(), ()>(())
    });
    assert_eq!(metered.work, local.work);
    assert_eq!(metered.work, accepted);
    assert_eq!(metered.peak_bytes, local.peak_bytes);
    let local = local.result.unwrap();
    let metered = metered.result.unwrap();
    assert!(metered.relation().same_owner(&relation));
    assert_eq!(metered.equalities(), local.equalities());
    assert_eq!(metered.is_possible(), local.is_possible());
}

#[test]
fn caller_refusals_bound_canonical_dictionary_work() {
    let predicate = predicate(1);
    let catalog = crate::AtomCatalog::new(atoms(
        &predicate,
        (0..17)
            .map(|row| vec![Value::String(format!("common-prefix-{row:02}"))])
            .collect(),
    ))
    .unwrap();
    let relation = Relation::from_refs(
        catalog.atoms().at(0).unwrap().predicate(),
        catalog.atoms(),
        Limits::default(),
    )
    .unwrap();
    refused_prefixes(&relation);
}
