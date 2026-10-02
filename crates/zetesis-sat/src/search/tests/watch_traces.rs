//! Frozen candidate traces from the unpacked watch implementation at 78a069b.
//! These fixtures compare ordered semantic candidates, every cumulative search
//! counter, and terminal outcomes under the explicit reference cost map below.
//! They do not assert stable-model membership. The four-atom input preserves
//! its admitted DAG so equivalent frontend layouts cannot change this trace.

use std::fmt::Write as _;

#[path = "../../../tests/fixtures/watch-traces/choices.rs"]
mod choices;

use zetesis_ferraris::Theory;

use super::{Budget, Cursor, LocalQuota};
use crate::{AdmissionLimits, Cancellation, SearchLimits, SearchStatistics, Solve, encoding};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

const QUEENS: [&str; 6] = [
    include_str!("../../../../../examples/correctness/standalone/n-queens/variant-01.lp"),
    include_str!("../../../../../examples/correctness/standalone/n-queens/variant-02.lp"),
    include_str!("../../../../../examples/correctness/standalone/n-queens/variant-03.lp"),
    include_str!("../../../../../examples/correctness/standalone/n-queens/variant-04.lp"),
    include_str!("../../../../../examples/correctness/standalone/n-queens/variant-05.lp"),
    include_str!("../../../../../examples/correctness/standalone/n-queens/variant-06.lp"),
];

fn choice_theory() -> Theory {
    Theory::new(
        choices::ATOMS,
        choices::NODES.to_vec(),
        choices::ROOTS.to_vec(),
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap()
}

fn choice_work() -> u64 {
    // The frozen generic trace takes 2294 operations. Compare that entire
    // trace before subtracting elided watch positions, the witness rescans
    // and the exact exclusion cost reduction for its ten distinct four-atom
    // projections.
    let traced = trace(&choice_theory(), true, SearchLimits::default());
    assert_eq!(
        traced.record,
        include_str!("../../../tests/fixtures/watch-traces/refined.txt")
    );
    2294 - reference_statistics(SearchStatistics::default(), 4, 10, traced.rescanned).work
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
    actual
}

fn source_trace(source: &str, refined: bool, limits: SearchLimits) -> Trace {
    let admitted = admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    trace(admitted.theory(), refined, limits)
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
                        rescanned
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
                            rescanned
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
                        rescanned
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
    for (source, expected) in QUEENS.into_iter().zip(expected) {
        assert_eq!(
            source_trace(source, false, SearchLimits::default()).record,
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
