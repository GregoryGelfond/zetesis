//! Stable insertion IDs partition canonical rows without another tuple owner.

use super::*;
use crate::oracle::relations::{Catalogs, Relational};
use crate::{
    Cancellation,
    oracle::{Limits, Statistics},
};
use zetesis_core::{Atom, Model, Predicate, Value};

fn atom(value: i32) -> Atom {
    Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(value)]).unwrap()
}

fn work(cancellation: &Cancellation) -> Work<'_> {
    let mut work = Work::source(cancellation, Limits::default().max_work);
    work.limits = Limits::default();
    work
}

fn mixed(work: &mut Work<'_>) -> Catalogs {
    let mut catalogs = Catalogs::default();
    for value in [4, 2] {
        catalogs.insert(atom(value), 0, work).unwrap();
    }
    catalogs.prepare_delta(work).unwrap();
    catalogs.advance(work).unwrap();
    for value in [3, 1] {
        catalogs.insert(atom(value), 0, work).unwrap();
    }
    // The partition control starts with a real published, canonically ordered
    // extent. It measures only the additional derived old/new ID preparation.
    catalogs.prepare(work).unwrap();
    catalogs
}

/// The view holds exactly the rows with these IDs, borrowed from the sole
/// owner, each run in ascending value order; `ids` and `values` are given in
/// canonical order and matched as a set, since the runs together are not
/// one sequence.
fn assert_rows(catalogs: &Catalogs, set: RowSet, ids: &[usize], values: &[i32]) {
    let predicate = Predicate::new("p", 1).unwrap();
    let source = catalogs.relation(&predicate).catalog().atoms();
    let rows = catalogs.selected(&predicate, set).unwrap();
    let mut seen: Vec<usize> = rows
        .all()
        .into_iter()
        .map(|row| {
            let row = row.atom().unwrap();
            let id = source
                .iter()
                .position(|atom| std::ptr::eq(atom, row))
                .unwrap();
            assert_eq!(
                row,
                &atom(values[ids.iter().position(|&i| i == id).unwrap()])
            );
            id
        })
        .collect();
    seen.sort_unstable();
    let mut expected = ids.to_vec();
    expected.sort_unstable();
    assert_eq!(seen, expected);
    for run in 0..rows.runs() {
        let run: Vec<&Atom> = (0..rows.run_len(run))
            .map(|position| rows.get(run, position).unwrap().atom().unwrap())
            .collect();
        assert!(run.windows(2).all(|pair| pair[0] < pair[1]));
    }
}

#[test]
fn age_tracks_insertion_identity_when_canonical_ranks_move() {
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation);
    let mut catalogs = mixed(&mut work);
    let before = catalogs.owned_bytes();
    catalogs.prepare_delta(&mut work).unwrap();
    assert_rows(&catalogs, RowSet::Current, &[3, 1, 2, 0], &[1, 2, 3, 4]);
    assert_rows(&catalogs, RowSet::Old, &[1, 0], &[2, 4]);
    assert_rows(&catalogs, RowSet::New, &[3, 2], &[1, 3]);
    // The views are the catalog's own runs: the partition owns no buffer.
    assert_eq!(catalogs.owned_bytes(), before);
    let predicate = Predicate::new("p", 1).unwrap();
    assert_eq!(catalogs.relation(&predicate).partition().old_end, 2);
    let charged = work.statistics.catalog_work;
    catalogs.prepare_delta(&mut work).unwrap();
    assert_eq!(work.statistics.catalog_work - charged, 1); // One cutoff check.
    assert_eq!(catalogs.owned_bytes(), before);
    catalogs.advance(&mut work).unwrap();
    catalogs.prepare_delta(&mut work).unwrap();
    assert_rows(&catalogs, RowSet::Old, &[3, 1, 2, 0], &[1, 2, 3, 4]);
    assert_rows(&catalogs, RowSet::New, &[], &[]);
}

#[test]
fn a_refused_merge_keeps_the_old_view_and_publishes_no_partial_run() {
    // As `mixed`, but the merge of the second round's rows is refused for
    // bytes: the first round's view stays borrowable, the delta views are
    // unavailable, and the merge succeeds once the ceiling is raised.
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation);
    let mut catalogs = Catalogs::default();
    for value in [4, 2] {
        catalogs.insert(atom(value), 0, &mut work).unwrap();
    }
    catalogs.prepare_delta(&mut work).unwrap();
    catalogs.advance(&mut work).unwrap();
    let retained = catalogs.owned_bytes();
    for value in [3, 1] {
        catalogs.insert(atom(value), 0, &mut work).unwrap();
    }
    let appended = catalogs.owned_bytes();
    work.limits.max_closure_bytes = usize::try_from(appended).unwrap();
    assert_eq!(catalogs.prepare_delta(&mut work), Err(Stop::StorageLimit));
    // A refused step may keep capacity it admitted first, never more than
    // the ceiling, and publishes no run: the delta views stay unavailable.
    assert!(catalogs.owned_bytes() <= appended);
    assert!(appended > retained);
    let predicate = Predicate::new("p", 1).unwrap();
    assert!(matches!(
        catalogs.selected(&predicate, RowSet::New),
        Err(Stop::InvalidProgram)
    ));
    work.limits = Limits::default();
    catalogs.prepare_delta(&mut work).unwrap();
    assert_rows(&catalogs, RowSet::Current, &[3, 1, 2, 0], &[1, 2, 3, 4]);
    assert_rows(&catalogs, RowSet::Old, &[1, 0], &[2, 4]);
    assert_rows(&catalogs, RowSet::New, &[3, 2], &[1, 3]);
}

