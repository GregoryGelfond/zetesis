//! Finite aggregate admission retains reduct conditions, scopes, and bounded evidence.

use std::collections::BTreeSet;
use std::path::Path;

use crate::support::atom_models::Models;
use zetesis_cpu::Cancellation;
use zetesis_reference_support::formula;
use zetesis_sat::{Limits, StableModels};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaFailure, FormulaLimits,
    FormulaResource, admit_extended, admit_formula,
};

fn models(input: &AdmittedFormula) -> Models {
    let mut search = StableModels::new(input.theory(), Limits::default(), Cancellation::default())
        .expect("search admission");
    let result = search
        .by_ref()
        .map(|result| {
            result
                .expect("complete search")
                .atoms()
                .map(|index| {
                    input
                        .atoms()
                        .at(index)
                        .unwrap()
                        .to_atom(zetesis_core::ValueLimits::default())
                        .unwrap()
                })
                .collect()
        })
        .collect();
    assert!(search.exhausted());
    result
}
fn native(source: &str) -> Models {
    models(&formula(source))
}
fn expected(sources: &[&str]) -> Models {
    sources.iter().flat_map(|source| native(source)).collect()
}

#[test]
fn arbitrary_choice_bounds_are_constraints_and_retain_conditional_support() {
    assert_eq!(native("2{a;b;c}2."), expected(&["a.b.", "a.c.", "b.c."]));
    assert_eq!(native("{a;b}1."), expected(&["", "a.", "b."]));
    assert_eq!(native("2{a;a:b}2. b."), Models::new());
    assert_eq!(native("1{a:a;b:b}."), Models::new());
    assert_eq!(native("0{a:a;b:b}1."), expected(&[""]));
    assert_eq!(
        native("n(2). N{a;b;c}N :- n(N)."),
        expected(&["n(2).a.b.", "n(2).a.c.", "n(2).b.c."])
    );
    assert!(
        admit_extended(
            "2{a;b}2.".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default()
        )
        .is_err()
    );
}

#[test]
fn full_tuples_are_deduplicated_without_simplifying_recursive_eligibility() {
    assert_eq!(native("p :- #count{1:p;1:not p}=1."), Models::new());
    assert_eq!(
        native("{p}. q :- #count{1:p;1:not p}=1."),
        expected(&["q.", "p.q."])
    );
    assert_eq!(
        native("edge(a,2). edge(b,2). q :- #sum{W,X:edge(X,W)}=4."),
        expected(&["edge(a,2).edge(b,2).q."])
    );
    assert_eq!(
        native("edge(a,2). edge(b,2). q :- #sum{W:edge(X,W)}=2."),
        expected(&["edge(a,2).edge(b,2).q."])
    );
    assert_eq!(native("q :- #count{:}=1."), expected(&["q."]));
}

#[test]
fn both_guards_and_default_negation_keep_numeric_comparisons_distinct() {
    assert_eq!(native("{a;b}. :- not 1{a;b}1."), expected(&["a.", "b."]));
    assert_eq!(
        native("{a}. b :- 0 < #count{1:a} <= 1."),
        expected(&["", "a.b."])
    );
    assert_eq!(native("a :- #sum{-1:a} != 0."), expected(&[""]));
    assert_eq!(native("a :- not #sum{-1:a} = 0."), expected(&["", "a."]));
    assert_eq!(native("a :- not not #count{1:a}>0."), expected(&["", "a."]));
}

#[test]
fn aggregate_local_variables_do_not_bind_globals_and_flat_tuple_filters_are_exact() {
    for source in ["p(X) :- #count{X:q(X)}>0.", "p :- #count{X:not q(X)}>0."] {
        assert!(
            matches!(
                admit_formula(
                    source.to_owned(),
                    AdmissionOptions::default(),
                    ExpansionLimits::default(),
                    FormulaLimits::default()
                ),
                Err(FormulaFailure::UnsafeVariable { .. })
            ),
            "{source}"
        );
    }
    assert_eq!(
        native("d(1..2). q(X) :- d(X), #count{Y:d(Y),Y!=X}=1."),
        expected(&["d(1..2). q(1..2)."])
    );
    assert_eq!(
        native("d(1..2). q(X,Y) :- d(X),d(Y),(X,Y)!=(1,1)."),
        expected(&["d(1..2).q(1,2).q(2,1).q(2,2)."])
    );
    assert_eq!(
        native("q :- (1,2)=(1,2). r :- (1,2)!=(1,2)."),
        expected(&["q."])
    );
}

#[test]
fn aggregate_limits_and_original_rule_evidence_are_independent() {
    let source = "{a;b}. q :- #count{1:a;2:b}>=1.";
    let input = formula(source);
    assert_eq!(input.source().text(), source);
    assert_eq!(input.formula_origins().len(), input.theory().roots().len());
    assert!(
        input
            .formula_origins()
            .iter()
            .all(|origins| !origins.is_empty())
    );
    let mut limits = FormulaLimits::default();
    limits.aggregate.max_elements = 1;
    assert!(matches!(
        admit_formula(
            source.to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::AggregateElements,
            ..
        })
    ));
    let mut limits = FormulaLimits::default();
    limits.aggregate.max_work = 0;
    assert!(matches!(
        admit_formula(
            source.to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits
        ),
        Err(FormulaFailure::Aggregate { .. })
    ));
    let mut limits = FormulaLimits::default();
    limits.aggregate.max_subsets = 1;
    assert!(matches!(
        admit_formula(
            "{a;b}. q :- #sum{-1:a;1:b}=0.".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits
        ),
        Err(FormulaFailure::Aggregate { .. })
    ));
}

#[test]
fn all_queens_encodings_exhaust_the_same_ninety_two_boards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/correctness/standalone/n-queens");
    let mut reference = None;
    for variant in [
        "variant-01.lp",
        "variant-02.lp",
        "variant-03.lp",
        "variant-04.lp",
        "variant-05.lp",
        "variant-06.lp",
    ] {
        let source =
            std::fs::read_to_string(root.join(variant)).expect("correctness example source");
        let input = formula(&source);
        assert_eq!(input.source().text(), source);
        let models = models(&input);
        assert_eq!(models.len(), 92, "{variant}");
        let mut boards = BTreeSet::new();
        for model in models {
            let queens: Vec<_> = model
                .iter()
                .filter(|atom| atom.predicate().name() == "queen_at")
                .collect();
            assert_eq!(queens.len(), 8);
            for (index, queen) in queens.iter().enumerate() {
                for other in &queens[index + 1..] {
                    assert_ne!(queen.values()[0], other.values()[0]);
                    assert_ne!(queen.values()[1], other.values()[1]);
                }
            }
            boards.insert(queens.into_iter().cloned().collect::<BTreeSet<_>>());
        }
        if let Some(expected) = &reference {
            assert_eq!(&boards, expected, "{variant}");
        } else {
            reference = Some(boards);
        }
    }
}
