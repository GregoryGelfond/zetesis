//! Synthetic complete producers exercise report lifecycle, not ASP correctness.

use super::*;
use crate::performance::matrix::{Reference, ReferencePolicy, WorkloadLimits};
use std::fs;
use std::os::unix::fs::PermissionsExt;

const UNSAT: &str = r#"{"Result":"UNSATISFIABLE","Models":{"More":"no","Number":0},"Call":[{}]}"#;
const SAT: &str = r#"{"Result":"SATISFIABLE","Models":{"More":"no","Number":1},"Call":[{"Witnesses":[{"Value":[]}]}]}"#;

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn executable(path: &Path, body: &str) {
    fs::write(
        path,
        format!("#!/bin/sh\nif [ \"$#\" -eq 1 ]; then printf m; exit 0; fi\n{body}\n"),
    )
    .unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

struct Fixture {
    directory: tempfile::TempDir,
    corpus: PathBuf,
    native: PathBuf,
    reference: PathBuf,
    report: PathBuf,
    workload: Workload,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let native = directory.path().join("native");
        let reference = directory.path().join("reference");
        let report = directory.path().join("report.json");
        let accepted = sample().capture.unwrap();
        executable(
            &native,
            &format!(
                "printf '%s' {}; printf '%s' {} >&2",
                quote(std::str::from_utf8(accepted.stdout()).unwrap()),
                quote(std::str::from_utf8(accepted.stderr()).unwrap())
            ),
        );
        executable(&reference, &format!("printf '%s' {}", quote(UNSAT)));
        let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/correctness");
        let checked = examples::load(&corpus, examples::Limits::default()).unwrap();
        let path = "standalone/n-queens/variant-01.lp";
        // An amended workload takes its complete family from the independent
        // producer; it must not silently retain the original N=8 count contract.
        let workload = Workload::amended(
            &checked,
            path,
            &[crate::performance::matrix::ConstantAmendment {
                source_path: path,
                name: "n",
                expected: 8,
                replacement: 10,
            }],
            crate::performance::matrix::WorkloadLimits::default(),
        )
        .unwrap();
        Self {
            directory,
            corpus,
            native,
            reference,
            report,
            workload,
        }
    }

    fn request(&self) -> Request<'_> {
        Request {
            corpus: &self.corpus,
            native: &self.native,
            reference: Some(Reference {
                executable: &self.reference,
                policy: ReferencePolicy::AllPhases,
            }),
            report: &self.report,
            plan: Plan::new(
                Suite::Queens,
                vec![crate::selected::NativeExecution::default()],
                NonZeroUsize::new(1).unwrap(),
                1,
                2,
            )
            .unwrap(),
            ..request()
        }
    }

    fn run(&self, request: &Request<'_>) -> Report {
        crate::performance::matrix::run_workloads(request, std::slice::from_ref(&self.workload))
            .unwrap()
    }

    /// The same campaign over the whole corpus, with no clingo taking part.
    fn clingo_free(&self) -> Request<'_> {
        Request {
            reference: None,
            plan: Plan::new(
                Suite::Corpus,
                vec![crate::selected::NativeExecution::default()],
                NonZeroUsize::MIN,
                1,
                2,
            )
            .unwrap(),
            ..self.request()
        }
    }

    /// An unchanged corpus entry whose recorded contract is unsatisfiable,
    /// which the synthetic producers' answers satisfy.
    fn unchanged(&self) -> Workload {
        let checked = examples::load(&self.corpus, examples::Limits::default()).unwrap();
        Workload::original(
            &checked,
            "scenarios/shortest-path/variant-01/04-no-path.lp",
            WorkloadLimits::default(),
        )
        .unwrap()
    }
}

fn run_unchanged(fixture: &Fixture, request: &Request<'_>) -> Report {
    crate::performance::matrix::run_workloads(request, &[fixture.unchanged()]).unwrap()
}

