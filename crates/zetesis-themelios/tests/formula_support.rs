//! Semantic regressions for relational support pruning. Expected complete
//! models were independently checked with clingo 5.8.2; this portable suite
//! evaluates every finite candidate using the exhaustive Ferraris oracle.

use std::collections::BTreeSet;

use zetesis_core::{Atom, Term};
use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, Limits, check};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits,
    FormulaFailure, FormulaLimits, FormulaResource, ProfileFeature, admit, admit_formula,
};

type Models = BTreeSet<BTreeSet<Atom>>;

fn admitted(source: &str) -> AdmittedFormula {
    admit_formula(
        source.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .expect("supported source profile")
}

fn exhaustive(input: &AdmittedFormula) -> Models {
    let count = input.atoms().len();
    assert!(count <= 16, "small independent enumeration carrier");
    let control = Control::default();
    let mut models = Models::new();
    for bits in 0..(1_usize << count) {
        let candidate = Interpretation::new(
            input.theory(),
            (0..count).filter(|&atom| bits & (1 << atom) != 0),
        )
        .expect("same-theory interpretation");
        if check(input.theory(), &candidate, Limits::default(), &control)
            .expect("complete subset minimality check")
            .accepted()
        {
            assert!(
                models.insert(
                    candidate
                        .atoms()
                        .map(|atom| input.atoms()[atom].clone())
                        .collect()
                )
            );
        }
    }
    models
}

fn expected(models: &[&str]) -> Models {
    // These hand-authored fixtures contain only nullary atoms and integer
    // arguments, so whitespace unambiguously separates complete identities.
    models
        .iter()
        .map(|model| {
            model
                .split_ascii_whitespace()
                .map(|source| {
                    let fact = admit(format!("{source}."), AdmissionOptions::default())
                        .expect("recorded scalar fact");
                    let head = fact.program().templates()[0].head().expect("fact head");
                    let values = head
                        .terms()
                        .iter()
                        .map(|term| match term {
                            Term::Constant(value) => value.clone(),
                            Term::Variable(_) => panic!("expected atoms are ground"),
                        })
                        .collect();
                    Atom::new(head.predicate().clone(), values).expect("ground atom arity")
                })
                .collect()
        })
        .collect()
}

fn assert_models(source: &str, models: &[&str]) {
    let expected = expected(models);
    assert_eq!(exhaustive(&admitted(source)), expected, "{source}");
    // Statement order must not determine which support round is sufficient.
    // The fixed grammar below contains no periods inside terms or strings.
    let mut statements: Vec<_> = source.split_terminator('.').collect();
    statements.reverse();
    let reversed = format!("{}.", statements.join("."));
    assert_eq!(exhaustive(&admitted(&reversed)), expected, "{reversed}");
}

#[test]
fn negative_gates_remain_in_the_reduct_and_do_not_restrict_the_upper_relation() {
    for (source, models) in [
        ("b. a :- not b.", &["b"][..]),
        ("a :- not b.", &["a"][..]),
        ("a :- not not a.", &["", "a"][..]),
        ("a :- not not b. b :- not not a.", &["", "a b"][..]),
        ("{a}. b :- a,not a.", &["", "a"][..]),
        (
            "d(1). p(X) :- d(X),not not p(X).",
            &["d(1)", "d(1) p(1)"][..],
        ),
        ("a :- a,1=2. b :- not a.", &["b"][..]),
    ] {
        assert_models(source, models);
    }
}

#[test]
fn positive_cycles_and_recursive_choice_eligibility_do_not_invent_support() {
    for (source, models) in [
        ("a :- a.", &[""][..]),
        ("a :- b. b :- a.", &[""][..]),
        ("{a:a}.", &[""][..]),
        ("{a:b}. b :- a.", &[""][..]),
        ("1 {a:a} 1.", &[][..]),
        ("{a:b;b:not not a}.", &["", "a b"][..]),
    ] {
        assert_models(source, models);
    }
}

#[test]
fn constraints_restrict_models_without_deriving_their_atoms() {
    assert_models(":- a.", &[""]);
    assert_models(":- not a.", &[]);
    assert_models("{a}. :- a.", &[""]);
}

#[test]
fn relational_joins_reach_complete_support_and_undo_failed_bindings() {
    assert_models("{seed}. a :- seed. b :- a. c :- b.", &["", "a b c seed"]);
    assert_models(
        "p(1). p(Y) :- p(X),edge(X,Y). edge(1,2).edge(2,3).edge(3,4).",
        &["edge(1,2) edge(2,3) edge(3,4) p(1) p(2) p(3) p(4)"],
    );
    assert_models(
        "{start}. p(1) :- start. p(Y) :- p(X),edge(X,Y). edge(1,2).edge(2,3).",
        &[
            "edge(1,2) edge(2,3)",
            "edge(1,2) edge(2,3) p(1) p(2) p(3) start",
        ],
    );
    assert_models("p :- edge(_, _). edge(1,2).", &["edge(1,2) p"]);
    assert_models(
        "p(X,Z) :- edge(X,Y),edge(Y,Z). edge(1,2). edge(2,3). edge(1,3).",
        &["edge(1,2) edge(1,3) edge(2,3) p(1,3)"],
    );
    assert_models(
        "p(X,X) :- edge(X,X). edge(1,2). edge(2,2).",
        &["edge(1,2) edge(2,2) p(2,2)"],
    );
}

#[test]
fn local_choice_joins_preserve_global_bindings_and_duplicate_eligibility() {
    assert_models(
        "{p(X):d(X)}. d(1;2).",
        &[
            "d(1) d(2)",
            "d(1) d(2) p(1)",
            "d(1) d(2) p(2)",
            "d(1) d(2) p(1) p(2)",
        ],
    );
    assert_models(
        "1 {p(X,Y):edge(X,Y)} 1 :- row(X). row(1;2). edge(1,2).edge(2,1).",
        &["edge(1,2) edge(2,1) p(1,2) p(2,1) row(1) row(2)"],
    );
    assert_models(
        "1 {p(X,Y):edge(X,Y)} 1 :- row(X). row(1;2). edge(1,2).",
        &[],
    );
    assert_models(
        "{p(X):r(X,Y)}. r(1,2).r(1,3).r(2,3).",
        &[
            "r(1,2) r(1,3) r(2,3)",
            "p(1) r(1,2) r(1,3) r(2,3)",
            "p(2) r(1,2) r(1,3) r(2,3)",
            "p(1) p(2) r(1,2) r(1,3) r(2,3)",
        ],
    );
    assert_models(
        "1 {p(X):r(X,Y)} 1. r(1,2).r(1,3).r(2,3).",
        &["p(1) r(1,2) r(1,3) r(2,3)", "p(2) r(1,2) r(1,3) r(2,3)"],
    );
}

#[test]
fn incomplete_support_is_a_located_refusal_not_a_smaller_program() {
    let source = "p(1). p(2) :- p(1). p(3) :- p(2).";
    for max_support_rounds in [0, 1] {
        let error = admit_formula(
            source.to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits {
                max_support_rounds,
                ..FormulaLimits::default()
            },
        )
        .expect_err("support closure has not reached its final no-change round");
        assert!(matches!(
            error,
            FormulaFailure::Limit {
                resource: FormulaResource::SupportRounds,
                limit,
                observed,
                ..
            } if limit == u128::from(max_support_rounds) && observed > limit
        ));
        assert!(!error.diagnostics().is_empty(), "located admission refusal");
    }
}

#[test]
fn objective_presence_does_not_claim_exactness_from_negative_producer_overapproximation() {
    // These sources have positive-only objective conditions. clingo's grounding
    // nevertheless removes some priorities because of negative producers. A
    // positive support upper bound cannot reproduce that presentation contract.
    // Other cases retain their priorities, so the initial profile deliberately
    // refuses this entire unresolved dependency fragment instead of guessing.
    for source in [
        "b. a :- not b. #minimize {1@7,k:a}.",
        "a :- not not b. #minimize {1@7,k:a}.",
        "a :- not not a. #minimize {1@7,k:a}.",
        "a. b :- a,not a. #minimize {3@1,k:a;7@2,k:b}.",
        "{a}. b :- a,not a. #minimize {3@1,k:a;7@2,k:b}.",
        "a :- not b. b :- not c. c. #minimize {1@7,k:a;1@9,k:b}.",
        "b. {a:not b}. #minimize {1@7,k:a}.",
        "{a:not not b}. #minimize {1@7,k:a}.",
    ] {
        let (program, _) = source.split_once("#minimize").expect("objective fixture");
        admitted(program);
        let error = admit_formula(
            source.to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .expect_err("objective dependency profile must remain explicit");
        assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
                    feature: ProfileFeature::ObjectiveNegativeDependency,
                    ..
                }))
            ),
            "{source}: {error}"
        );
        assert!(!error.diagnostics().is_empty());
    }
}

#[test]
fn objective_dependency_restrictions_do_not_reject_negative_constraints() {
    for (source, model) in [
        ("{a}. :- not a. #minimize {1@7,k:a}.", "a"),
        ("{a}. :- a. #minimize {1@7,k:a}.", ""),
    ] {
        let input = admitted(source);
        assert!(input.objectives().is_present());
        assert_eq!(input.objectives().priorities(), &[7]);
        assert_eq!(exhaustive(&input), expected(&[model]));
    }
}
