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
    // Both programs retain exactly the same source facts, analyzed owner
    // sidecar and p atoms. Only the completed body bindings differ: fifty
    // against two thousand. Reusing a transient frame must not charge it for
    // each extra binding; both comparisons also retain the same input slots.
    let fewer = minimal_scalar_bytes("d(1..50). e(1..40). p(X) :- d(X), e(Y), Y<=1.");
    let more = minimal_scalar_bytes("d(1..50). e(1..40). p(X) :- d(X), e(Y), Y<=40.");
    assert!(
        more <= fewer,
        "{more} bytes for two thousand bindings against {fewer} with the same input carrier"
    );
}

fn scalar_usage(source: &str) -> usize {
    admit_formula(
        source.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
    .expansion_usage()
    .scalar_bytes
}

#[test]
fn arithmetic_scratch_does_not_accumulate_per_substitution() {
    // The facts, generated p atoms and two-slot body frame are identical.
    // Only r's first argument changes the number of complete joins: 50 or
    // 2,000. Subtract each program's nonarithmetic control to isolate the
    // same compiled expression from repeated transient evaluation storage.
    let usages = ["r(Y,1)", "r(1,1)"].map(|selector| {
        let source = format!("d(1..50).e(1..40).r(1,1).p(X):-d(X),e(Y),{selector}");
        let plain = scalar_usage(&format!("{source}."));
        let arithmetic = scalar_usage(&format!("{source},1/X>=0."));
        (plain, arithmetic)
    });
    let [
        (fewer_plain, fewer_arithmetic),
        (more_plain, more_arithmetic),
    ] = usages;
    assert_eq!(
        fewer_arithmetic + more_plain,
        more_arithmetic + fewer_plain,
        "same-carrier scalar usage: fewer={usages:?} (plain, arithmetic)"
    );
}

#[test]
fn local_family_scratch_does_not_accumulate_per_outer_binding() {
    // Each outer row visits the same one-element local family. Both programs
    // retain identical fact and possible-head carriers; the local family
    // needs only one live frame whether there are 50 or 2,000 outer rows.
    let usages = ["r(Y,1)", "r(1,1)"].map(|selector| {
        let prefix = "d(1..50).e(1..40).r(1,1).s(1).";
        let body = format!("d(X),e(Y),{selector}.");
        let plain = scalar_usage(&format!("{prefix}{{p(X):s(Z)}}:-{body}"));
        let arithmetic = scalar_usage(&format!("{prefix}{{p(X):s(Z),1/Z=1}}:-{body}"));
        (plain, arithmetic)
    });
    let [
        (fewer_plain, fewer_arithmetic),
        (more_plain, more_arithmetic),
    ] = usages;
    assert_eq!(
        fewer_arithmetic - fewer_plain,
        more_arithmetic - more_plain,
        "same-carrier local scalar usage: {usages:?} (plain, arithmetic)"
    );
}
