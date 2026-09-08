//! Bounded synthetic producers exercise the observation/evidence contracts.
#![cfg(any(target_os = "linux", target_os = "macos"))]

use serde_json::{Value, json};
use std::error::Error as _;
use std::fmt::Write as _;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use zetesis_validation::{
    examples,
    performance::{self, Case, Decision, Phase, Producer, Schedule},
};

struct Fixture {
    directory: tempfile::TempDir,
    corpus: PathBuf,
    native: PathBuf,
    reference: PathBuf,
    report: PathBuf,
}
fn quote(path: &Path) -> String {
    format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"))
}
fn executable(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
impl Fixture {
    fn new(native_prefix: &str, mutate: impl Fn(&mut String)) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/kr-domains");
        let corpus = directory.path().join("corpus");
        fs::create_dir(&corpus).unwrap();
        for file in ["manifest.json", "LICENSE"] {
            fs::copy(original.join(file), corpus.join(file)).unwrap();
        }
        let verified = examples::load(&original, examples::Limits::default()).unwrap();
        for source in verified.files() {
            let path = corpus.join(source.path());
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, source.source()).unwrap();
        }
        let diagnostics = directory.path().join("statistics.txt");
        fs::write(
            &diagnostics,
            format!(
                "{}{}",
                include_str!("support/phase_statistics.txt"),
                include_str!("support/stage_statistics.txt")
            ),
        )
        .unwrap();
        let mut native_script =
            String::from("if [ \"$#\" -eq 1 ]; then echo native-fixture; exit 0; fi\n");
        native_script.push_str(native_prefix);
        writeln!(
            native_script,
            "for arg do if [ \"$arg\" = '--stats' ]; then /bin/cat {} >&2; fi; input=$arg; done",
            quote(&diagnostics)
        )
        .unwrap();
        let mut reference_script = String::from(
            "if [ \"$#\" -eq 1 ]; then echo reference-fixture; exit 0; fi\nfor input do :; done\n",
        );
        for (index, selected) in Case::ALL.into_iter().enumerate() {
            let case = verified
                .cases()
                .iter()
                .find(|case| case.path() == selected.path())
                .unwrap();
            let (mut native, reference) = producer_reports(case.contract());
            if index == 0 {
                mutate(&mut native);
            }
            for (producer, bytes, script) in [
                ("native", native.into_bytes(), &mut native_script),
                (
                    "reference",
                    serde_json::to_vec(&reference).unwrap(),
                    &mut reference_script,
                ),
            ] {
                let output = directory.path().join(format!("{producer}-{index}.txt"));
                fs::write(&output, bytes).unwrap();
                writeln!(
                    script,
                    "case \"$input\" in *{}) /bin/cat {}; exit 0;; esac",
                    selected.path(),
                    quote(&output)
                )
                .unwrap();
            }
        }
        let native = directory.path().join("native");
        let reference = directory.path().join("reference");
        executable(&native, &(native_script + "exit 4"));
        executable(&reference, &(reference_script + "exit 4"));
        let report = directory.path().join("report.json");
        Self {
            directory,
            corpus,
            native,
            reference,
            report,
        }
    }
    fn request(&self) -> performance::Request<'_> {
        performance::Request {
            corpus: &self.corpus,
            native: &self.native,
            reference: &self.reference,
            report: &self.report,
            schedule: Schedule::new(0, 1).unwrap(),
            limits: performance::Limits::default(),
        }
    }
    fn run(&self) -> performance::Report {
        performance::run(&self.request()).unwrap()
    }
}