#[test]
fn completed_matrix_publishes_its_exact_schedule_and_capture_total() {
    let fixture = Fixture::new();
    let report = fixture.run(&fixture.request());
    assert!(report.passed(), "{report:?}");
    assert_eq!(report.samples().len(), 8); // 2 producers × (census + warmup + 2 timed).
    assert_eq!(report.metadata().len(), 3);
    let retained: usize = report
        .metadata()
        .iter()
        .chain(
            report
                .samples()
                .iter()
                .map(|sample| sample.capture().unwrap()),
        )
        .map(|capture| capture.stdout().len() + capture.stderr().len())
        .sum();
    assert_eq!(report.total_capture_bytes(), retained);
    assert_eq!(
        report.workloads().unwrap()[0].identity(),
        fixture.workload.identity()
    );
    report.publish().unwrap();
    let published: Value = serde_json::from_slice(&fs::read(&fixture.report).unwrap()).unwrap();
    assert_eq!(published["passed"], true);
    assert_eq!(published["accounted"], true);
    assert_eq!(published["report"]["samples"].as_array().unwrap().len(), 8);
    assert_eq!(published["report"]["total_capture_bytes"], retained);
}

#[test]
fn qualification_campaign_retains_complete_checks_without_measurements() {
    let fixture = Fixture::new();
    let mut request = fixture.request();
    request.plan = Plan::qualification(
        Suite::Queens,
        vec![crate::selected::NativeExecution::default()],
        NonZeroUsize::MIN,
    )
    .unwrap();
    let report = fixture.run(&request);
    assert!(report.passed(), "{report:?}");
    assert_eq!(report.samples().len(), 2);
    assert!(
        report
            .samples()
            .iter()
            .all(|sample| sample.slot().phase == Phase::Qualification)
    );
    assert!(
        report
            .summary()
            .cells
            .iter()
            .all(|cell| cell.timing.is_none() && cell.peak_rss_bytes.is_none())
    );
    report.publish().unwrap();
    let published: Value = serde_json::from_slice(&fs::read(&fixture.report).unwrap()).unwrap();
    assert!(matches!(
        crate::performance::series::compare(&[crate::performance::series::Labelled {
            label: "qualification",
            report: &published,
        }]),
        Err(crate::performance::series::ViewError::NoTimedPopulation { .. })
    ));
}

#[test]
fn plain_scalability_run_refuses_to_omit_authored_inputs() {
    let fixture = Fixture::new();
    let mut request = fixture.request();
    request.plan = Plan::qualification(
        Suite::Scalability,
        vec![crate::selected::NativeExecution::default()],
        NonZeroUsize::MIN,
    )
    .unwrap();
    assert!(matches!(
        crate::performance::matrix::run(&request),
        Err(Error::Configuration(
            "scalability suite requires explicit authored workloads"
        ))
    ));
}

#[test]
fn cancelled_scalability_keeps_every_workload_and_requested_position() {
    let fixture = Fixture::new();
    let mut request = fixture.request();
    request.plan = Plan::qualification(
        Suite::Scalability,
        vec![crate::selected::NativeExecution::default()],
        NonZeroUsize::MIN,
    )
    .unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let report = crate::performance::scalability::run_with_cancellation(
        &request,
        &root,
        true,
        NativeInvocation::Legacy,
        &AtomicBool::new(true),
    )
    .unwrap();
    assert!(report.accounted());
    assert!(!report.passed());
    assert!(report.metadata().is_empty());
    assert_eq!(report.workloads().unwrap().len(), 10);
    assert_eq!(
        report
            .workloads()
            .unwrap()
            .iter()
            .filter(|workload| workload.is_authored())
            .count(),
        7
    );
    assert_eq!(report.samples().len(), 20);
    assert!(
        report
            .samples()
            .iter()
            .all(|sample| sample.capture().is_none()
                && sample.decision() == Decision::NotAttempted
                && sample.slot().phase == Phase::Qualification)
    );
    assert!(
        report
            .after()
            .iter()
            .all(crate::selected::Change::unchanged)
    );
}

#[test]
fn scalability_wrapper_preserves_explicit_admission_limits() {
    let fixture = Fixture::new();
    let mut request = fixture.request();
    request.plan = Plan::qualification(
        Suite::Scalability,
        vec![crate::selected::NativeExecution::default()],
        NonZeroUsize::MIN,
    )
    .unwrap();
    request.limits.corpus.manifest_bytes = 1;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    assert!(matches!(
        crate::performance::scalability::run_with_cancellation(
            &request,
            &root,
            false,
            NativeInvocation::Legacy,
            &AtomicBool::new(true),
        ),
        Err(Error::Corpus(_))
    ));
}

