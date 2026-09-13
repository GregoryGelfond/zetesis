//! Synthetic complete producers exercise report lifecycle, not ASP correctness.

use super::*;
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
    _directory: tempfile::TempDir,
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
        let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/kr-domains");
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
            _directory: directory,
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
            reference: &self.reference,
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