fn producer_reports(contract: &examples::Contract) -> (String, Value) {
    // These fixtures establish interchange comparison, not ASP semantics.
    // The independent real campaign remains separately qualified.
    let symbols = contract
        .witnesses()
        .first()
        .map_or(contract.required_symbols(), Vec::as_slice);
    let count = contract.model_count().unwrap_or(1);
    let status = if contract.cost().is_some() {
        "OPTIMUM FOUND"
    } else {
        "SATISFIABLE"
    };
    let mut native = String::new();
    for model in 1..=count {
        writeln!(native, "Answer: {model}\n{}", symbols.join(" ")).unwrap();
        if let Some(cost) = contract.cost() {
            writeln!(
                native,
                "Optimization: {}",
                cost.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" ")
            )
            .unwrap();
        }
    }
    writeln!(native, "{status}\nModels: {count}\nCoverage: exhausted").unwrap();
    let witness = if let Some(cost) = contract.cost() {
        json!({"Value":symbols,"Costs":cost})
    } else {
        json!({"Value":symbols})
    };
    let witness_count = count + u64::from(contract.cost().is_some());
    let mut summary = json!({"More":"no","Number":witness_count});
    if let Some(cost) = contract.cost() {
        summary["Optimum"] = json!("yes");
        summary["Optimal"] = json!(count);
        summary["Costs"] = json!(cost);
    }
    (
        native,
        json!({"Result":status,"Models":summary,"Call":[{"Witnesses":vec![witness; usize::try_from(witness_count).unwrap()]}]}),
    )
}

#[test]
fn all_qualification_pairs_precede_timed_observations() {
    let slots = Schedule::default().slots();
    assert!(
        slots[..6]
            .iter()
            .all(|slot| slot.phase == Phase::Qualification)
    );
    assert_eq!(
        slots
            .iter()
            .filter(|slot| slot.phase == Phase::Timed)
            .count(),
        126
    );
    assert_eq!(
        slots
            .iter()
            .filter(|slot| slot.phase == Phase::Diagnostics)
            .count(),
        3
    );
}

#[test]
fn each_input_alternates_the_first_timed_producer() {
    for case in Case::ALL {
        let slots: Vec<_> = Schedule::new(0, 4)
            .unwrap()
            .slots()
            .into_iter()
            .filter(|slot| slot.case == case && slot.phase == Phase::Timed)
            .collect();
        let first: Vec<_> = slots.chunks_exact(2).map(|pair| pair[0].producer).collect();
        assert!(first.windows(2).all(|pair| pair[0] != pair[1]));
        assert_eq!(
            first
                .iter()
                .filter(|producer| **producer == Producer::Native)
                .count(),
            2
        );
    }
}

#[test]
fn matching_producers_preserve_every_scheduled_capture() {
    let fixture = Fixture::new("", |_| {});
    let report = fixture.run();
    assert!(report.passed(), "{report:?}");
    assert_eq!(report.samples().len(), 15);
    assert!(
        report
            .samples()
            .iter()
            .all(|sample| sample.capture().elapsed_ns().is_some())
    );
    assert!(
        report
            .samples()
            .iter()
            .filter(|sample| sample.slot().phase == Phase::Diagnostics)
            .all(|sample| sample.diagnostics().is_some())
    );
    report.publish().unwrap();
    let view: Value = serde_json::from_slice(&fs::read(&fixture.report).unwrap()).unwrap();
    assert_eq!(view["passed"], true);
    assert!(
        view["peak_rss"]
            .as_str()
            .unwrap()
            .starts_with("unavailable:")
    );
}

#[test]
fn qualification_mismatch_prevents_timed_launches() {
    let fixture = Fixture::new("", |text| {
        *text = text.replace("assign(s,9)", "assign(s,8)");
    });
    let report = fixture.run();
    assert!(!report.passed());
    assert_eq!(
        report.samples().last().unwrap().decision(),
        Decision::ModelMismatch
    );
    assert!(
        report
            .samples()
            .iter()
            .all(|sample| sample.slot().phase == Phase::Qualification)
    );
}

#[test]
fn valid_output_with_a_failed_exit_cannot_qualify() {
    let fixture = Fixture::new("", |_| {});
    let producer = fs::read_to_string(&fixture.native)
        .unwrap()
        .replace("; exit 0;; esac", "; exit 7;; esac");
    fs::write(&fixture.native, producer).unwrap();
    let report = fixture.run();
    assert!(!report.passed());
    let sample = report.samples().last().unwrap();
    assert!(sample.capture().stdout().starts_with(b"Answer:"));
    assert_eq!(sample.capture().exit().unwrap().code, Some(7));
    assert_eq!(sample.decision(), Decision::InvocationFailure);
    assert!(sample.selected_models().is_none());
}

