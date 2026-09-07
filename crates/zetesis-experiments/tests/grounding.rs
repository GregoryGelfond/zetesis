//! Public profiling contracts, independent of any performance threshold.

use std::{
    io::{self, Write},
    path::PathBuf,
};

use clap::Parser;
use zetesis_experiments::grounding::{
    CaptureRefusal, Configuration, Error, FingerprintUnavailable, Mode, Report, SubjectFingerprint,
    profile, write_report,
};
use zetesis_experiments::{CommandOptions, Experiment};

fn source(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/grounding")
        .join(name)
}

fn configuration() -> Configuration {
    Configuration {
        repetitions: 1,
        ..Configuration::default()
    }
}

fn qualified_identity() -> Report {
    let report = profile(source("identity.lp"), configuration()).unwrap();
    assert!(report.complete, "{:?}", report.failure);
    report
}

#[test]
fn modes_preserve_complete_native_models() {
    let report = qualified_identity();
    assert_eq!(report.atoms, ["hidden", "-p", "q"]);
    let models = &report.qualification.as_ref().unwrap().interpretations;
    assert_eq!(models, &[vec![0, 1], vec![0, 1, 2]]);
    assert_eq!(report.samples.len(), 3);
    for sample in &report.samples {
        assert!(sample.admitted);
        assert_eq!(sample.subject_equal, Some(true));
        let checked = sample.models.as_ref().unwrap();
        assert!(checked.exhausted);
        assert_eq!(&checked.interpretations, models);
        assert_eq!(checked.verified_models, 2);
    }
}

#[test]
fn every_sample_retains_the_complete_execution_fingerprint() {
    let report = qualified_identity();
    assert!(matches!(
        report.subject_fingerprint,
        Some(SubjectFingerprint::Available { .. })
    ));
    for sample in &report.samples {
        assert_eq!(sample.subject_fingerprint, report.subject_fingerprint);
    }
}

#[test]
fn subject_encoding_ceiling_is_inclusive() {
    let report = qualified_identity();
    let Some(SubjectFingerprint::Available { bytes, .. }) = report.subject_fingerprint else {
        panic!("complete supported subject fingerprint");
    };
    let mut config = configuration();
    config.capture.max_subject_bytes = bytes;
    assert!(profile(source("identity.lp"), config).unwrap().complete);
    config.capture.max_subject_bytes = bytes - 1;
    let refused = profile(source("identity.lp"), config).unwrap();
    assert!(!refused.complete);
    assert!(
        refused.nodes.is_some(),
        "native admission already succeeded"
    );
    assert!(refused.subject_fingerprint.is_none(), "no prefix digest");
    assert!(
        refused.qualification.is_none(),
        "capture refusal precedes solve"
    );
    assert!(
        matches!(refused.failure, Some(Error::Limit { resource: "subject_encoding_bytes", limit }) if limit == bytes - 1)
    );
}

#[test]
fn term_observations_preserve_internal_qualification() {
    let report = profile(source("term-observation.lp"), configuration()).unwrap();
    assert!(report.complete, "{:?}", report.failure);
    assert_eq!(
        report.subject_fingerprint,
        Some(SubjectFingerprint::Unavailable {
            reason: FingerprintUnavailable::TermObservations,
        })
    );
    assert_eq!(
        report.qualification.as_ref().unwrap().interpretations,
        [vec![0]]
    );
    for sample in &report.samples {
        assert_eq!(sample.subject_equal, Some(true));
        assert_eq!(sample.subject_fingerprint, report.subject_fingerprint);
        assert!(sample.models.as_ref().unwrap().exhausted);
    }
}

