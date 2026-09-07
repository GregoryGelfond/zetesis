//! Constructed values remain data in original and frozen source interpretations.
#[path = "support/finite_bindings.rs"]
mod reference;
use reference::{Models, atom_text, exhaustive, holds, native, values};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits, ExpansionResource,
    FormulaFailure, FormulaLimits, FormulaResource, admit_formula,
};

fn competition_case() -> serde_json::Value {
    include_str!("../../../validation/upstream/clingo-5.8.2/cases.jsonl")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|case| case["id"] == "aspcomp13/aspcomp2013_05/01")
        .unwrap()
}
fn competition() -> String {
    competition_case()["source"].as_str().unwrap().to_owned()
}

fn limited(
    source: &str,
    expansion: ExpansionLimits,
    limits: FormulaLimits,
) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        expansion,
        limits,
    )
}
fn input(source: &str) -> AdmittedFormula {
    limited(source, ExpansionLimits::default(), FormulaLimits::default())
        .unwrap_or_else(|error| panic!("{source}: {error}"))
}
// Handwritten substitutions preserve whole rule bodies and choice-group scope.
const CASES: &[(&str, &str)] = &[
    (
        "d(1).p(g(--f(X),-f(X))) :- d(X).",
        "d(1).p(g(f(1),-f(1))) :- d(1).",
    ),
    ("d(1).p(g((),f(),f,X)):-d(X).", "d(1).p(g((),f,f,1)):-d(1)."),
    ("q:-not p(f(X/0)),d(X).", ""),
    ("p(a).p(f(X)):-p(X),X=a.", "p(a).p(f(a)):-p(a)."),
    (
        "d(1).{p(f(X)):p(f(1)),d(X)}.",
        "d(1).{p(f(1)):p(f(1)),d(1)}.",
    ),
    (
        "d(1).p(f(X)):-d(X).m(M):-M=#min{Y:p(Y)}.",
        "d(1).p(f(1)):-d(1).m(M):-M=#min{Y:p(Y)}.",
    ),
    (
        "p(1).p(X+1):-p(X),X<2,not not p(X+1).",
        "p(1).p(2):-p(1),not not p(2).",
    ),
    ("d(1).{p(f(X)):d(X)}.", "d(1).{p(f(1)):d(1)}."),
    ("d(1).p(f(X)):-d(X).", "d(1).p(f(1)):-d(1)."),
    ("d(1).p(g(f(X),X+1)):-d(X).", "d(1).p(g(f(1),2)):-d(1)."),
    ("d(f(1)).p(g(X)):-d(X).", "d(f(1)).p(g(f(1))):-d(f(1))."),
    ("d(1).p(-f(X)):-d(X).", "d(1).p(-f(1)):-d(1)."),
    ("d(1).p(--f(X)):-d(X).", "d(1).p(f(1)):-d(1)."),
    ("d(1).p((X,X+1)):-d(X).", "d(1).p((1,2)):-d(1)."),
    ("d(1).p((X,)):-d(X).", "d(1).p((1,)):-d(1)."),
    (
        "d(1).1{p(f(X));p(g(X))}1:-d(X).",
        "d(1).1{p(f(1));p(g(1))}1:-d(1).",
    ),
    (
        "d(1).{q}.1{p(f(X)):d(X);p(f(1)):q}1.",
        "d(1).{q}.1{p(f(1)):d(1);p(f(1)):q}1.",
    ),
    (
        "d(1..2).1{p(f(X),Y):Y=1..2}1:-d(X).",
        "d(1..2).1{p(f(1),1);p(f(1),2)}1:-d(1).1{p(f(2),1);p(f(2),2)}1:-d(2).",
    ),
    ("d(1).p(f(X))|q(X):-d(X).", "d(1).p(f(1))|q(1):-d(1)."),
    (
        "d(1).{p(f(1))}.q:-d(X),not p(f(X)).",
        "d(1).{p(f(1))}.q:-d(1),not p(f(1)).",
    ),
    (
        "d(1).{p(f(1))}.q:-not p(f(X)),d(X).",
        "d(1).{p(f(1))}.q:-not p(f(1)),d(1).",
    ),
    (
        "d(1).{p(f(1))}.q:-d(X),not not p(f(X)).",
        "d(1).{p(f(1))}.q:-d(1),not not p(f(1)).",
    ),
    (
        "d(1).q:-d(X),not p(f(X),X+1).",
        "d(1).q:-d(1),not p(f(1),2).",
    ),
    (
        "d(1).{-p(f(1))}.q:-not -p(f(X)),d(X).",
        "d(1).{-p(f(1))}.q:-not -p(f(1)),d(1).",
    ),
    ("d(1).{q:not p(f(X)),d(X)}.", "d(1).{q:not p(f(1)),d(1)}."),
    ("1{p(f(X)):d(X)}1.", "1{}1."),
    ("{p(f(X)):d(X)}.", "{} ."),
    (
        "d(1).p(f(X);g(X)):-d(X).",
        "d(1).p(f(1)):-d(1).p(g(1)):-d(1).",
    ),
    ("d(1).p(Y):-d(X),Y=f(X).", "d(1).p(f(1)):-d(1)."),
];