#[test]
fn equal_counts_do_not_hide_different_displays() {
    let fixture = Fixture::new("", |_| {});
    let output = fixture.directory.path().join("native-1.txt");
    let text = fs::read_to_string(&output)
        .unwrap()
        .replace("\n\n", "\nunexpected\n");
    fs::write(output, text).unwrap();
    let report = fixture.run();
    let sample = report.samples().last().unwrap();
    assert_eq!(sample.selected_models(), Some(92));
    assert_eq!(sample.decision(), Decision::ModelMismatch);
}

#[test]
fn missing_diagnostics_invalidate_the_instrumented_observation() {
    let fixture = Fixture::new("", |_| {});
    fs::write(
        fixture.directory.path().join("statistics.txt"),
        b"no statistics\n",
    )
    .unwrap();
    let report = fixture.run();
    assert!(!report.passed());
    let sample = report.samples().last().unwrap();
    assert_eq!(sample.slot().phase, Phase::Diagnostics);
    assert_eq!(sample.decision(), Decision::InvalidDiagnostics);
}

#[test]
fn private_source_mutation_invalidates_input_identity() {
    let fixture = Fixture::new(
        "for input do :; done\nprintf '\\nchanged.\\n' >> \"$input\"\n",
        |_| {},
    );
    let report = fixture.run();
    assert!(!report.passed());
    assert!(report.after().iter().any(|change| !change.unchanged()));
}

#[test]
fn timed_failures_are_not_replaced() {
    let fixture = Fixture::new(
        "count=0\nif [ -f \"$0.count\" ]; then count=$(/bin/cat \"$0.count\"); fi\ncount=$((count + 1))\nprintf '%s' \"$count\" > \"$0.count\"\nif [ \"$count\" -gt 3 ]; then exit 7; fi\n",
        |_| {},
    );
    let report = fixture.run();
    assert!(!report.passed());
    assert_eq!(
        report.samples().last().unwrap().decision(),
        Decision::InvocationFailure
    );
    assert_eq!(report.samples().last().unwrap().slot().phase, Phase::Timed);
    assert_eq!(
        report
            .samples()
            .last()
            .unwrap()
            .capture()
            .exit()
            .unwrap()
            .code,
        Some(7)
    );
}

#[test]
fn changed_executable_bytes_invalidate_the_report() {
    let fixture = Fixture::new("printf '\\n# changed\\n' >> \"$0\"\n", |_| {});
    let report = fixture.run();
    assert!(!report.passed());
    assert!(report.after().iter().any(|change| !change.unchanged()));
}

#[test]
fn stalled_producer_retains_deadline_evidence() {
    let fixture = Fixture::new("exec /bin/sleep 5\n", |_| {});
    let mut request = fixture.request();
    request.limits.process.timeout = Duration::from_millis(100);
    let report = performance::run(&request).unwrap();
    assert!(!report.passed());
    assert!(
        report
            .samples()
            .iter()
            .map(performance::Sample::capture)
            .chain(report.metadata())
            .any(|capture| capture.stop() == Some(zetesis_validation::process::Stop::Deadline))
    );
    assert!(report.unresolved_children().is_empty());
}

#[test]
fn cumulative_capture_ceiling_retains_a_bounded_prefix() {
    let fixture = Fixture::new("", |_| {});
    let mut request = fixture.request();
    request.limits.max_total_capture_bytes = 20;
    let report = performance::run(&request).unwrap();
    assert!(!report.passed());
    assert!(report.total_capture_bytes() <= 20);
}

