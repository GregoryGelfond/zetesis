//! Failure receipts account for actual dictionary work without publishing a query.

use super::{atoms, predicate};
use crate::Value;
use crate::relation::{Failure, Limits, Relation, Resource};

#[test]
fn completed_attempt_matches_the_convenience_query() {
    let predicate = predicate(1);
    let atoms = atoms(&predicate, vec![vec![Value::Number(7)], vec![Value::String("7".into())]]);
    let relation = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
    let value = Value::String("7".into());
    let keys = [(0, &value), (0, &value)];
    let ordinary = relation.query(&keys, Limits::default()).unwrap();
    let attempt = relation.query_attempt(&keys, Limits::default());
    let query = attempt.result.unwrap();
    assert!(query.relation().same_owner(&relation));
    assert_eq!(query.equalities(), ordinary.equalities());
    assert_eq!(query.is_possible(), ordinary.is_possible());
    assert_eq!(query.work(), ordinary.work());
    assert_eq!(attempt.work, query.work());
    assert_eq!(attempt.peak_bytes, relation.storage().retained_bytes + query.retained_bytes());
}

#[test]
fn every_work_interruption_retains_its_lookup_prefix() {
    let predicate = predicate(1);
    let atoms = atoms(&predicate, (0..17).map(|row| vec![Value::String(format!("common-prefix-{row:02}"))]).collect());
    let relation = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
    let value = Value::String("common-prefix-16".into());
    let missing = Value::Number(16);
    let keys = [(0, &value), (0, &missing)];
    let complete = relation.query_attempt(&keys, Limits::default());
    let query = complete.result.unwrap();
    assert!(!query.is_possible());
    assert!(complete.work > 4);
    for maximum in 0..=u64::try_from(complete.work).unwrap() {
        let attempt = relation.query_attempt(&keys, Limits { max_work: maximum, ..Limits::default() });
        // This lookup charges individual descriptor/text steps. No failed step
        // is committed, and the later missing key still completes normally.
        assert_eq!(attempt.work, u128::from(maximum));
        assert_eq!(attempt.peak_bytes, complete.peak_bytes);
        if u128::from(maximum) == complete.work {
            assert_eq!(attempt.result.unwrap().equalities(), query.equalities());
        } else {
            assert!(matches!(attempt.result, Err(Failure::Limit {
                resource: Resource::Work, observed, limit,
            }) if observed == u128::from(maximum) + 1 && limit == u128::from(maximum)));
        }
    }
    assert_eq!(relation.row(16).unwrap().value(0), Some(&value));
    assert_eq!(relation.row_count(), atoms.len());
}

#[test]
fn later_invalid_columns_keep_prior_missing_key_work() {
    let predicate = predicate(1);
    let atoms = atoms(&predicate, vec![vec![Value::Number(7)]]);
    let relation = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
    let missing = Value::Number(8);
    let prefix = relation.query_attempt(&[(0, &missing)], Limits::default());
    assert!(!prefix.result.unwrap().is_possible());
    let failure = relation.query_attempt(&[(0, &missing), (1, &missing)], Limits::default());
    assert!(matches!(failure.result, Err(Failure::Column)));
    assert_eq!(failure.work, prefix.work + 1);
    assert!(failure.peak_bytes > relation.storage().retained_bytes);
}

#[test]
fn refused_initial_frame_admits_no_query_work() {
    let predicate = predicate(0);
    let atoms = atoms(&predicate, vec![vec![]]);
    let relation = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
    let attempt = relation.query_attempt(&[], Limits { max_bytes: 0, ..Limits::default() });
    assert!(matches!(attempt.result, Err(Failure::Limit { resource: Resource::Bytes, limit: 0, .. })));
    assert_eq!(attempt.work, 0);
    assert_eq!(attempt.peak_bytes, 0);
    assert_eq!(relation.row_count(), 1);
}

#[test]
fn query_capacity_is_admitted_before_dictionary_work() {
    let predicate = predicate(1);
    let atoms = atoms(&predicate, vec![vec![Value::Number(7)]]);
    let relation = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
    let value = Value::Number(7);
    let keys = [(0, &value), (0, &value)];
    let complete = relation.query_attempt(&keys, Limits::default());
    assert!(complete.result.unwrap().is_possible());
    let exact = Limits { max_bytes: complete.peak_bytes, ..Limits::default() };
    assert!(relation.query_attempt(&keys, exact).result.is_ok());
    let short = relation.query_attempt(&keys, Limits { max_bytes: exact.max_bytes - 1, ..exact });
    assert!(matches!(short.result, Err(Failure::Limit { resource: Resource::Bytes, .. })));
    assert_eq!(short.work, 0);
}
