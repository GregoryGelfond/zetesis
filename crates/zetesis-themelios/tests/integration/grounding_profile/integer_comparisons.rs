//! Comparisons of integer arithmetic are decided by their integer comparison
//! forms, and every other comparison by the general evaluator, with the same
//! answers.

use zetesis_reference_support::{exhaustive, formula};
use zetesis_test_support::records::Records;
use zetesis_themelios::{ComparisonForms, FormulaLimits};

use super::{Observer, compile};

/// The route receipts over every observed phase, and how preparation
/// compiled the comparison literals.
#[derive(Debug)]
struct Receipts {
    /// Rows decided by an integer comparison form.
    decided: u64,
    /// Rows of a literal with a form that fell back to the general evaluator.
    declined: u64,
    /// The prepared program's comparison literals, with and without a form.
    forms: ComparisonForms,
}

fn receipts(source: &str) -> Receipts {
    let observer = Observer::default();
    let admitted = compile(source, &FormulaLimits::default(), Some(&observer)).unwrap();
    let records = observer.records.borrow();
    let total = |field: fn(&super::Record) -> Option<u64>| {
        records.iter().map(|record| field(record).unwrap()).sum()
    };
    Receipts {
        decided: total(|record| record.work.integer_comparisons),
        declined: total(|record| record.work.integer_comparison_declines),
        forms: admitted.comparison_forms(),
    }
}

fn answers(source: &str) -> Records {
    exhaustive(&formula(source))
}

fn one_answer(atoms: &[&str]) -> Records {
    [(atoms.iter().copied().map(str::to_owned).collect(), None)].into()
}

#[test]
fn integer_comparisons_are_decided_by_their_forms() {
    let source = "q(1..4). r(X,Y) :- q(X), q(Y), X < Y, X + Y = 5.";
    let receipts = receipts(source);
    assert!(receipts.decided > 0, "{receipts:?}");
    assert_eq!(receipts.declined, 0, "{receipts:?}");
    assert_eq!(receipts.forms.with_form, 2, "{receipts:?}");
    assert_eq!(receipts.forms.without_form(), 0, "{receipts:?}");
    let atoms = ["q(1)", "q(2)", "q(3)", "q(4)", "r(1,4)", "r(2,3)"];
    assert_eq!(answers(source), one_answer(&atoms));
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
    assert_eq!(receipts.forms.with_form, 1, "{receipts:?}");
    assert_eq!(answers(source), one_answer(&["p(1)", "p(a)", "r(1)"]));
}

#[test]
fn a_symbolic_constant_leaves_the_literal_without_a_form() {
    // X < a has no form: its constant is not a number. Its rows never reach a
    // form, so nothing is decided or declines; the general evaluator decides
    // each.
    let source = "p(1). p(b). r(X) :- p(X), X < a.";
    let receipts = receipts(source);
    assert_eq!(receipts.forms.nonnumeric_constant, 1, "{receipts:?}");
    assert_eq!(receipts.forms.without_form(), 1, "{receipts:?}");
    assert_eq!((receipts.decided, receipts.declined), (0, 0));
    assert_eq!(answers(source), one_answer(&["p(1)", "p(b)", "r(1)"]));
}

#[test]
fn a_constructor_operand_leaves_the_literal_without_a_form() {
    // f(X) > 2 constructs a function term, which ranks above every number.
    let source = "p(1). p(2). r(X) :- p(X), f(X) > 2.";
    let receipts = receipts(source);
    assert_eq!(receipts.forms.constructor, 1, "{receipts:?}");
    assert_eq!(receipts.forms.without_form(), 1, "{receipts:?}");
    assert_eq!((receipts.decided, receipts.declined), (0, 0));
    let atoms = ["p(1)", "p(2)", "r(1)", "r(2)"];
    assert_eq!(answers(source), one_answer(&atoms));
}

#[test]
fn argument_checks_are_decided_by_their_forms() {
    // The computed argument of p(X+1) is captured and checked for equality.
    let source = "q(1..3). p(2..3). r(X) :- q(X), p(X+1).";
    let receipts = receipts(source);
    assert!(receipts.decided > 0, "{receipts:?}");
    assert_eq!(receipts.declined, 0, "{receipts:?}");
    assert_eq!(receipts.forms.with_form, 1, "{receipts:?}");
    let atoms = ["q(1)", "q(2)", "q(3)", "p(2)", "p(3)", "r(1)", "r(2)"];
    assert_eq!(answers(source), one_answer(&atoms));
}

#[test]
fn a_rejected_row_is_checked_by_its_form() {
    // Y = 10/X fails on X = 0, so that row's remaining checks decide whether
    // it is excluded: X - 5 < 0 is fully bound there and its form decides it,
    // so the comparison is decided on every row and no row declines.
    let source = "p(0..2). r(X,Y) :- p(X), Y = 10/X, X - 5 < 0.";
    let receipts = receipts(source);
    assert_eq!(receipts.decided, 9, "{receipts:?}");
    assert_eq!(receipts.declined, 0, "{receipts:?}");
    let atoms = ["p(0)", "p(1)", "p(2)", "r(1,10)", "r(2,5)"];
    assert_eq!(answers(source), one_answer(&atoms));
}

#[test]
fn every_compiled_comparison_has_its_form_compiled() {
    // Comparisons in a body, an argument check, choice and aggregate
    // conditions, a conditional literal's condition and an objective
    // condition: six literals, each compiled with its form. The equality that
    // generates h's argument X+1 becomes a binding, not a comparison.
    let source = "d(1..3). e(2..4). \
        h(X+1) :- d(X), X != 2. \
        g(X) :- d(X), e(X+1). \
        {c(X) : d(X), X < 3}. \
        s(N) :- N = #sum{X : d(X), X > 1}. \
        a :- d(Y) : e(Y), Y < 4. \
        :~ c(X), X = 1. [X]";
    let forms = receipts(source).forms;
    assert_eq!(forms.with_form, 6, "{forms:?}");
    assert_eq!(forms.without_form(), 0, "{forms:?}");
}