#[test]
fn report_publication_never_replaces_existing_evidence() {
    let fixture = Fixture::new("", |_| {});
    let report = fixture.run();
    fs::write(&fixture.report, b"existing evidence").unwrap();
    assert!(report.publish().is_err());
    assert_eq!(fs::read(&fixture.report).unwrap(), b"existing evidence");
}

#[test]
fn executable_paths_cannot_be_publication_destinations() {
    let fixture = Fixture::new("", |_| {});
    let mut request = fixture.request();
    request.report = &fixture.native;
    assert!(performance::run(&request).is_err());
}

#[test]
fn serialized_report_ceiling_refuses_publication() {
    let fixture = Fixture::new("", |_| {});
    let mut request = fixture.request();
    request.limits.max_report_bytes = 0;
    let report = performance::run(&request).unwrap();
    assert!(report.publish().is_err());
    assert!(!fixture.report.exists());
}

#[test]
fn schedules_refuse_unbounded_measurement_populations() {
    for (warmups, repetitions) in [(6, 1), (0, 0), (0, 42), (usize::MAX, usize::MAX)] {
        let error = Schedule::new(warmups, repetitions).unwrap_err();
        assert!(matches!(error, performance::Error::Configuration(_)));
        assert!(
            error
                .to_string()
                .contains("warmups must be 0..=5 and repetitions 1..=41")
        );
        assert!(error.source().is_none());
    }
}

#[test]
fn the_largest_schedule_preserves_every_authored_round() {
    let schedule = Schedule::new(5, 41).unwrap();
    for (phase, rounds) in [
        (Phase::Warmup, schedule.warmups()),
        (Phase::Timed, schedule.repetitions()),
    ] {
        for case in Case::ALL {
            let slots: Vec<_> = schedule
                .slots()
                .into_iter()
                .filter(|slot| slot.phase == phase && slot.case == case)
                .collect();
            assert_eq!(slots.len(), rounds * 2);
            for (round, pair) in slots.chunks_exact(2).enumerate() {
                assert_eq!(pair[0].round, round);
                assert_eq!(pair[1].round, round);
                assert_ne!(pair[0].producer, pair[1].producer);
            }
        }
    }
}

#[test]
fn missing_corpus_retains_the_typed_setup_cause() {
    let fixture = Fixture::new("", |_| {});
    fs::remove_file(fixture.corpus.join("manifest.json")).unwrap();
    let error = performance::run(&fixture.request()).unwrap_err();
    assert!(matches!(error, performance::Error::Corpus(_)));
    assert!(error.to_string().contains("manifest.json"));
    assert!(
        error
            .source()
            .unwrap()
            .downcast_ref::<examples::Error>()
            .is_some()
    );
    assert!(!fixture.report.exists());
}

#[test]
fn executable_seal_limits_precede_process_execution() {
    let fixture = Fixture::new("", |_| {});
    let mut request = fixture.request();
    request.limits.max_executable_bytes = 0;
    let error = performance::run(&request).unwrap_err();
    assert!(matches!(
        &error,
        performance::Error::Boundary(zetesis_validation::selected::Error::Bytes { limit: 0, .. })
    ));
    assert!(error.to_string().contains("byte"));
    assert!(
        error
            .source()
            .unwrap()
            .downcast_ref::<zetesis_validation::selected::Error>()
            .is_some()
    );
    assert!(!fixture.report.exists());
}

#[test]
fn aliased_executables_cannot_claim_independent_comparison() {
    let fixture = Fixture::new("", |_| {});
    fs::remove_file(&fixture.reference).unwrap();
    fs::hard_link(&fixture.native, &fixture.reference).unwrap();
    let error = performance::run(&fixture.request()).unwrap_err();
    assert!(matches!(
        error,
        performance::Error::Configuration("native and reference executable identities must differ")
    ));
}

#[test]
fn relative_executable_paths_cannot_use_implicit_search() {
    let fixture = Fixture::new("", |_| {});
    let mut request = fixture.request();
    request.native = Path::new("native");
    assert!(matches!(
        performance::run(&request),
        Err(performance::Error::Configuration(
            "native and reference executable paths must be absolute"
        ))
    ));
}

