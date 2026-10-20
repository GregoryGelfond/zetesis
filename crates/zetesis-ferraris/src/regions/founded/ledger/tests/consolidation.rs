//! Retention consumes completed certificates; original-source eligibility is
//! established by capture, separately from this bounded transaction.

use super::*;
use crate::regions::founded::explanations::Explanation;
use crate::regions::founded::ledger::{MAX_OUTPUTS, MAX_ROWS, row::Row};

type Entry = (Vec<(usize, bool)>, Vec<usize>);
type Contents = Vec<Entry>;

fn explanation(conditions: &[(usize, bool)], outputs: &[usize]) -> Explanation {
    Explanation {
        conditions: conditions.to_vec(),
        unsupported: outputs.to_vec(),
    }
}

fn contents(ledger: &Ledger) -> Contents {
    ledger
        .rows
        .iter()
        .map(|row| (row.conditions().collect(), row.outputs().collect()))
        .collect()
}

fn retain(ledger: &mut Ledger, value: Explanation) {
    assert!(
        ledger
            .retain(value, &mut Work::new(u64::MAX), &mut 0)
            .unwrap()
    );
}

#[test]
fn identical_conditions_share_outputs_and_partial_forcing() {
    let mut ledger = Ledger::default();
    retain(&mut ledger, explanation(&[(0, true), (1, false)], &[63]));
    retain(
        &mut ledger,
        explanation(&[(0, true), (1, false)], &[64, 127]),
    );
    assert_eq!(ledger.len(), 1);
    assert_eq!(ledger.outputs(0).collect::<Vec<_>>(), [63, 64, 127]);
    let held = [1, 1 << 63];
    assert_eq!(
        ledger.deduction(0, &held, &[0, 0], &mut Work::new(u64::MAX)),
        Ok(Deduction::Require {
            atom: 1,
            value: true
        })
    );
}

#[test]
fn stronger_replacement_keeps_unrelated_row_order() {
    let mut ledger = Ledger::default();
    for (conditions, outputs) in [
        (vec![(0, true), (1, false)], vec![4]),
        (vec![(2, false)], vec![5]),
        (vec![(0, true), (3, true)], vec![6]),
        (vec![(2, true)], vec![7]),
    ] {
        retain(&mut ledger, explanation(&conditions, &outputs));
    }
    retain(&mut ledger, explanation(&[(0, true)], &[4, 6]));
    assert_eq!(
        contents(&ledger),
        vec![
            (vec![(0, true)], vec![4, 6]),
            (vec![(2, false)], vec![5]),
            (vec![(2, true)], vec![7]),
        ]
    );
}

#[test]
fn merely_overlapping_conditions_do_not_collapse() {
    let mut ledger = Ledger::default();
    retain(&mut ledger, explanation(&[(0, true), (1, true)], &[4]));
    retain(&mut ledger, explanation(&[(0, true), (2, true)], &[4]));
    retain(&mut ledger, explanation(&[(0, false)], &[4]));
    retain(&mut ledger, explanation(&[(0, true)], &[5]));
    assert_eq!(ledger.len(), 4);
    assert_eq!(
        ledger.deduction(0, &[1], &[0], &mut Work::new(u64::MAX)),
        Ok(Deduction::None)
    );
}

#[test]
fn output_union_bound_keeps_separate_complete_rows() {
    let mut ledger = Ledger::default();
    retain(
        &mut ledger,
        explanation(&[(100, true)], &(0..MAX_OUTPUTS).collect::<Vec<_>>()),
    );
    retain(&mut ledger, explanation(&[(100, true)], &[MAX_OUTPUTS]));
    assert_eq!(ledger.len(), 2);
    assert_eq!(ledger.output_count(0), MAX_OUTPUTS);
    assert_eq!(ledger.outputs(1).collect::<Vec<_>>(), [MAX_OUTPUTS]);
}

#[test]
fn union_retirement_rechecks_rows_before_the_merged_row() {
    let mut ledger = Ledger::default();
    retain(&mut ledger, explanation(&[(0, true), (1, true)], &[4, 5]));
    retain(&mut ledger, explanation(&[(0, true)], &[4]));
    retain(&mut ledger, explanation(&[(0, true)], &[5]));
    assert_eq!(contents(&ledger), vec![(vec![(0, true)], vec![4, 5])]);
}

fn initial() -> Ledger {
    let mut ledger = Ledger::default();
    ledger.prepare(&Program::new(false).owner);
    retain(&mut ledger, explanation(&[(0, true), (1, true)], &[3, 4]));
    retain(&mut ledger, explanation(&[(0, true)], &[3]));
    ledger.remaining = u64::MAX;
    ledger
}

