//! Select useful row requirements before retaining discovery IDs.

use super::*;
use crate::regions::{Closure, Known, Step, Subject, Width, Work, chain_position, witnesses};

const ROWS: usize = 8;

fn push(nodes: &mut FormulaNodes, node: NodeView<'_>) -> usize {
    let mut transaction = nodes.transaction();
    let root = transaction.push(node, usize::MAX, usize::MAX).unwrap();
    transaction.commit();
    root
}

/// Each native row extends the same three-leaf Boolean cone by a different
/// open atom. Requiring a native endpoint does not decide a cone leaf. The
/// extra consumer keeps the inner connective shared, so discovery expands.
fn fixture(positive: bool, implied: &[usize]) -> (Theory, usize, Vec<usize>) {
    let mut nodes = FormulaNodes::new(
        FormulaParts::new((0..ROWS + 4).map(Node::atom).collect(), Vec::new()).unwrap(),
    );
    let mut guards = Vec::new();
    for row in 0..ROWS {
        guards.push(
            nodes
                .push_threshold_family(
                    &[0, 1, 2, row + 3],
                    &[1, 1, 1, 1],
                    &[if positive { 1 } else { 4 }],
                    usize::MAX,
                    usize::MAX,
                )
                .unwrap()[0],
        );
    }
    let connective = |nodes: &mut FormulaNodes, children: &[usize]| {
        push(
            nodes,
            if positive {
                NodeView::Or(children)
            } else {
                NodeView::And(children)
            },
        )
    };
    let inner = connective(&mut nodes, &[0, 1]);
    let source = connective(&mut nodes, &[inner, 2]);
    push(&mut nodes, NodeView::Implies(inner, ROWS + 3));
    let falsum = push(&mut nodes, NodeView::False);
    let required = |nodes: &mut FormulaNodes, node| {
        if positive {
            node
        } else {
            push(nodes, NodeView::Implies(node, falsum))
        }
    };
    let mut roots = vec![required(&mut nodes, source)];
    for &row in implied {
        roots.push(required(&mut nodes, guards[row]));
    }
    (
        Theory::new(
            ROWS + 4,
            nodes.into_parts(),
            roots,
            AdmissionLimits::default(),
        )
        .unwrap(),
        source,
        guards,
    )
}

fn compact(knowledge: &mut Knowledge) -> &mut Known<u16> {
    let Width::Compact16(known) = &mut knowledge.width else {
        panic!("the small fixed-region fixture uses compact counters");
    };
    known
}

/// Delaying just this bridge closes the same fixed all-open region with its
/// explicitly required subset of row bounds. The test then invokes the actual
/// bridge kernel and drains its pending events, rather than inventing sums.
fn delayed(theory: &Theory, index: &Narrower) -> (Region, Knowledge) {
    let mut without = index.clone();
    without.thresholds.connectives = None;
    let mut region = Region::all_open(theory.atom_count());
    let mut known = without.knowledge();
    let result = close(
        theory,
        &without,
        &mut region,
        &mut known,
        &mut NarrowingScratch::default(),
    );
    assert_ne!(result.0, Narrowing::Refuted);
    assert!((0..theory.atom_count()).all(|atom| region.is_open(atom)));
    (region, known)
}

fn warmed(index: &Narrower, chain: usize) -> NarrowingScratch {
    assert!(witnesses::expands(index, chain));
    let mut scratch = NarrowingScratch::default();
    scratch
        .prepare_thresholds(index.thresholds.rows.len())
        .unwrap();
    scratch
        .witnesses
        .cone(index, chain, &mut Work::new(u64::MAX))
        .unwrap();
    scratch
}

fn assert_closed(actual: &Known<u16>, expected: &Known<u16>) {
    assert_eq!(actual.masks.words, expected.masks.words);
    assert_eq!(actual.thresholds, expected.thresholds);
    assert_eq!(actual.seeded, expected.seeded);
    for position in 0..actual.neutral_operands.len() {
        assert_eq!(
            actual.neutral_operands.get(position),
            expected.neutral_operands.get(position)
        );
    }
    for atom in 0..actual.unknown.len() {
        assert_eq!(actual.unknown.get(atom), expected.unknown.get(atom));
    }
}