#[test]
fn expired_campaigns_launch_no_producers() {
    let fixture = Fixture::new("", |_| {});
    let mut request = fixture.request();
    request.limits.campaign_timeout = Duration::ZERO;
    let report = performance::run(&request).unwrap();
    assert!(matches!(report.faults(), [performance::Fault::Deadline]));
    assert!(report.metadata().is_empty());
    assert!(report.samples().is_empty());
    assert!(!report.passed());
    assert!(
        report
            .after()
            .iter()
            .all(zetesis_validation::selected::Change::unchanged)
    );
}

#[test]
fn an_unrepresentable_deadline_is_a_configuration_error() {
    let fixture = Fixture::new("", |_| {});
    let mut request = fixture.request();
    request.limits.campaign_timeout = Duration::MAX;
    assert!(matches!(
        performance::run(&request),
        Err(performance::Error::Configuration(
            "campaign deadline is not representable"
        ))
    ));
}

#[test]
fn zero_capture_capacity_launches_no_producers() {
    let fixture = Fixture::new("", |_| {});
    let mut request = fixture.request();
    request.limits.max_total_capture_bytes = 0;
    let report = performance::run(&request).unwrap();
    assert!(matches!(
        report.faults(),
        [performance::Fault::CaptureBudget]
    ));
    assert!(report.metadata().is_empty());
    assert!(report.samples().is_empty());
    assert_eq!(report.total_capture_bytes(), 0);
}

#[test]
fn spawn_failure_cannot_become_a_timed_observation() {
    let fixture = Fixture::new("", |_| {});
    fs::set_permissions(&fixture.native, fs::Permissions::from_mode(0o600)).unwrap();
    let report = fixture.run();
    assert!(!report.passed());
    assert!(report.samples().is_empty());
    assert!(matches!(report.faults(), [performance::Fault::Metadata]));
    let capture = &report.metadata()[0];
    assert_eq!(
        capture.failure().unwrap().kind(),
        zetesis_validation::selected::InvocationFault::Spawn
    );
    assert!(capture.cleanup_failure().is_none());
    assert!(capture.elapsed_ns().is_none());
    assert!(capture.stop().is_none());
    assert!(capture.exit().is_none());
    report.publish().unwrap();
    let view: Value = serde_json::from_slice(&fs::read(&fixture.report).unwrap()).unwrap();
    assert_eq!(view["passed"], false);
    assert_eq!(view["metadata"][0]["failure"]["kind"]["stage"], "spawn");
}

#[test]
fn malformed_native_reports_stop_the_qualification_prefix() {
    let fixture = Fixture::new("", |text| *text = String::from("SATISFIABLE\n"));
    let report = fixture.run();
    let sample = report.samples().last().unwrap();
    assert_eq!(sample.decision(), Decision::InvalidReport);
    assert!(sample.detail().is_some());
    assert!(sample.selected_models().is_none());
    assert!(
        report
            .samples()
            .iter()
            .all(|sample| sample.slot().phase == Phase::Qualification)
    );
}

#[test]
fn raw_capture_accounting_includes_separate_metadata() {
    let fixture = Fixture::new("", |_| {});
    let mut request = fixture.request();
    request.limits.max_total_capture_bytes = 1_048_576;
    let report = performance::run(&request).unwrap();
    assert!(report.passed());
    let retained: usize = report
        .samples()
        .iter()
        .map(performance::Sample::capture)
        .chain(report.metadata())
        .map(|capture| capture.stdout().len() + capture.stderr().len())
        .sum();
    assert_eq!(report.total_capture_bytes(), retained);
    assert!(retained <= report.limits().max_total_capture_bytes);
    assert_eq!(
        report
            .samples()
            .iter()
            .map(performance::Sample::slot)
            .collect::<Vec<_>>(),
        report.schedule().slots()
    );
    let task = report
        .samples()
        .iter()
        .find(|sample| sample.slot().case == Case::TaskAllocation)
        .unwrap();
    assert_eq!(task.cost(), Some([5].as_slice()));
}