#[test]
fn every_interrupted_replacement_preserves_retained_certificates() {
    let mut completed = initial();
    let mut measured = Work::new(u64::MAX);
    assert!(
        completed
            .retain(explanation(&[(0, true)], &[4]), &mut measured, &mut 0)
            .unwrap()
    );
    assert_eq!(completed.len(), 1);
    for stop in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for prefix in 0..measured.spent {
            let mut ledger = initial();
            let owner = ledger.owner.clone().unwrap();
            let before = contents(&ledger);
            let mut permits = SinglePermits::new(prefix, stop);
            let mut work = Work::reserved(&mut permits);
            assert_eq!(
                ledger.retain(explanation(&[(0, true)], &[4]), &mut work, &mut 0),
                Err(stop)
            );
            assert_eq!(work.spent, prefix);
            assert_eq!(contents(&ledger), before);
            assert!(ledger.owner.as_ref().unwrap().same_instance(&owner));
            retain(&mut ledger, explanation(&[(0, true)], &[4]));
            assert_eq!(contents(&ledger), contents(&completed));
        }
    }
    for prefix in 0..measured.spent {
        let mut ledger = initial();
        let before = contents(&ledger);
        ledger.remaining = prefix;
        let mut work = Work::new(u64::MAX);
        assert!(
            !ledger
                .retain(explanation(&[(0, true)], &[4]), &mut work, &mut 0)
                .unwrap()
        );
        assert_eq!(contents(&ledger), before);
        assert_eq!(work.spent, prefix);
        assert_eq!(ledger.remaining, 0);
    }
}

#[test]
fn full_ledger_keeps_the_existing_capture_boundary() {
    let mut ledger = Ledger::default();
    for atom in 0..MAX_ROWS {
        retain(&mut ledger, explanation(&[(atom, true)], &[MAX_ROWS]));
    }
    assert_eq!(ledger.len(), MAX_ROWS);
    assert!(!ledger.can_capture());
}

#[test]
fn appending_counts_the_staged_row_header() {
    let mut ledger = Ledger::default();
    retain(&mut ledger, explanation(&[], &[0]));
    // After reservation, both the new vector slot and the staged row exist.
    // Publication transfers the buffers, so final retained storage omits only
    // that temporary header from this point's live storage.
    assert!(ledger.peak_bytes() >= ledger.retained_bytes() + size_of::<Row>() as u128);
}

#[test]
fn independent_cycles_share_a_captured_condition() {
    // a<->b, c<->d, a<-x, c<-x, and the independent choice x.
    let owner = theory(
        5,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::atom(3),
            Node::atom(4),
            Node::falsum(),
            Node::implies(4, 5),
            Node::or_pair([4, 6]),
            Node::implies(1, 0),
            Node::implies(0, 1),
            Node::implies(3, 2),
            Node::implies(2, 3),
            Node::implies(4, 0),
            Node::implies(4, 2),
        ],
        (7..14).collect(),
    );
    let program = Program {
        index: Narrower::new(&owner),
        producers: crate::producers(&owner, RegionLimits::default(), &Cancellation::default())
            .unwrap()
            .producers
            .unwrap(),
        owner,
    };
    let mut scratch = NarrowingScratch::default();
    for atom in [0, 2] {
        let (result, counts) = program.close(
            &mut region(&[(atom, true), (4, false)]),
            &mut program.index.knowledge(),
            &mut scratch,
        );
        assert_eq!(result, Narrowing::Refuted);
        assert_eq!(counts.support_explanations.certified, 1);
    }
    assert_eq!(scratch.explanations.len(), 1);
    assert_eq!(
        scratch.explanations.outputs(0).collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    let (result, counts) = program.close(
        &mut region(&[(3, true)]),
        &mut program.index.knowledge(),
        &mut scratch,
    );
    assert!(matches!(result, Narrowing::Fixed { .. }));
    assert!(counts.support_explanations.forced > 0);
}

#[test]
fn consolidation_preserves_every_complete_interpretation() {
    let mut ledger = Ledger::default();
    let statements = [
        (vec![(0, true), (1, false)], vec![2]),
        (vec![(0, true)], vec![3]),
        (vec![(0, true)], vec![2]),
        (vec![(1, true)], vec![1]),
    ];
    for (conditions, outputs) in &statements {
        retain(&mut ledger, explanation(conditions, outputs));
    }
    let after = contents(&ledger);
    let satisfies = |rows: &[Entry], mask: usize| {
        rows.iter().all(|(conditions, outputs)| {
            !conditions
                .iter()
                .all(|&(atom, value)| (mask & (1 << atom) != 0) == value)
                || outputs.iter().all(|&atom| mask & (1 << atom) == 0)
        })
    };
    for mask in 0..16 {
        assert_eq!(satisfies(&statements, mask), satisfies(&after, mask));
    }
    assert!(ledger.rows.iter().any(|row| matches!(row, Row::Scalar(_))));
}
