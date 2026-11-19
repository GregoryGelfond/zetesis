use super::*;
use crate::atom_registry::AtomRegistry;
use crate::catalog::{DeclaredPredicate, TermKey};
use crate::test_support::PERMIT;
use crate::{Atom, Predicate, TemplateTerm, Value, ValueLimits};
use std::cell::Cell;
use std::convert::Infallible;

fn limits() -> Limits {
    Limits {
        max_atoms: 128,
        max_bytes: 1 << 22,
    }
}

/// An owner with `predicates` declared and the numbers `0..terms` imported
/// and committed, so a split's read prefix names them.
fn owner(
    predicates: &[Predicate],
    terms: i32,
) -> (AtomRegistry, Vec<DeclaredPredicate>, Vec<TermKey>) {
    let mut owner = AtomRegistry::new();
    let mut append = owner.appender();
    let declared = predicates
        .iter()
        .map(|predicate| {
            append
                .declare_predicate_with(predicate, limits(), PERMIT)
                .unwrap()
        })
        .collect();
    let keys = (0..terms)
        .map(|number| {
            append
                .import_term_with(
                    (&Value::Number(number)).into(),
                    TermLimits::default(),
                    limits(),
                    PERMIT,
                )
                .unwrap()
        })
        .collect();
    owner.commit_with(limits(), PERMIT).unwrap();
    (owner, declared, keys)
}

/// Discover `rows` of `edge/2` one by one, or as a prepared block: the
/// positions, in row order, the atoms and the work charged.
fn discover(rows: &[(usize, usize)], prepared: bool) -> (Vec<usize>, Vec<Atom>, u64, u128) {
    let (mut owner, declared, keys) = owner(&[Predicate::new("edge", 2).unwrap()], 64);
    let (prefix, mut append) = owner.split();
    let read = prefix.read();
    let predicate = read.predicate(&declared[0]).unwrap();
    let values = read.assignment();
    let charged = Cell::new(0_u64);
    let count = || {
        charged.set(charged.get() + 1);
        Ok::<(), Infallible>(())
    };
    let facts = prepared.then(|| {
        append
            .prepare_facts_with(predicate, rows.len(), limits(), count)
            .unwrap()
    });
    let mut positions = Vec::new();
    for &(left, right) in rows {
        let terms = [left, right].map(|n| TemplateTerm::Constant(read.term(&keys[n]).unwrap()));
        let pattern = PatternRef::from_parts(predicate, &terms).unwrap();
        let position = match &facts {
            Some(facts) => append.insert_fact_with(
                facts,
                pattern,
                values.as_slice(),
                (TermLimits::default(), limits()),
                count,
            ),
            None => append.insert_pattern_with(
                pattern,
                values.as_slice(),
                TermLimits::default(),
                limits(),
                count,
            ),
        }
        .unwrap();
        positions.push(position);
    }
    if let Some(facts) = facts {
        append.finish_facts_with(facts, limits(), count).unwrap();
    }
    let atoms = (0..append.len())
        .map(|id| {
            append
                .get(id)
                .unwrap()
                .to_atom(ValueLimits::default())
                .unwrap()
        })
        .collect();
    let bytes = append.storage_bytes();
    (positions, atoms, charged.get(), bytes)
}

#[test]
fn a_prepared_block_discovers_what_rows_one_by_one_discover_for_less_work() {
    let rows = [
        (1, 0),
        (2, 0),
        (3, 1),
        (2, 0),
        (4, 3),
        (5, 3),
        (6, 5),
        (1, 0),
    ];
    let (single, single_atoms, single_work, single_bytes) = discover(&rows, false);
    let (block, block_atoms, block_work, block_bytes) = discover(&rows, true);
    assert_eq!(single_bytes, block_bytes, "the same capacities either way");
    for count in 1..64 {
        // Every third row repeats an earlier one.
        let rows = (0..count)
            .map(|n| {
                if n % 3 == 2 {
                    (n - 1, 64 - n)
                } else {
                    (n, 63 - n)
                }
            })
            .collect::<Vec<_>>();
        let (.., single) = discover(&rows, false);
        let (.., block) = discover(&rows, true);
        assert_eq!(single, block, "{count} rows");
    }
    assert_eq!(single, block);
    assert_eq!(single_atoms, block_atoms);
    assert_eq!(block[1], block[3], "a repeated row keeps its position");
    assert!(block_work < single_work, "{block_work} >= {single_work}");
}

#[test]
fn a_prepared_block_refuses_another_predicate() {
    let predicates = [
        Predicate::new("edge", 1).unwrap(),
        Predicate::new("node", 1).unwrap(),
    ];
    let (mut owner, declared, keys) = owner(&predicates, 1);
    let (prefix, mut append) = owner.split();
    let read = prefix.read();
    let values = read.assignment();
    let facts = append
        .prepare_facts_with(read.predicate(&declared[0]).unwrap(), 1, limits(), PERMIT)
        .unwrap();
    let terms = [TemplateTerm::Constant(read.term(&keys[0]).unwrap())];
    let pattern = PatternRef::from_parts(read.predicate(&declared[1]).unwrap(), &terms).unwrap();
    assert!(matches!(
        append.insert_fact_with(
            &facts,
            pattern,
            values.as_slice(),
            (TermLimits::default(), limits()),
            PERMIT,
        ),
        Err(AssignedFailure::Assignment(AssignmentError::Read(
            ReadError::ForeignCatalog
        )))
    ));
    assert_eq!(append.len(), 0);
}
