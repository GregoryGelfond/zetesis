//! Prepared owners retain semantics independently of source files and execution limits.

use std::collections::BTreeSet;

use crate::support::options::serial_config as config;
use clap::Parser;
use zetesis_algebra::Cancellation;
use zetesis_cli::{
    Completion, Interruption, Options, Original, PreparedInput, PreparedProfile, Session,
    SolveConfig, SolvePhase, run_finalized,
};
use zetesis_core::Model;
use zetesis_ferraris::{PositiveError, PositiveResource};
use zetesis_reference_support::{BundleFixture, formula};
use zetesis_themelios::{
    BundleAdmissionOptions, ExpansionLimits, FormulaLimits, admit_bundle_formula,
};

/// Sources whose entry includes `body` and shows nothing.
fn included_body(body: &str) -> BundleFixture {
    let sources = BundleFixture::new();
    sources.write("entry.lp", "#include \"body.lp\". #show.");
    sources.write("body.lp", body);
    sources
}

fn atoms(model: &Model) -> Vec<String> {
    model
        .atoms()
        .iter()
        .map(|atom| atom.predicate().name().to_string())
        .collect()
}

#[test]
fn formula_bundles_outlive_their_source_files() {
    let sources = included_body("hidden. {a}. #minimize{1@2,k:hidden}.");
    let path = sources.directory().to_path_buf();
    let owner = admit_bundle_formula(
        sources.bundle(),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    drop(sources);
    assert!(!path.exists());

    let input = PreparedInput::formula_bundle(&owner);
    assert_eq!(input.profile(), PreparedProfile::Formula);
    assert!(std::ptr::eq(input.metadata().unwrap(), owner.metadata()));
    let mut session = Session::new(input, config(), Cancellation::default()).unwrap();
    let mut models = BTreeSet::new();
    for result in session.by_ref() {
        let model = result.unwrap();
        let Original::Theory(theory) = model.original() else {
            panic!("formula subject")
        };
        assert!(theory.same_instance(owner.theory()));
        assert_eq!(model.score().unwrap().costs(), &[(2, 1)]);
        models.insert(atoms(model.interpretation()));
    }
    assert_eq!(
        models,
        BTreeSet::from([vec!["hidden".into()], vec!["a".into(), "hidden".into()]])
    );
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    assert_eq!(outcome.incumbent().unwrap().tied_models, 2);
    assert_eq!(outcome.verified_models(), 2);
    assert!(outcome.optimum_proved());
}

#[test]
fn certificate_setup_exhaustion_is_incomplete() {
    let owner = formula("a.");
    // The public candidate stream establishes the exact setup allowance.
    // Spending that allowance cannot fund a subsequent certificate attempt.
    let stream = zetesis_stable::StableModels::new(
        owner.theory(),
        zetesis_stable::Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    let setup_work = stream.statistics().search.work;
    assert!(setup_work > 0);
    let mut session = Session::new(
        PreparedInput::formula(&owner),
        SolveConfig {
            max_search_work: setup_work,
            stats: true,
            ..config()
        },
        Cancellation::default(),
    )
    .unwrap();
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert_eq!(
        outcome.interruption(),
        Some(Interruption::Countermodel(
            zetesis_stable::Incomplete::WorkLimit
        ))
    );
    assert_eq!(outcome.verified_models(), 0);
    assert!(!outcome.unsatisfiable());
    assert!(!outcome.optimum_proved());
    let statistics = outcome.countermodel_statistics().unwrap();
    assert_eq!(statistics.search.work, setup_work);
    assert_eq!(statistics.candidates, 0);
    let certificate = statistics.certified.unwrap();
    assert_eq!(
        certificate.refusal,
        Some(zetesis_stable::CertificateError::Positive(
            PositiveError::Limit {
                resource: PositiveResource::Work,
                observed: 1,
                limit: 0,
            }
        ))
    );
    assert!(certificate.tight_refusal.is_none());
    assert!(certificate.plan.is_none());
    assert_eq!(certificate.construction_work, 0);
    assert_eq!(certificate.checks, 0);
    assert_eq!(
        session
            .phase_timings()
            .unwrap()
            .get(SolvePhase::CertificateSetup)
            .unwrap()
            .calls,
        1
    );
    assert!(session.next().is_none());
    assert!(session.next().is_none());
}

#[test]
fn consumer_stop_leaves_formula_coverage_unknown() {
    let owner = formula("a | b. #show.");
    let mut session = Session::new(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .unwrap();
    let answer = session.next().unwrap().unwrap();
    assert!(
        matches!(atoms(answer.interpretation()).as_slice(), [atom] if atom == "a" || atom == "b")
    );

    let outcome = session.stop();
    let Original::Theory(theory) = outcome.original().unwrap() else {
        panic!("formula subject")
    };
    assert!(theory.same_instance(owner.theory()));
    assert_eq!(outcome.verified_models(), 1);
    assert_eq!(outcome.completion(), None);
    assert_eq!(outcome.interruption(), None);
    assert!(!outcome.unsatisfiable());
    assert!(!outcome.optimum_proved());
}

#[test]
fn raw_model_conversion_preserves_hidden_atoms() {
    let owner = formula("a. hidden. #show a.");
    let answer = Session::new(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .unwrap()
    .next()
    .unwrap()
    .unwrap();
    let original = answer.interpretation().clone();
    let detached = answer.into_interpretation();
    assert_eq!(detached, original);
    assert_eq!(atoms(&detached), ["a", "hidden"]);
}

#[test]
fn silent_diagnostics_preserve_finalized_evidence() {
    let options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--workers",
        "1",
        "--models",
        "0",
        "--color",
        "never",
    ])
    .unwrap();
    let mut output = Vec::new();
    let result =
        run_finalized("a.".into(), &options, &mut output, &Cancellation::default()).unwrap();
    let report = result.report().unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!(report.models, 1);
    assert_eq!(result.semantic().verified_models(), 1);
    assert_eq!(result.publication().models(), 1);
    assert!(result.publication().summary());
    let text = String::from_utf8(output).unwrap();
    assert!(text.starts_with(crate::support::human::banner()));
    assert!(text.contains("Answer: 1\na\nSATISFIABLE\nModels: 1\n\nTime:"));
}
