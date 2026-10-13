//! Joins decide comparisons of integer arithmetic with machine integers, and
//! leave every other comparison to the general evaluator, with the same answers.

use zetesis_reference_support::{exhaustive, formula};
use zetesis_test_support::records::Records;
use zetesis_themelios::FormulaLimits;

use super::{Observer, compile};

/// The route receipts over every observed phase.
#[derive(Debug, Default)]
struct Receipts {
    /// Rows decided by an integer comparison form.
    decided: u64,
    /// Rows of a literal with a form that fell back to the general evaluator.
    declined: u64,
    /// Literals compiled into a form, once per join.
    forms: u64,
    /// Literals found to have no form, once per join.
    without_form: u64,
}

fn receipts(source: &str) -> Receipts {
    let observer = Observer::default();
    compile(source, &FormulaLimits::default(), Some(&observer)).unwrap();
    let records = observer.records.borrow();
    let total = |field: fn(&super::Record) -> Option<u64>| {
        records.iter().map(|record| field(record).unwrap()).sum()
    };
    Receipts {
        decided: total(|record| record.work.integer_comparisons),
        declined: total(|record| record.work.integer_comparison_declines),
        forms: total(|record| record.work.integer_comparison_forms),
        without_form: total(|record| record.work.comparisons_without_integer_form),
    }
}

fn answers(source: &str) -> Records {
    exhaustive(&formula(source))
}

#[test]
fn integer_comparisons_are_decided_by_their_forms() {
    let source = "q(1..4). r(X,Y) :- q(X), q(Y), X < Y, X + Y = 5.";
    let receipts = receipts(source);
    assert!(receipts.decided > 0, "{receipts:?}");
    assert!(receipts.forms > 0, "{receipts:?}");
    assert_eq!(receipts.declined, 0, "{receipts:?}");
    assert_eq!(receipts.without_form, 0, "{receipts:?}");
    let atoms = ["q(1)", "q(2)", "q(3)", "q(4)", "r(1,4)", "r(2,3)"];
    let expected: Records = [(atoms.map(str::to_owned).into(), None)].into();
    assert_eq!(answers(source), expected);
}

#[test]
fn a_symbolic_row_declines_to_the_general_order() {
    // X < 2 has a form; on the row X = a the form declines, and the general
    // evaluator decides `a < 2` false in the ASP order, which ranks numbers
    // below constants.
    let source = "p(1). p(a). r(X) :- p(X), X < 2.";
    let receipts = receipts(source);
    assert!(receipts.decided > 0, "{receipts:?}");
    assert!(receipts.declined > 0, "{receipts:?}");
    let atoms = ["p(1)", "p(a)", "r(1)"];
    let expected: Records = [(atoms.map(str::to_owned).into(), None)].into();
    assert_eq!(answers(source), expected);
}

#[test]
fn a_symbolic_constant_leaves_the_literal_without_a_form() {
    // X < a has no form: its constant is not a number. Its rows never reach a
    // form, so nothing declines; the general evaluator decides each.
    let source = "p(1). p(b). r(X) :- p(X), X < a.";
    let receipts = receipts(source);
    assert!(receipts.without_form > 0, "{receipts:?}");
    assert_eq!(receipts.decided, 0, "{receipts:?}");
    assert_eq!(receipts.declined, 0, "{receipts:?}");
    let atoms = ["p(1)", "p(b)", "r(1)"];
    let expected: Records = [(atoms.map(str::to_owned).into(), None)].into();
    assert_eq!(answers(source), expected);
}

#[test]
fn a_constructor_operand_leaves_the_literal_without_a_form() {
    // f(X) > 2 constructs a function term, which ranks above every number.
    let source = "p(1). p(2). r(X) :- p(X), f(X) > 2.";
    let receipts = receipts(source);
    assert!(receipts.without_form > 0, "{receipts:?}");
    assert_eq!(receipts.decided, 0, "{receipts:?}");
    let atoms = ["p(1)", "p(2)", "r(1)", "r(2)"];
    let expected: Records = [(atoms.map(str::to_owned).into(), None)].into();
    assert_eq!(answers(source), expected);
}

#[test]
fn argument_checks_are_decided_by_their_forms() {
    // The computed argument of p(X+1) is captured and checked for equality.
    let source = "q(1..3). p(2..3). r(X) :- q(X), p(X+1).";
    let receipts = receipts(source);
    assert!(receipts.decided > 0, "{receipts:?}");
    assert_eq!(receipts.declined, 0, "{receipts:?}");
    let atoms = ["q(1)", "q(2)", "q(3)", "p(2)", "p(3)", "r(1)", "r(2)"];
    let expected: Records = [(atoms.map(str::to_owned).into(), None)].into();
    assert_eq!(answers(source), expected);
}
