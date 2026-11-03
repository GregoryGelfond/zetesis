//! Above `SCAN_ROWS` rows a matching step reads the occurrence index, which
//! covers conditions and outputs; a narrowing's first step and the step after
//! a change test every row.

use super::*;
use crate::regions::ledger::SCAN_ROWS;

/// Rows `i` held -> `1000 + i` absent, one more than the scan threshold.
fn large() -> Ledger {
    let mut ledger = owned(
        &Program::new(false).theory,
        LearningLimits {
            tier_rows: 256,
            ..LearningLimits::default()
        },
    );
    for atom in 0..=SCAN_ROWS {
        assert!(add(&mut ledger, &[(atom, true)], &[(1000 + atom, false)]));
    }
    ledger
}

fn candidates(ledger: &mut Ledger, decided: &[usize]) -> Vec<usize> {
    let mut rows = Vec::new();
    ledger
        .candidates(decided, &mut rows, &mut Meter::local(u64::MAX))
        .unwrap();
    rows
}

#[test]
fn a_narrowings_first_step_tests_every_row() {
    let mut ledger = large();
    candidates(&mut ledger, &[]);
    ledger.begin_narrowing();
    assert_eq!(
        candidates(&mut ledger, &[]),
        (0..=SCAN_ROWS).collect::<Vec<_>>()
    );
    assert_eq!(ledger.indexed_steps, 0);
}

#[test]
fn a_later_step_tests_the_rows_of_a_decided_condition() {
    let mut ledger = large();
    candidates(&mut ledger, &[]);
    assert_eq!(candidates(&mut ledger, &[3]), [3]);
    assert_eq!(ledger.indexed_steps, 1);
}

#[test]
fn the_index_covers_outputs() {
    let mut ledger = large();
    candidates(&mut ledger, &[]);
    assert_eq!(candidates(&mut ledger, &[1005]), [5]);
}

#[test]
fn a_retention_returns_the_next_step_to_every_row() {
    let mut ledger = large();
    candidates(&mut ledger, &[]);
    assert!(add(&mut ledger, &[(500, true)], &[(2000, false)]));
    assert_eq!(candidates(&mut ledger, &[]).len(), SCAN_ROWS + 2);
    assert_eq!(candidates(&mut ledger, &[2000]), [SCAN_ROWS + 1]);
}

#[test]
fn a_small_tier_scans_without_recording_decisions() {
    let mut ledger = large();
    ledger.rows.pop();
    assert!(!ledger.records_decisions());
    candidates(&mut ledger, &[]);
    assert_eq!(candidates(&mut ledger, &[]).len(), SCAN_ROWS);
    assert_eq!(ledger.indexed_steps, 0);
}

#[test]
fn a_stopped_rebuild_is_never_read() {
    let mut ledger = large();
    candidates(&mut ledger, &[]);
    let mut rows = Vec::new();
    let mut permits = SinglePermits::new(3, Stop::WorkLimit);
    let mut work = Meter::reserved(&mut permits);
    assert_eq!(
        ledger
            .candidates(&[7], &mut rows, &mut work)
            .map_err(zetesis_algebra::Failure::into_stop),
        Err(Stop::WorkLimit)
    );
    assert_eq!(candidates(&mut ledger, &[7]), [7]);
}

/// `Program::new`'s theory with `extra` further independent choices after its
/// five atoms.
fn padded(extra: usize) -> Program {
    let atoms = 5 + extra;
    let mut nodes: Vec<_> = (0..atoms).map(Node::atom).collect();
    let falsum = nodes.len();
    nodes.push(Node::falsum());
    let mut roots = Vec::new();
    let mut negations = vec![0; atoms];
    for (atom, negation) in negations.iter_mut().enumerate().skip(2) {
        *negation = nodes.len();
        nodes.push(Node::implies(atom, falsum));
        roots.push(nodes.len());
        nodes.push(Node::or_pair([atom, *negation]));
    }
    for (body, head) in [(1, 0), (0, 1), (2, 0), (negations[3], 1)] {
        roots.push(nodes.len());
        nodes.push(Node::implies(body, head));
    }
    let mut program = Program::of(theory(atoms, nodes, roots));
    program.owner = LearningOwner::theory(
        &program.theory,
        LearningLimits {
            tier_rows: 256,
            ..LearningLimits::default()
        },
    );
    program
}

