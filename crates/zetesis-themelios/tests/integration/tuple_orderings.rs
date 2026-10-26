//! An ordering between syntactic tuples that still contain variables compares
//! complete values in ASP term order, wherever a comparison can stand.
use crate::support::finite_bindings as reference;
use reference::{Models, exhaustive, expected, native};
use zetesis_reference_support::{admit, formula};
use zetesis_themelios::{FormulaFailure, FormulaLimits};

/// Each source with its complete models, as clingo 5.8.2 gives them: every
/// ordering relation, unequal arities, mixed value types, nested tuples,
/// evaluated components, default and double negation, comparison chains,
/// aggregate element conditions and guards, choice conditions, constraints,
/// disjunctive heads and conditional literals.
fn cases() -> Vec<(&'static str, Models)> {
    vec![
        (
            "a(1;2).p(A,C):-a(A),a(C),(A,1)<(C,0).",
            expected(&[&["a(1)", "a(2)", "p(1,2)"]]),
        ),
        (
            "a(1;2).p(A,C):-a(A),a(C),(A,1)<=(C,1).",
            expected(&[&["a(1)", "a(2)", "p(1,1)", "p(1,2)", "p(2,2)"]]),
        ),
        (
            "a(1;2).p(A,C):-a(A),a(C),(A,0)>(C,1).",
            expected(&[&["a(1)", "a(2)", "p(2,1)"]]),
        ),
        (
            "a(1;2).p(A,C):-a(A),a(C),(A,1)>=(C,1).",
            expected(&[&["a(1)", "a(2)", "p(1,1)", "p(2,1)", "p(2,2)"]]),
        ),
        (
            "a(1;2).p(A):-a(A),(A,A)<(0,0,0).q(A):-a(A),(A,)>(9,9).",
            expected(&[&["a(1)", "a(2)", "p(1)", "p(2)"]]),
        ),
        (
            "a(1;2).p(A):-a(A),(A,b)<(1,c).q(A):-a(A),(A,f(A))<(A,\"s\").r(A):-a(A),(A,\"s\")<(A,f(A)).",
            expected(&[&["a(1)", "a(2)", "p(1)", "r(1)", "r(2)"]]),
        ),
        (
            "a(1;2).p(A):-a(A),((A,1),A)<((2,1),0).q(A):-a(A),(A,(A,A))>=(2,(2,1)).",
            expected(&[&["a(1)", "a(2)", "p(1)", "q(2)"]]),
        ),
        (
            "a(1;2).p(A):-a(A),(A+1,-A)<(3,-1).q(A):-a(A),(A\\2,A*2)<=(0,4).",
            expected(&[&["a(1)", "a(2)", "p(1)", "p(2)", "q(2)"]]),
        ),
        (
            "a(1;2).p(A):-a(A),not (A,0)<(2,0).q(A):-a(A),not not (A,0)<(2,0).",
            expected(&[&["a(1)", "a(2)", "p(2)", "q(1)"]]),
        ),
        (
            "a(1;2).p(A):-a(A),(0,0)<(A,0)<(2,0).q(A):-a(A),(A,1)<=(2,1)<=(A,A).",
            expected(&[&["a(1)", "a(2)", "p(1)", "q(2)"]]),
        ),
        (
            "r(1,3).r(2,1).r(3,2).n(N):-N=#count{L:r(L,M),(M,L)<(2,9)}.",
            expected(&[&["n(2)", "r(1,3)", "r(2,1)", "r(3,2)"]]),
        ),
        (
            "r(1,3).r(2,1).r(3,2).s(N):-N=#sum{M,L:r(L,M),(M,L)>=(2,3)}.",
            expected(&[&["r(1,3)", "r(2,1)", "r(3,2)", "s(5)"]]),
        ),
        (
            "r(1,3).r(2,1).r(3,2).m(N):-N=#min{(M,L):r(L,M),(M,L)>(1,2)}.x(N):-N=#max{L:r(L,M),(M,L)<=(2,3)}.",
            expected(&[&["m((2,3))", "r(1,3)", "r(2,1)", "r(3,2)", "x(3)"]]),
        ),
        (
            "r(1,3).r(2,1).r(3,2).k(K):-r(K,_),2<=#count{L:r(L,M),(M,L)<(K,K)}.",
            expected(&[&["k(3)", "r(1,3)", "r(2,1)", "r(3,2)"]]),
        ),
        (
            "a(1;2).{s(A):a(A),(A,0)<(2,0)}.",
            expected(&[&["a(1)", "a(2)"], &["a(1)", "a(2)", "s(1)"]]),
        ),
        (
            "a(1;2).{p(A):a(A)}.:-p(A),p(B),(A,B)<(1,2).",
            expected(&[&["a(1)", "a(2)"], &["a(1)", "a(2)", "p(2)"]]),
        ),
        (
            "a(1;2).p(A)|q(A):-a(A),(A,0)<(2,0).",
            expected(&[&["a(1)", "a(2)", "p(1)"], &["a(1)", "a(2)", "q(1)"]]),
        ),
        (
            "a(1;2).b(2).d(1).all:-b(Y),d(X):a(X),(X,Y)<(Y,Y).",
            expected(&[&["a(1)", "a(2)", "all", "b(2)", "d(1)"]]),
        ),
    ]
}

#[test]
fn open_tuple_orderings_match_independent_complete_models() {
    for (source, wanted) in cases() {
        let program = formula(source);
        assert_eq!(native(&program), wanted, "{source}");
        assert_eq!(exhaustive(&program), wanted, "{source}");
    }
}

#[test]
fn a_tuple_of_lower_arity_orders_first() {
    // Arity decides before any component: (A,A) precedes every triple, and
    // no singleton follows a pair, whatever the components hold.
    let program = formula("a(1;2).p(A):-a(A),(A,A)<(0,0,0).q(A):-a(A),(A,)>(9,9).");
    assert_eq!(
        native(&program),
        expected(&[&["a(1)", "a(2)", "p(1)", "p(2)"]])
    );
}

#[test]
fn an_open_tuple_ordering_binds_no_component() {
    // An ordering only compares bound values: unlike tuple equality, it
    // cannot supply X, which no positive literal binds.
    let source = "p(X):-(X,1)<(2,1).";
    let error = admit(source, &FormulaLimits::default()).unwrap_err();
    assert!(
        matches!(error, FormulaFailure::UnsafeVariable { .. }),
        "{source}: {error}"
    );
}

#[test]
fn open_tuple_equality_still_binds_its_components() {
    // Equality keeps its destructuring form, which supplies each component.
    let program = formula("p(X,Y):-(X,Y)=(1,2).");
    assert_eq!(native(&program), expected(&[&["p(1,2)"]]));
}

#[test]
fn an_aggregate_element_condition_counts_by_tuple_order() {
    // The element condition compares each (M,L) pair with the outer key: (1,2)
    // and (2,3) precede (2,9); (3,1) does not.
    let program = formula("r(1,3).r(2,1).r(3,2).n(N):-N=#count{L:r(L,M),(M,L)<(2,9)}.");
    assert_eq!(
        native(&program),
        expected(&[&["n(2)", "r(1,3)", "r(2,1)", "r(3,2)"]])
    );
}

#[test]
#[ignore = "requires clingo: open tuple orderings match clingo full models"]
fn open_tuple_orderings_match_clingo() {
    reference::original_sources_match_clingo_full_models(
        cases().iter().map(|(source, _)| *source),
        formula,
    );
}