#[test]
fn qualification_only_never_launches_a_reference_measurement() {
    let fixture = Fixture::new();
    executable(
        &fixture.reference,
        &format!(
            "if [ -e reference-seen ]; then exit 65; fi; : > reference-seen; printf '%s' {}",
            quote(UNSAT)
        ),
    );
    let mut request = fixture.request();
    request.reference = Some(Reference {
        executable: &fixture.reference,
        policy: ReferencePolicy::QualificationOnly,
    });
    let report = fixture.run(&request);
    assert!(report.passed(), "{report:?}");
    assert_eq!(report.samples().len(), 5);
    let reference = report
        .summary()
        .cells
        .into_iter()
        .find(|cell| cell.producer == Producer::Reference)
        .unwrap();
    assert_eq!(reference.decisions.len(), 1);
    assert_eq!(reference.decisions[0].positions, 1);
    assert!(reference.timing.is_none());
    assert!(reference.peak_rss_bytes.is_none());
}

#[test]
fn a_clingo_free_campaign_schedules_no_reference() {
    let fixture = Fixture::new();
    let report = run_unchanged(&fixture, &fixture.clingo_free());
    // One native profile: its census, one warmup and two timed rounds.
    assert_eq!(report.samples().len(), 4, "{report:?}");
    assert!(
        report
            .samples()
            .iter()
            .all(|sample| sample.slot().producer != Producer::Reference)
    );
    assert_eq!(report.reference_policy(), None);
    // Clingo's version is owed only when clingo takes part.
    assert_eq!(report.metadata().len(), 2);
}

#[test]
fn a_clingo_free_campaign_whose_cases_all_qualify_passes() {
    let fixture = Fixture::new();
    let report = run_unchanged(&fixture, &fixture.clingo_free());
    assert!(report.passed(), "{report:?}");
    assert!(
        report
            .summary()
            .cells
            .iter()
            .all(|cell| cell.qualification == crate::performance::matrix::Qualification::Contract)
    );
}

#[test]
fn a_clingo_free_family_must_satisfy_the_recorded_contract() {
    // The unsatisfiable entry answered satisfiable: the contract is the authority.
    let fixture = Fixture::new();
    let (_, stderr) = crate::performance::matrix::fixtures::fixture();
    executable(
        &fixture.native,
        &format!(
            "printf '%s' {}; printf '%s' {} >&2",
            quote(&native_family::document(&["a"]).to_string()),
            quote(&stderr)
        ),
    );
    let report = run_unchanged(&fixture, &fixture.clingo_free());
    assert_eq!(
        report.samples()[0].decision,
        Decision::ParityMismatch,
        "{report:?}"
    );
    assert!(!report.passed());
}

#[test]
fn a_workload_without_a_contract_needs_clingo_and_is_not_launched() {
    let fixture = Fixture::new();
    // The amended board has no recorded contract; a launched solve would fail.
    executable(&fixture.native, "exit 70");
    let report = fixture.run(&fixture.clingo_free());
    let census = &report.samples()[0];
    assert_eq!(census.decision, Decision::NeedsClingo, "{report:?}");
    assert!(census.capture().is_none());
    assert!(
        report.samples()[1..]
            .iter()
            .all(|sample| sample.decision == Decision::NotAttempted && sample.blocked_by == Some(0))
    );
    assert!(report.accounted());
    assert!(!report.passed());
    assert!(
        report.summary().cells.iter().all(
            |cell| cell.qualification == crate::performance::matrix::Qualification::NeedsClingo
        )
    );
}

