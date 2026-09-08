//! Frozen candidate traces from the unpacked watch implementation at 78a069b.
//! These fixtures compare ordered semantic candidates, every cumulative search
//! counter, and terminal outcomes under the explicit reference cost map below.
//! They do not assert stable-model membership.

use std::fmt::Write as _;

use super::{Budget, Cursor, LocalQuota};
use crate::{AdmissionLimits, Control, SearchLimits, SearchStatistics, Solve, encoding};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

const QUEENS: [&str; 6] = [
    include_str!("../../../../examples/kr-domains/standalone/n-queens/variant-01.lp"),
    include_str!("../../../../examples/kr-domains/standalone/n-queens/variant-02.lp"),
    include_str!("../../../../examples/kr-domains/standalone/n-queens/variant-03.lp"),
    include_str!("../../../../examples/kr-domains/standalone/n-queens/variant-04.lp"),
    include_str!("../../../../examples/kr-domains/standalone/n-queens/variant-05.lp"),
    include_str!("../../../../examples/kr-domains/standalone/n-queens/variant-06.lp"),
];

const CHOICES: &str = "1 { p(1..4) } 2.";

#[test]
#[ignore = "bounded storage report; no clock, RSS or solver-performance measurement"]
fn report_queens_watch_storage() {
    for (index, source) in QUEENS.into_iter().enumerate() {
        let admitted = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        let control = Control::default();
        let mut charged = Budget {
            quota: LocalQuota,
            limits: SearchLimits::default(),
            control: &control,
            statistics: SearchStatistics::default(),
        };
        let cnf = encoding::encode(
            admitted.theory(),
            None,
            AdmissionLimits::default(),
            &mut charged,
        )
        .unwrap();
        let state = super::State::new(&cnf, &mut charged).unwrap();
        let requested = std::mem::size_of_val(state.heads.as_slice())
            + std::mem::size_of_val(state.next.as_slice());
        let cell_bytes = std::mem::size_of_val(&state.heads[0]);
        let capacity_bytes = (state.heads.capacity() + state.next.capacity()) * cell_bytes;
        assert_eq!(state.heads.len(), 2 * cnf.variables());
        assert_eq!(state.next.len(), 2 * cnf.clauses().len());
        println!(
            "WATCH_STORAGE variant={:02} variables={} clauses={} cell_bytes={} requested_bytes={} capacity_bytes={} search_scratch_bytes={}",
            index + 1,
            cnf.variables(),
            cnf.clauses().len(),
            cell_bytes,
            requested,
            capacity_bytes,
            super::scratch_bytes(cnf.variables() as u128, cnf.clauses().len() as u128)
        );
    }
}

// Historical fixtures charge both watched positions in every binary
// replacement attempt. The new path inspects neither. Restore exactly these
// omitted charges for comparison; actual solver statistics remain untouched.
fn reference_statistics(mut actual: SearchStatistics) -> SearchStatistics {
    let omitted = super::propagation_profile::snapshot()
        .binary_attempts
        .checked_mul(2)
        .unwrap();
    actual.work = actual.work.checked_add(omitted).unwrap();
    actual
}

fn trace(source: &str, refined: bool, limits: SearchLimits) -> String {
    super::propagation_profile::reset();
    let admitted = admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let theory = admitted.theory();
    let control = Control::default();
    let mut charged = Budget {
        quota: LocalQuota,
        limits,
        control: &control,
        statistics: SearchStatistics::default(),
    };
    let mut cnf = encoding::encode(theory, None, AdmissionLimits::default(), &mut charged).unwrap();
    let mut cursor = if refined {
        Cursor::refined(theory.atom_count())
    } else {
        Cursor::projected(theory.atom_count())
    };
    let mut record = String::new();
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
                let candidate =
                    encoding::interpretation(theory, &assignment, &mut charged).unwrap();
                writeln!(
                    record,
                    "model {:?}; {:?}",
                    candidate.atoms().collect::<Vec<_>>(),
                    reference_statistics(charged.statistics)
                )
                .unwrap();
                if let Err(error) = encoding::block(&mut cnf, &candidate, &mut charged) {
                    writeln!(
                        record,
                        "block {error:?}; {:?}",
                        reference_statistics(charged.statistics)
                    )
                    .unwrap();
                    return record;
                }
            }
            terminal => {
                writeln!(
                    record,
                    "{terminal:?}; {:?}",
                    reference_statistics(charged.statistics)
                )
                .unwrap();
                return record;
            }
        }
    }
    panic!("bounded fixture must terminate within 93 observations");
}

#[test]
#[ignore = "bounded refined-cursor work profile; no clock or stable-model claim"]
fn profile_refined_choice_trace() {
    let record = trace(CHOICES, true, SearchLimits::default());
    println!(
        "REFERENCE_COST_TRACE (omitted binary positions restored)\n{record}PROFILE {:?}",
        super::propagation_profile::snapshot()
    );
}

#[test]
fn binary_elision_preserves_queens_reference_traces() {
    let expected = [
        include_str!("../fixtures/watch-traces/queens-01.txt"),
        include_str!("../fixtures/watch-traces/queens-02.txt"),
        include_str!("../fixtures/watch-traces/queens-03.txt"),
        include_str!("../fixtures/watch-traces/queens-04.txt"),
        include_str!("../fixtures/watch-traces/queens-05.txt"),
        include_str!("../fixtures/watch-traces/queens-06.txt"),
    ];
    for (source, expected) in QUEENS.into_iter().zip(expected) {
        assert_eq!(trace(source, false, SearchLimits::default()), expected);
    }
}

#[test]
fn binary_elision_preserves_the_refined_reference_trace() {
    assert_eq!(
        trace(CHOICES, true, SearchLimits::default()),
        include_str!("../fixtures/watch-traces/refined.txt")
    );
}

#[test]
fn reduced_work_ceiling_permits_the_complete_trace() {
    // Frozen baseline: 2294 charges, including two no-op positions in each
    // of 36 binary replacement attempts. The exact new ceiling is 2222.
    let limits = SearchLimits {
        max_work: 2222,
        max_decisions: 9,
    };
    assert_eq!(
        trace(CHOICES, true, limits),
        include_str!("../fixtures/watch-traces/exact.txt")
    );
}

#[test]
fn reduced_work_ceiling_stops_one_tick_short() {
    let limits = SearchLimits {
        max_work: 2221,
        max_decisions: 9,
    };
    assert_eq!(
        trace(CHOICES, true, limits),
        include_str!("../fixtures/watch-traces/work-short.txt")
    );
}

#[test]
fn binary_elision_preserves_the_decision_stop() {
    let limits = SearchLimits {
        max_work: 2222,
        max_decisions: 8,
    };
    assert_eq!(
        trace(CHOICES, true, limits),
        include_str!("../fixtures/watch-traces/decision-short.txt")
    );
}
