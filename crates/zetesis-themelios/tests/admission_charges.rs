//! Admission charges count the operations the formula route performs, so a
//! ceiling refuses a program for its size, not for an allowance's shape.

use std::fmt::Write as _;

use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

/// `n` facts and `n - 1` two-literal rules over distinct predicates: the
/// producer plan must relate every body occurrence to its dependency edge.
fn producer_chain(predicates: usize) -> String {
    let mut source = String::new();
    for index in 0..predicates {
        writeln!(source, "p{index}.").unwrap();
    }
    for index in 0..predicates.saturating_sub(1) {
        writeln!(source, "q{index} :- p{index}, p{}.", index + 1).unwrap();
    }
    source
}

/// The least formula work ceiling that admits `source`, by bisection.
fn minimal_work(source: &str) -> u64 {
    let admits = |max_work: u64| {
        admit_formula(
            source.to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits {
                max_work,
                ..FormulaLimits::default()
            },
        )
        .is_ok()
    };
    let (mut low, mut high) = (0, 1u64 << 40);
    assert!(admits(high));
    while low < high {
        let middle = low + (high - low) / 2;
        if admits(middle) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

/// The least scalar byte budget that admits `source`, by bisection.
fn minimal_scalar_bytes(source: &str) -> usize {
    let admits = |max_scalar_bytes: usize| {
        admit_formula(
            source.to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits {
                max_scalar_bytes,
                ..ExpansionLimits::default()
            },
            FormulaLimits::default(),
        )
        .is_ok()
    };
    let (mut low, mut high) = (0, 1usize << 34);
    assert!(admits(high));
    while low < high {
        let middle = low + (high - low) / 2;
        if admits(middle) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

#[test]
fn producer_plan_charges_grow_with_rules_times_log_predicates() {
    // Doubling the predicates doubles the rules and adds one to the
    // logarithm; a charge proportional to rules × predicates would
    // quadruple. The linear remainder of admission keeps the ratio near two.
    let (small, large) = (
        minimal_work(&producer_chain(200)),
        minimal_work(&producer_chain(400)),
    );
    assert!(
        large < small * 3,
        "work grew from {small} to {large} for twice the predicates"
    );
}

#[test]
fn the_head_allowance_is_charged_once_per_new_atom_not_per_proposal() {
    // Both programs derive the same atoms; the second proposes p(1) fifty
    // times as often through a wider join. Its byte requirement may differ
    // by the wider frame, not by the repeated proposals.
    let once = minimal_scalar_bytes("d(1..50). p(1) :- d(X).");
    let repeated = minimal_scalar_bytes("d(1..50). p(1) :- d(X), d(Y).");
    assert!(
        repeated <= once + 256,
        "{repeated} bytes for repeated proposals against {once}"
    );
}

#[test]
fn binding_frames_are_not_charged_to_the_cumulative_budget() {
    // Two thousand bindings against fifty: the atoms are the same, so the
    // cumulative byte requirement is the same up to a frame's width.
    let fewer = minimal_scalar_bytes("d(1..50). e(1..1). p(X) :- d(X), e(Y).");
    let more = minimal_scalar_bytes("d(1..50). e(1..40). p(X) :- d(X), e(Y).");
    assert!(
        more <= fewer + 40 * 16,
        "{more} bytes for two thousand bindings against {fewer}"
    );
}