#[test]
fn a_reference_changes_the_schedule_only_by_its_own_positions() {
    let plan = Plan::new(
        Suite::Queens,
        vec![crate::selected::NativeExecution::default(); 2],
        NonZeroUsize::MIN,
        1,
        2,
    )
    .unwrap()
    .with_memory(1)
    .unwrap();
    let key = |slot: &Slot| {
        (
            slot.case,
            format!("{:?}", slot.phase),
            slot.round,
            slot.producer.index(),
        )
    };
    let mut without = plan.slots(3, None).unwrap();
    without.sort_by_key(key);
    assert!(
        without
            .iter()
            .all(|slot| slot.producer != Producer::Reference)
    );
    for (policy, references) in [
        // Every phase: census, warmup, two timed and one memory round per case.
        (ReferencePolicy::AllPhases, 3 * 5),
        (ReferencePolicy::QualificationOnly, 3),
    ] {
        let with = plan.slots(3, Some(policy)).unwrap();
        let mut natives: Vec<_> = with
            .iter()
            .copied()
            .filter(|slot| slot.producer != Producer::Reference)
            .collect();
        natives.sort_by_key(key);
        assert_eq!(natives, without, "{policy:?}");
        assert_eq!(with.len() - natives.len(), references, "{policy:?}");
    }
}

#[test]
fn a_saved_report_records_the_policy_its_run_used() {
    let fixture = Fixture::new();
    let clingo_free = run_unchanged(&fixture, &fixture.clingo_free());
    clingo_free.publish().unwrap();
    let published: Value = serde_json::from_slice(&fs::read(&fixture.report).unwrap()).unwrap();
    assert_eq!(
        published["report"]["plan"]["reference_policy"],
        "clingo_free"
    );
    // The schedule rebuilt from the recorded policy is the one the run executed.
    assert_eq!(published["accounted"], true);

    let with_clingo_path = fixture.directory.path().join("with-clingo.json");
    let request = Request {
        report: &with_clingo_path,
        ..fixture.request()
    };
    let with_clingo = fixture.run(&request);
    with_clingo.publish().unwrap();
    let published: Value = serde_json::from_slice(&fs::read(&with_clingo_path).unwrap()).unwrap();
    assert_eq!(
        published["report"]["plan"]["reference_policy"],
        "all_phases"
    );
    assert_eq!(published["accounted"], true);
}

fn changed_hidden_family(profiles: usize) -> Report {
    let fixture = Fixture::new();
    executable(&fixture.reference, &format!("printf '%s' {}", quote(SAT)));
    let (_, stderr) = crate::performance::matrix::fixtures::fixture();
    executable(
        &fixture.native,
        &format!(
            "if [ -e native-seen ]; then printf '%s' {}; else : > native-seen; printf '%s' {}; fi; printf '%s' {} >&2",
            quote(&native_family::document(&["b"]).to_string()),
            quote(&native_family::document(&["a"]).to_string()),
            quote(&stderr)
        ),
    );
    let mut request = fixture.request();
    request.plan = Plan::new(
        Suite::Queens,
        vec![crate::selected::NativeExecution::default(); profiles],
        NonZeroUsize::MIN,
        1,
        2,
    )
    .unwrap();
    fixture.run(&request)
}

#[test]
fn native_qualification_compares_hidden_atoms_across_profiles() {
    let report = changed_hidden_family(2);
    assert!(report.accounted());
    assert!(!report.passed());
    let samples: Vec<_> = report
        .samples()
        .iter()
        .filter(|sample| {
            sample.slot().phase == Phase::Qualification
                && matches!(sample.slot().producer, Producer::Native { .. })
        })
        .collect();
    assert_eq!(samples[0].decision(), Decision::Pass);
    assert_eq!(samples[1].decision(), Decision::ParityMismatch);
    assert_eq!(samples[0].selected_models, samples[1].selected_models);
    assert!(samples[1].capture().is_some());
    assert!(
        report
            .samples()
            .iter()
            .filter(|sample| sample.slot().phase != Phase::Qualification
                && sample.slot().producer == Producer::Native { profile: 1 })
            .all(|sample| sample.decision() == Decision::NotAttempted)
    );
}

#[test]
fn repeated_native_hidden_mismatch_stops_that_cell() {
    let report = changed_hidden_family(1);
    let failed = report
        .samples()
        .iter()
        .position(|sample| {
            sample.slot().phase == Phase::Warmup
                && sample.slot().producer == Producer::Native { profile: 0 }
        })
        .unwrap();
    assert_eq!(
        report.samples()[failed].decision(),
        Decision::ParityMismatch
    );
    assert!(report.accounted());
    assert!(!report.passed());
    for sample in report.samples().iter().filter(|sample| {
        sample.slot().phase == Phase::Timed
            && sample.slot().producer == Producer::Native { profile: 0 }
    }) {
        assert_eq!(sample.decision(), Decision::NotAttempted);
        assert_eq!(sample.blocked_by(), Some(failed));
        assert!(sample.capture().is_none());
    }
}