#[test]
fn observations_retain_their_actual_execution_context() {
    let fixture = Fixture::new(
        "printf 'captured-working-directory=%s\\n' \"$PWD\" >&2\n",
        |_| {},
    );
    let report = fixture.run();
    assert!(report.passed());
    let finished = report.finished_unix_ns().unwrap();
    for sample in report.samples() {
        let capture = sample.capture();
        let expected = match sample.slot().producer {
            Producer::Native => &fixture.native,
            Producer::Reference => &fixture.reference,
        };
        assert_eq!(capture.executable(), expected);
        let source = Path::new(capture.arguments().last().unwrap());
        assert!(source.starts_with(capture.directory()));
        assert!(source.ends_with(sample.slot().case.path()));
        assert!(!capture.directory().exists());
        let started = capture.started_unix_ns().unwrap();
        assert!((report.started_unix_ns()..=finished).contains(&started));
        if sample.slot().producer == Producer::Native {
            // The shell reports physical PWD; macOS temporary roots may use a
            // /var spelling whose surviving parent canonicalizes to /private/var.
            let physical_directory = capture
                .directory()
                .parent()
                .unwrap()
                .canonicalize()
                .unwrap()
                .join(capture.directory().file_name().unwrap());
            let actual = format!(
                "captured-working-directory={}\n",
                physical_directory.display()
            );
            assert!(String::from_utf8_lossy(capture.stderr()).contains(&actual));
        }
    }
    assert!(
        report
            .before()
            .iter()
            .any(|seal| seal.requested() == fixture.native)
    );
}

#[test]
fn invalid_utf8_diagnostics_are_not_silently_replaced() {
    let fixture = Fixture::new("", |_| {});
    fs::write(fixture.directory.path().join("statistics.txt"), [0xff]).unwrap();
    let report = fixture.run();
    let sample = report.samples().last().unwrap();
    assert_eq!(sample.decision(), Decision::InvalidDiagnostics);
    assert_eq!(sample.capture().stderr(), [0xff]);
    assert!(sample.detail().unwrap().contains("utf-8"));
}

#[test]
fn a_phase_record_does_not_substitute_for_exclusive_stages() {
    let fixture = Fixture::new("", |_| {});
    fs::write(
        fixture.directory.path().join("statistics.txt"),
        include_bytes!("support/phase_statistics.txt"),
    )
    .unwrap();
    let report = fixture.run();
    let sample = report.samples().last().unwrap();
    assert_eq!(sample.decision(), Decision::InvalidDiagnostics);
    assert!(
        sample
            .detail()
            .unwrap()
            .contains("missing native stage timings")
    );
}

#[test]
fn diagnostics_cannot_change_the_grounding_policy() {
    let fixture = Fixture::new("", |_| {});
    let path = fixture.directory.path().join("statistics.txt");
    let original = fs::read_to_string(&path).unwrap();
    assert!(original.contains("stage grounding_mode: eager"));
    fs::write(
        path,
        original.replace("stage grounding_mode: eager", "stage grounding_mode: mixed"),
    )
    .unwrap();
    let report = fixture.run();
    let sample = report.samples().last().unwrap();
    assert_eq!(sample.decision(), Decision::InvalidDiagnostics);
    assert!(sample.detail().unwrap().contains("not eager"));
}

#[test]
fn phase_and_stage_driver_intervals_must_agree() {
    let fixture = Fixture::new("", |_| {});
    let path = fixture.directory.path().join("statistics.txt");
    let original = fs::read_to_string(&path).unwrap();
    let changed = original.replace(
        "phase driver: elapsed_ns=1000",
        "phase driver: elapsed_ns=1001",
    );
    assert_ne!(changed, original);
    fs::write(path, changed).unwrap();
    let report = fixture.run();
    let sample = report.samples().last().unwrap();
    assert_eq!(sample.decision(), Decision::InvalidDiagnostics);
    assert!(sample.diagnostics().is_none());
    assert!(
        sample
            .detail()
            .unwrap()
            .contains("native phase/stage evidence")
    );
}

