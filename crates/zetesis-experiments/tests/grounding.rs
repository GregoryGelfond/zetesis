//! Public profiling contracts, independent of any performance threshold.

use std::{
    io::{self, Write},
    path::PathBuf,
    sync::Arc,
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
fn table_joins_preserve_the_indexed_subject() {
    let mut config = configuration();
    config.grounding.joins = zetesis_themelios::JoinStrategy::Table;
    let report = profile(source("table-joins.lp"), config).unwrap();
    assert!(report.complete, "{:?}", report.failure);
    assert_eq!(report.reference_join_strategy, "indexed");
    for sample in &report.samples {
        assert_eq!(sample.subject_equal, Some(true));
        assert_eq!(sample.subject_fingerprint, report.subject_fingerprint);
    }
}

#[test]
fn a_table_refusal_retains_the_indexed_reference() {
    let mut config = configuration();
    config.grounding.joins = zetesis_themelios::JoinStrategy::Table;
    // The source's equality postings fit; adding the table indices does not.
    // This must fail if the reference accidentally inherits the measured policy.
    config.formula.max_support_index_entries = 24;
    let report = profile(source("table-joins.lp"), config).unwrap();
    assert!(!report.complete);
    let Some(Error::Admission(error)) = &report.failure else {
        panic!("expected table index refusal: {:?}", report.failure);
    };
    assert!(matches!(
        error.error(),
        zetesis_themelios::FormulaFailure::Limit {
            resource: zetesis_themelios::FormulaResource::SupportIndexEntries,
            limit: 24,
            ..
        }
    ));
    let reference = report
        .qualification
        .as_ref()
        .expect("indexed reference qualifies");
    assert!(reference.exhausted);
    assert_eq!(reference.interpretations.len(), 2);
    assert_eq!(report.samples.len(), 1);
    assert!(!report.samples[0].admitted);
    assert!(report.samples[0].models.is_none());
}

#[test]
fn table_join_models_retain_whole_row_witnesses() {
    let mut config = configuration();
    config.grounding.joins = zetesis_themelios::JoinStrategy::Table;
    let report = profile(source("table-joins.lp"), config).unwrap();
    assert!(report.complete, "{:?}", report.failure);
    let common = [
        "domain(1)",
        "domain(2)",
        "-edge(1,1)",
        "-edge(1,2)",
        "-edge(2,1)",
        "-edge(2,2)",
        "diagonal(1)",
        "diagonal(2)",
    ];
    let expected: std::collections::BTreeSet<std::collections::BTreeSet<String>> = (1..=2)
        .map(|selected| {
            common
                .iter()
                .map(|atom| (*atom).to_owned())
                .chain([
                    format!("choose({selected})"),
                    format!("witness({selected},1)"),
                    format!("witness({selected},2)"),
                ])
                .collect()
        })
        .collect();
    for sample in &report.samples {
        let models = sample.models.as_ref().unwrap();
        assert!(models.exhausted);
        assert_eq!(models.interpretations.len(), 2);
        let observed: std::collections::BTreeSet<std::collections::BTreeSet<String>> = models
            .interpretations
            .iter()
            .map(|model| {
                model
                    .iter()
                    .map(|&atom| report.atoms[atom].clone())
                    .collect()
            })
            .collect();
        assert_eq!(expected, observed);
    }
}

#[test]
fn table_measurement_observes_actual_selection() {
    let mut config = configuration();
    config.grounding.joins = zetesis_themelios::JoinStrategy::Table;
    let report = profile(source("table-joins.lp"), config).unwrap();
    assert!(report.complete, "{:?}", report.failure);
    let detailed = report
        .samples
        .iter()
        .find(|sample| sample.mode == Mode::Detailed)
        .unwrap();
    let work = detailed
        .phases
        .iter()
        .fold(zetesis_themelios::GroundingWork::default(), |sum, phase| {
            sum.checked_sum(phase.work)
        });
    assert!(work.table_preparations.unwrap() > 0);
    assert!(work.table_probes.unwrap() > 0);
    assert!(work.table_rows.unwrap() > 0);
    assert!(work.table_prepare_work.unwrap() > 0);
    assert!(work.table_query_work.unwrap() > 0);
    assert!(work.table_index_bytes.unwrap() > 0);
    assert!(work.support_peak_bytes.unwrap() >= work.table_index_bytes.unwrap());
}

#[test]
fn table_strategy_is_recorded_in_the_report() {
    let options = CommandOptions::try_parse_from([
        "zetesis-bench",
        "grounding",
        "input.lp",
        "--joins",
        "table",
    ])
    .unwrap();
    let Some(Experiment::Grounding(options)) = options.command else {
        panic!("grounding command expected");
    };
    let config = options.configuration();
    assert_eq!(
        config.grounding.joins,
        zetesis_themelios::JoinStrategy::Table
    );
    let serialized = serde_json::to_value(config).unwrap();
    assert_eq!(serialized["grounding"]["joins"], "table");
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
fn model_capture_refusal_publishes_its_verified_prefix() {
    let mut config = configuration();
    config.capture.max_models = 1;
    let report = profile(source("identity.lp"), config).unwrap();
    let Some(Error::Limit {
        resource: "models",
        limit: 1,
    }) = report.failure.as_ref()
    else {
        panic!("expected an actual model capture refusal")
    };
    let models = report.qualification.as_ref().unwrap();
    assert_eq!(models.interpretations.len(), 1);
    assert!(matches!(
        models.interpretations[0].as_slice(),
        [0, 1] | [0, 1, 2]
    ));
    let retained = models.interpretations.as_ptr();
    let before = serde_json::to_value(&report).unwrap();
    let record = published_failure(&report);
    assert_eq!(record, before);
    assert_eq!(record["complete"], false);
    assert_eq!(record["failure"]["code"], "capture_limit");
    assert_eq!(record["failure"]["detail"], "models exceeded 1");
    assert_eq!(record["atoms"], serde_json::json!(["hidden", "-p", "q"]));
    assert_eq!(record["qualification"]["verified_models"], 2);
    assert_eq!(record["qualification"]["exhausted"], false);
    assert_eq!(
        record["qualification"]["interpretations"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(record["subject_fingerprint"]["status"], "available");
    assert_eq!(record["sources"].as_array().unwrap().len(), 2);
    assert_eq!(record["samples"], serde_json::json!([]));
    assert_eq!(serde_json::to_value(&report).unwrap(), before);
    assert_eq!(
        report
            .qualification
            .as_ref()
            .unwrap()
            .interpretations
            .as_ptr(),
        retained
    );
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
fn objective_presence_limit_is_serialized() {
    let mut config = Configuration::default();
    config.formula.max_objective_presence_entries = 7_019;
    config.formula.max_support_index_entries = 2_347;
    let encoded = serde_json::to_value(config).unwrap();
    assert_eq!(encoded["formula"]["max_objective_presence_entries"], 7_019);
}

#[test]
fn support_byte_limit_is_serialized() {
    let mut config = Configuration::default();
    config.formula.max_support_bytes = 81_013;
    let encoded = serde_json::to_value(config).unwrap();
    assert_eq!(encoded["formula"]["max_support_bytes"], 81_013);
}

#[test]
fn native_limits_are_numeric_json_fields() {
    let config = Configuration::default();
    let encoded = serde_json::to_value(config).unwrap();
    for (field, count) in [
        ("bundle", 5),
        ("admission", 4),
        ("expansion", 7),
        ("formula", 20),
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
    cause: Arc<SinkClosed>,
}

#[derive(Debug)]
struct SinkClosed;

impl std::fmt::Display for SinkClosed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("test sink closed")
    }
}

impl std::error::Error for SinkClosed {}

impl Write for PrefixWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                Arc::clone(&self.cause),
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
fn writer_failure_preserves_the_exact_report_prefix() {
    use std::error::Error as _;
    let report = qualified_identity();
    let before = serde_json::to_value(&report).unwrap();
    let mut complete = Vec::new();
    write_report(&report, &mut complete).unwrap();
    assert_eq!(complete.last(), Some(&b'\n'));
    for prefix in [0, 1, 31, complete.len() - 1] {
        let cause = Arc::new(SinkClosed);
        let mut output = PrefixWriter {
            remaining: prefix,
            bytes: Vec::new(),
            cause: Arc::clone(&cause),
        };
        let error = write_report(&report, &mut output).unwrap_err();
        let Error::Output(original) = &error else {
            panic!("expected the original writer failure")
        };
        assert_eq!(output.bytes, complete[..prefix]);
        assert_eq!(original.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(error.to_string(), "test sink closed");
        assert_eq!(error.code(), "output");
        let exposed = error.source().unwrap().downcast_ref::<io::Error>().unwrap();
        assert!(std::ptr::eq(exposed, original));
        let inner = exposed
            .get_ref()
            .unwrap()
            .downcast_ref::<Arc<SinkClosed>>()
            .unwrap();
        assert!(Arc::ptr_eq(inner, &cause));
        assert_eq!(serde_json::to_value(&report).unwrap(), before);
    }
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

fn published_failure(report: &Report) -> serde_json::Value {
    assert!(!report.complete);
    let mut bytes = Vec::new();
    write_report(report, &mut bytes).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[test]
fn missing_source_retains_its_native_failure() {
    use std::error::Error as _;
    let report = profile(source("missing.lp"), configuration()).unwrap();
    let Some(Error::Source(_)) = report.failure else {
        panic!("source refusal expected")
    };
    assert!(report.failure.as_ref().unwrap().source().is_some());
    let record = published_failure(&report);
    assert_eq!(record["failure"]["code"], "source");
    assert!(
        record["failure"]["detail"]
            .as_str()
            .unwrap()
            .contains("missing.lp")
    );
    assert!(report.sources.is_empty());
    assert!(report.samples.is_empty());
}

#[test]
fn admission_failure_preserves_the_loaded_source_catalog() {
    use std::error::Error as _;
    let mut config = configuration();
    config.formula.max_work = 0;
    let report = profile(source("arithmetic.lp"), config).unwrap();
    let Some(Error::Admission(_)) = report.failure else {
        panic!("admission refusal expected")
    };
    assert!(report.failure.as_ref().unwrap().source().is_some());
    let record = published_failure(&report);
    assert_eq!(record["failure"]["code"], "admission");
    assert!(!record["sources"].as_array().unwrap().is_empty());
    assert!(report.qualification.is_none());
    assert!(report.samples.is_empty());
}

#[test]
fn search_failure_cannot_erase_admitted_subject_evidence() {
    use std::error::Error as _;
    let mut config = configuration();
    config.search.search.max_work = 0;
    let report = profile(source("identity.lp"), config).unwrap();
    let Some(Error::Search(_)) = report.failure else {
        panic!("search refusal expected")
    };
    assert!(report.failure.as_ref().unwrap().source().is_some());
    let record = published_failure(&report);
    assert_eq!(record["failure"]["code"], "search_incomplete");
    assert_eq!(record["qualification"]["exhausted"], false);
    assert_eq!(record["subject_fingerprint"]["status"], "available");
    assert!(!report.atoms.is_empty());
}

#[test]
fn phase_capture_failure_retains_earlier_samples() {
    let mut config = configuration();
    config.capture.max_phase_records = 0;
    let report = profile(source("arithmetic.lp"), config).unwrap();
    let record = published_failure(&report);
    assert_eq!(record["failure"]["code"], "capture");
    assert_eq!(report.samples.len(), 3);
    assert!(
        report.samples[..2]
            .iter()
            .all(|sample| sample.models.as_ref().unwrap().exhausted)
    );
    let failed = &report.samples[2];
    assert!(failed.admitted);
    assert_eq!(failed.capture_refusal, Some(CaptureRefusal::RecordLimit));
    assert!(failed.models.is_none());
}

fn encoded_limit(report: &mut Report, include_newline: bool) -> usize {
    // The limit itself is a numeric field in the record. Its digit width must
    // stabilize before testing the JSON/newline boundary; measured clock digit
    // widths and source-path lengths cannot then make this test intermittent.
    for _ in 0..usize::BITS {
        let bytes = serde_json::to_vec(&report).unwrap().len() + usize::from(include_newline);
        if report.configuration.capture.max_output_bytes == bytes {
            return bytes;
        }
        report.configuration.capture.max_output_bytes = bytes;
    }
    panic!("numeric output-limit framing did not stabilize")
}

#[test]
fn output_ceiling_includes_the_terminal_newline() {
    let mut report = qualified_identity();
    let complete_bytes = encoded_limit(&mut report, true);
    let mut bytes = Vec::new();
    write_report(&report, &mut bytes).unwrap();
    assert_eq!(bytes.len(), complete_bytes);
    assert_eq!(bytes.last(), Some(&b'\n'));
    let json_bytes = encoded_limit(&mut report, false);
    let mut refused = Vec::new();
    assert!(
        matches!(write_report(&report,&mut refused),Err(Error::Limit { resource:"output_bytes",limit }) if limit == json_bytes)
    );
    assert!(refused.is_empty());
}