#[test]
fn later_reference_mismatch_disables_only_its_own_cell() {
    let fixture = Fixture::new();
    executable(
        &fixture.reference,
        &format!(
            "if [ -e reference-seen ]; then printf '%s' {}; else : > reference-seen; printf '%s' {}; fi",
            quote(SAT),
            quote(UNSAT)
        ),
    );
    let report = fixture.run(&fixture.request());
    assert!(report.accounted());
    assert!(!report.passed());
    let failed = report
        .samples()
        .iter()
        .position(|sample| {
            sample.slot().phase == Phase::Warmup && sample.slot().producer == Producer::Reference
        })
        .unwrap();
    assert_eq!(
        report.samples()[failed].decision(),
        Decision::ParityMismatch
    );
    for sample in report.samples() {
        if matches!(sample.slot().producer, Producer::Native { .. }) {
            assert_eq!(sample.decision(), Decision::Pass);
        } else if sample.slot().phase == Phase::Timed {
            assert_eq!(sample.decision(), Decision::NotAttempted);
            assert_eq!(sample.blocked_by(), Some(failed));
            assert!(sample.capture().is_none());
        }
    }
    assert!(report.faults().is_empty());
    assert!(report.unresolved_children().is_empty());
}

#[test]
fn invalid_native_census_is_retained_and_never_replaced() {
    let fixture = Fixture::new();
    executable(&fixture.native, "printf '{malformed'; printf diagnosis >&2");
    let report = fixture.run(&fixture.request());
    assert!(report.accounted());
    let failed = report
        .samples()
        .iter()
        .position(|sample| {
            sample.slot().phase == Phase::Qualification
                && matches!(sample.slot().producer, Producer::Native { .. })
        })
        .unwrap();
    assert_eq!(report.samples()[failed].decision(), Decision::InvalidReport);
    let capture = report.samples()[failed].capture().unwrap();
    assert_eq!(capture.stdout(), b"{malformed");
    assert_eq!(capture.stderr(), b"diagnosis");
    for sample in report.samples() {
        if sample.slot().producer == Producer::Reference {
            assert_eq!(sample.decision(), Decision::Pass);
        } else if sample.slot().phase != Phase::Qualification {
            assert_eq!(sample.decision(), Decision::NotAttempted);
            assert_eq!(sample.blocked_by(), Some(failed));
            assert!(sample.capture().is_none());
        }
    }
}

#[test]
fn failed_reference_census_keeps_native_answers_unqualified() {
    for (body, expected) in [
        ("printf '{malformed'".to_owned(), Decision::InvalidReport),
        (
            format!("printf '%s' {}; exit 65", quote(UNSAT)),
            Decision::InvocationFailure,
        ),
    ] {
        let fixture = Fixture::new();
        executable(&fixture.reference, &body);
        let report = fixture.run(&fixture.request());
        assert!(report.accounted());
        assert!(!report.passed());
        let samples = report.samples();
        assert_eq!(samples.len(), 8);
        let reference = samples
            .iter()
            .position(|sample| {
                sample.slot().phase == Phase::Qualification
                    && sample.slot().producer == Producer::Reference
            })
            .unwrap();
        let native = samples
            .iter()
            .position(|sample| {
                sample.slot().phase == Phase::Qualification
                    && matches!(sample.slot().producer, Producer::Native { .. })
            })
            .unwrap();
        assert_eq!(samples[reference].decision(), expected);
        assert!(samples[reference].capture().is_some());
        // Its complete native answer and actual route observation are retained,
        // but cannot establish parity without an independent complete family.
        assert_eq!(samples[native].decision(), Decision::ReferenceUnavailable);
        assert_eq!(samples[native].selected_models, Some(0));
        assert!(samples[native].observation().is_some());
        assert!(samples[native].capture().unwrap().complete(false));
        for sample in samples {
            if sample.slot().phase != Phase::Qualification {
                assert_eq!(sample.decision(), Decision::NotAttempted);
                assert!(sample.capture().is_none());
                let cause = if sample.slot().producer == Producer::Reference {
                    reference
                } else {
                    native
                };
                assert_eq!(sample.blocked_by(), Some(cause));
            }
        }
        assert!(report.faults().is_empty());
        assert!(report.unresolved_children().is_empty());
        report.publish().unwrap();
        let published: Value = serde_json::from_slice(&fs::read(&fixture.report).unwrap()).unwrap();
        assert_eq!(published["passed"], false);
        assert_eq!(published["accounted"], true);
    }
}

