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

/// Rows offered to the whole-row matcher, and expressions evaluated, while
/// completing possible support and while instantiating rules.
#[derive(Default)]
struct JoinRows {
    active: Cell<bool>,
    support: Cell<u64>,
    instantiation: Cell<u64>,
    evaluations: Cell<u64>,
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
        if phase == GroundingPhase::RuleInstantiation {
            let rows = work.join_rows.expect("finite visits");
            self.instantiation
                .set(self.instantiation.get().checked_add(rows).unwrap());
        }
        if phase == GroundingPhase::SupportCompletion {
            let rows = work.join_rows.expect("finite visits");
            self.support
                .set(self.support.get().checked_add(rows).unwrap());
            let evaluations = work.expression_evaluations.expect("finite evaluations");
            self.evaluations
                .set(self.evaluations.get().checked_add(evaluations).unwrap());
        }
    }
}

/// The rows the support completion phases read and the expressions they
/// evaluate. The work ceiling is raised so that it measures the order and
/// not the admission of the program.
fn support_counts(source: &str) -> (u64, u64) {
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
    (rows.support.get(), rows.evaluations.get())
}

/// The rows the rule instantiation phases read, under the same ceiling.
fn instantiation_rows(source: &str) -> u64 {
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
    rows.instantiation.get()
}

fn support_rows(source: &str) -> u64 {
    support_counts(source).0
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

#[test]
fn a_comparison_is_evaluated_once_at_the_depth_that_binds_it() {
    // X < Y is decided once d(Y) is joined: two expressions for each of the
    // 1,600 pairs, and none again for the 31,200 rows d(Z) adds beneath the
    // 780 surviving pairs, whose verdict the deeper prefix inherits.
    let (_, evaluations) = support_counts("d(1..40). p(X,Y,Z) :- d(X), d(Y), X < Y, d(Z).");
    assert_eq!(evaluations, 2 * 1_600);
}

#[test]
fn rule_instantiation_reads_only_the_substitutions_the_comparisons_leave() {
    // A substitution X < Y excludes is not an instance, so instantiation
    // prunes it as possible support does: 40 + 1,600 + 780 × 40 rows, not
    // the full 65,640-row product validated before the exclusion rule.
    let rows = instantiation_rows("d(1..40). p(X,Y,Z) :- d(X), d(Y), X < Y, d(Z).");
    assert_eq!(rows, 40 + 1_600 + 780 * 40);
}

#[test]
fn a_comparison_a_prefix_decided_is_not_evaluated_again_for_the_complete_row() {
    // q(1). r(2).  X < Y is decided once r binds Y: two evaluations, one per
    // side. The tuple comparison waits for the complete row and costs four
    // more, and nothing else is evaluated: the decided comparison is not
    // read a second time.
    let (_, evaluations) =
        support_counts("q(1). r(2). p(X,Y) :- q(X), r(Y), X < Y, (X,Y) != (1,2).");
    assert_eq!(evaluations, 6);
}
