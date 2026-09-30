//! Finite source-to-Ferraris admission preserves support, scopes, and evidence.

use crate::support::atom_models::Models;
use crate::support::objective_boundaries;

use std::collections::BTreeSet;
use std::fs;

use zetesis_clingo_support as oracle;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::Theory;
use zetesis_reference_support::formula;
use zetesis_sat::{Limits, StableModels};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, BundleAdmissionOptions, BundleLimits, ExpansionFailure,
    ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource, ProfileFeature, SourceBundle,
    admit_bundle_formula, admit_extended, admit_formula,
};

fn models(theory: &Theory, atoms: zetesis_core::catalog::Atoms<'_>) -> Models {
    let mut search = StableModels::new(theory, Limits::default(), Cancellation::default())
        .expect("bounded search");
    let models = search
        .by_ref()
        .map(|model| {
            model
                .expect("complete native search")
                .atoms()
                .map(|index| {
                    atoms
                        .at(index)
                        .unwrap()
                        .to_atom(zetesis_core::ValueLimits::default())
                        .unwrap()
                })
                .collect()
        })
        .collect();
    assert!(search.exhausted(), "full stable model set exhausted");
    models
}
fn native(source: &str) -> Models {
    let admitted = formula(source);
    models(admitted.theory(), admitted.atoms())
}
fn expected(sources: &[&str]) -> Models {
    sources.iter().flat_map(|source| native(source)).collect()
}

#[test]
fn recursive_eligibility_and_bounds_do_not_create_support() {
    for source in [
        "1{a:a}1.",
        "1{a:b}1. b :- a.",
        "1{a:not b}1. b :- a.",
        "1{a:not not a}1. :- a.",
        "1{}1.",
    ] {
        assert!(native(source).is_empty(), "{source}");
    }
    assert_eq!(native("{a:a}."), expected(&[""]));
    assert_eq!(native("1{a:not not a}1."), expected(&["a."]));
}

#[test]
fn duplicate_head_atoms_combine_eligibility_and_count_once() {
    assert_eq!(native("1{a:b;a:c}1. b. c."), expected(&["a. b. c."]));
    assert_eq!(
        native("1{a:b;a:c;d}1. b. c."),
        expected(&["a. b. c.", "b. c. d."])
    );
    assert_eq!(
        native("1{a:b;a:c}1. {b}. {c}."),
        expected(&["a. b.", "a. c.", "a. b. c."])
    );
    assert_eq!(native("1{a:d(X)}1. d(1..2)."), expected(&["a. d(1..2)."]));
}

#[test]
fn global_and_per_element_local_variables_have_distinct_scopes() {
    assert_eq!(
        native("d(1..2). e(3). 1{a(X):d(X);b(X):e(X)}1."),
        expected(&[
            "d(1..2). e(3). a(1).",
            "d(1..2). e(3). a(2).",
            "d(1..2). e(3). b(3)."
        ])
    );
    assert_eq!(native("d(1..2). 1{a(X,Y):d(Y)}1 :- d(X).").len(), 4);
    assert_eq!(native("d(1..2). 1{a:d(_)}1."), expected(&["d(1..2). a."]));
    assert_eq!(
        native("d(1..2). 1{a(X):d(X),X!=2}1."),
        expected(&["d(1..2). a(1)."])
    );
    assert_eq!(native("d(1). 1{a(X+1):d(X)}1."), expected(&["d(1). a(2)."]));
}

#[test]
fn unsafe_scopes_and_unsupported_body_or_choice_expressions_are_refused() {
    for source in [
        "1{a(X):not b(X)}1.",
        "1{a(X):d(Y)}1.",
        "1{a(_):d(_)}1.",
        "1{a(X):d(X)}1 :- not q(X).",
        "a :- d(X+1).",
    ] {
        assert!(
            admit_formula(
                source.to_owned(),
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                FormulaLimits::default()
            )
            .is_err(),
            "{source}"
        );
    }
}

