//! The positive join is ordered by what each literal binds: a comparison is
//! decided as soon as its variables are bound, a literal whose variables are
//! all bound is a test, not a generator, and a round's new rows are joined
//! first.

use std::cell::Cell;
use std::fmt::Write as _;

use themelios_base::span::Location;
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaLimits, GroundingObserver, GroundingOutcome,
    GroundingPhase, GroundingWork, admit_formula_with_grounding_observer,
};

/// Rows offered to the whole-row matcher while completing possible support,
/// the phase whose joins prune on comparisons; rule instantiation retains
/// every complete row for validation and reads the full product.
#[derive(Default)]
struct JoinRows {
    active: Cell<bool>,
    support: Cell<u64>,
}

impl GroundingObserver for JoinRows {
    fn enter(&self) {
        assert!(!self.active.replace(true));
    }
    fn exit(&self) {
        assert!(self.active.replace(false));
    }
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        phase: GroundingPhase,
        _: Option<Location>,
        outcome: GroundingOutcome,
        work: GroundingWork,
    ) {
        assert_eq!(outcome, GroundingOutcome::Completed);
        if phase == GroundingPhase::SupportCompletion {
            let rows = work.join_rows.expect("finite visits");
            self.support
                .set(self.support.get().checked_add(rows).unwrap());
        }
    }
}

/// The rows the support completion phases read. The work ceiling is raised
/// so that it measures the order and not the admission of the program.
fn support_rows(source: &str) -> u64 {
    let rows = JoinRows::default();
    admit_formula_with_grounding_observer(
        source.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits {
            max_work: 100_000_000,
            ..FormulaLimits::default()
        },
        Some(&rows),
    )
    .unwrap();
    rows.support.get()
}

#[test]
fn a_waiting_comparison_is_decided_before_an_unrelated_relation_is_joined() {
    // Over d(1..40), X < Y holds for 780 of the 1,600 pairs. Joining d(Y)
    // second decides it before d(Z) multiplies the rows: 40 + 1,600 + 780 × 40
    // rows in the round that derives p, and none in the round that finds no
    // new d row, whose pivot is joined first and offers nothing. Which letter
    // names the third variable is not a criterion.
    let named_z = support_rows("d(1..40). p(X,Y,Z) :- d(X), d(Y), X < Y, d(Z).");
    let named_w = support_rows("d(1..40). p(X,Y,W) :- d(X), d(Y), X < Y, d(W).");
    assert_eq!(named_z, 40 + 1_600 + 780 * 40);
    assert_eq!(named_w, named_z);
}

#[test]
fn a_literal_whose_variables_are_bound_is_a_test_and_precedes_generators() {
    // Ten a values, one thousand c values that include them, and five hundred
    // b facts over the ten. After a(X) binds X, c(X) is one probe per row:
    // ten rows before b(X,Y), five hundred after it. Y reaches the head so
    // that b is joined rather than checked for a witness.
    let mut source = String::from("a(1..10). c(1..1000). ");
    for x in 1..=10 {
        for y in 1..=50 {
            write!(source, "b({x},{y}). ").unwrap();
        }
    }
    source.push_str("r(X,Y) :- a(X), c(X), b(X,Y).");
    assert_eq!(support_rows(&source), 10 + 10 + 500);
}
