//! Frozen candidate traces from the unpacked watch implementation at 78a069b.
//! These fixtures compare ordered semantic candidates, every cumulative search
//! counter, and terminal outcomes under the explicit reference cost map below.
//! They do not assert stable-model membership. The admitted inputs preserve
//! their DAGs so equivalent frontend layouts cannot change these traces.

use std::fmt::Write as _;

#[path = "../../../tests/fixtures/watch-traces/choices.rs"]
mod choices;

#[path = "../../../tests/fixtures/watch-traces/queens-01.rs"]
mod queens01;

#[path = "../../../tests/fixtures/watch-traces/queens-02.rs"]
mod queens02;

#[path = "../../../tests/fixtures/watch-traces/queens-03.rs"]
mod queens03;

#[path = "../../../tests/fixtures/watch-traces/queens-04.rs"]
mod queens04;

#[path = "../../../tests/fixtures/watch-traces/queens-05.rs"]
mod queens05;

#[path = "../../../tests/fixtures/watch-traces/queens-06.rs"]
mod queens06;

use zetesis_ferraris::{Node, Theory};

use super::{Budget, Cursor, LocalQuota};
use crate::{AdmissionLimits, Cancellation, SearchLimits, SearchStatistics, Solve, encoding};

const QUEENS: [(usize, &[Node], &[usize]); 6] = [
    (queens01::ATOMS, &queens01::NODES, &queens01::ROOTS),
    (queens02::ATOMS, &queens02::NODES, &queens02::ROOTS),
    (queens03::ATOMS, &queens03::NODES, &queens03::ROOTS),
    (queens04::ATOMS, &queens04::NODES, &queens04::ROOTS),
    (queens05::ATOMS, &queens05::NODES, &queens05::ROOTS),
    (queens06::ATOMS, &queens06::NODES, &queens06::ROOTS),
];

