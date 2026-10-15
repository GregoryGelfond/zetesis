//! Explicit physical qualification of complete-theory ranked support checking.
//! These tests select Metal or Vulkan; adapter absence never silently skips qualification.

mod native;

use crate::support::physical;

use zetesis_backend::GpuApi;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{
    AdmissionLimits, Interpretation, Limits, Node, Theory, TightCheckLimits, TightPlan,
    TightPlanLimits, TightVerdict, Verdict, check,
};
use zetesis_wgpu::{
    GpuErrorKind, GpuKernelOptions, GpuOptions, GpuSelection, GpuTightOracle, GpuWorkgroupSize,
    TightGpuActivity, TightGpuError, TightGpuLimits, TightSupport,
};

fn certificate(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> TightPlan {
    let theory = Theory::new(
        atoms,
        zetesis_ferraris::FormulaParts::new(nodes, Vec::new()).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap();
    TightPlan::compile(
        &theory,
        TightPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap()
}

fn oracle(backend: GpuApi, support: TightSupport) -> GpuTightOracle {
    let oracle = if support == TightSupport::Atomic {
        let context = zetesis_wgpu::GpuContext::new_selected_with_timing(
            GpuOptions::default(),
            GpuSelection { api: backend },
        )
        .unwrap();
        GpuTightOracle::from_context(&context).unwrap()
    } else {
        GpuTightOracle::new_with_support(
            GpuOptions::default(),
            GpuSelection { api: backend },
            support,
        )
        .unwrap()
    };
    physical::verify(backend, oracle.info());
    assert_eq!(oracle.support(), support);
    println!(
        "tight support adapter={} construction={support:?}",
        oracle.info().name()
    );
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

// All producer heads except the gate have an optional head guarded by atom 0.
// Each word keeps an unsupported neighbor at bit 2. Repeated roots produce
// duplicate writes, while different enabled heads contend for the same word.
fn conditional_support(atoms: usize) -> TightPlan {
    let mut nodes = vec![
        Node::falsum(),
        Node::atom(0),
        Node::implies(1, 0),
        Node::or_pair([1, 2]),
    ];
    let mut roots = vec![3];
    for head in (1..atoms).filter(|head| head % 32 != 2) {
        let atom = nodes.len();
        nodes.extend([
            Node::atom(head),
            Node::implies(atom, 0),
            Node::or_pair([atom, atom + 1]),
            Node::implies(1, atom + 2),
        ]);
        roots.push(atom + 3);
        if head % 3 == 0 {
            roots.push(atom + 3);
        }
    }
    certificate(atoms, nodes, roots)
}

#[test]
fn conditional_support_matches_exhaustive_reducts() {
    let certificate = conditional_support(4);
    for candidate in inputs(certificate.theory()) {
        let scalar = certificate
            .check(
                &candidate,
                TightCheckLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        let exact = check(
            certificate.theory(),
            &candidate,
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(
            matches!(scalar.verdict, TightVerdict::Stable),
            exact.accepted()
        );
    }
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
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(results.len(), candidates.len());
        if let Some(elapsed) = oracle.last_compute_time() {
            assert_eq!(
                oracle.context().compute_timing(),
                zetesis_backend::ComputeTimingStatus::Enabled
            );
            assert!(!elapsed.is_zero());
        }
        let stats = *oracle.last_batch_stats().unwrap();
        assert_eq!(stats.dispatches, 1);
        assert_eq!(stats.candidates, candidates.len() as u64);
        assert_eq!(
            stats.work,
            results
                .iter()
                .map(zetesis_wgpu::TightGpuCheck::work)
                .sum::<u64>()
        );
        assert_eq!(stats.theory_uploaded, repeat == 0);
        assert_eq!(stats.transport_allocated, repeat == 0);
        assert_eq!(stats.original_truth_reused, repeat != 0);
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
                .check(
                    candidate,
                    TightCheckLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
            assert_eq!(
                result.verdict(),
                scalar.verdict,
                "candidate {:?}",
                candidate.atoms().collect::<Vec<_>>()
            );
            let exact = check(
                theory,
                candidate,
                Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
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
        certificate(0, vec![Node::falsum()], vec![0]),
        certificate(2, vec![Node::atom(0), Node::atom(1)], vec![1, 0, 1]),
        // Double negation alone supplies no producer for its true atom.
        certificate(
            1,
            vec![
                Node::falsum(),
                Node::atom(0),
                Node::implies(1, 0),
                Node::implies(2, 0),
            ],
            vec![3],
        ),
    ];
    // Positive bodies mention lower-ranked atoms; a fourth carrier atom has no
    // producer. Every body form is evaluated with both truth values where possible.
    for body in [0, 1, 2, 4, 9, 10, 11] {
        let mut nodes = vec![
            Node::falsum(),
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::implies(1, 0),
            Node::implies(2, 0),
            Node::implies(3, 0),
            Node::or_pair([1, 4]),
            Node::or_pair([5, 2]),
            Node::and_pair([1, 2]),
            Node::or_pair([1, 2]),
            Node::implies(4, 0),
        ];
        nodes.push(Node::implies(body, 3));
        fixtures.push(certificate(4, nodes, vec![12, 8, 7]));
    }
    for width in [3, 65, 129] {
        for conjunction in [false, true] {
            let span = zetesis_ferraris::OperandSpan {
                start: 0,
                length: width,
            };
            let parts = zetesis_ferraris::FormulaParts::new(
                vec![
                    Node::atom(0),
                    Node::atom(1),
                    Node::atom(2),
                    if conjunction {
                        Node::and_span(span)
                    } else {
                        Node::or_span(span)
                    },
                    Node::implies(3, 2),
                ],
                (0..width).map(|i| i % 2).collect(),
            )
            .unwrap();
            let theory = Theory::new(3, parts, vec![0, 1, 4], AdmissionLimits::default()).unwrap();
            fixtures.push(
                TightPlan::compile(
                    &theory,
                    TightPlanLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap(),
            );
        }
    }
    fixtures
}

#[test]
fn tight_fixture_verdicts_match_exhaustive_reducts() {
    let mut census = [0usize; 3];
    for certificate in fixtures() {
        for candidate in inputs(certificate.theory()) {
            let scalar = certificate
                .check(
                    &candidate,
                    TightCheckLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
            let exact = check(
                certificate.theory(),
                &candidate,
                Limits::default(),
                &Cancellation::default(),
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
    // Each of the six wide fixtures adds one answer and eight nonmodels,
    // including the duplicated empty interpretation in its input batch.
    assert_eq!(census, [43, 81, 61]);
}

#[test]
#[ignore = "requires Metal: support matches exact reduct semantics"]
fn metal_support_matches_exact_reduct_semantics() {
    qualify_support_matches_exact_reduct_semantics(GpuApi::Metal, TightSupport::Atomic);
    qualify_support_matches_exact_reduct_semantics(GpuApi::Metal, TightSupport::Grouped);
}

#[test]
#[ignore = "requires Vulkan: support matches exact reduct semantics"]
fn vulkan_support_matches_exact_reduct_semantics() {
    qualify_support_matches_exact_reduct_semantics(GpuApi::Vulkan, TightSupport::Atomic);
    qualify_support_matches_exact_reduct_semantics(GpuApi::Vulkan, TightSupport::Grouped);
}

fn qualify_support_matches_exact_reduct_semantics(backend: GpuApi, support: TightSupport) {
    let mut oracle = oracle(backend, support);
    let compared: usize = fixtures()
        .iter()
        .map(|certificate| compare(&mut oracle, certificate))
        .sum();
    assert_eq!(compared, 370);
    println!("tight support exact candidate occurrences={compared}");
    qualify_kernel_options(&oracle);
}

fn qualify_kernel_options(oracle: &GpuTightOracle) {
    let context = oracle.context().clone();
    println!("tight granted features={:?}", context.features());
    for workgroup_size in [GpuWorkgroupSize::Threads32, GpuWorkgroupSize::Threads64] {
        for optional_features in [false, true] {
            let options = GpuKernelOptions {
                workgroup_size,
                optional_features,
                ..GpuKernelOptions::default()
            };
            let mut configured =
                GpuTightOracle::from_context_with_kernel(&context, oracle.support(), options)
                    .unwrap();
            assert!(configured.context().same_instance(&context));
            assert_eq!(configured.kernel_options(), options);
            assert_eq!(configured.support(), oracle.support());
            println!("tight qualified options={options:?}");
            native::qualify(&mut configured);
            qualify_dependency_schedules(&mut configured);
        }
    }
}

#[test]
#[ignore = "requires Metal: support preserves batch isolation"]
fn metal_support_preserves_batch_isolation() {
    qualify_support_preserves_batch_isolation(GpuApi::Metal, TightSupport::Atomic);
    qualify_support_preserves_batch_isolation(GpuApi::Metal, TightSupport::Grouped);
}

#[test]
#[ignore = "requires Vulkan: support preserves batch isolation"]
fn vulkan_support_preserves_batch_isolation() {
    qualify_support_preserves_batch_isolation(GpuApi::Vulkan, TightSupport::Atomic);
    qualify_support_preserves_batch_isolation(GpuApi::Vulkan, TightSupport::Grouped);
}

fn qualify_support_preserves_batch_isolation(backend: GpuApi, support: TightSupport) {
    let mut oracle = oracle(backend, support);
    for atoms in [4, 33, 65, 129] {
        compare_conditional_support(&mut oracle, atoms);
    }
    compare_skewed_support(&mut oracle);
    compare_word_witnesses(&mut oracle);
    for atoms in [1, 31, 32, 33, 63, 64, 65, 4097] {
        let certificate = certificate(atoms, vec![Node::atom(atoms - 1)], vec![0]);
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
                    &Cancellation::default(),
                )
                .unwrap();
            assert_eq!(results.len(), worlds);
            for (candidate, result) in candidates.iter().zip(results) {
                assert_eq!(
                    result.verdict(),
                    certificate
                        .check(
                            candidate,
                            TightCheckLimits::default(),
                            &Cancellation::default()
                        )
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

fn compare_word_witnesses(oracle: &mut GpuTightOracle) {
    let certificate = certificate(129, vec![Node::atom(0), Node::atom(128)], vec![0, 1]);
    let theory = certificate.theory();
    // Every bit offset appears as the least unsupported atom. A later missing
    // bit also tests the global minimum across words, including empty words.
    let mut candidates: Vec<_> = (1..128)
        .map(|atom| Interpretation::new(theory, [0, atom, 127, 128]).unwrap())
        .collect();
    candidates.push(Interpretation::new(theory, [0, 128]).unwrap());
    candidates.push(Interpretation::new(theory, [1, 63, 64]).unwrap());
    candidates.push(Interpretation::new(theory, [0, 1, 63, 64]).unwrap());
    let results = oracle
        .check_batch(
            &certificate,
            &candidates,
            TightGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(results.len(), candidates.len());
    for (atom, result) in (1..128).zip(&results) {
        assert_eq!(
            result.verdict(),
            TightVerdict::Residual {
                unsupported_atom: atom
            }
        );
    }
    assert_eq!(results[127].verdict(), TightVerdict::Stable);
    assert_eq!(results[128].verdict(), TightVerdict::NotModel { root: 0 });
    assert_eq!(results[129].verdict(), TightVerdict::NotModel { root: 1 });
    assert!(results.iter().all(|result| result.work() == 17));
}

fn compare_skewed_support(oracle: &mut GpuTightOracle) {
    // Most producers belong to the last word, the middle word is empty, and
    // neighboring unsupported bits must survive the large duplicate group.
    let certificate = certificate(
        65,
        vec![Node::atom(0), Node::atom(64)],
        std::iter::repeat_n(1, 257).chain([0]).collect(),
    );
    let mut candidates = vec![
        Interpretation::new(certificate.theory(), []).unwrap(),
        Interpretation::new(certificate.theory(), [0]).unwrap(),
        Interpretation::new(certificate.theory(), [0, 64]).unwrap(),
        Interpretation::new(certificate.theory(), [0, 32, 64]).unwrap(),
        Interpretation::new(certificate.theory(), [0, 1, 64]).unwrap(),
    ];
    for _ in 0..3 {
        let results = oracle
            .check_batch(
                &certificate,
                &candidates,
                TightGpuLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(results.len(), candidates.len());
        assert_eq!(oracle.activity().submissions, 1);
        assert_eq!(
            oracle.activity().completed_candidates,
            candidates.len() as u64
        );
        for (candidate, result) in candidates.iter().zip(results) {
            let expected = certificate
                .check(
                    candidate,
                    TightCheckLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
            assert_eq!(result.verdict(), expected.verdict);
        }
        candidates.rotate_left(1);
    }
}

fn compare_conditional_support(oracle: &mut GpuTightOracle, atoms: usize) {
    let certificate = conditional_support(atoms);
    let theory = certificate.theory();
    let supported = (0..atoms).filter(|atom| atom % 32 != 2);
    let patterns = [
        Interpretation::new(theory, []).unwrap(),
        Interpretation::new(theory, supported.clone()).unwrap(),
        Interpretation::new(theory, supported.clone().chain([2])).unwrap(),
        Interpretation::new(theory, supported.filter(|atom| *atom != 0)).unwrap(),
        Interpretation::new(theory, [0]).unwrap(),
    ];
    for worlds in [5, 33] {
        let mut candidates: Vec<_> = (0..worlds)
            .map(|world| patterns[world % patterns.len()].clone())
            .collect();
        for repeat in 0..3 {
            let results = oracle
                .check_batch(
                    &certificate,
                    &candidates,
                    TightGpuLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
            assert_eq!(results.len(), worlds);
            for (candidate, result) in candidates.iter().zip(results) {
                let expected = certificate
                    .check(
                        candidate,
                        TightCheckLimits::default(),
                        &Cancellation::default(),
                    )
                    .unwrap();
                assert_eq!(result.verdict(), expected.verdict);
            }
            let stats = oracle.last_batch_stats().unwrap();
            assert_eq!(stats.dispatches, 1);
            assert_eq!(stats.candidates, u64::try_from(worlds).unwrap());
            assert_eq!(stats.theory_uploaded, worlds == 5 && repeat == 0);
            assert_eq!(stats.transport_allocated, repeat == 0);
            assert_eq!(stats.original_truth_reused, repeat != 0);
            candidates.rotate_left(1);
        }
    }
}

#[test]
#[ignore = "requires Metal: support refusals preserve reusable residency"]
fn metal_support_refusals_preserve_reusable_residency() {
    qualify_support_refusals_preserve_reusable_residency(GpuApi::Metal, TightSupport::Atomic);
    qualify_support_refusals_preserve_reusable_residency(GpuApi::Metal, TightSupport::Grouped);
}

#[test]
#[ignore = "requires Vulkan: support refusals preserve reusable residency"]
fn vulkan_support_refusals_preserve_reusable_residency() {
    qualify_support_refusals_preserve_reusable_residency(GpuApi::Vulkan, TightSupport::Atomic);
    qualify_support_refusals_preserve_reusable_residency(GpuApi::Vulkan, TightSupport::Grouped);
}

fn qualify_support_refusals_preserve_reusable_residency(backend: GpuApi, support: TightSupport) {
    let mut oracle = oracle(backend, support);
    qualify_prefixes(&mut oracle);
    qualify_original_truth_reuse(&mut oracle);
    qualify_boolean_change_bits(&mut oracle);
    let certificate = certificate(1, vec![Node::atom(0)], vec![0]);
    let candidate = Interpretation::new(certificate.theory(), [0]).unwrap();
    let input = [candidate];
    let expected = oracle
        .check_batch(
            &certificate,
            &input,
            TightGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    check_staging_reuse(&mut oracle, &certificate);
    oracle
        .check_batch(
            &certificate,
            &input,
            TightGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    let bytes = oracle.last_batch_stats().unwrap().accounted_bytes;
    for limits in [
        TightGpuLimits {
            timeout: std::time::Duration::ZERO,
            ..Default::default()
        },
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
            .check_batch(&certificate, &input, limits, &Cancellation::default())
            .unwrap_err();
        assert!(
            matches!(error,TightGpuError::Gpu(ref error) if error.kind()==GpuErrorKind::Capacity)
        );
        assert_eq!(oracle.activity(), TightGpuActivity::default());
        assert!(oracle.last_batch_stats().is_none());
    }
    let cancelled = Cancellation::default();
    cancelled.cancel();
    assert_eq!(
        oracle
            .check_batch(
                &certificate,
                &input,
                TightGpuLimits {
                    timeout: std::time::Duration::ZERO,
                    ..TightGpuLimits::default()
                },
                &cancelled,
            )
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
            .check_batch(&certificate, &input, tight, &Cancellation::default())
            .unwrap(),
        expected
    );
    assert!(!oracle.last_batch_stats().unwrap().theory_uploaded);
    assert!(!oracle.last_batch_stats().unwrap().transport_allocated);
}

#[test]
#[ignore = "requires Metal: support residency tracks theory identity"]
fn metal_support_residency_tracks_theory_identity() {
    qualify_support_residency_tracks_theory_identity(GpuApi::Metal, TightSupport::Atomic);
    qualify_support_residency_tracks_theory_identity(GpuApi::Metal, TightSupport::Grouped);
}

#[test]
#[ignore = "requires Vulkan: support residency tracks theory identity"]
fn vulkan_support_residency_tracks_theory_identity() {
    qualify_support_residency_tracks_theory_identity(GpuApi::Vulkan, TightSupport::Atomic);
    qualify_support_residency_tracks_theory_identity(GpuApi::Vulkan, TightSupport::Grouped);
}

fn qualify_support_residency_tracks_theory_identity(backend: GpuApi, support: TightSupport) {
    let mut oracle = oracle(backend, support);
    let certificate = certificate(1, vec![Node::atom(0)], vec![0]);
    let input = [Interpretation::new(certificate.theory(), [0]).unwrap()];
    let expected = oracle
        .check_batch(
            &certificate,
            &input,
            TightGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    let foreign = Theory::new(
        1,
        zetesis_ferraris::FormulaParts::new(vec![Node::atom(0)], Vec::new()).unwrap(),
        vec![0],
        AdmissionLimits::default(),
    )
    .unwrap();
    let error = oracle
        .check_batch(
            &certificate,
            &[Interpretation::new(&foreign, [0]).unwrap()],
            TightGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap_err();
    assert!(matches!(error,TightGpuError::Gpu(ref error) if error.kind()==GpuErrorKind::Seed));
    assert_eq!(oracle.activity(), TightGpuActivity::default());
    let new_certificate = TightPlan::compile(
        &foreign,
        TightPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let new_input = [Interpretation::new(&foreign, [0]).unwrap()];
    assert_eq!(
        oracle
            .check_batch(
                &new_certificate,
                &new_input,
                TightGpuLimits::default(),
                &Cancellation::default()
            )
            .unwrap(),
        expected
    );
    assert!(oracle.last_batch_stats().unwrap().theory_uploaded);
    let cancelled = Cancellation::default();
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
                &Cancellation::default()
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
                &Cancellation::default()
            )
            .unwrap(),
        expected
    );
    assert!(oracle.last_batch_stats().unwrap().theory_uploaded);
}

fn check_staging_reuse(oracle: &mut GpuTightOracle, certificate: &TightPlan) {
    let present = Interpretation::new(certificate.theory(), [0]).unwrap();
    let empty = Interpretation::new(certificate.theory(), []).unwrap();
    for candidates in [
        [present.clone(), empty.clone(), present.clone()],
        [empty.clone(), present, empty],
    ] {
        let checks = oracle
            .check_batch(
                certificate,
                &candidates,
                TightGpuLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        for (candidate, result) in candidates.iter().zip(checks) {
            let expected = certificate
                .check(
                    candidate,
                    TightCheckLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
            assert_eq!(result.verdict(), expected.verdict);
        }
        assert!(!oracle.last_batch_stats().unwrap().theory_uploaded);
    }
    let stats = oracle.last_batch_stats().unwrap();
    assert!(!stats.transport_allocated);
    assert_eq!(stats.candidates, 3);
}

// The same original truth and producer checks run under both schedules. This
// exercises the actual shader branch, including roots that are out of node order.
fn qualify_dependency_schedules(oracle: &mut GpuTightOracle) {
    let mut scheduled = 0;
    let mut serial = 0;
    for certificate in fixtures() {
        let candidates = inputs(certificate.theory());
        let expected = oracle
            .check_batch(
                &certificate,
                &candidates,
                TightGpuLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        let levels = oracle.last_batch_stats().unwrap().truth_levels;
        if levels == 0 {
            serial += 1;
            continue;
        }
        scheduled += 1;
        let limits = TightGpuLimits {
            max_work_per_candidate: expected[0].work() - u64::from(levels),
            ..TightGpuLimits::default()
        };
        let actual = oracle
            .check_batch(&certificate, &candidates, limits, &Cancellation::default())
            .unwrap();
        assert_eq!(oracle.last_batch_stats().unwrap().truth_levels, 0);
        assert!(!oracle.last_batch_stats().unwrap().theory_uploaded);
        for (expected, actual) in expected.iter().zip(actual) {
            assert_eq!(expected.verdict(), actual.verdict());
            assert_eq!(expected.work() - u64::from(levels), actual.work());
        }
    }
    assert!(scheduled > 0 && serial > 0);
    let certificate = certificate(0, vec![], vec![]);
    oracle
        .check_batch(
            &certificate,
            &[],
            TightGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(oracle.last_compute_time(), None);
}

fn qualify_prefixes(oracle: &mut GpuTightOracle) {
    let certificate = conditional_support(2);
    let input = inputs(certificate.theory());
    let exact = oracle
        .check_batch(
            &certificate,
            &input,
            TightGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    oracle.clear_residency();
    oracle
        .check_batch(
            &certificate,
            &input[..2],
            TightGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    let cold_bytes = oracle.last_batch_stats().unwrap().accounted_bytes;
    oracle.clear_residency();
    let first = oracle
        .check_prefix(
            &certificate,
            &input,
            TightGpuLimits {
                max_batch_bytes: cold_bytes,
                ..Default::default()
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(first.checks(), &exact[..2]);
    assert!(std::ptr::eq(first.candidates(), &raw const input[..2]));
    assert!(oracle.last_batch_stats().unwrap().theory_uploaded);
    assert_eq!(oracle.activity().submitted_candidates, 2);
    let mut completed = first.checks().to_vec();
    while completed.len() < input.len() {
        let suffix = &input[completed.len()..];
        let next = oracle
            .check_prefix(
                &certificate,
                suffix,
                TightGpuLimits {
                    max_candidates: 2,
                    max_batch_bytes: cold_bytes,
                    ..Default::default()
                },
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(next.checks().len(), suffix.len().min(2));
        assert!(std::ptr::eq(
            next.candidates(),
            &raw const suffix[..next.checks().len()]
        ));
        assert!(!oracle.last_batch_stats().unwrap().theory_uploaded);
        completed.extend_from_slice(next.checks());
    }
    assert_eq!(completed.len(), exact.len());
    for (actual, expected) in completed.iter().zip(exact) {
        assert_eq!(actual.verdict(), expected.verdict());
    }
    let one_bytes = oracle.last_batch_stats().unwrap().accounted_bytes;
    let error = oracle
        .check_prefix(
            &certificate,
            &input,
            TightGpuLimits {
                max_batch_bytes: one_bytes - 1,
                ..Default::default()
            },
            &Cancellation::default(),
        )
        .err()
        .unwrap();
    assert!(matches!(error, TightGpuError::Gpu(error) if error.kind() == GpuErrorKind::Capacity));
    assert_eq!(oracle.activity().submissions, 0);
    assert!(oracle.last_batch_stats().is_none());
    let empty = oracle
        .check_prefix(
            &certificate,
            &[],
            TightGpuLimits {
                max_candidates: 0,
                max_batch_bytes: 0,
                ..Default::default()
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert!(empty.checks().is_empty());
    assert_eq!(oracle.activity().submissions, 0);
}

// Hot unchanged, sparse and dense membership changes preserve the full CPU
// verdict, including changed-false words (2) and complete producer support.
fn qualify_original_truth_reuse(oracle: &mut GpuTightOracle) {
    let certificate = conditional_support(4);
    let theory = certificate.theory();
    let worlds = |bits: &[u32]| -> Vec<_> {
        bits.iter()
            .map(|bits| {
                Interpretation::new(theory, (0..4).filter(|atom| bits & (1 << atom) != 0)).unwrap()
            })
            .collect()
    };
    let batches = [[0, 15, 3], [0, 15, 3], [1, 7, 2], [14, 8, 13]];
    let mut cold_work = 0;
    let mut hot_work = 0;
    for (index, bits) in batches.iter().enumerate() {
        let input = worlds(bits);
        let checked = oracle
            .check_batch(
                &certificate,
                &input,
                TightGpuLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        let stats = oracle.last_batch_stats().unwrap();
        assert_eq!(stats.original_truth_reused, index != 0);
        for (candidate, checked) in input.iter().zip(&checked) {
            assert_eq!(
                checked.verdict(),
                certificate
                    .check(
                        candidate,
                        TightCheckLimits::default(),
                        &Cancellation::default()
                    )
                    .unwrap()
                    .verdict
            );
        }
        if index == 0 {
            cold_work = checked[0].work();
        } else {
            hot_work = checked[0].work();
        }
    }
    assert!(hot_work > cold_work);
    let input = worlds(&batches[3]);
    let fallback = oracle
        .check_batch(
            &certificate,
            &input,
            TightGpuLimits {
                max_work_per_candidate: cold_work,
                ..Default::default()
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert!(!oracle.last_batch_stats().unwrap().original_truth_reused);
    assert!(fallback.iter().all(|check| check.work() == cold_work));
    qualify_truth_invalidation(oracle, &certificate, &input);
}

// An absorbing first operand can hide a changing later operand. The native
// parent consumes the Boolean output's change bit on the next retained pass.
fn qualify_boolean_change_bits(oracle: &mut GpuTightOracle) {
    for conjunction in [false, true] {
        let certificate = wide_boolean_parent(conjunction);
        let theory = certificate.theory();
        let mut cold_work = 0;
        let mut levels = 0;
        // Bits 0 and 1 are optional atoms; bit 2 needs the numeric body's support.
        // The second candidate changes only an operand behind an absorbing value.
        let bits = if conjunction {
            [4, 6, 7, 3, 5, 4]
        } else {
            [5, 7, 6, 2, 4, 5]
        };
        for (index, bits) in bits.into_iter().enumerate() {
            let candidate =
                Interpretation::new(theory, (0..3).filter(|atom| bits & (1 << atom) != 0)).unwrap();
            let expected = certificate
                .check(
                    &candidate,
                    TightCheckLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
            let actual = oracle
                .check_batch(
                    &certificate,
                    &[candidate],
                    TightGpuLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
            assert_eq!(actual[0].verdict(), expected.verdict);
            let stats = oracle.last_batch_stats().unwrap();
            assert_eq!(stats.original_truth_reused, index != 0);
            if index == 0 {
                cold_work = actual[0].work();
                levels = stats.truth_levels;
                assert!(levels > 0);
            }
        }
        let candidate = Interpretation::new(theory, [0, 1, 2]).unwrap();
        let serial = oracle
            .check_batch(
                &certificate,
                &[candidate],
                TightGpuLimits {
                    max_work_per_candidate: cold_work - u64::from(levels),
                    ..TightGpuLimits::default()
                },
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(oracle.last_batch_stats().unwrap().truth_levels, 0);
        assert!(!oracle.last_batch_stats().unwrap().original_truth_reused);
        assert_eq!(serial[0].verdict(), TightVerdict::Stable);
    }
}

fn wide_boolean_parent(conjunction: bool) -> TightPlan {
    use zetesis_ferraris::{FormulaNodes, FormulaParts, NodeView, Threshold};
    let mut nodes = FormulaNodes::new(
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::atom(2),
                Node::falsum(),
                Node::implies(0, 3),
                Node::or_pair([0, 4]),
                Node::implies(1, 3),
                Node::or_pair([1, 6]),
            ],
            vec![],
        )
        .unwrap(),
    );
    let operands: Vec<_> = (0..65).map(|index| index % 2).collect();
    let mut transaction = nodes.transaction();
    let body = transaction
        .push(
            if conjunction {
                NodeView::And(&operands)
            } else {
                NodeView::Or(&operands)
            },
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    let numeric = transaction
        .push(
            NodeView::Threshold(Threshold {
                conditions: &[body, body],
                weights: &[1, 1],
                bound: 2,
            }),
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    let root = transaction
        .push(NodeView::Implies(numeric, 2), usize::MAX, usize::MAX)
        .unwrap();
    transaction.commit();
    let theory = Theory::new(
        3,
        nodes.into_parts(),
        vec![5, 7, root],
        AdmissionLimits::default(),
    )
    .unwrap();
    TightPlan::compile(
        &theory,
        TightPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap()
}

fn qualify_truth_invalidation(
    oracle: &mut GpuTightOracle,
    certificate: &TightPlan,
    input: &[Interpretation],
) {
    let cancelled = Cancellation::default();
    cancelled.cancel();
    assert_eq!(
        oracle
            .check_batch(certificate, input, TightGpuLimits::default(), &cancelled)
            .unwrap_err(),
        TightGpuError::Stopped(Stop::Cancelled)
    );
    oracle
        .check_batch(
            certificate,
            input,
            TightGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(!oracle.last_batch_stats().unwrap().original_truth_reused);
    // A short final prefix replaces exact-shape truth storage; increasing the
    // count again must not retain inactive slots from the former larger batch.
    for slice in [&input[..1], input] {
        oracle
            .check_batch(
                certificate,
                slice,
                TightGpuLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert!(!oracle.last_batch_stats().unwrap().original_truth_reused);
    }
    let foreign = conditional_support(4);
    let error = oracle
        .check_batch(
            certificate,
            &inputs(foreign.theory()),
            TightGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap_err();
    assert!(matches!(error, TightGpuError::Gpu(ref fault) if fault.kind() == GpuErrorKind::Seed));
    oracle
        .check_batch(
            certificate,
            input,
            TightGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(!oracle.last_batch_stats().unwrap().original_truth_reused);
}