#[test]
fn an_interrupted_delta_preparation_charges_exactly_its_limit() {
    let cancellation = Cancellation::default();
    let mut observed = work(&cancellation);
    let mut reference = mixed(&mut observed);
    observed.statistics = Statistics::default();
    reference.prepare_delta(&mut observed).unwrap();
    let needed = observed.statistics.work;
    assert_eq!(needed, observed.statistics.catalog_work);
    for max_work in 0..needed {
        let mut limited = work(&cancellation);
        let mut catalogs = mixed(&mut limited);
        limited.statistics = Statistics::default();
        limited.limits.max_work = max_work;
        assert_eq!(catalogs.prepare_delta(&mut limited), Err(Stop::WorkLimit));
        assert_eq!(limited.statistics.work, max_work);
        assert_eq!(limited.statistics.catalog_work, max_work);
        // The catalog merged its runs when `mixed` prepared it, so every view
        // is already whole; the interrupted check published nothing new.
        assert_rows(&catalogs, RowSet::Current, &[3, 1, 2, 0], &[1, 2, 3, 4]);
        limited.limits = Limits::default();
        catalogs.prepare_delta(&mut limited).unwrap();
        assert_rows(&catalogs, RowSet::Old, &[1, 0], &[2, 4]);
        assert_rows(&catalogs, RowSet::New, &[3, 2], &[1, 3]);
    }
}

#[test]
fn extraction_retires_delta_identity_but_retains_empty_capacity() {
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation);
    let mut catalogs = mixed(&mut work);
    catalogs.prepare_delta(&mut work).unwrap();
    let predicate = Predicate::new("p", 1).unwrap();
    let model = catalogs.take_model(&mut work).unwrap();
    assert_eq!(model, Model::new([1, 2, 3, 4].map(atom)));
    assert_eq!(catalogs.relation(&predicate).partition().old_end, 0);
    catalogs.insert(atom(9), 0, &mut work).unwrap();
    catalogs.prepare_delta(&mut work).unwrap();
    assert_rows(&catalogs, RowSet::Old, &[], &[]);
    assert_rows(&catalogs, RowSet::New, &[0], &[9]);
    assert_eq!(model, Model::new([1, 2, 3, 4].map(atom)));
}

#[test]
fn signed_typed_rows_keep_their_own_partition() {
    use zetesis_core::{Sign, ValueLimits, ValueNode};
    let structured = Value::from_nodes(
        vec![
            ValueNode::Function {
                name: "f".into(),
                arity: 1,
                sign: Sign::Negative,
            },
            ValueNode::Number(1),
        ],
        ValueLimits::default(),
    )
    .unwrap();
    let values = [
        Value::String("1".into()),
        structured,
        Value::Number(1),
        Value::Symbol("1".into()),
    ];
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation);
    let mut catalogs = Catalogs::default();
    let predicates = [Sign::Positive, Sign::Negative]
        .map(|sign| Predicate::with_sign("typed", 1, sign).unwrap());
    for predicate in &predicates {
        for value in &values[..2] {
            catalogs
                .insert(
                    Atom::new(predicate.clone(), vec![value.clone()]).unwrap(),
                    0,
                    &mut work,
                )
                .unwrap();
        }
    }
    catalogs.prepare_delta(&mut work).unwrap();
    catalogs.advance(&mut work).unwrap();
    for predicate in &predicates {
        for value in &values[2..] {
            catalogs
                .insert(
                    Atom::new(predicate.clone(), vec![value.clone()]).unwrap(),
                    0,
                    &mut work,
                )
                .unwrap();
        }
    }
    catalogs.prepare_delta(&mut work).unwrap();
    for predicate in &predicates {
        for (set, ids) in [(RowSet::Old, [0, 1]), (RowSet::New, [2, 3])] {
            let rows = catalogs.selected(predicate, set).unwrap();
            let all = rows.all();
            assert_eq!(all.len(), ids.len());
            for (actual, id) in all.into_iter().zip(ids) {
                let actual = actual.atom().unwrap();
                assert_eq!(actual.predicate(), predicate);
                assert_eq!(actual.values(), &[values[id].clone()]);
                assert!(std::ptr::eq(
                    actual,
                    &raw const catalogs.relation(predicate).catalog().atoms()[id]
                ));
            }
        }
    }
}
