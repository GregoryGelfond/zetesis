//! Positive specialization preserves exact full families and bounded setup.

use std::collections::BTreeSet;
use std::convert::Infallible;
use std::num::NonZeroUsize;
use zetesis_ferraris::{AdmissionLimits, Interpretation, Node, PositivePlanLimits, Theory};
use zetesis_sat::{
    BatchError, BatchLimits, BatchVerdict, CertificateError, CertificateLimits, CertificateOrder,
    CertificatePlanStatistics, CompletionExecutor, Control, Incomplete, Limits, StableModels,
};

fn theory(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(atoms, nodes, roots, AdmissionLimits::default()).unwrap()
}

fn cycle(seed: bool, constraint: bool) -> Theory {
    let mut roots = vec![2, 3];
    if seed {
        roots.push(0);
    }
    if constraint {
        roots.push(5);
    }
    theory(
        3,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::Implies(0, 1),
            Node::Implies(1, 0),
            Node::False,
            Node::Implies(1, 4),
        ],
        roots,
    )
}

fn independent(original: &Theory) -> BTreeSet<Vec<usize>> {
    (0..1usize << original.atom_count())
        .filter_map(|mask| {
            let candidate = Interpretation::new(
                original,
                (0..original.atom_count()).filter(|a| mask & (1 << a) != 0),
            )
            .unwrap();
            zetesis_ferraris::check(
                original,
                &candidate,
                zetesis_ferraris::Limits::default(),
                &Control::default(),
            )
            .unwrap()
            .accepted()
            .then(|| candidate.atoms().collect())
        })
        .collect()
}

fn collect(stream: &mut StableModels) -> BTreeSet<Vec<usize>> {
    stream
        .by_ref()
        .map(|value| value.unwrap().atoms().collect())
        .collect()
}

#[test]
fn positive_plans_preserve_every_small_atomic_rule_family() {
    // Every subset of two facts, two cyclic rules and two positive constraints;
    // the third carrier atom is deliberately unmentioned in every theory.
    let nodes = vec![
        Node::Atom(0),
        Node::Atom(1),
        Node::Implies(0, 1),
        Node::Implies(1, 0),
        Node::False,
        Node::Implies(0, 4),
        Node::Implies(1, 4),
    ];
    let possible = [0, 1, 2, 3, 5, 6];
    for mask in 0..64 {
        let original = theory(
            3,
            nodes.clone(),
            possible
                .iter()
                .enumerate()
                .filter_map(|(bit, root)| (mask & (1 << bit) != 0).then_some(*root))
                .collect(),
        );
        let expected = independent(&original);
        let mut stream =
            StableModels::new(&original, Limits::default(), Control::default()).unwrap();
        assert!(
            stream
                .enable_class_checking(
                    CertificateLimits::default(),
                    CertificateOrder::PositiveFirst
                )
                .unwrap()
        );
        assert!(matches!(
            stream.statistics().certified.unwrap().plan,
            Some(CertificatePlanStatistics::Positive(_))
        ));
        assert_eq!(collect(&mut stream), expected);
        assert!(stream.exhausted());
        assert!(stream.statistics().candidates <= 1);
        assert_eq!(stream.statistics().countermodel_queries, 0);
        assert_eq!(stream.statistics().reduct.preparation, None);
    }
}

#[test]
fn positive_batches_preserve_the_unique_family() {
    for original in [cycle(false, false), cycle(true, false), cycle(true, true)] {
        for workers in [1, 4] {
            let mut stream =
                StableModels::new(&original, Limits::default(), Control::default()).unwrap();
            assert!(
                stream
                    .enable_class_checking(
                        CertificateLimits::default(),
                        CertificateOrder::PositiveFirst
                    )
                    .unwrap()
            );
            let mut pool = CompletionExecutor::new(NonZeroUsize::new(workers).unwrap()).unwrap();
            let mut actual = BTreeSet::new();
            while !stream.exhausted() {
                let rows = stream
                    .next_batch_with_completion(
                        BatchLimits {
                            max_candidates: NonZeroUsize::new(4).unwrap(),
                            max_pending_bytes: 4096,
                        },
                        &mut pool,
                        |owner, candidates| {
                            assert!(owner.same_instance(&original));
                            assert!(candidates.len() <= 1);
                            Ok::<_, Infallible>(vec![BatchVerdict::Residual; candidates.len()])
                        },
                    )
                    .unwrap();
                for row in rows {
                    assert!(actual.insert(row.atoms().collect()));
                }
            }
            assert_eq!(actual, independent(&original));
            assert_eq!(stream.batch_statistics().pending, 0);
            assert_eq!(stream.statistics().reduct.preparation, None);
            assert_eq!(stream.statistics().countermodel_queries, 0);
            let stats = stream.statistics().certified.unwrap();
            assert_eq!(stats.checks, stream.statistics().candidates);
            assert_eq!(stats.residuals, 0);
        }
    }
}

