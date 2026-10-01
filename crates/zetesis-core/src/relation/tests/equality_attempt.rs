//! Single equalities and owned queries share typed resolution and permit order.

use super::{atoms, predicate};
use crate::catalog::TermRef;
use crate::relation::{Failure, Limits, QueryFailure, Relation, Resource};
use crate::{Value, ValueLimits, ValueNode};

fn agree(relation: &Relation<'_>, values: &[TermRef<'_>]) {
    for &value in values {
        let query = relation.query_attempt(&[(0, value)], Limits::default());
        let resolved = query.result.unwrap();
        let mut accepted = 0;
        let attempt = relation.equality_attempt_with(
            0,
            value,
            Limits {
                max_bytes: relation.storage().retained_bytes,
                ..Limits::default()
            },
            || {
                accepted += 1;
                Ok::<(), ()>(())
            },
        );
        assert!(attempt.relation().same_owner(relation));
        let equality = attempt.result.unwrap();
        assert_eq!(equality.as_slice(), resolved.equalities());
        assert_eq!(equality.is_some(), resolved.is_possible());
        assert_eq!(attempt.work, query.work);
        assert_eq!(attempt.work, accepted);
        assert_eq!(attempt.peak_bytes, relation.storage().retained_bytes);
    }
}

#[test]
fn scalar_and_structural_equalities_match_owned_queries_without_scratch() {
    let predicate = predicate(1);
    let values = [
        Value::Number(7),
        Value::String("7".into()),
        Value::Symbol("7".into()),
        Value::from_nodes(
            vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(7)],
            ValueLimits::default(),
        )
        .unwrap(),
        Value::Supremum,
    ];
    let atoms = atoms(
        &predicate,
        values[..4]
            .iter()
            .cloned()
            .map(|value| vec![value])
            .collect(),
    );
    let relation = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
    agree(
        &relation,
        &values.iter().map(TermRef::from).collect::<Vec<_>>(),
    );
}

#[test]
fn canonical_and_foreign_terms_share_the_single_equality_resolver() {
    use crate::atom_interner::{AtomInterner, Limits as AtomLimits};
    use crate::relation::Catalog;
    let predicate = predicate(1);
    let atom_limits = AtomLimits::for_atoms(8, 1024 * 1024);
    let mut owner = AtomInterner::default();
    let declared = owner
        .declare_predicate_with(&predicate, atom_limits, || Ok::<_, ()>(()))
        .unwrap();
    let mut catalog = Catalog::new(owner.read(), declared, Limits::default()).unwrap();
    for atom in atoms(
        &predicate,
        vec![vec![Value::Number(7)], vec![Value::String("7".into())]],
    ) {
        let canonical = owner
            .entry_atom_with(&atom, atom_limits, || Ok::<_, ()>(()))
            .unwrap()
            .insert_ref_with(atom_limits, || Ok::<_, ()>(()))
            .unwrap();
        catalog.insert(canonical, Limits::default()).unwrap();
    }
    let relation = catalog.view(owner.read()).unwrap();
    let terms: Vec<_> = (0..relation.row_count())
        .map(|row| relation.row(row).unwrap().value(0).unwrap())
        .collect();
    agree(&relation, &terms);
    let foreign =
        crate::AtomCatalog::new(atoms(&predicate, vec![vec![Value::String("7".into())]])).unwrap();
    agree(
        &relation,
        &[foreign.atoms().at(0).unwrap().values().at(0).unwrap()],
    );
}

#[test]
fn single_equality_refusals_preserve_every_accepted_prefix() {
    let predicate = predicate(1);
    let atoms = atoms(
        &predicate,
        (0..17)
            .map(|row| vec![Value::String(format!("common-prefix-{row:02}"))])
            .collect(),
    );
    let relation = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
    let value = Value::String("common-prefix-16".into());
    let complete =
        relation.equality_attempt_with(0, (&value).into(), Limits::default(), || Ok::<(), ()>(()));
    assert!(complete.result.unwrap().is_some());
    for maximum in 0..complete.work {
        let mut accepted = 0;
        let mut calls = 0;
        let failed = relation.equality_attempt_with(0, (&value).into(), Limits::default(), || {
            calls += 1;
            if accepted == maximum {
                return Err("parent");
            }
            accepted += 1;
            Ok(())
        });
        assert!(matches!(
            failed.result,
            Err(QueryFailure::Stopped("parent"))
        ));
        assert_eq!(failed.work, maximum);
        assert_eq!(accepted, maximum);
        assert_eq!(calls, maximum + 1);
        let mut permits = 0;
        let bounded = relation.equality_attempt_with(
            0,
            (&value).into(),
            Limits {
                max_work: u64::try_from(maximum).unwrap(),
                ..Limits::default()
            },
            || {
                permits += 1;
                Ok::<(), ()>(())
            },
        );
        assert!(
            matches!(bounded.result, Err(QueryFailure::Relation(Failure::Limit { resource: Resource::Work, observed, limit })) if observed == maximum + 1 && limit == maximum)
        );
        assert_eq!(bounded.work, maximum);
        assert_eq!(permits, maximum);
    }
}

#[test]
fn single_equality_refusal_precedes_dictionary_inspection() {
    let predicate = predicate(1);
    let atoms = atoms(&predicate, vec![vec![Value::Number(7)]]);
    let relation = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
    let value = Value::Number(8);
    let mut calls = 0;
    let failed = relation.equality_attempt_with(
        0,
        (&value).into(),
        Limits {
            max_bytes: relation.storage().retained_bytes - 1,
            ..Limits::default()
        },
        || {
            calls += 1;
            Ok::<(), ()>(())
        },
    );
    assert!(matches!(
        failed.result,
        Err(QueryFailure::Relation(Failure::Limit {
            resource: Resource::Bytes,
            ..
        }))
    ));
    assert_eq!((calls, failed.work, failed.peak_bytes), (0, 0, 0));
    let invalid =
        relation.equality_attempt_with(1, (&value).into(), Limits::default(), || Ok::<(), ()>(()));
    assert!(matches!(
        invalid.result,
        Err(QueryFailure::Relation(Failure::Column))
    ));
    assert_eq!(invalid.work, 1);
}