#[test]
fn constructed_models_match_explicit_substitution() {
    for &(source, expanded) in CASES {
        let source = input(source);
        let expanded = input(expanded);
        assert_eq!(native(&source), native(&expanded));
        assert_eq!(native(&source), exhaustive(&source));
    }
}

#[test]
fn construction_preserves_every_frozen_pair() {
    let mut pairs = 0;
    for &(source, expanded) in CASES {
        let source = input(source);
        let expanded = input(expanded);
        let atoms: BTreeSet<_> = source
            .atoms()
            .iter()
            .chain(expanded.atoms())
            .map(atom_text)
            .collect();
        assert!(atoms.len() <= 7);
        let names: Vec<_> = atoms.into_iter().collect();
        let mask = |bits: usize, admitted: &AdmittedFormula| {
            admitted
                .atoms()
                .iter()
                .enumerate()
                .fold(0, |result, (i, atom)| {
                    result
                        | (usize::from(
                            bits & (1
                                << names
                                    .iter()
                                    .position(|name| *name == atom_text(atom))
                                    .unwrap())
                                != 0,
                        ) << i)
                })
        };
        for outer in 0..1 << names.len() {
            let a = values(source.theory(), mask(outer, &source), None);
            let b = values(expanded.theory(), mask(outer, &expanded), None);
            assert_eq!(holds(source.theory(), &a), holds(expanded.theory(), &b));
            for inner in 0..1 << names.len() {
                assert_eq!(
                    holds(
                        source.theory(),
                        &values(source.theory(), mask(inner, &source), Some(&a))
                    ),
                    holds(
                        expanded.theory(),
                        &values(expanded.theory(), mask(inner, &expanded), Some(&b))
                    )
                );
                pairs += 1;
            }
        }
    }
    println!("frozen_pairs={pairs}");
    assert!(pairs > 4_000);
}

#[test]
fn competition_models_match_retained_reference() {
    let models = native(&input(&competition()));
    assert_eq!(models.len(), 12);
    assert!(models.iter().all(|model| model.len() == 17));
    let expected: Models = competition_case()["models"]
        .as_array()
        .unwrap()
        .iter()
        .map(|model| {
            model
                .as_array()
                .unwrap()
                .iter()
                .map(|atom| atom.as_str().unwrap().to_owned())
                .collect()
        })
        .collect();
    assert_eq!(models, expected);
}

#[test]
fn negative_arguments_do_not_bind_inputs() {
    for source in [
        "q:-not p(f(X)).",
        "q:-not p(f(X)),Z=2..1.",
        "{p(f(Y)):X=2..1}.",
        "q:-not not p(X+1).",
        "d(1).q:-d(X),not p(f(Y)).",
        "{p(f(X))}.",
    ] {
        assert!(
            matches!(
                limited(source, ExpansionLimits::default(), FormulaLimits::default()),
                Err(FormulaFailure::UnsafeVariable { .. })
            ),
            "{source}"
        );
    }
}

#[test]
fn anonymous_constructor_facts_remain_refused() {
    assert!(
        limited(
            "p(f(_)).",
            ExpansionLimits::default(),
            FormulaLimits::default()
        )
        .is_err()
    );
}

