//! Frozen candidate traces from the unpacked watch implementation at 78a069b.
//! These fixtures compare ordered semantic candidates, every cumulative search
//! counter, and terminal outcomes. They do not assert stable-model membership.

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

fn trace(source: &str, refined: bool, limits: SearchLimits) -> String {
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
                    charged.statistics
                )
                .unwrap();
                if let Err(error) = encoding::block(&mut cnf, &candidate, &mut charged) {
                    writeln!(record, "block {error:?}; {:?}", charged.statistics).unwrap();
                    return record;
                }
            }
            terminal => {
                writeln!(record, "{terminal:?}; {:?}", charged.statistics).unwrap();
                return record;
            }
        }
    }
    panic!("bounded fixture must terminate within 93 observations");
}

#[test]
fn queens_candidate_traces_match_the_frozen_baseline() {
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
fn refined_candidate_trace_matches_the_frozen_baseline() {
    assert_eq!(
        trace(CHOICES, true, SearchLimits::default()),
        include_str!("../fixtures/watch-traces/refined.txt")
    );
}

#[test]
fn exact_search_ceilings_preserve_the_frozen_trace() {
    let limits = SearchLimits {
        max_work: 2294,
        max_decisions: 9,
    };
    assert_eq!(
        trace(CHOICES, true, limits),
        include_str!("../fixtures/watch-traces/exact.txt")
    );
}

#[test]
fn short_work_ceiling_preserves_the_frozen_stop() {
    let limits = SearchLimits {
        max_work: 2293,
        max_decisions: 9,
    };
    assert_eq!(
        trace(CHOICES, true, limits),
        include_str!("../fixtures/watch-traces/work-short.txt")
    );
}

#[test]
fn short_decision_ceiling_preserves_the_frozen_stop() {
    let limits = SearchLimits {
        max_work: 2294,
        max_decisions: 8,
    };
    assert_eq!(
        trace(CHOICES, true, limits),
        include_str!("../fixtures/watch-traces/decision-short.txt")
    );
}
