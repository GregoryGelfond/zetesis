//! Bench commands map to bounded library plans and independent views.
use zetesis_cli::{
    Invocation,
    benchmark::{self, BenchCommand},
};
use zetesis_presentation::Layout;

fn command(arguments: &[&str]) -> BenchCommand {
    let Invocation::Bench(command) = Invocation::try_parse_from(
        ["zetesis", "bench"]
            .into_iter()
            .chain(arguments.iter().copied()),
    )
    .unwrap() else {
        panic!("expected benchmark command")
    };
    command
}

#[test]
fn installed_corpus_defaults_to_canonical_solve() {
    let BenchCommand::Corpus(options) = command(&["corpus", "--report", "new.json"]) else {
        panic!("expected corpus")
    };
    assert_eq!(
        options.invocation(),
        zetesis_validation::performance::matrix::NativeInvocation::Solve
    );
    assert_eq!(
        options.threads.get(),
        std::thread::available_parallelism()
            .map_or(1, std::num::NonZeroUsize::get)
            .min(4)
    );
    assert_eq!(options.completion_workers.get(), 1);
    assert_eq!(options.clingo_threads.get(), 1);
    assert_eq!(
        options.plan().unwrap().suite(),
        zetesis_validation::performance::matrix::Suite::Corpus
    );
}

#[test]
fn ordinary_corpus_keeps_reference_measurements() {
    use zetesis_validation::performance::{Phase, matrix};
    let BenchCommand::Corpus(options) = command(&["corpus", "--report", "new.json"]) else {
        panic!("expected corpus")
    };
    let plan = options.plan().unwrap();
    assert_eq!(plan.profiles().len(), 1);
    assert_eq!(plan.reference_policy(), matrix::ReferencePolicy::AllPhases);
    assert!(
        plan.slots(1)
            .unwrap()
            .iter()
            .any(|slot| slot.producer == matrix::Producer::Reference && slot.phase == Phase::Timed)
    );
}

#[test]
fn grounder_comparison_varies_only_materialization() {
    use zetesis_validation::selected::Grounder;
    let BenchCommand::Corpus(options) = command(&[
        "corpus",
        "--report",
        "new.json",
        "--compare-grounders",
        "--threads",
        "4",
        "--completion-workers",
        "2",
        "--batch-size",
        "17",
    ]) else {
        panic!("expected corpus")
    };
    let plan = options.plan().unwrap();
    let [eager, lazy] = plan.profiles() else {
        panic!("expected matched profiles")
    };
    assert_eq!(eager.grounder, Grounder::Eager);
    assert_eq!(lazy.grounder, Grounder::Lazy);
    assert_eq!(eager.workers.get(), 4);
    assert_eq!(eager.completion_workers.get(), 2);
    assert_eq!(eager.batch_size.get(), 17);
    let normalized = zetesis_validation::selected::NativeExecution {
        grounder: eager.grounder,
        ..*lazy
    };
    assert_eq!(
        serde_json::to_value(eager).unwrap(),
        serde_json::to_value(normalized).unwrap()
    );
}

#[test]
fn grounder_comparison_qualifies_reference_once() {
    use zetesis_validation::performance::{Phase, matrix};
    let BenchCommand::Corpus(options) =
        command(&["corpus", "--report", "new.json", "--compare-grounders"])
    else {
        panic!("expected corpus")
    };
    let plan = options.plan().unwrap();
    assert_eq!(
        plan.reference_policy(),
        matrix::ReferencePolicy::QualificationOnly
    );
    let slots = plan.slots(1).unwrap();
    let reference: Vec<_> = slots
        .iter()
        .filter(|slot| slot.producer == matrix::Producer::Reference)
        .collect();
    assert_eq!(reference.len(), 1);
    assert_eq!(reference[0].phase, Phase::Qualification);
    for profile in 0..2 {
        assert!(
            slots
                .iter()
                .any(|slot| slot.producer == matrix::Producer::Native { profile }
                    && slot.phase == Phase::Memory)
        );
    }
}

#[test]
fn grounder_comparison_refuses_an_explicit_grounder() {
    for grounder in ["auto", "eager", "lazy"] {
        let error = Invocation::try_parse_from([
            "zetesis",
            "bench",
            "corpus",
            "--report",
            "new.json",
            "--compare-grounders",
            "--grounder",
            grounder,
        ])
        .unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }
}

#[cfg(feature = "gpu")]
#[test]
fn primitive_json_is_an_unstyled_event_stream() {
    let command = command(&[
        "primitives",
        "relation",
        "--backend",
        "cpu",
        "--threads",
        "1",
        "--rows",
        "1",
        "--queries",
        "1",
        "--warmups",
        "0",
        "--repetitions",
        "1",
        "--json",
        "--color",
        "always",
    ]);
    let mut output = Vec::new();
    assert_eq!(
        benchmark::execute(&command, Layout::default(), &mut output, &mut Vec::new()).unwrap(),
        benchmark::Completion::Passed
    );
    let text = String::from_utf8(output).unwrap();
    assert!(!text.contains('\u{1b}'));
    let events: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(events.first().unwrap()["event"], "start");
    assert_eq!(events.last().unwrap()["event"], "complete");
    assert_eq!(events.last().unwrap()["observations"], 4);
}

#[cfg(feature = "gpu")]
#[test]
fn primitive_human_view_uses_completed_typed_samples() {
    let command = command(&[
        "primitives",
        "relation",
        "--backend",
        "cpu",
        "--threads",
        "1",
        "--rows",
        "1",
        "--queries",
        "1",
        "--warmups",
        "0",
        "--repetitions",
        "1",
    ]);
    let mut output = Vec::new();
    benchmark::execute(&command, Layout::default(), &mut output, &mut Vec::new()).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("Primitive measurements — complete matched samples"));
    assert!(text.contains("Elapsed ms"));
    assert!(text.contains("Scalar"));
    assert!(text.contains("Rayon"));
    assert!(!text.contains("\"event\""));
}

