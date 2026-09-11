//! Completed priority presence for flat extrema observers.

#[path = "support/source_records.rs"]
mod source_records;
#[path = "support/source_cases.rs"]
mod source_cases;
#[path = "support/source_oracle.rs"]
mod source_oracle;

use source_cases::cases;
use source_records::{admit, exhaustive};
use zetesis_themelios::{
    AdmissionFailure, ExpansionFailure, FormulaFailure, FormulaLimits, FormulaResource,
    ProfileFeature,
};

const HISTORICAL: &str = include_str!("fixtures/objective-extrema-refusals.jsonl");
const CONTROLS: &str = include_str!("fixtures/objective-flat-presence.jsonl");

fn flat_cases() -> Vec<source_cases::Case> {
    cases(HISTORICAL)
        .into_iter()
        .take(7)
        .chain(cases(CONTROLS))
        .collect()
}

#[test]
fn flat_extrema_preserve_full_model_cost_records() {
    for case in flat_cases() {
        let input = admit(&case.source, &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        assert_eq!(exhaustive(&input), case.records, "{}", case.name);
    }
}

#[test]
fn presence_keeps_the_original_reduct_subject() {
    for program in [
        "b.{a}.n(N):-N=#max{2:a;foo:b}.p(X):-n(X).",
        "{a}.n(N):-N=#max{2:a;foo:a}.p(X):-n(X).",
        "b.{a}.n(N):-N=#min{2:b;foo:a}.p(X):-n(X).",
    ] {
        let original = admit(program, &FormulaLimits::default()).unwrap();
        let observed = admit(
            &format!("{program}#minimize{{X@7:p(X)}}."),
            &FormulaLimits::default(),
        )
        .unwrap();
        // Node identity gives the same truth at every original and frozen pair,
        // including unrealized proposals and candidates rejected by the reduct.
        assert_eq!(observed.atoms(), original.atoms());
        assert_eq!(observed.theory().nodes(), original.theory().nodes());
        assert_eq!(observed.theory().roots(), original.theory().roots());
        assert_eq!(observed.formula_origins(), original.formula_origins());
        assert!(
            observed
                .atoms()
                .iter()
                .any(|atom| source_records::canonical(atom) == "n(2)")
        );
    }
}

#[test]
fn presence_completion_respects_inclusive_limits() {
    let source =
        "b.{a}.n(N):-N=#max{2:a;foo:b}.p(X):-n(X).q(Y):-p(Y).#minimize{Y@7:q(Y);0@3:q(Z)}.";
    let expected = exhaustive(&admit(source, &FormulaLimits::default()).unwrap());
    for resource in [
        FormulaResource::Work,
        FormulaResource::Substitutions,
        FormulaResource::ObjectivePresenceEntries,
    ] {
        let attempt = |maximum| {
            let mut limits = FormulaLimits::default();
            if resource == FormulaResource::Work {
                limits.max_work = maximum;
            } else if resource == FormulaResource::Substitutions {
                limits.max_substitutions = maximum;
            } else {
                limits.max_objective_presence_entries = usize::try_from(maximum).unwrap();
            }
            admit(source, &limits)
        };
        let (mut lower, mut upper) = (0, 65_536);
        assert!(attempt(upper).is_ok());
        while lower + 1 < upper {
            let middle = lower + (upper - lower) / 2;
            if attempt(middle).is_ok() {
                upper = middle;
            } else {
                lower = middle;
            }
        }
        assert_eq!(exhaustive(&attempt(upper).unwrap()), expected);
        let error = attempt(upper - 1).unwrap_err();
        assert!(!error.diagnostics().is_empty());
        assert!(
            matches!(error, FormulaFailure::Limit {resource: actual, observed, limit, ..}
            if actual == resource && observed == u128::from(upper) && limit == u128::from(upper - 1))
        );
        assert_eq!(exhaustive(&attempt(upper).unwrap()), expected);
    }
}

#[test]
fn distinct_carriers_share_the_storage_allowance() {
    let source = "b.{a}.n(N):-N=#max{2:a;foo:b}.p(X):-n(X).m(M):-M=#max{4:a;bar:b}.#minimize{X@7:p(X);Y@3:m(Y);0@1:n(Z)}.";
    let expected = exhaustive(&admit(source, &FormulaLimits::default()).unwrap());
    let attempt = |maximum| {
        admit(
            source,
            &FormulaLimits {
                max_objective_presence_entries: maximum,
                ..Default::default()
            },
        )
    };
    let exact = (1..128).find(|&maximum| attempt(maximum).is_ok()).unwrap();
    assert_eq!(exhaustive(&attempt(exact).unwrap()), expected);
    assert!(
        matches!(attempt(exact-1), Err(FormulaFailure::Limit {resource:FormulaResource::ObjectivePresenceEntries, observed, limit, ..}) if observed == exact as u128 && limit == (exact-1) as u128)
    );
    assert_eq!(exhaustive(&attempt(exact).unwrap()), expected);
}

#[test]
fn excluded_weights_do_not_hide_endpoint_refusals() {
    let source = "b.{a}.n(N):-N=#max{(-2147483647-1):a;foo:b}.#maximize{X@7:n(X)}.";
    let error = admit(source, &FormulaLimits::default()).unwrap_err();
    assert!(!error.diagnostics().is_empty());
    assert!(
        matches!(
            error,
            FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
                feature: ProfileFeature::Aggregate,
                ..
            }))
        ),
        "{error:?}"
    );
}

