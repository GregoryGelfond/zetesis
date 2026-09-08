//! Explicit physical qualification of complete-theory ranked support checking.
//! These tests select Metal or Vulkan; adapter absence never silently skips qualification.

#[path = "support/physical.rs"]
mod physical;

use zetesis_cpu::{Control, Stop};
use zetesis_ferraris::{
    AdmissionLimits, Interpretation, Limits, Node, Theory, TightCheckLimits, TightPlan,
    TightPlanLimits, TightVerdict, Verdict, check,
};
use zetesis_wgpu::{
    GpuErrorKind, GpuOptions, GpuTightOracle, TightGpuActivity, TightGpuError, TightGpuLimits,
};

fn certificate(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> TightPlan {
    let theory = Theory::new(atoms, nodes, roots, AdmissionLimits::default()).unwrap();
    TightPlan::compile(&theory, TightPlanLimits::default(), &Control::default()).unwrap()
}

fn oracle(backend: physical::Backend) -> GpuTightOracle {
    let oracle = GpuTightOracle::new_selected(GpuOptions::default(), backend.selection()).unwrap();
    backend.verify(oracle.info());
    println!("tight support adapter={}", oracle.info().name());
    oracle
}

fn inputs(theory: &Theory) -> Vec<Interpretation> {
    assert!(theory.atom_count() <= 4);
    let mut candidates: Vec<_> = (0..1usize << theory.atom_count())
        .map(|bits| {
            Interpretation::new(
                theory,
                (0..theory.atom_count()).filter(|atom| bits & (1 << atom) != 0),
            )
            .unwrap()
        })
        .collect();
    // Preserve duplicates and reverse order on the second call. This detects
    // accidental set semantics or stale per-world truth/support on reuse.
    candidates.push(candidates[0].clone());
    candidates
}

fn compare(oracle: &mut GpuTightOracle, certificate: &TightPlan) -> usize {
    let theory = certificate.theory();
    let mut candidates = inputs(theory);
    for repeat in 0..2 {
        let results = oracle
            .check_batch(
                certificate,
                &candidates,
                TightGpuLimits::default(),
                &Control::default(),
            )
            .unwrap();
        assert_eq!(results.len(), candidates.len());
        let stats = *oracle.last_batch_stats().unwrap();
        assert_eq!(stats.dispatches, 1);
        assert_eq!(stats.candidates, candidates.len() as u64);
        assert_eq!(
            stats.work,
            results.iter().map(zetesis_wgpu::TightGpuCheck::work).sum()
        );
        assert_eq!(stats.theory_uploaded, repeat == 0);
        assert_eq!(stats.transport_allocated, repeat == 0);
        let activity = oracle.activity();
        assert_eq!(activity.submissions, 1);
        assert_eq!(activity.submitted_candidates, stats.candidates);
        assert_eq!(activity.completed_candidates, stats.candidates);
        assert_eq!(activity.scheduled_work, stats.work);
        assert_eq!(activity.completed_work, stats.work);
        assert_eq!(activity.uploaded_bytes, stats.uploaded_bytes);
        assert_eq!(activity.downloaded_bytes, stats.downloaded_bytes);
        for (candidate, result) in candidates.iter().zip(&results) {
            let scalar = certificate
                .check(candidate, TightCheckLimits::default(), &Control::default())
                .unwrap();
            assert_eq!(
                result.verdict(),
                scalar.verdict,
                "candidate {:?}",
                candidate.atoms().collect::<Vec<_>>()
            );
            let exact = check(theory, candidate, Limits::default(), &Control::default()).unwrap();
            match result.verdict() {
                TightVerdict::Stable => assert!(exact.accepted()),
                TightVerdict::NotModel { root } => {
                    assert!(
                        matches!(exact.verdict(), Verdict::NotModel { root: actual } if *actual == root)
                    );
                }
                TightVerdict::Residual { .. } => {
                    assert!(!matches!(exact.verdict(), Verdict::NotModel { .. }));
                    assert!(!exact.accepted());
                }
            }
        }
        candidates.reverse();
    }
    candidates.len() * 2
}

fn fixtures() -> Vec<TightPlan> {
    let mut fixtures = vec![
        certificate(0, vec![], vec![]),
        certificate(0, vec![Node::False], vec![0]),
        certificate(2, vec![Node::Atom(0), Node::Atom(1)], vec![1, 0, 1]),
        // Double negation alone supplies no producer for its true atom.
        certificate(
            1,
            vec![
                Node::False,
                Node::Atom(0),
                Node::Implies(1, 0),
                Node::Implies(2, 0),
            ],
            vec![3],
        ),
    ];
    // Positive bodies mention lower-ranked atoms; a fourth carrier atom has no
    // producer. Every body form is evaluated with both truth values where possible.
    for body in [0, 1, 2, 4, 9, 10, 11] {
        let mut nodes = vec![
            Node::False,
            Node::Atom(0),
            Node::Atom(1),
            Node::Atom(2),
            Node::Implies(1, 0),
            Node::Implies(2, 0),
            Node::Implies(3, 0),
            Node::Or(1, 4),
            Node::Or(5, 2),
            Node::And(1, 2),
            Node::Or(1, 2),
            Node::Implies(4, 0),
        ];
        nodes.push(Node::Implies(body, 3));
        fixtures.push(certificate(4, nodes, vec![12, 8, 7]));
    }
    fixtures
}

#[test]
fn tight_fixture_verdicts_match_exhaustive_reducts() {
    let mut census = [0usize; 3];
    for certificate in fixtures() {
        for candidate in inputs(certificate.theory()) {
            let scalar = certificate
                .check(&candidate, TightCheckLimits::default(), &Control::default())
                .unwrap();
            let exact = check(
                certificate.theory(),
                &candidate,
                Limits::default(),
                &Control::default(),
            )
            .unwrap();
            match scalar.verdict {
                TightVerdict::Stable => {
                    assert!(exact.accepted());
                    census[0] += 1;
                }
                TightVerdict::NotModel { root } => {
                    assert!(
                        matches!(exact.verdict(),Verdict::NotModel{root:actual} if *actual==root)
                    );
                    census[1] += 1;
                }
                TightVerdict::Residual { .. } => {
                    assert!(!exact.accepted());
                    assert!(!matches!(exact.verdict(), Verdict::NotModel { .. }));
                    census[2] += 1;
                }
            }
        }
    }
    assert_eq!(census, [37, 33, 61]);
}

#[test]
#[ignore = "requires actual Metal; explicit physical qualification"]
fn metal_support_matches_exact_reduct_semantics() {
    qualify_support_matches_exact_reduct_semantics(physical::Backend::Metal);
}

#[test]
#[ignore = "requires an actual Vulkan GPU; explicit physical qualification"]
fn vulkan_support_matches_exact_reduct_semantics() {
    qualify_support_matches_exact_reduct_semantics(physical::Backend::Vulkan);
}

fn qualify_support_matches_exact_reduct_semantics(backend: physical::Backend) {
    let mut oracle = oracle(backend);
    let compared: usize = fixtures()
        .iter()
        .map(|certificate| compare(&mut oracle, certificate))
        .sum();
    assert_eq!(compared, 262);
    println!("tight support exact candidate occurrences={compared}");
}

#[test]
#[ignore = "requires actual Metal; explicit physical qualification"]
fn metal_support_preserves_batch_isolation() {
    qualify_support_preserves_batch_isolation(physical::Backend::Metal);
}

#[test]
#[ignore = "requires an actual Vulkan GPU; explicit physical qualification"]
fn vulkan_support_preserves_batch_isolation() {
    qualify_support_preserves_batch_isolation(physical::Backend::Vulkan);
}

fn qualify_support_preserves_batch_isolation(backend: physical::Backend) {
    let mut oracle = oracle(backend);
    for atoms in [1, 31, 32, 33, 63, 64, 65, 4097] {
        let certificate = certificate(atoms, vec![Node::Atom(atoms - 1)], vec![0]);
        let theory = certificate.theory();
        let patterns = [
            Interpretation::new(theory, []).unwrap(),
            Interpretation::new(theory, [atoms - 1]).unwrap(),
            Interpretation::new(theory, 0..atoms).unwrap(),
        ];
        for worlds in [1, 31, 32, 33, 63, 64, 65] {
            let candidates: Vec<_> = (0..worlds)
                .map(|world| patterns[world % 3].clone())
                .collect();
            let results = oracle
                .check_batch(
                    &certificate,
                    &candidates,
                    TightGpuLimits::default(),
                    &Control::default(),
                )
                .unwrap();
            assert_eq!(results.len(), worlds);
            for (candidate, result) in candidates.iter().zip(results) {
                assert_eq!(
                    result.verdict(),
                    certificate
                        .check(candidate, TightCheckLimits::default(), &Control::default())
                        .unwrap()
                        .verdict
                );
            }
            let stats = oracle.last_batch_stats().unwrap();
            assert_eq!(stats.theory_uploaded, worlds == 1);
            assert!(stats.transport_allocated);
        }
    }
}

#[test]
#[ignore = "requires actual Metal; explicit physical qualification"]
fn metal_support_refusals_preserve_reusable_residency() {
    qualify_support_refusals_preserve_reusable_residency(physical::Backend::Metal);
}

#[test]
#[ignore = "requires an actual Vulkan GPU; explicit physical qualification"]
fn vulkan_support_refusals_preserve_reusable_residency() {
    qualify_support_refusals_preserve_reusable_residency(physical::Backend::Vulkan);
}

fn qualify_support_refusals_preserve_reusable_residency(backend: physical::Backend) {
    let mut oracle = oracle(backend);
    let certificate = certificate(1, vec![Node::Atom(0)], vec![0]);
    let candidate = Interpretation::new(certificate.theory(), [0]).unwrap();
    let input = [candidate];
    let expected = oracle
        .check_batch(
            &certificate,
            &input,
            TightGpuLimits::default(),
            &Control::default(),
        )
        .unwrap();
    oracle
        .check_batch(
            &certificate,
            &input,
            TightGpuLimits::default(),
            &Control::default(),
        )
        .unwrap();
    let bytes = oracle.last_batch_stats().unwrap().accounted_bytes;
    for limits in [
        TightGpuLimits {
            max_batch_bytes: bytes - 1,
            ..Default::default()
        },
        TightGpuLimits {
            max_work_per_candidate: expected[0].work() - 1,
            ..Default::default()
        },
        TightGpuLimits {
            max_candidates: 0,
            ..Default::default()
        },
    ] {
        let error = oracle
            .check_batch(&certificate, &input, limits, &Control::default())
            .unwrap_err();
        assert!(
            matches!(error,TightGpuError::Gpu(ref error) if error.kind()==GpuErrorKind::Capacity)
        );
        assert_eq!(oracle.activity(), TightGpuActivity::default());
        assert!(oracle.last_batch_stats().is_none());
    }
    let cancelled = Control::default();
    cancelled.cancel();
    assert_eq!(
        oracle
            .check_batch(&certificate, &input, TightGpuLimits::default(), &cancelled)
            .unwrap_err(),
        TightGpuError::Stopped(Stop::Cancelled)
    );
    assert_eq!(oracle.activity(), TightGpuActivity::default());
    let tight = TightGpuLimits {
        max_batch_bytes: bytes,
        max_work_per_candidate: expected[0].work(),
        max_candidates: 1,
        ..Default::default()
    };
    assert_eq!(
        oracle
            .check_batch(&certificate, &input, tight, &Control::default())
            .unwrap(),
        expected
    );
    assert!(!oracle.last_batch_stats().unwrap().theory_uploaded);
    assert!(!oracle.last_batch_stats().unwrap().transport_allocated);
}

#[test]
#[ignore = "requires actual Metal; explicit physical qualification"]
fn metal_support_residency_tracks_theory_identity() {
    qualify_support_residency_tracks_theory_identity(physical::Backend::Metal);
}

#[test]
#[ignore = "requires an actual Vulkan GPU; explicit physical qualification"]
fn vulkan_support_residency_tracks_theory_identity() {
    qualify_support_residency_tracks_theory_identity(physical::Backend::Vulkan);
}

fn qualify_support_residency_tracks_theory_identity(backend: physical::Backend) {
    let mut oracle = oracle(backend);
    let certificate = certificate(1, vec![Node::Atom(0)], vec![0]);
    let input = [Interpretation::new(certificate.theory(), [0]).unwrap()];
    let expected = oracle
        .check_batch(
            &certificate,
            &input,
            TightGpuLimits::default(),
            &Control::default(),
        )
        .unwrap();
    let foreign = Theory::new(1, vec![Node::Atom(0)], vec![0], AdmissionLimits::default()).unwrap();
    let error = oracle
        .check_batch(
            &certificate,
            &[Interpretation::new(&foreign, [0]).unwrap()],
            TightGpuLimits::default(),
            &Control::default(),
        )
        .unwrap_err();
    assert!(matches!(error,TightGpuError::Gpu(ref error) if error.kind()==GpuErrorKind::Seed));
    assert_eq!(oracle.activity(), TightGpuActivity::default());
    let new_certificate =
        TightPlan::compile(&foreign, TightPlanLimits::default(), &Control::default()).unwrap();
    let new_input = [Interpretation::new(&foreign, [0]).unwrap()];
    assert_eq!(
        oracle
            .check_batch(
                &new_certificate,
                &new_input,
                TightGpuLimits::default(),
                &Control::default()
            )
            .unwrap(),
        expected
    );
    assert!(oracle.last_batch_stats().unwrap().theory_uploaded);
    let cancelled = Control::default();
    cancelled.cancel();
    assert_eq!(
        oracle
            .check_batch(&certificate, &[], TightGpuLimits::default(), &cancelled)
            .unwrap_err(),
        TightGpuError::Stopped(Stop::Cancelled)
    );
    assert!(
        oracle
            .check_batch(
                &certificate,
                &[],
                TightGpuLimits {
                    max_batch_bytes: 0,
                    max_work_per_candidate: 0,
                    max_candidates: 0,
                    ..Default::default()
                },
                &Control::default()
            )
            .unwrap()
            .is_empty()
    );
    assert_eq!(oracle.activity(), TightGpuActivity::default());
    assert!(oracle.last_batch_stats().is_none());
    oracle.clear_residency();
    assert_eq!(
        oracle
            .check_batch(
                &certificate,
                &input,
                TightGpuLimits::default(),
                &Control::default()
            )
            .unwrap(),
        expected
    );
    assert!(oracle.last_batch_stats().unwrap().theory_uploaded);
}
