//! Bounded synthetic producers exercise the observation/evidence contracts.
#![cfg(any(target_os = "linux", target_os = "macos"))]

use serde_json::{Value, json};
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
