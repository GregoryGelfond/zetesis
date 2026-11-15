//! Typed inputs use the existing cancellation and sealed-input lifecycle.

use super::*;

fn typed(fixture: &Fixture) -> (PathBuf, Vec<Workload>) {
    let path = fixture.directory.path().join("selection.json");
    fs::write(
        &path,
        serde_json::to_vec(&json!({
            "schema_version":2,"sources":[],"cases":[
                {"kind":"generated","family":"chain","size":4},
                {"kind":"generated","family":"chain","size":5}
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let workloads = crate::performance::bundles::load(&path, WorkloadLimits::default()).unwrap();
    (path, workloads)
}

#[test]
fn cancelled_typed_selection_keeps_every_position() {
    let fixture = Fixture::new();
    let (path, workloads) = typed(&fixture);
    let report = crate::performance::matrix::run_workloads_with_cancellation(
        &fixture.request(),
        &workloads,
        NativeInvocation::Legacy,
        &AtomicBool::new(true),
    )
    .unwrap();
    assert!(report.metadata().next().is_none());
    assert!(report.accounted());
    assert!(!report.passed());
    assert!(matches!(report.faults(), [Fault::Cancelled]));
    assert_eq!(
        report
            .workloads()
            .unwrap()
            .iter()
            .map(Workload::identity)
            .collect::<Vec<_>>(),
        workloads.iter().map(Workload::identity).collect::<Vec<_>>()
    );
    assert_eq!(report.samples().len(), 16);
    assert!(report.samples().iter().all(|sample| sample.capture().is_none()
        && sample.decision() == Decision::NotAttempted));
    assert_eq!(
        report
            .before()
            .iter()
            .filter(|seal| seal.requested() == path)
            .count(),
        2
    );
    assert!(
        report
            .after()
            .iter()
            .all(crate::selected::Change::unchanged)
    );
}

#[test]
fn typed_selection_mutation_invalidates_existing_seals() {
    let fixture = Fixture::new();
    let (path, workloads) = typed(&fixture);
    let corpus = examples::load(&fixture.corpus, examples::Limits::default()).unwrap();
    let before = seal_inputs(
        &corpus,
        &BTreeSet::new(),
        &fixture.request(),
        Some(&workloads),
    )
    .unwrap();
    assert_eq!(
        before
            .iter()
            .filter(|seal| seal.requested() == path)
            .count(),
        2
    );
    assert!(
        before
            .iter()
            .map(identity::recheck)
            .all(|change| change.unchanged())
    );
    let mut bytes = fs::read(&path).unwrap();
    bytes.push(b' ');
    fs::write(&path, bytes).unwrap();
    let after: Vec<_> = before.iter().map(identity::recheck).collect();
    for (prior, current) in before.iter().zip(&after) {
        assert_eq!(current.unchanged(), prior.requested() != path);
    }
    assert!(workloads.iter().all(|workload| {
        workload
            .validate(&corpus, crate::performance::Limits::default())
            .is_err()
    }));
}