#[test]
fn positive_cycle_refusal_preserves_the_tight_only_door() {
    let original = cycle(true, false);
    let mut tight = StableModels::new(&original, Limits::default(), Control::default()).unwrap();
    assert!(
        !tight
            .enable_certified_checking(zetesis_ferraris::TightPlanLimits::default())
            .unwrap()
    );
    assert!(matches!(
        tight.statistics().certified.unwrap().refusal,
        Some(CertificateError::Tight(
            zetesis_ferraris::TightError::PositiveCycle { .. }
        ))
    ));
    assert_eq!(collect(&mut tight), independent(&original));
    assert!(tight.statistics().reduct.preparation.is_some());
    let mut both = StableModels::new(&original, Limits::default(), Control::default()).unwrap();
    assert!(
        both.enable_class_checking(CertificateLimits::default(), CertificateOrder::TightFirst)
            .unwrap()
    );
    let stats = both.statistics().certified.unwrap();
    assert!(stats.tight_refusal.is_some());
    assert!(matches!(
        stats.plan,
        Some(CertificatePlanStatistics::Positive(_))
    ));
    assert_eq!(stats.refusal, None);
}

#[test]
fn positive_grammar_refusal_can_select_the_tight_plan() {
    let original = theory(
        1,
        vec![
            Node::Atom(0),
            Node::False,
            Node::Implies(0, 1),
            Node::Or(0, 2),
        ],
        vec![3],
    );
    let mut stream = StableModels::new(&original, Limits::default(), Control::default()).unwrap();
    assert!(
        stream
            .enable_class_checking(
                CertificateLimits::default(),
                CertificateOrder::PositiveFirst
            )
            .unwrap()
    );
    let stats = stream.statistics().certified.unwrap();
    assert!(stats.positive_refusal.is_some());
    assert!(stats.positive_attempt.unwrap().work > 0);
    assert!(matches!(
        stats.plan,
        Some(CertificatePlanStatistics::Tight(_))
    ));
    assert_eq!(stats.refusal, None);
    assert_eq!(collect(&mut stream), independent(&original));
    assert!(stream.exhausted());
}