#[test]
fn scalar_failures_remain_admission_errors() {
    for source in [
        "d(a).p(f(X+1)):-d(X).",
        "d(1).q:-not p(f(X/0)),d(X).",
        "d(0).q:-d(X),not p(1/X).",
        "d(2147483647).p(f(X+1)):-d(X).",
    ] {
        assert!(
            matches!(
                limited(source, ExpansionLimits::default(), FormulaLimits::default()),
                Err(FormulaFailure::Expansion(
                    ExpansionFailure::Evaluation { .. }
                ))
            ),
            "{source}"
        );
    }
}

#[test]
fn unsupported_value_families_remain_refused() {
    for source in [
        "d(1).p(f(X..X+1)):-d(X).",
        "d(1).q:-d(X),not p(f(X;2)).",
        "p(f(N)):-N=#count{}.",
    ] {
        assert!(
            limited(source, ExpansionLimits::default(), FormulaLimits::default()).is_err(),
            "{source}"
        );
    }
}

#[test]
fn recursive_construction_cannot_finish_truncated() {
    let source = "p(0).p(f(X)):-p(X).";
    let limits = FormulaLimits {
        max_support_rounds: 3,
        ..FormulaLimits::default()
    };
    assert!(matches!(
        limited(source, ExpansionLimits::default(), limits),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportRounds,
            ..
        })
    ));
}

#[test]
fn construction_preserves_statement_origins() {
    let admitted = input("d(1).\np(f(X)):-d(X).\n");
    assert_eq!(
        admitted.formula_origins().len(),
        admitted.theory().roots().len()
    );
    assert!(
        admitted
            .formula_origins()
            .iter()
            .all(|origins| !origins.is_empty())
    );
}

#[test]
fn construction_scalar_bytes_have_an_inclusive_limit() {
    let source = "d(1).p(long_constructor_name(X)):-d(X).";
    let mut lower = 0;
    let mut upper = 1 << 20;
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        let expansion = ExpansionLimits {
            max_scalar_bytes: middle,
            ..ExpansionLimits::default()
        };
        if limited(source, expansion, FormulaLimits::default()).is_ok() {
            upper = middle;
        } else {
            lower = middle + 1;
        }
    }
    let expansion = ExpansionLimits {
        max_scalar_bytes: lower,
        ..ExpansionLimits::default()
    };
    assert_eq!(
        native(&limited(source, expansion, FormulaLimits::default()).unwrap()),
        native(&input(source))
    );
    let expansion = ExpansionLimits {
        max_scalar_bytes: lower - 1,
        ..ExpansionLimits::default()
    };
    assert!(matches!(
        limited(source, expansion, FormulaLimits::default()),
        Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::ScalarBytes,
            ..
        }))
    ));
}

#[test]
#[ignore = "requires independently installed clingo"]
fn complete_models_match_clingo() {
    let competition = competition();
    for source in CASES
        .iter()
        .map(|(source, _)| *source)
        .chain([competition.as_str()])
    {
        let result = reference::external(source, true);
        let models: Models = result["Call"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
            .map(|witness| {
                witness["Value"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|atom| atom.as_str().unwrap().to_owned())
                    .collect()
            })
            .collect();
        assert_eq!(native(&input(source)), models, "{source}");
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config { cases: 128, rng_seed: proptest::test_runner::RngSeed::Fixed(20_260_907), ..proptest::test_runner::Config::default() })]
    #[test]
    fn source_parameters_determine_constructor_identity(
        name in "[a-z][a-z0-9_]{0,10}",
        negative in proptest::bool::ANY,
        input_value in -100_i16..=100,
        children in proptest::collection::vec(-100_i16..=100, 0..6),
    ) {
        let sign = if negative { "-" } else { "" };
        let suffix = children.iter().fold(String::new(), |mut text, value| { write!(text, ",{value}").unwrap(); text });
        let source = format!("d({input_value}).p({sign}{name}(X{suffix})):-d(X).");
        let expected: Models = [BTreeSet::from([format!("d({input_value})"), format!("p({sign}{name}({input_value}{suffix}))")])].into_iter().collect();
        proptest::prop_assert_eq!(native(&input(&source)), expected);
    }
}