#[test]
fn chain_controls_have_the_expected_complete_model() {
    let expected: std::collections::BTreeSet<_> = (0..64)
        .map(|n| format!("vertex({n})"))
        .chain((0..63).map(|n| format!("edge({n},{})", n + 1)))
        .chain((0..64).map(|n| format!("reach({n})")))
        .collect();
    for name in ["sparse-arithmetic.lp", "plain-chain.lp"] {
        let report = profile(source(name), configuration()).unwrap();
        assert!(report.complete, "{name}: {:?}", report.failure);
        assert_eq!(
            report
                .atoms
                .iter()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>(),
            expected
        );
        let models = report.qualification.unwrap();
        assert!(models.exhausted);
        assert_eq!(models.verified_models, 1);
        assert_eq!(
            models.interpretations,
            [(0..report.atoms.len()).collect::<Vec<_>>()]
        );
    }
}

#[test]
fn unavailable_grounding_differs_from_measured_grounding() {
    let report = qualified_identity();
    for sample in &report.samples {
        assert!(sample.admission_elapsed_ns.is_some());
        assert_eq!(
            sample.grounding_elapsed_ns.is_some(),
            sample.mode != Mode::Unobserved
        );
        assert_eq!(sample.phases.is_empty(), sample.mode != Mode::Detailed);
        let sum: u64 = sample
            .phases
            .iter()
            .map(|phase| phase.elapsed_ns.unwrap())
            .sum();
        if let Some(grounding) = sample.grounding_elapsed_ns {
            assert!(sum <= grounding);
        }
    }
}

#[test]
fn phase_locations_resolve_to_original_files() {
    let report = qualified_identity();
    assert_eq!(report.sources.len(), 2);
    let mut seen = std::collections::BTreeSet::new();
    for phase in &report.samples[2].phases {
        if let Some(location) = phase.location {
            let source = report
                .sources
                .iter()
                .find(|source| source.source == location.source)
                .unwrap();
            assert!(location.start_byte < location.end_byte);
            assert!(usize::try_from(location.end_byte).unwrap() <= source.bytes);
            seen.insert(location.source);
        }
    }
    assert_eq!(seen.len(), 2);
}

#[test]
fn source_hashes_cover_exact_original_bytes() {
    use sha2::{Digest, Sha256};
    for identity in qualified_identity().sources {
        let original = std::fs::read(identity.path).unwrap();
        assert_eq!(identity.bytes, original.len());
        assert_eq!(identity.sha256, format!("{:x}", Sha256::digest(original)));
    }
}

#[test]
fn rounds_rotate_condition_order() {
    let config = Configuration {
        repetitions: 3,
        ..configuration()
    };
    let report = profile(source("inconsistent.lp"), config).unwrap();
    assert!(report.complete);
    let actual: Vec<_> = report
        .samples
        .iter()
        .map(|sample| (sample.round, sample.mode))
        .collect();
    assert_eq!(
        actual,
        [
            (0, Mode::Unobserved),
            (0, Mode::Boundary),
            (0, Mode::Detailed),
            (1, Mode::Boundary),
            (1, Mode::Detailed),
            (1, Mode::Unobserved),
            (2, Mode::Detailed),
            (2, Mode::Unobserved),
            (2, Mode::Boundary),
        ]
    );
}

#[test]
fn zero_models_with_exhaustion_qualify() {
    let mut config = configuration();
    config.capture.max_models = 0;
    config.capture.max_model_atoms = 0;
    let report = profile(source("inconsistent.lp"), config).unwrap();
    assert!(report.complete);
    for models in report
        .samples
        .iter()
        .filter_map(|sample| sample.models.as_ref())
    {
        assert!(models.exhausted);
        assert!(models.interpretations.is_empty());
        assert_eq!(models.verified_models, 0);
    }
}

#[test]
fn model_limit_retains_verified_prefix() {
    let mut config = configuration();
    config.capture.max_models = 1;
    let report = profile(source("identity.lp"), config).unwrap();
    assert!(!report.complete);
    assert!(matches!(
        report.failure,
        Some(Error::Limit {
            resource: "models",
            limit: 1
        })
    ));
    let models = report.qualification.unwrap();
    assert_eq!(models.interpretations.len(), 1);
    assert_eq!(models.verified_models, 2);
    assert!(!models.exhausted);
    assert!(report.samples.is_empty());
}