#[test]
fn selected_rows_preserve_order_and_closed_knowledge() {
    for positive in [false, true] {
        for implied in [vec![], vec![0, 2, 4, 6], (0..ROWS).collect()] {
            let (theory, source, guards) = fixture(positive, &implied);
            let index = Narrower::new(&theory);
            let chain = chain_position(index.chain_of[source].unwrap());
            let (region, mut actual) = delayed(&theory, &index);
            let expected_rows: Vec<_> = guards
                .iter()
                .enumerate()
                .filter(|(position, _)| !implied.contains(position))
                .map(|(_, &guard)| index.thresholds.row(&theory, guard))
                .collect();
            let mut scratch = warmed(&index, chain);
            let bytes = scratch.witnesses.retained_bytes();
            let mut work = Work::new(u64::MAX);
            let mut closure = Closure {
                known: compact(&mut actual).borrow(),
                lists: &mut scratch,
            };
            let result = closure
                .threshold_connective(&index, chain, positive, &mut work)
                .unwrap();
            assert!(result != Step::Contradiction);
            assert_eq!(closure.lists.witnesses.candidates(), expected_rows);
            if expected_rows.is_empty() {
                // Warming prepared only the cone: an entirely implied bucket
                // must neither retain discovery IDs nor allocate their vector.
                assert_eq!(closure.lists.witnesses.retained_bytes(), bytes);
                assert!(result == Step::Unchanged);
            }
            let result = closure
                .close(
                    Subject::from(OriginalSubject::new(&theory, None)),
                    &index,
                    &region,
                    &mut work,
                    &mut NarrowingStatistics::default(),
                )
                .unwrap();
            assert!(result != Step::Contradiction);
            let mut expected = index.knowledge();
            let mut expected_region = region.clone();
            let result = close(
                &theory,
                &index,
                &mut expected_region,
                &mut expected,
                &mut NarrowingScratch::default(),
            );
            assert_ne!(result.0, Narrowing::Refuted);
            assert_eq!(expected_region, region);
            assert_closed(compact(&mut actual), compact(&mut expected));
        }
    }
}

#[test]
fn opposite_bounds_survive_selection_until_refuted() {
    for positive in [false, true] {
        let (theory, source, guards) = fixture(positive, &[0, 2]);
        let index = Narrower::new(&theory);
        let chain = chain_position(index.chain_of[source].unwrap());
        let (_, mut known) = delayed(&theory, &index);
        let rows: Vec<_> = guards
            .iter()
            .map(|&guard| index.thresholds.row(&theory, guard))
            .collect();
        // Isolate a pending opposite requirement before the row's numeric
        // check. This deliberately is not reusable closed Knowledge: the
        // bridge must authenticate the still-useful proposal and then refute.
        let state = &mut compact(&mut known).thresholds[rows[5]];
        if positive {
            state.upper = Some(1);
        } else {
            state.lower = 4;
        }
        let mut scratch = warmed(&index, chain);
        let mut closure = Closure {
            known: compact(&mut known).borrow(),
            lists: &mut scratch,
        };
        assert!(
            closure
                .threshold_connective(&index, chain, positive, &mut Work::new(u64::MAX))
                .unwrap()
                == Step::Contradiction
        );
        let expected: Vec<_> = rows
            .iter()
            .enumerate()
            .filter(|(position, _)| ![0, 2].contains(position))
            .map(|(_, &row)| row)
            .collect();
        assert_eq!(closure.lists.witnesses.candidates(), expected);
        for position in [1, 3, 4, 5] {
            let state = closure.known.thresholds[rows[position]];
            assert_eq!(
                if positive {
                    state.lower
                } else {
                    state.upper.unwrap()
                },
                if positive { 1 } else { 4 }
            );
        }
        // Processing stopped at the contradiction, not at discovery. The
        // surviving tail was not authenticated or installed.
        let tail = closure.known.thresholds[rows[6]];
        assert_eq!(tail.lower, 0);
        assert_eq!(tail.upper, None);
    }
}

#[test]
fn interrupted_selection_retries_without_partial_ids() {
    let (theory, source, _) = fixture(true, &(0..ROWS).collect::<Vec<_>>());
    let index = Narrower::new(&theory);
    let chain = chain_position(index.chain_of[source].unwrap());
    let (_, before) = delayed(&theory, &index);
    let mut scratch = warmed(&index, chain);
    let run = |knowledge: &mut Knowledge, scratch: &mut NarrowingScratch, work: &mut Work<'_>| {
        Closure {
            known: compact(knowledge).borrow(),
            lists: scratch,
        }
        .threshold_connective(&index, chain, true, work)
    };
    let mut complete = Work::new(u64::MAX);
    assert!(run(&mut before.clone(), &mut scratch, &mut complete).unwrap() == Step::Unchanged);
    for remaining in 0..complete.spent {
        for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
            let mut quota = Allowance { remaining, failure };
            let mut work = Work::reserved(&mut quota);
            let mut known = before.clone();
            assert!(
                matches!(run(&mut known, &mut scratch, &mut work), Err(stop) if stop == failure)
            );
            assert_eq!(work.spent, remaining);
            work.settle();
            assert_closed(compact(&mut known), compact(&mut before.clone()));
            let mut retry = Work::new(complete.spent);
            assert!(run(&mut known, &mut scratch, &mut retry).unwrap() == Step::Unchanged);
            assert_eq!(retry.spent, complete.spent);
            assert!(scratch.witnesses.candidates().is_empty());
        }
    }
}