fn admitted(atoms: usize, nodes: &[Node], roots: &[usize]) -> Theory {
    Theory::new(
        atoms,
        zetesis_ferraris::FormulaParts::new(nodes.to_vec(), vec![]).unwrap(),
        roots.to_vec(),
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap()
}

fn choice_theory() -> Theory {
    admitted(choices::ATOMS, &choices::NODES, &choices::ROOTS)
}

fn choice_work() -> u64 {
    // The frozen generic trace takes 2294 operations. The current encoder
    // additionally charges each operand once. Restore that setup cost before
    // subtracting elided watch positions, witness rescans and exact exclusion
    // savings for the ten distinct four-atom projections.
    let theory = choice_theory();
    let encoding_operands = u64::try_from(theory.parts().occurrences()).unwrap();
    assert_eq!(encoding_operands, 80);
    let traced = trace(&theory, true, SearchLimits::default());
    assert_eq!(
        traced.record,
        include_str!("../../../tests/fixtures/watch-traces/refined.txt")
    );
    2294 + encoding_operands
        - reference_statistics(SearchStatistics::default(), 4, 10, traced.rescanned, 0).work
}

/// The record of one traversal and the witness-rescan charges the historical
/// implementation paid for it, restored for comparison.
struct Trace {
    record: String,
    rescanned: u64,
}

// Historical witnesses were checked against every base clause, one charge
// per literal up to and including the first true one; the watch scheme's
// invariant makes that scan redundant and the current implementation only
// asserts it in debug builds.
fn rescan_charges(cnf: &crate::Cnf, assignment: &crate::Assignment) -> u64 {
    cnf.clauses()
        .map(|clause| {
            clause
                .iter()
                .position(|literal| {
                    assignment.value(literal.variable()) == Some(literal.positive())
                })
                .map_or(clause.len(), |first| first + 1) as u64
        })
        .sum()
}

// Historical fixtures scan both watched positions in every binary replacement
// attempt, and a prefix of the three positions in every ternary attempt.
// Restore omitted positions under that cost map; solver statistics stay intact.
fn reference_statistics(
    mut actual: SearchStatistics,
    width: usize,
    excluded: u64,
    rescanned: u64,
    encoding_operands: u64,
) -> SearchStatistics {
    let profile = super::propagation_profile::snapshot();
    let omitted = profile
        .binary_attempts
        .checked_mul(2)
        .and_then(|binary| binary.checked_add(profile.ternary_elided_positions))
        .and_then(|omitted| omitted.checked_add(rescanned))
        .unwrap();
    actual.work = actual.work.checked_add(omitted).unwrap();
    // The old block path copied W literals, then checked a clause and W
    // indices and traversed W bits: 3W+1. Direct transactional insertion costs
    // W+3 for every new nonempty key (prefix visits + suffix writes partition
    // its W bits, plus admission, entry and leaf). Empty keys cost 2 vs 1.
    // Dedicated exclusion tests assert these counts independently. This map
    // changes only historical trace comparison, never solver statistics.
    actual.work = if width == 0 {
        actual.work.checked_sub(excluded).unwrap()
    } else {
        actual
            .work
            .checked_add((u64::try_from(width).unwrap() * 2 - 2) * excluded)
            .unwrap()
    };
    // The original-only encoder now reads every operand once, including
    // implication operands. Historical fixtures charged the node/gate visits
    // but not these E reads. Every recorded observation follows completed
    // encoding, so remove that one setup charge solely in this reference map.
    actual.work = actual.work.checked_sub(encoding_operands).unwrap();
    actual
}

fn trace(theory: &Theory, refined: bool, limits: SearchLimits) -> Trace {
    super::propagation_profile::reset();
    let cancellation = Cancellation::default();
    let mut charged = Budget {
        quota: LocalQuota,
        limits,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let cnf = encoding::encode(theory, None, AdmissionLimits::default(), &mut charged).unwrap();
    let encoding_operands = u64::try_from(theory.parts().occurrences()).unwrap();
    let mut cursor =
        Cursor::projected(theory.atom_count(), crate::ProjectionLimits::default()).unwrap();
    if refined {
        cursor.restart();
    }
    let mut record = String::new();
    let mut excluded = 0;
    let mut rescanned = 0;
    writeln!(
        record,
        "atoms={} nodes={} roots={} variables={} clauses={}",
        theory.atom_count(),
        theory.nodes().len(),
        theory.roots().len(),
        cnf.variables(),
        cnf.clauses().len()
    )
    .unwrap();
    // At most 93 observations for each authored queens fixture, or eleven for
    // the four-atom cardinality fixture. Exceeding this is an observable failure.
    for _ in 0..=92 {
        match cursor.query(&cnf, &mut charged) {
            Solve::Sat(assignment) => {
                rescanned += rescan_charges(&cnf, &assignment);
                let candidate =
                    encoding::interpretation(theory, &assignment, &mut charged).unwrap();
                writeln!(
                    record,
                    "model {:?}; {:?}",
                    candidate.atoms().collect::<Vec<_>>(),
                    reference_statistics(
                        charged.statistics,
                        theory.atom_count(),
                        excluded,
                        rescanned,
                        encoding_operands
                    )
                )
                .unwrap();
                if let Err(error) = cursor.exclude(&cnf, &candidate, &mut charged) {
                    writeln!(
                        record,
                        "block {error:?}; {:?}",
                        reference_statistics(
                            charged.statistics,
                            theory.atom_count(),
                            excluded,
                            rescanned,
                            encoding_operands
                        )
                    )
                    .unwrap();
                    return Trace { record, rescanned };
                }
                excluded += 1;
            }
            terminal => {
                writeln!(
                    record,
                    "{terminal:?}; {:?}",
                    reference_statistics(
                        charged.statistics,
                        theory.atom_count(),
                        excluded,
                        rescanned,
                        encoding_operands
                    )
                )
                .unwrap();
                return Trace { record, rescanned };
            }
        }
    }
    panic!("bounded fixture must terminate within 93 observations");
}

#[test]
fn replacement_elision_preserves_queens_reference_traces() {
    let expected = [
        include_str!("../../../tests/fixtures/watch-traces/queens-01.txt"),
        include_str!("../../../tests/fixtures/watch-traces/queens-02.txt"),
        include_str!("../../../tests/fixtures/watch-traces/queens-03.txt"),
        include_str!("../../../tests/fixtures/watch-traces/queens-04.txt"),
        include_str!("../../../tests/fixtures/watch-traces/queens-05.txt"),
        include_str!("../../../tests/fixtures/watch-traces/queens-06.txt"),
    ];
    for ((atoms, nodes, roots), expected) in QUEENS.into_iter().zip(expected) {
        let theory = admitted(atoms, nodes, roots);
        assert_eq!(
            trace(&theory, false, SearchLimits::default()).record,
            expected
        );
    }
}

#[test]
fn replacement_elision_preserves_the_refined_reference_trace() {
    assert_eq!(
        trace(&choice_theory(), true, SearchLimits::default()).record,
        include_str!("../../../tests/fixtures/watch-traces/refined.txt")
    );
}

#[test]
fn reduced_work_ceiling_permits_the_complete_trace() {
    let limits = SearchLimits {
        max_work: choice_work(),
        max_decisions: 9,
    };
    assert_eq!(
        trace(&choice_theory(), true, limits).record,
        include_str!("../../../tests/fixtures/watch-traces/refined.txt")
    );
}

#[test]
fn reduced_work_ceiling_stops_one_tick_short() {
    let limits = SearchLimits {
        max_work: choice_work() - 1,
        max_decisions: 9,
    };
    assert_eq!(
        trace(&choice_theory(), true, limits).record,
        include_str!("../../../tests/fixtures/watch-traces/work-short.txt")
    );
}

#[test]
fn replacement_elision_preserves_the_decision_stop() {
    let limits = SearchLimits {
        max_work: choice_work(),
        max_decisions: 8,
    };
    assert_eq!(
        trace(&choice_theory(), true, limits).record,
        include_str!("../../../tests/fixtures/watch-traces/decision-short.txt")
    );
}