#[test]
fn atom_index_limit_is_inclusive() {
    let mut config = configuration();
    config.capture.max_model_atoms = 5;
    assert!(profile(source("identity.lp"), config).unwrap().complete);
    config.capture.max_model_atoms = 4;
    let report = profile(source("identity.lp"), config).unwrap();
    assert!(!report.complete);
    assert!(matches!(
        report.failure,
        Some(Error::Limit {
            resource: "model_atom_indices",
            limit: 4
        })
    ));
    assert!(report.qualification.unwrap().retained_atom_indices <= 4);
}

#[test]
fn phase_record_limit_is_inclusive() {
    let required = qualified_identity().samples[2].phases.len();
    let mut config = configuration();
    config.capture.max_phase_records = required;
    assert!(profile(source("identity.lp"), config).unwrap().complete);
    config.capture.max_phase_records = required - 1;
    let report = profile(source("identity.lp"), config).unwrap();
    assert!(!report.complete);
    assert!(matches!(
        report.failure,
        Some(Error::Capture(CaptureRefusal::RecordLimit))
    ));
    assert_eq!(report.samples.last().unwrap().phases.len(), required - 1);
    assert_eq!(
        report.samples.last().unwrap().capture_refusal,
        Some(CaptureRefusal::RecordLimit)
    );
}

#[test]
fn atom_catalog_limit_is_inclusive() {
    let mut config = configuration();
    config.capture.max_atom_text_bytes = 9;
    assert!(profile(source("identity.lp"), config).unwrap().complete);
    config.capture.max_atom_text_bytes = 8;
    let report = profile(source("identity.lp"), config).unwrap();
    assert!(!report.complete);
    assert!(matches!(
        report.failure,
        Some(Error::Limit {
            resource: "atom_text_bytes",
            ..
        })
    ));
}

#[test]
fn catalog_spelling_preserves_value_distinctions() {
    let report = profile(source("values.lp"), configuration()).unwrap();
    assert!(report.complete, "{:?}", report.failure);
    let actual: std::collections::BTreeSet<_> = report.atoms.iter().map(String::as_str).collect();
    let expected = [
        "p(7)",
        "p(\"#inf\")",
        "p(\"#sup\")",
        "p(\"a\\n\\\"\\\\\")",
        "-p(\"signed\")",
    ];
    assert_eq!(actual, expected.into_iter().collect());
    assert_eq!(
        report.qualification.unwrap().interpretations,
        [vec![0, 1, 2, 3, 4]]
    );
}

#[test]
fn source_path_limit_is_inclusive() {
    let required = qualified_identity()
        .sources
        .iter()
        .map(|source| source.path.as_os_str().as_encoded_bytes().len())
        .sum();
    let mut config = configuration();
    config.capture.max_source_path_bytes = required;
    assert!(profile(source("identity.lp"), config).unwrap().complete);
    config.capture.max_source_path_bytes = required - 1;
    let report = profile(source("identity.lp"), config).unwrap();
    assert!(!report.complete);
    assert!(matches!(
        report.failure,
        Some(Error::Limit {
            resource: "source_path_bytes",
            ..
        })
    ));
    assert!(report.samples.is_empty());
}

#[test]
fn objectives_refuse_enumeration_qualification() {
    let report = profile(source("objective.lp"), configuration()).unwrap();
    assert!(!report.complete);
    assert!(matches!(report.failure, Some(Error::Objective)));
    assert!(report.qualification.is_none());
    assert!(report.samples.is_empty());
}

#[test]
fn search_refusal_cannot_qualify() {
    let mut config = configuration();
    config.search.search.max_work = 0;
    let report = profile(source("identity.lp"), config).unwrap();
    assert!(!report.complete);
    assert!(matches!(report.failure, Some(Error::Search(_))));
    assert!(!report.qualification.unwrap().exhausted);
}