#[test]
fn competition_matches_bounded_completion() {
    use std::num::NonZeroUsize;
    use zetesis_sat::{
        BatchLimits, BatchVerdict, CompletionExecutor, Control, Limits, StableModels,
    };
    let admitted = input(&competition());
    let expected = native(&admitted);
    for workers in [1, 2] {
        let mut executor = CompletionExecutor::new(NonZeroUsize::new(workers).unwrap()).unwrap();
        let mut search =
            StableModels::new(admitted.theory(), Limits::default(), Control::default()).unwrap();
        let mut models = Models::new();
        while !search.exhausted() {
            let batch = search
                .next_batch_with_completion(
                    BatchLimits {
                        max_candidates: NonZeroUsize::new(4).unwrap(),
                        max_pending_bytes: 1 << 20,
                    },
                    &mut executor,
                    |_, candidates| {
                        Ok::<_, std::convert::Infallible>(vec![
                            BatchVerdict::Residual;
                            candidates.len()
                        ])
                    },
                )
                .unwrap();
            for model in batch {
                assert!(
                    models.insert(
                        model
                            .atoms()
                            .map(|index| atom_text(&admitted.atoms()[index]))
                            .collect()
                    )
                );
            }
        }
        assert_eq!(models, expected);
    }
}

#[test]
fn hidden_constructors_retain_semantic_identity() {
    let source = "d(1).p(f(X)):-d(X).#show.";
    let admitted = input(source);
    assert!(
        admitted
            .atoms()
            .iter()
            .all(|atom| !admitted.metadata().output().includes(atom))
    );
    assert_eq!(native(&admitted), native(&input("d(1).p(f(X)):-d(X).")));
}

#[test]
fn constructed_keys_retain_objective_contributions() {
    let admitted = input("d(1).{p(f(X)):d(X)}.#minimize{1@0,Y:p(Y)}.");
    let models = native(&admitted);
    assert_eq!(models.len(), 2);
    let mut costs = Vec::new();
    for names in models {
        let model = zetesis_core::Model::new(
            admitted
                .atoms()
                .iter()
                .filter(|atom| names.contains(&atom_text(atom)))
                .cloned(),
        );
        let score = zetesis_objective::evaluate(
            admitted.objectives(),
            &model,
            zetesis_objective::Limits::default(),
            &zetesis_cpu::Control::default(),
        )
        .unwrap();
        costs.push(score.score().costs().to_vec());
    }
    costs.sort();
    assert_eq!(costs, vec![vec![(0, 0)], vec![(0, 1)]]);
}

#[test]
fn constructed_assignment_limit_is_inclusive() {
    let source = "d(1..2).p(f(X)):-d(X).";
    let limits = FormulaLimits {
        max_assignment_values: 2,
        ..FormulaLimits::default()
    };
    assert_eq!(
        native(&limited(source, ExpansionLimits::default(), limits).unwrap()),
        native(&input(source))
    );
    let limits = FormulaLimits {
        max_assignment_values: 1,
        ..FormulaLimits::default()
    };
    assert!(matches!(
        limited(source, ExpansionLimits::default(), limits),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::AssignmentValues,
            observed: 2,
            limit: 1,
            ..
        })
    ));
}

#[test]
fn constructor_inputs_count_toward_slot_limit() {
    let source = "d(1).p(f(X)):-d(X).";
    for (slots, succeeds) in [(2, true), (1, false)] {
        let mut options = AdmissionOptions::default();
        options.core_limits.max_variables_per_template = slots;
        let result = admit_formula(
            source.into(),
            options,
            ExpansionLimits::default(),
            FormulaLimits::default(),
        );
        if succeeds {
            assert!(result.is_ok());
        } else {
            assert!(matches!(
                result,
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Variables,
                    observed: 2,
                    limit: 1,
                    ..
                })
            ));
        }
    }
}

#[test]
fn incomplete_constructor_work_returns_no_admission() {
    let limits = FormulaLimits {
        max_work: 0,
        ..FormulaLimits::default()
    };
    assert!(matches!(
        limited("d(1).p(f(X)):-d(X).", ExpansionLimits::default(), limits),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            observed: 1,
            limit: 0,
            ..
        })
    ));
}

#[test]
fn recursive_analysis_cannot_waive_support_limits() {
    let admitted = input("p(a).p(f(X)):-p(X),X=a.");
    assert!(matches!(
        admitted.source_analysis().safety().finiteness(),
        themelios_analysis::Verdict::Unknown { .. }
    ));
    assert_eq!(
        admitted.source_analysis(),
        &themelios_analysis::Analysis::of(admitted.analyzed_program())
    );
}