#[test]
fn exclusive_stage_totals_cannot_exceed_the_driver_interval() {
    let fixture = Fixture::new("", |_| {});
    let path = fixture.directory.path().join("statistics.txt");
    let original = fs::read_to_string(&path).unwrap();
    let changed = original.replace(
        "stage solving: calls=3; elapsed_ns=500",
        "stage solving: calls=3; elapsed_ns=1500",
    );
    assert_ne!(changed, original);
    fs::write(path, changed).unwrap();
    let report = fixture.run();
    let sample = report.samples().last().unwrap();
    assert_eq!(sample.decision(), Decision::InvalidDiagnostics);
    assert!(
        sample
            .detail()
            .unwrap()
            .contains("inconsistent exclusive stage timing partition")
    );
}

fn cli(fixture: &Fixture, repetitions: &str) -> zetesis_validation::process::Capture {
    use zetesis_validation::process::{self, Invocation, Limits};
    let arguments = [
        fixture.corpus.clone().into_os_string(),
        "--zetesis".into(),
        fixture.native.clone().into_os_string(),
        "--clingo".into(),
        fixture.reference.clone().into_os_string(),
        "--report".into(),
        fixture.report.clone().into_os_string(),
        "--warmups".into(),
        "0".into(),
        "--repetitions".into(),
        repetitions.into(),
    ];
    let outcome = process::invoke(
        Invocation {
            executable: Path::new(env!("CARGO_BIN_EXE_zetesis-perf")),
            arguments: &arguments,
            directory: fixture.directory.path(),
        },
        Limits {
            timeout: Duration::from_secs(15),
            max_output_bytes: 16_384,
            cleanup_timeout: Duration::from_secs(1),
        },
    )
    .unwrap();
    let (capture, pending) = outcome.into_parts();
    assert!(pending.is_none());
    assert_eq!(capture.stop(), process::Stop::Completed);
    assert!(capture.failure().is_none());
    assert!(capture.cleanup_failure().is_none());
    capture
}

#[test]
fn the_cli_publishes_its_complete_selected_campaign() {
    let fixture = Fixture::new("", |_| {});
    let capture = cli(&fixture, "1");
    assert_eq!(capture.exit().unwrap().code, Some(0));
    assert!(
        String::from_utf8_lossy(capture.stdout())
            .starts_with("pass: 15 retained observations; evidence ")
    );
    let view: Value = serde_json::from_slice(&fs::read(&fixture.report).unwrap()).unwrap();
    assert_eq!(view["passed"], true);
    assert_eq!(view["samples"].as_array().unwrap().len(), 15);
    assert_eq!(view["schedule"], json!({"warmups":0,"repetitions":1}));
}

#[test]
fn the_cli_does_not_publish_an_invalid_schedule() {
    let fixture = Fixture::new("", |_| {});
    let capture = cli(&fixture, "0");
    assert_eq!(capture.exit().unwrap().code, Some(2));
    assert!(capture.stdout().is_empty());
    assert!(String::from_utf8_lossy(capture.stderr()).contains("repetitions 1..=41"));
    assert!(!fixture.report.exists());
}

#[test]
fn the_cli_retains_failed_qualification_as_failed_evidence() {
    let fixture = Fixture::new("", |text| {
        *text = text.replace("assign(s,9)", "assign(s,8)");
    });
    let capture = cli(&fixture, "1");
    assert_eq!(capture.exit().unwrap().code, Some(1));
    assert!(
        String::from_utf8_lossy(capture.stdout())
            .starts_with("fail: 2 retained observations; evidence ")
    );
    let view: Value = serde_json::from_slice(&fs::read(&fixture.report).unwrap()).unwrap();
    assert_eq!(view["passed"], false);
    assert_eq!(view["samples"][1]["decision"], "model_mismatch");
}