#[test]
fn exact_metadata_budget_cannot_launch_a_replacement_sample() {
    let fixture = Fixture::new();
    let mut request = fixture.request();
    request.limits.max_total_capture_bytes = 3; // Three successful one-byte metadata replies.
    let report = fixture.run(&request);
    assert!(report.accounted());
    assert_eq!(report.metadata().len(), 3);
    assert!(
        report
            .metadata()
            .iter()
            .all(|capture| capture.complete(false))
    );
    assert_eq!(report.total_capture_bytes(), 3);
    assert!(matches!(report.faults(), [Fault::CaptureBudget]));
    assert!(report.samples().iter().all(|sample| {
        sample.decision() == Decision::NotAttempted
            && sample.capture().is_none()
            && sample.blocked_by().is_none()
    }));
    report.publish().unwrap();
    assert!(!report.passed());
}

#[test]
fn cancelled_matrix_launches_no_metadata() {
    let fixture = Fixture::new();
    let report = crate::performance::matrix::run_workloads_with_cancellation(
        &fixture.request(),
        std::slice::from_ref(&fixture.workload),
        NativeInvocation::Legacy,
        &AtomicBool::new(true),
    )
    .unwrap();
    assert!(report.metadata().is_empty());
    assert!(report.accounted());
    assert!(!report.passed());
    assert!(matches!(report.faults(), [Fault::Cancelled]));
    assert!(report.samples().iter().all(|sample| sample.capture().is_none()
        && sample.decision() == Decision::NotAttempted));
}

#[test]
fn cancelled_matrix_retains_the_active_position() {
    let fixture = Fixture::new();
    let ready = fixture.directory.path().join("ready");
    executable(
        &fixture.reference,
        &format!(": > {}; exec sleep 5", quote(ready.to_str().unwrap())),
    );
    let cancelled = AtomicBool::new(false);
    let (report, observed) = std::thread::scope(|scope| {
        let trigger = scope.spawn(|| {
            let deadline = Instant::now() + std::time::Duration::from_secs(3);
            while !ready.exists() && Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            let observed = ready.exists();
            cancelled.store(true, Ordering::Relaxed);
            observed
        });
        let report = crate::performance::matrix::run_workloads_with_cancellation(
            &fixture.request(),
            std::slice::from_ref(&fixture.workload),
            NativeInvocation::Legacy,
            &cancelled,
        );
        (report.unwrap(), trigger.join().unwrap())
    });
    assert!(observed);
    assert!(report.accounted());
    assert!(!report.passed());
    assert!(report.unresolved_children().is_empty());
    let attempted: Vec<_> = report
        .samples()
        .iter()
        .filter(|sample| sample.capture().is_some())
        .collect();
    assert_eq!(attempted.len(), 1);
    assert_eq!(attempted[0].decision(), Decision::Cancelled);
    assert_eq!(
        attempted[0].capture().unwrap().stop(),
        Some(process::Stop::Cancelled)
    );
    assert_eq!(
        attempted[0].capture().unwrap().exit().unwrap().signal,
        Some(9)
    );
    assert!(
        report
            .samples()
            .iter()
            .filter(|sample| sample.capture().is_none())
            .all(|sample| sample.decision() == Decision::NotAttempted)
    );
    report.publish().unwrap();
    let published: Value = serde_json::from_slice(&fs::read(&fixture.report).unwrap()).unwrap();
    assert_eq!(published["passed"], false);
    assert_eq!(published["accounted"], true);
}