#[cfg(not(feature = "gpu"))]
#[test]
fn cpu_build_refuses_unavailable_primitive_profiles() {
    let command = command(&["primitives", "relation", "--backend", "cpu"]);
    let mut output = Vec::new();
    assert!(matches!(
        benchmark::execute(&command, Layout::default(), &mut output, &mut Vec::new()),
        Err(benchmark::Error::Unavailable(_))
    ));
    assert!(output.is_empty());
}

#[cfg(feature = "gpu")]
#[test]
fn failed_primitive_keeps_its_primary_failure() {
    struct Refuse;
    impl std::io::Write for Refuse {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected view refusal"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("partial.jsonl");
    let command = command(&[
        "primitives",
        "relation",
        "--backend",
        "cpu",
        "--threads",
        "1",
        "--rows",
        "1",
        "--queries",
        "1",
        "--warmups",
        "0",
        "--repetitions",
        "1",
        "--report-bytes",
        "0",
        "--report",
        path.to_str().unwrap(),
    ]);
    let Err(benchmark::Error::Reporting { primary, secondary }) =
        benchmark::execute(&command, Layout::default(), &mut Refuse, &mut Vec::new())
    else {
        panic!("both failures must be retained")
    };
    assert!(matches!(
        *primary,
        benchmark::Error::Primitive(zetesis_experiments::command::Error::Relation(
            zetesis_experiments::relation_measurement::Error::Output(_)
        ))
    ));
    assert!(
        matches!(*secondary, benchmark::Error::Io(ref error) if error.to_string() == "injected view refusal")
    );
    assert!(std::fs::read(path).unwrap().is_empty());
}

#[test]
fn json_preparation_failure_is_one_document() {
    let command = command(&["compare", "--report", "not-labelled", "--json"]);
    let mut output = Vec::new();
    assert!(matches!(
        benchmark::execute(&command, Layout::default(), &mut output, &mut Vec::new()),
        Err(benchmark::Error::ReportArgument(_))
    ));
    let document: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(document["format"], "zetesis-benchmark-failure");
    assert_eq!(document["schema"], 1);
    assert_eq!(document["status"], "failed");
    assert_eq!(document["command"], "compare");
    assert_eq!(document["error"]["kind"], "report_argument");
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn cancelled_corpus_publishes_unattempted_positions() {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("must-not-launch");
    std::fs::write(&executable, b"no executable is needed before cancellation").unwrap();
    let reference = directory.path().join("reference-must-not-launch");
    std::fs::write(&reference, b"distinct reference identity; never executable").unwrap();
    let report = directory.path().join("cancelled.json");
    let corpus =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/correctness");
    let command = command(&[
        "corpus",
        corpus.to_str().unwrap(),
        "--suite",
        "baseline",
        "--zetesis",
        executable.to_str().unwrap(),
        "--clingo",
        reference.to_str().unwrap(),
        "--report",
        report.to_str().unwrap(),
        "--warmups",
        "0",
        "--repetitions",
        "1",
        "--memory-runs",
        "0",
        "--json",
    ]);
    let mut output = Vec::new();
    let completion = benchmark::execute_with_cancellation(
        &command,
        Layout::default(),
        &mut output,
        &mut Vec::new(),
        &std::sync::atomic::AtomicBool::new(true),
    )
    .unwrap();
    assert_eq!(completion, benchmark::Completion::NonPass);
    let summary: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(summary["passed"], false);
    assert_eq!(summary["accounted"], true);
    let evidence: serde_json::Value =
        serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
    assert_eq!(evidence["report"]["metadata"], serde_json::json!([]));
    assert_eq!(
        evidence["report"]["faults"],
        serde_json::json!([{ "kind": "cancelled" }])
    );
    let samples = evidence["report"]["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 12);
    assert!(
        samples
            .iter()
            .all(|sample| sample["decision"] == "not_attempted" && sample["capture"].is_null())
    );
}

#[test]
fn failed_failure_publication_retains_preparation() {
    struct Refuse;
    impl std::io::Write for Refuse {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("failure sink refused"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let command = command(&["compare", "--report", "not-labelled", "--json"]);
    let Err(benchmark::Error::Reporting { primary, secondary }) =
        benchmark::execute(&command, Layout::default(), &mut Refuse, &mut Vec::new())
    else {
        panic!("both failures must survive")
    };
    assert!(matches!(*primary, benchmark::Error::ReportArgument(_)));
    assert!(matches!(*secondary, benchmark::Error::Json(_)));
}

#[cfg(feature = "gpu")]
#[test]
fn attempted_json_never_gets_a_second_document() {
    struct PartialFailure {
        attempts: usize,
        prefix: Vec<u8>,
    }
    impl std::io::Write for PartialFailure {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            self.attempts += 1;
            self.prefix.push(b'{');
            Err(std::io::Error::other("failure after changing sink"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let command = command(&[
        "primitives",
        "relation",
        "--backend",
        "cpu",
        "--rows",
        "1",
        "--queries",
        "1",
        "--json",
    ]);
    let mut output = PartialFailure {
        attempts: 0,
        prefix: Vec::new(),
    };
    assert!(matches!(
        benchmark::execute(&command, Layout::default(), &mut output, &mut Vec::new()),
        Err(benchmark::Error::Primitive(_))
    ));
    assert_eq!(output.attempts, 1);
    assert_eq!(output.prefix, b"{");
}