#[test]
fn invalid_repetition_counts_refuse_before_loading() {
    for repetitions in [0, 12, usize::MAX] {
        let config = Configuration {
            repetitions,
            ..configuration()
        };
        assert!(matches!(
            profile(source("missing.lp"), config),
            Err(Error::Configuration(_))
        ));
    }
}

#[test]
fn native_limits_are_numeric_json_fields() {
    let config = Configuration::default();
    let encoded = serde_json::to_value(config).unwrap();
    for (field, count) in [
        ("bundle", 5),
        ("admission", 4),
        ("expansion", 7),
        ("formula", 18),
        ("search", 4),
        ("certificate", 4),
        ("capture", 7),
    ] {
        assert_eq!(encoded[field].as_object().unwrap().len(), count, "{field}");
    }
    assert_eq!(encoded["formula"]["max_work"], config.formula.max_work);
    assert_eq!(
        encoded["capture"]["max_subject_bytes"],
        config.capture.max_subject_bytes
    );
    assert_eq!(
        encoded["formula"]["observation"]["max_origins"],
        config.formula.observation.max_origins
    );
    assert_eq!(
        encoded["search"]["search"]["max_decisions"],
        config.search.search.max_decisions
    );
    assert_eq!(
        encoded["admission"]["core_limits"]["max_predicate_arity"],
        config.admission.core_limits.max_predicate_arity
    );
}

#[test]
fn report_byte_refusal_publishes_nothing() {
    let mut report = qualified_identity();
    report.configuration.capture.max_output_bytes = 0;
    let mut output = Vec::new();
    assert!(matches!(
        write_report(&report, &mut output),
        Err(Error::Limit {
            resource: "output_bytes",
            limit: 0
        })
    ));
    assert!(output.is_empty());
}

struct PrefixWriter {
    remaining: usize,
    bytes: Vec<u8>,
}
impl Write for PrefixWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "test sink closed",
            ));
        }
        let count = self.remaining.min(bytes.len());
        self.bytes.extend_from_slice(&bytes[..count]);
        self.remaining -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn writer_prefix_failure_is_not_success() {
    let report = qualified_identity();
    let mut output = PrefixWriter {
        remaining: 31,
        bytes: Vec::new(),
    };
    assert!(matches!(
        write_report(&report, &mut output),
        Err(Error::Output(_))
    ));
    assert_eq!(output.bytes.len(), 31);
    assert!(serde_json::from_slice::<serde_json::Value>(&output.bytes).is_err());
}

#[test]
fn command_adapts_to_the_library_configuration() {
    let command = CommandOptions::try_parse_from([
        "zetesis-bench",
        "grounding",
        "source.lp",
        "--repetitions",
        "2",
        "--max-models",
        "7",
        "--max-subject-bytes",
        "19",
    ])
    .unwrap();
    let Some(Experiment::Grounding(options)) = command.command else {
        panic!("grounding command");
    };
    assert_eq!(options.source, PathBuf::from("source.lp"));
    assert_eq!(options.configuration().repetitions, 2);
    assert_eq!(options.configuration().capture.max_models, 7);
    assert_eq!(options.configuration().capture.max_subject_bytes, 19);
}

#[test]
fn incomplete_command_publishes_its_refusal() {
    let path = source("objective.lp");
    let command =
        CommandOptions::try_parse_from(["zetesis-bench", "grounding", path.to_str().unwrap()])
            .unwrap();
    let Some(Experiment::Grounding(options)) = command.command else {
        panic!("grounding command");
    };
    let mut output = Vec::new();
    assert!(matches!(
        zetesis_experiments::grounding::run(&options, &mut output),
        Err(Error::Objective)
    ));
    let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(report["complete"], false);
    assert_eq!(report["failure"]["code"], "objective_unsupported");
}