/// The padded program's tier: the two rows of `signed.rs`'s fixture and,
/// with `padding`, a tautology for each extra atom.
fn padded_tier(program: &Program, padding: bool) -> NarrowingScratch {
    let mut scratch = NarrowingScratch::default();
    scratch.learning.explanations.prepare(&program.owner);
    scratch.learning.explanations.remaining = u64::MAX;
    let ledger = &mut scratch.learning.explanations;
    assert!(add(
        ledger,
        &[(2, false), (3, true)],
        &[(0, false), (1, false)]
    ));
    assert!(add(ledger, &[(0, true), (3, true)], &[(2, true)]));
    if padding {
        for atom in 5..program.theory.atom_count() {
            assert!(add(ledger, &[(atom, true)], &[(atom, true)]));
        }
    }
    scratch
}

fn close_carrier(
    program: &Program,
    region: &mut Region,
    knowledge: &mut Knowledge,
    scratch: &mut NarrowingScratch,
) -> (Narrowing, NarrowingStatistics) {
    let attempt = program.index.narrow_known(
        OriginalSubject::carrier(&program.owner, None),
        region,
        knowledge,
        scratch,
        RegionLimits::default(),
        &Cancellation::default(),
    );
    (attempt.result.unwrap(), attempt.statistics)
}

fn region_of(program: &Program, decisions: &[(usize, bool)]) -> Region {
    let mut region = Region::all_open(program.theory.atom_count());
    for &(atom, value) in decisions {
        assert!(if value {
            region.hold(atom)
        } else {
            region.cut(atom)
        });
    }
    region
}

#[test]
fn a_narrowing_from_inherited_knowledge_tests_every_row() {
    let program = padded(SCAN_ROWS);
    let mut tier = padded_tier(&program, true);
    // A first narrowing leaves the tier past its full scan.
    close_carrier(
        &program,
        &mut region_of(&program, &[(5, true)]),
        &mut program.index.knowledge(),
        &mut tier,
    );
    // Knowledge closed elsewhere, as a stolen region's, decides x and y; the
    // child's one new decision mentions neither.
    let mut parent = region_of(&program, &[(2, false), (3, true)]);
    let mut knowledge = program.index.knowledge();
    close_carrier(
        &program,
        &mut parent,
        &mut knowledge,
        &mut NarrowingScratch::default(),
    );
    assert_eq!(parent.decision(0), None);
    let mut child = parent;
    assert!(child.hold(6));
    let (result, counts) = close_carrier(&program, &mut child, &mut knowledge, &mut tier);
    assert!(matches!(result, Narrowing::Fixed { .. }));
    assert!(child.is_cut(0) && child.is_cut(1));
    assert_eq!(counts.support_explanations.cut, 2);
}

#[test]
fn indexed_matching_agrees_with_scanning_in_every_partial_region() {
    let program = padded(SCAN_ROWS);
    // Both rows hold for every answer set (`signed.rs`); the padding rows are
    // tautologies over atoms every region leaves open.
    let close = |region: &mut Region, scratch: &mut NarrowingScratch| {
        close_carrier(&program, region, &mut program.index.knowledge(), scratch)
    };
    let mut scanned = padded_tier(&program, false);
    let mut indexed = padded_tier(&program, true);
    assert!(indexed.learning.explanations.records_decisions());
    for mut code in 0..243 {
        let mut initial = Region::all_open(program.theory.atom_count());
        for atom in 0..5 {
            match code % 3 {
                1 => assert!(initial.cut(atom)),
                2 => assert!(initial.hold(atom)),
                _ => {}
            }
            code /= 3;
        }
        let mut expected = initial.clone();
        let mut actual = initial;
        let (wanted, wanted_counts) = close(&mut expected, &mut scanned);
        let (result, counts) = close(&mut actual, &mut indexed);
        assert_eq!(result, wanted);
        if result != Narrowing::Refuted {
            for atom in 0..5 {
                assert_eq!(actual.decision(atom), expected.decision(atom));
            }
        }
        let (wanted, counts) = (
            wanted_counts.support_explanations,
            counts.support_explanations,
        );
        assert_eq!(
            (counts.matched, counts.forced, counts.cut, counts.refuted),
            (wanted.matched, wanted.forced, wanted.cut, wanted.refuted)
        );
    }
    assert!(indexed.learning.explanations.indexed_steps > 0);
}