#[test]
fn zero_presence_storage_refuses_a_mixed_certificate() {
    let limits = FormulaLimits {
        max_objective_presence_entries: 0,
        ..Default::default()
    };
    let error = admit(
        "b.{a}.n(N):-N=#max{2:a;foo:b}.#minimize{X@7:n(X)}.",
        &limits,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        FormulaFailure::Limit {
            resource: FormulaResource::ObjectivePresenceEntries,
            observed: 1,
            limit: 0,
            ..
        }
    ));
}

#[test]
fn homogeneous_carriers_need_no_presence_storage() {
    for source in [
        "{a}.n(N):-N=#max{2:a}.#minimize{X@7:n(X)}.",
        "a.#minimize{0:a}.",
    ] {
        let limits = FormulaLimits {
            max_objective_presence_entries: 0,
            ..Default::default()
        };
        assert_eq!(
            exhaustive(&admit(source, &limits).unwrap()),
            exhaustive(&admit(source, &FormulaLimits::default()).unwrap())
        );
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config { cases: 128, ..Default::default() })]

    #[test]
    fn optional_correlation_retains_numeric_presence(weight in -8_i32..9, priority in -3_i32..4) {
        let source = format!("{{a}}.n(N):-N=#max{{{weight}:a;foo:a}}.#minimize{{X@{priority}:n(X)}}.");
        let input = admit(&source, &FormulaLimits::default()).unwrap();
        proptest::prop_assert_eq!(input.objectives().priorities(), &[priority]);
        let records = exhaustive(&input);
        proptest::prop_assert_eq!(records.len(), 2);
        proptest::prop_assert!(records.iter().all(|(_, costs)| costs.as_deref() == Some(&[0])));
    }

    #[test]
    fn required_symbol_excludes_only_its_weight(weight in -8_i32..9, priority in -3_i32..4) {
        let source = format!("b.{{a}}.n(N):-N=#max{{{weight}:a;foo:b}}.#minimize{{X@{priority}:n(X);0@{},k:n(Y)}}.", priority-1);
        let input = admit(&source, &FormulaLimits::default()).unwrap();
        proptest::prop_assert_eq!(input.objectives().priorities(), &[priority-1]);
        let records = exhaustive(&input);
        proptest::prop_assert_eq!(records.len(), 2);
        proptest::prop_assert!(records.iter().all(|(_, costs)| costs.as_deref() == Some(&[0])));
    }
}

#[test]
#[ignore = "requires independent clingo for complete original records"]
fn flat_presence_sources_match_fresh_clingo() {
    for case in flat_cases() {
        let capture = source_oracle::capture(&case.source);
        let output = source_oracle::output(&capture);
        let records = source_oracle::model_records(&output);
        assert_eq!(records, case.records, "{}", case.name);
        println!(
            "reference_json: {}",
            serde_json::json!({
                "name":case.name,"source":case.source,"stdout":output,
                "stderr":std::str::from_utf8(&capture.stderr).expect("UTF-8 oracle diagnostics"),"status":capture.status.code().expect("normal oracle exit checked"),
                "arguments":["0","--outf=2","--opt-mode=enum","--warn=none"]
            })
        );
        assert_eq!(source_oracle::records(&case.source), case.records);
    }
}