#[test]
fn bound_arithmetic_comparisons_are_tests_and_unbound_inequalities_remain_unsafe() {
    assert_eq!(
        native("d(1..3). p(X) :- d(X), X*X-1=3. q(X) :- d(X), X<2."),
        expected(&["d(1..3). p(2). q(1)."])
    );
    assert_eq!(
        native("d(-2..2). p(X) :- d(X), |X|>=2."),
        expected(&["d(-2..2). p(-2). p(2)."])
    );
    assert!(matches!(
        admit_formula(
            "p(X) :- X<1.".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default()
        ),
        Err(FormulaFailure::UnsafeVariable { .. })
    ));
}

#[test]
fn existing_scalar_fact_and_normal_rule_profile_keeps_its_models() {
    for source in [
        "",
        "a :- not b. b :- not a.",
        "{a}. b :- not not a.",
        "#const n=base+1. #const base=2. d(1..n). {q(X)} :- d(X).",
        "p(3..1).",
        "p((1;2),a;3,b).",
        r#"p("a b"). {q(X)} :- p(X)."#,
    ] {
        let plain = admit_extended(
            source.to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
        )
        .expect("existing extended profile");
        let cancellation = Cancellation::default();
        let expected: Models = zetesis_cpu::Candidates::new(
            plain.program(),
            zetesis_cpu::CandidateLimits::default(),
            cancellation.clone(),
        )
        .filter_map(|seed| {
            let checked = zetesis_cpu::check(
                plain.program(),
                &seed.expect("candidate"),
                zetesis_cpu::Limits::default(),
                &cancellation,
            )
            .expect("closure");
            checked.accepted().then(|| {
                checked
                    .closure()
                    .atoms()
                    .iter()
                    .map(|atom| atom.to_atom(zetesis_core::ValueLimits::default()).unwrap())
                    .collect::<BTreeSet<_>>()
            })
        })
        .collect();
        assert_eq!(native(source), expected, "{source}");
    }
}

#[test]
fn supportedness_guards_bound_classical_transitive_candidates_without_changing_closure() {
    assert_eq!(
        native("edge(1,2;2,3;3,4). reach(X,Y) :- edge(X,Y). reach(X,Z) :- reach(X,Y), edge(Y,Z)."),
        expected(&["edge(1,2;2,3;3,4). reach(1,2;1,3;1,4;2,3;2,4;3,4)."])
    );
    assert_eq!(native("a :- b. b :- a."), expected(&[""]));
}

#[test]
fn strong_negated_choices_supply_signed_atoms() {
    assert_eq!(native("1{-a}1."), expected(&["-a."]));
}

#[test]
fn closed_comparison_chains_supply_local_bindings() {
    // Closed comparison chains now supply a finite local binding, while the
    // complete original guard is retained. This is an admitted source upgrade.
    assert_eq!(native("a :- 1<X<3."), expected(&["a."]));
}

#[test]
fn extended_profile_refuses_bounded_choices() {
    assert!(
        admit_extended(
            "1{a;b}1.".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default()
        )
        .is_err()
    );
}

#[test]
fn signed_choice_bounds_do_not_create_support() {
    assert_eq!(native("1{not a}1."), Models::from([BTreeSet::new()]));
    assert!(native("1{not not a}1.").is_empty());
}

#[test]
fn negative_maximize_preserves_scored_answers() {
    objective_boundaries::check("#maximize{1:not a}.");
}

#[test]
fn conditional_disjuncts_preserve_the_complete_source_family() {
    for (source, expected_sources) in [
        ("a|b:c.", &["a."][..]),
        ("c.a|b:c.", &["a.c.", "b.c."][..]),
        ("{c}.a|b:c.", &["a.", "a.c.", "b.c."][..]),
    ] {
        assert_eq!(native(source), expected(expected_sources), "{source}");
    }
}

