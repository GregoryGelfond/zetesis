//! Prepared owners retain semantics independently of source files and execution limits.

use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    num::NonZeroUsize,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use clap::Parser;
use zetesis_cli::{
    Backend, Completion, Grounder, Interruption, Options, PreparedInput, PreparedProfile, Session,
    SolveConfig, SolveError, SolvePhase, Subject, run_finalized,
};
use zetesis_core::{Model, StaticError};
use zetesis_cpu::Control;
use zetesis_ferraris::{PositiveError, PositiveResource};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, BundleAdmissionOptions, BundleLimits, ExpansionLimits,
    FormulaLimits, SourceBundle, admit_bundle_extended, admit_bundle_formula, admit_extended,
    admit_formula,
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
const MAX_DIRECTORY_ATTEMPTS: usize = 16;

struct Sources(PathBuf);

impl Sources {
    fn new(body: &str) -> Self {
        let directory = Self::directory();
        fs::write(directory.0.join("entry.lp"), "#include \"body.lp\". #show.").unwrap();
        fs::write(directory.0.join("body.lp"), body).unwrap();
        directory
    }

    fn directory() -> Self {
        for _ in 0..MAX_DIRECTORY_ATTEMPTS {
            let serial = NEXT_DIRECTORY
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    value.checked_add(1)
                })
                .expect("temporary source serial exhausted");
            let path = std::env::temp_dir().join(format!(
                "zetesis-prepared-boundaries-{}-{serial}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("temporary source directory: {error}"),
            }
        }
        panic!("temporary source collision limit reached")
    }

    fn load(&self) -> SourceBundle {
        SourceBundle::load(self.0.join("entry.lp"), BundleLimits::default()).unwrap()
    }
}

impl Drop for Sources {
    fn drop(&mut self) {
        // This fixture owns only the directory it created successfully.
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        workers: NonZeroUsize::MIN,
        models: 0,
        ..Default::default()
    }
}

fn formula(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}

fn atoms(model: &Model) -> Vec<String> {
    model
        .atoms()
        .iter()
        .map(|atom| atom.predicate().name().to_string())
        .collect()
}

#[test]
fn relational_bundles_outlive_their_source_files() {
    let sources = Sources::new("a. {b}.");
    let path = sources.0.clone();
    let owner = admit_bundle_extended(
        sources.load(),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap();
    drop(sources);
    assert!(!path.exists());

    let input = PreparedInput::bundle(&owner);
    assert_eq!(input.profile(), PreparedProfile::Relational);
    assert!(std::ptr::eq(input.metadata().unwrap(), owner.metadata()));
    let mut session = Session::new(input, config(), Control::default()).unwrap();
    let mut models = BTreeSet::new();
    for result in session.by_ref() {
        let model = result.unwrap();
        let Subject::Program(program) = model.subject() else {
            panic!("relational subject")
        };
        assert!(program.same_instance(owner.program()));
        models.insert(atoms(model.interpretation()));
    }
    assert_eq!(
        models,
        BTreeSet::from([vec!["a".into()], vec!["a".into(), "b".into()]])
    );
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    assert_eq!(outcome.verified_models(), 2);
    assert!(!outcome.unsatisfiable());
}

#[test]
fn formula_bundles_outlive_their_source_files() {
    let sources = Sources::new("hidden. {a}. #minimize{1@2,k:hidden}.");
    let path = sources.0.clone();
    let owner = admit_bundle_formula(
        sources.load(),
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
    let mut session = Session::new(input, config(), Control::default()).unwrap();
    let mut models = BTreeSet::new();
    for result in session.by_ref() {
        let model = result.unwrap();
        let Subject::Theory(theory) = model.subject() else {
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
    // The public candidate stream establishes the exact encoding allowance.
    // Spending that allowance cannot fund a subsequent certificate attempt.
    let encoded = zetesis_sat::StableModels::new(
        owner.theory(),
        zetesis_sat::Limits::default(),
        Control::default(),
    )
    .unwrap();
    let encoding_work = encoded.statistics().search.work;
    assert!(encoding_work > 0);
    let mut session = Session::new(
        PreparedInput::formula(&owner),
        SolveConfig {
            max_search_work: encoding_work,
            stats: true,
            ..config()
        },
        Control::default(),
    )
    .unwrap();
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert_eq!(
        outcome.interruption(),
        Some(Interruption::Countermodel(
            zetesis_sat::Incomplete::WorkLimit
        ))
    );
    assert_eq!(outcome.verified_models(), 0);
    assert!(!outcome.unsatisfiable());
    assert!(!outcome.optimum_proved());
    let statistics = outcome.countermodel_statistics().unwrap();
    assert_eq!(statistics.search.work, encoding_work);
    assert_eq!(statistics.candidate_queries, 0);
    let certificate = statistics.certified.unwrap();
    assert_eq!(
        certificate.refusal,
        Some(zetesis_sat::CertificateError::Positive(
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
    let mut session =
        Session::new(PreparedInput::formula(&owner), config(), Control::default()).unwrap();
    let answer = session.next().unwrap().unwrap();
    assert!(
        matches!(atoms(answer.interpretation()).as_slice(), [atom] if atom == "a" || atom == "b")
    );

    let outcome = session.stop();
    let Subject::Theory(theory) = outcome.subject().unwrap() else {
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
    let answer = Session::new(PreparedInput::formula(&owner), config(), Control::default())
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
fn eager_setup_refusal_leaves_the_owner_reusable() {
    let owner = admit_extended(
        "a. b :- a.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap();
    let input = PreparedInput::admitted(&owner);
    let failure = Session::new(
        input,
        SolveConfig {
            grounder: Grounder::Eager,
            max_ground_rules: 0,
            stats: true,
            ..config()
        },
        Control::default(),
    )
    .err()
    .unwrap();
    let zetesis_cli::SolveError::Static(error) = failure.cause.as_ref() else {
        panic!("expected static materialization refusal: {failure}")
    };
    assert!(
        matches!(error, StaticError::LimitExceeded { resource: "ground rules", limit: 0, actual } if *actual > 0)
    );
    assert_eq!(
        failure
            .source()
            .unwrap()
            .downcast_ref::<SolveError>()
            .unwrap()
            .source()
            .unwrap()
            .downcast_ref::<StaticError>(),
        Some(error)
    );
    assert!(failure.semantic().is_none());
    assert!(
        failure
            .phase_timings
            .as_ref()
            .unwrap()
            .get(SolvePhase::ExecutionSetup)
            .is_some()
    );

    let mut session = Session::new(
        input,
        SolveConfig {
            grounder: Grounder::Eager,
            ..config()
        },
        Control::default(),
    )
    .unwrap();
    let answer = session.next().unwrap().unwrap();
    assert_eq!(atoms(answer.interpretation()), ["a", "b"]);
    assert!(session.next().is_none());
    assert_eq!(
        session.outcome().unwrap().completion(),
        Some(Completion::Exhausted)
    );
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
    let result = run_finalized("a.".into(), &options, &mut output, &Control::default()).unwrap();
    let report = result.report().unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!(report.models, 1);
    assert_eq!(result.semantic().verified_models(), 1);
    assert_eq!(result.publication().models(), 1);
    assert!(result.publication().summary());
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "Answer: 1\na\nSATISFIABLE\nCoverage: exhausted\nModels: 1; candidates examined: 1; gate tuples discovered: 0\n"
    );
}