#[test]
fn optional_positive_bytes_refusal_keeps_general_completion() {
    let original = cycle(true, false);
    let mut stream = StableModels::new(&original, Limits::default(), Control::default()).unwrap();
    assert!(
        !stream
            .enable_class_checking(
                CertificateLimits {
                    positive: PositivePlanLimits {
                        max_bytes: 0,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                CertificateOrder::PositiveFirst
            )
            .unwrap()
    );
    let stats = stream.statistics().certified.unwrap();
    assert!(matches!(
        stats.positive_refusal,
        Some(zetesis_ferraris::PositiveError::Limit {
            resource: zetesis_ferraris::PositiveResource::Bytes,
            ..
        })
    ));
    assert_eq!(stats.positive_attempt.unwrap().retained_bytes, 0);
    assert_eq!(stats.restriction_clauses, 0);
    assert_eq!(collect(&mut stream), independent(&original));
    assert!(stream.exhausted());
    assert!(stream.statistics().reduct.preparation.is_some());
}

#[test]
fn positive_setup_work_has_an_inclusive_boundary() {
    let original = cycle(true, false);
    let mut probe = StableModels::new(&original, Limits::default(), Control::default()).unwrap();
    let initial = probe.statistics().search.work;
    assert!(
        probe
            .enable_class_checking(
                CertificateLimits::default(),
                CertificateOrder::PositiveFirst
            )
            .unwrap()
    );
    let required = probe.statistics().search.work;
    let full = probe.statistics().certified.unwrap();
    assert_eq!(
        required - initial,
        full.construction_work + full.restriction_work
    );
    assert_eq!(full.restriction_clauses, original.atom_count());
    for ceiling in initial..=required {
        let mut limits = Limits::default();
        limits.search.max_work = ceiling;
        let mut stream = StableModels::new(&original, limits, Control::default()).unwrap();
        let result = stream.enable_class_checking(
            CertificateLimits::default(),
            CertificateOrder::PositiveFirst,
        );
        assert_eq!(stream.statistics().search.work, ceiling);
        assert_eq!(stream.statistics().candidate_queries, 0);
        assert_eq!(stream.statistics().stable_models, 0);
        if ceiling == required {
            assert_eq!(result, Ok(true));
        } else {
            assert_eq!(result, Err(Incomplete::WorkLimit));
            assert_eq!(stream.statistics().certified.unwrap().plan, None);
            assert_eq!(
                stream.statistics().certified.unwrap().restriction_clauses,
                0
            );
            assert!(stream.next().is_none());
            assert!(!stream.exhausted());
        }
    }
}

#[test]
fn repeated_class_configuration_keeps_the_original_receipt() {
    let original = cycle(true, false);
    let mut stream = StableModels::new(&original, Limits::default(), Control::default()).unwrap();
    assert!(
        stream
            .enable_class_checking(
                CertificateLimits::default(),
                CertificateOrder::PositiveFirst
            )
            .unwrap()
    );
    let before = stream.statistics();
    assert!(
        stream
            .enable_certified_checking(zetesis_ferraris::TightPlanLimits {
                max_work: 0,
                ..Default::default()
            })
            .unwrap()
    );
    assert_eq!(stream.statistics(), before);
    assert_eq!(collect(&mut stream), independent(&original));
    assert!(stream.exhausted());
}

#[test]
fn a_positive_check_failure_keeps_the_pending_subject() {
    let original = cycle(true, false);
    let mut stream = StableModels::new(
        &original,
        Limits {
            // Independent proposal validation fits. The positive check additionally
            // scans the complete atom carrier for equality with the least model.
            max_verification_work: (original.nodes().len() + original.roots().len()) as u64,
            ..Default::default()
        },
        Control::default(),
    )
    .unwrap();
    assert!(
        stream
            .enable_class_checking(
                CertificateLimits::default(),
                CertificateOrder::PositiveFirst
            )
            .unwrap()
    );
    let error = stream
        .next_batch(
            BatchLimits {
                max_candidates: NonZeroUsize::new(2).unwrap(),
                max_pending_bytes: 4096,
            },
            |_, rows| {
                assert_eq!(rows.len(), 1);
                assert!(rows[0].theory().same_instance(&original));
                assert_eq!(rows[0].atoms().collect::<Vec<_>>(), vec![0, 1]);
                Ok::<_, Infallible>(vec![BatchVerdict::Residual])
            },
        )
        .unwrap_err();
    assert!(matches!(
        error,
        BatchError::Search(Incomplete::Verification(zetesis_cpu::Stop::WorkLimit))
    ));
    let stats = stream.statistics();
    assert_eq!(stats.stable_models, 0);
    assert_eq!(stats.countermodel_queries, 0);
    assert_eq!(stats.certified.unwrap().failed, 1);
    assert!(stats.certified.unwrap().positive_check_peak_bytes.is_some());
    assert_eq!(stream.batch_statistics().pending, 1);
    assert_eq!(stream.batch_statistics().committed, 0);
    assert!(!stream.exhausted());
}

#[test]
fn refused_positive_units_leave_the_full_candidate_region() {
    let original = theory(3, vec![], vec![]);
    let mut limits = Limits::default();
    // The original candidate CNF is empty. Exactly one of the three least-model
    // units fits, so a later refusal detects whether that prefix was rolled back.
    limits.admission.max_clauses = 1;
    let mut stream = StableModels::new(&original, limits, Control::default()).unwrap();
    assert!(
        stream
            .enable_class_checking(
                CertificateLimits::default(),
                CertificateOrder::PositiveFirst
            )
            .unwrap()
    );
    let setup = stream.statistics().certified.unwrap();
    assert_eq!(
        setup.restriction_refusal,
        Some(zetesis_sat::AdmissionError::Limit {
            resource: zetesis_sat::Resource::Clauses,
            observed: 2,
            limit: 1,
        })
    );
    assert_eq!(setup.restriction_clauses, 0);
    assert_eq!(setup.restriction_work, 4);
    assert!(matches!(
        setup.plan,
        Some(CertificatePlanStatistics::Tight(_))
    ));
    assert_eq!(collect(&mut stream), BTreeSet::from([vec![]]));
    assert!(stream.exhausted());
    assert_eq!(
        stream.statistics().candidates,
        8,
        "all original classical assignments remain reachable"
    );
    assert_eq!(stream.statistics().countermodels, 7);
}

#[test]
fn a_failed_constraint_preserves_the_empty_answer_family() {
    // not not a is an original constraint with a classical model {a}. There
    // is no positive producer for a, so its unique possible answer is empty,
    // which fails the constraint. Empty candidate coverage is therefore sound
    // without establishing classical inconsistency of the original theory.
    let original = theory(
        1,
        vec![
            Node::Atom(0),
            Node::False,
            Node::Implies(0, 1),
            Node::Implies(2, 1),
        ],
        vec![3],
    );
    let classical = Interpretation::new(&original, [0]).unwrap();
    assert!(
        zetesis_ferraris::models(
            &original,
            &classical,
            zetesis_ferraris::Limits::default(),
            &Control::default()
        )
        .unwrap()
    );
    assert!(independent(&original).is_empty());
    let mut stream = StableModels::new(&original, Limits::default(), Control::default()).unwrap();
    assert!(
        stream
            .enable_class_checking(
                CertificateLimits::default(),
                CertificateOrder::PositiveFirst
            )
            .unwrap()
    );
    let setup = stream.statistics().certified.unwrap();
    assert!(matches!(
        setup.plan,
        Some(CertificatePlanStatistics::Positive(_))
    ));
    assert_eq!(setup.restriction_clauses, 1);
    assert!(collect(&mut stream).is_empty());
    assert!(stream.exhausted());
    assert_eq!(stream.statistics().candidates, 0);
    assert_eq!(stream.statistics().certified.unwrap().checks, 0);
    assert_eq!(stream.statistics().reduct.preparation, None);
    assert!(
        zetesis_ferraris::models(
            stream.theory(),
            &classical,
            zetesis_ferraris::Limits::default(),
            &Control::default()
        )
        .unwrap()
    );
}