#[test]
fn parameterized_program_parts_remain_located_refusals() {
    let error = admit_formula(
        "#program base(x). a.".to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
            feature: ProfileFeature::ProgramPart,
            ..
        }))
    ));
    assert!(!error.diagnostics().is_empty());
}

#[test]
fn limit_diagnostics_report_required_counts() {
    let error = admit_formula(
        "a. b.".to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits {
            theory: zetesis_ferraris::AdmissionLimits {
                max_atoms: 1,
                ..zetesis_ferraris::AdmissionLimits::default()
            },
            ..FormulaLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "formula Atoms limit 1 exceeded (needed at least 2); raise the applicable resource limit or simplify the program"
    );
}

#[test]
fn each_formula_resource_refuses_without_returning_a_partial_theory() {
    let cases = [
        (
            FormulaLimits {
                max_domain_values: 1,
                ..FormulaLimits::default()
            },
            FormulaResource::DomainValues,
        ),
        (
            FormulaLimits {
                max_substitutions: 1,
                ..FormulaLimits::default()
            },
            FormulaResource::Substitutions,
        ),
        (
            FormulaLimits {
                max_work: 1,
                ..FormulaLimits::default()
            },
            FormulaResource::Work,
        ),
        (
            FormulaLimits {
                max_origin_locations: 1,
                ..FormulaLimits::default()
            },
            FormulaResource::Origins,
        ),
        (
            FormulaLimits {
                theory: zetesis_ferraris::AdmissionLimits {
                    max_atoms: 1,
                    ..zetesis_ferraris::AdmissionLimits::default()
                },
                ..FormulaLimits::default()
            },
            FormulaResource::Atoms,
        ),
        (
            FormulaLimits {
                theory: zetesis_ferraris::AdmissionLimits {
                    max_nodes: 1,
                    ..zetesis_ferraris::AdmissionLimits::default()
                },
                ..FormulaLimits::default()
            },
            FormulaResource::Nodes,
        ),
        (
            FormulaLimits {
                theory: zetesis_ferraris::AdmissionLimits {
                    max_roots: 1,
                    ..zetesis_ferraris::AdmissionLimits::default()
                },
                ..FormulaLimits::default()
            },
            FormulaResource::Roots,
        ),
    ];
    for (limits, expected) in cases {
        let error = admit_formula(
            "d(1..2). 1{a(X):d(X)}1.".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits,
        )
        .expect_err("bounded refusal");
        assert!(
            matches!(error, FormulaFailure::Limit { resource, .. } if resource == expected),
            "{error}"
        );
        assert!(!error.diagnostics().is_empty());
    }
}

#[test]
fn queens_source_admits_eighty_original_atoms_with_output_metadata() {
    let source = include_str!("../../../../examples/correctness/standalone/n-queens/variant-01.lp");
    let admitted = formula(source);
    assert_eq!(admitted.source().text(), source);
    assert_eq!(admitted.atoms().len(), 80);
    assert_eq!(admitted.theory().atom_count(), 80);
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
    assert_eq!(
        admitted
            .atoms()
            .iter()
            .filter(|atom| admitted.metadata().output().includes(*atom))
            .count(),
        64
    );
}

struct Fixture(tempfile::TempDir);
impl Fixture {
    fn new() -> Self {
        Self(tempfile::tempdir().expect("fixture"))
    }
    fn bundle(&self) -> SourceBundle {
        SourceBundle::load(self.0.path().join("entry.lp"), BundleLimits::default())
            .expect("original include graph")
    }
}

#[test]
fn bundle_constants_scopes_metadata_and_failures_keep_original_files() {
    let fixture = Fixture::new();
    fs::write(
        fixture.0.path().join("entry.lp"),
        "#include \"data.lp\". 1{a(X):d(X)}1. #show a/1.",
    )
    .expect("entry");
    fs::write(
        fixture.0.path().join("data.lp"),
        "#const n=2. d(1..n). #defined a/1.",
    )
    .expect("included data");
    let admitted = admit_bundle_formula(
        fixture.bundle(),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .expect("formula bundle");
    assert_eq!(models(admitted.theory(), admitted.atoms()).len(), 2);
    assert_eq!(admitted.metadata().directives().len(), 2);
    let sources: BTreeSet<_> = admitted
        .formula_origins()
        .iter()
        .flatten()
        .map(|origin| origin.source)
        .collect();
    assert_eq!(sources.len(), 2);
    for origin in admitted.formula_origins().iter().flatten() {
        assert!(
            admitted
                .bundle()
                .get(origin.source)
                .expect("origin file")
                .source()
                .slice(origin.span)
                .is_ok()
        );
    }
    fs::write(fixture.0.path().join("data.lp"), "d(1). 1{b(X):not d(X)}1.")
        .expect("unsafe included source");
    let error = admit_bundle_formula(
        fixture.bundle(),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .expect_err("unsafe included source");
    let FormulaFailure::UnsafeVariable { location, .. } = error.error() else {
        panic!("{error}");
    };
    assert!(
        error
            .bundle()
            .get(location.source)
            .expect("retained original file")
            .path()
            .ends_with("data.lp")
    );
}

fn clingo(source: &str) -> Models {
    let run = oracle::run_accepting(
        source,
        &["--outf=2", "--models=0", "--warn=none"],
        &[0, 10, 20, 30],
        oracle::Limits::default(),
    );
    let json = oracle::json(&run);
    assert_eq!(json["Models"]["More"], "no", "oracle exhausted");
    let mut models = BTreeSet::new();
    for call in json["Call"].as_array().expect("calls") {
        if let Some(witnesses) = call["Witnesses"].as_array() {
            for witness in witnesses {
                let mut facts = String::new();
                for atom in witness["Value"].as_array().expect("atoms") {
                    facts.push_str(atom.as_str().expect("atom identity"));
                    facts.push_str(".\n");
                }
                let admitted = admit_extended(
                    facts,
                    AdmissionOptions::default(),
                    ExpansionLimits::default(),
                )
                .expect("original scalar atom identities");
                let model = admitted
                    .program()
                    .templates()
                    .iter()
                    .map(|template| {
                        template
                            .head()
                            .expect("fact")
                            .key(&[] as &[zetesis_core::Value])
                            .expect("ground source pattern")
                            .to_atom(zetesis_core::ValueLimits::default())
                            .expect("ground")
                    })
                    .collect();
                assert!(models.insert(model));
            }
        }
    }
    models
}

#[test]
#[ignore = "requires an independently installed clingo 5.8.x oracle"]
fn conditional_formula_models_match_exhausted_clingo_on_deterministic_cases() {
    let mut sources: Vec<String> = [
        "1{a:a}1.",
        "1{a:b}1. b :- a.",
        "1{a:not b}1. b :- a.",
        "1{a:not not a}1.",
        "1{}1.",
        "{a:a}.",
        "d(1..2). 1{a:d(X)}1.",
        "d(1..2). e(3). 1{a(X):d(X);b(X):e(X)}1.",
        "d(1..2). 1{a(X,Y):d(Y)}1 :- d(X).",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    for mask in 0..8 {
        let facts: String = ["b.", "c.", "d."]
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .map(|(_, fact)| *fact)
            .collect();
        for head in ["a:b;a:c;e:d", "a:not b;a:c;e:not d", "a:b;a:not not c;e:d"] {
            sources.push(format!("{facts} 1{{{head}}}1."));
            sources.push(format!("{facts} {{{head}}}."));
        }
    }
    for size in 1..=3 {
        for cutoff in -1..=4 {
            sources.push(format!("d(1..{size}). 1{{a(X):d(X),X*X-X>={cutoff}}}1."));
        }
    }
    for source in sources {
        assert_eq!(native(&source), clingo(&source), "{source}");
    }
}
