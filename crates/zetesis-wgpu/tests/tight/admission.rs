use super::*;
use zetesis_ferraris::{AdmissionLimits, Node, Theory, TightPlanLimits};

fn certificate() -> TightPlan {
    let theory = Theory::new(1, vec![Node::Atom(0)], vec![0], AdmissionLimits::default()).unwrap();
    TightPlan::compile(&theory, TightPlanLimits::default(), &Control::default()).unwrap()
}

#[test]
fn cold_admission_accounts_graph_upload() {
    let certificate = certificate();
    let input = [Interpretation::new(certificate.theory(), [0]).unwrap()];
    let admission = Admission::new(
        &certificate,
        &input,
        TightGpuLimits::default(),
        &wgpu::Limits::default(),
        None,
        0,
        &Control::default(),
    )
    .unwrap();
    assert!(admission.fresh.is_some());
    assert!(admission.stats.theory_uploaded);
    assert!(admission.stats.transport_allocated);
    assert_eq!(admission.stats.candidates, 1);
    assert_eq!(admission.stats.work, 5);
    assert_eq!(admission.stats.dispatches, 1);
    assert_eq!(admission.stats.uploaded_bytes, 32 + 4 + 16 + 4 + 16);
    assert_eq!(admission.stats.downloaded_bytes, 24);
    assert_eq!(admission.plan.epoch, 1);
}

#[test]
fn matching_identity_reuses_immutable_upload() {
    let certificate = certificate();
    let graph = Graph::new(&certificate, &wgpu::Limits::default()).unwrap();
    let input = [Interpretation::new(certificate.theory(), [0]).unwrap()];
    let admission = Admission::new(
        &certificate,
        &input,
        TightGpuLimits::default(),
        &wgpu::Limits::default(),
        Some((&graph, true)),
        4,
        &Control::default(),
    )
    .unwrap();
    assert!(admission.fresh.is_none());
    assert!(!admission.stats.theory_uploaded);
    assert!(!admission.stats.transport_allocated);
    assert_eq!(admission.stats.uploaded_bytes, 36);
    assert_eq!(admission.plan.epoch, 5);
}

#[test]
fn changed_batch_shape_replaces_only_transport() {
    let certificate = certificate();
    let graph = Graph::new(&certificate, &wgpu::Limits::default()).unwrap();
    let input = [Interpretation::new(certificate.theory(), [0]).unwrap()];
    let admission = Admission::new(
        &certificate,
        &input,
        TightGpuLimits::default(),
        &wgpu::Limits::default(),
        Some((&graph, false)),
        0,
        &Control::default(),
    )
    .unwrap();
    assert!(admission.fresh.is_none());
    assert!(!admission.stats.theory_uploaded);
    assert!(admission.stats.transport_allocated);
}

#[test]
fn independent_equal_theories_replace_the_graph() {
    let old = certificate();
    let new = certificate();
    let graph = Graph::new(&old, &wgpu::Limits::default()).unwrap();
    let input = [Interpretation::new(new.theory(), [0]).unwrap()];
    let admission = Admission::new(
        &new,
        &input,
        TightGpuLimits::default(),
        &wgpu::Limits::default(),
        Some((&graph, true)),
        1,
        &Control::default(),
    )
    .unwrap();
    assert!(admission.fresh.is_some());
    assert!(admission.stats.theory_uploaded);
    assert!(admission.stats.transport_allocated);
}

#[test]
fn foreign_candidates_refuse_cache_replacement() {
    let old = certificate();
    let new = certificate();
    let graph = Graph::new(&old, &wgpu::Limits::default()).unwrap();
    let input = [Interpretation::new(old.theory(), [0]).unwrap()];
    let error = Admission::new(
        &new,
        &input,
        TightGpuLimits::default(),
        &wgpu::Limits::default(),
        Some((&graph, true)),
        1,
        &Control::default(),
    )
    .err()
    .unwrap();
    assert_eq!(error.kind(), GpuErrorKind::Seed);
    assert!(graph.theory.same_instance(old.theory()));
}

#[test]
fn exhausted_epochs_never_wrap_to_old_receipts() {
    let error = Admission::new(
        &certificate(),
        &[],
        TightGpuLimits::default(),
        &wgpu::Limits::default(),
        None,
        u32::MAX,
        &Control::default(),
    )
    .err()
    .unwrap();
    assert_eq!(error.kind(), GpuErrorKind::Capacity);
    assert_eq!(error.detail(), "tight epoch exhausted");
}

#[test]
fn cancelled_admission_never_creates_a_plan() {
    let control = Control::default();
    control.cancel();
    assert_eq!(
        Admission::new(
            &certificate(),
            &[],
            TightGpuLimits::default(),
            &wgpu::Limits::default(),
            None,
            0,
            &control
        )
        .err()
        .unwrap()
        .interruption,
        Some(zetesis_cpu::Stop::Cancelled)
    );
}
