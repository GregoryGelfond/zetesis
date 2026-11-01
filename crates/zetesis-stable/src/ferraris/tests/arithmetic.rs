//! Root arithmetic joins each original-candidate route before traversal.
use super::*;

fn theory(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(
        atoms,
        zetesis_ferraris::FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap()
}

fn contradiction() -> Theory {
    theory(
        1,
        vec![
            Node::falsum(),
            Node::atom(0),
            Node::implies(1, 0),
            Node::implies(2, 0),
        ],
        vec![2, 3],
    )
}

fn routes(theory: &Theory, cancellation: &Cancellation) -> [StableModels; 3] {
    let workers = NonZeroUsize::new(2).unwrap();
    [
        StableModels::new(theory, Limits::default(), cancellation.clone()).unwrap(),
        StableModels::with_region_workers_engaging(
            theory,
            workers,
            Engagement::Immediate,
            Limits::default(),
            cancellation.clone(),
        )
        .unwrap(),
        StableModels::with_region_producers(
            theory,
            workers,
            Limits::default(),
            cancellation.clone(),
        )
        .unwrap(),
    ]
}

#[test]
fn root_refutation_absorbs_before_every_walk() {
    let cancellation = Cancellation::default();
    let searches = routes(&contradiction(), &cancellation);
    cancellation.cancel();
    for mut search in searches {
        assert!(matches!(search.proposer, Proposer::Refuted(_)));
        assert!(search.index.get().is_none());
        let initial = search.statistics();
        let regions = initial.regions.unwrap();
        assert!(regions.parity.root_refuted);
        assert_eq!(
            regions.generator_order.status,
            GeneratorOrderStatus::NotPrepared
        );
        assert_eq!(regions.counts.regions, 0);
        assert!(regions.frontier.is_none());
        assert!(regions.workers.is_none());
        assert!(search.next().is_none());
        assert!(search.exhausted());
        assert!(search.index.get().is_none());
        assert_eq!(search.statistics().search.work, initial.search.work);
    }
}

#[test]
fn refuted_batch_needs_no_slate_or_checker() {
    let cancellation = Cancellation::default();
    let searches = routes(&contradiction(), &cancellation);
    cancellation.cancel();
    for mut search in searches {
        let initial = search.statistics().search.work;
        let answers = search
            .next_batch(
                BatchLimits {
                    max_candidates: NonZeroUsize::new(1).unwrap(),
                    max_pending_bytes: 0,
                },
                |_, _| -> Result<Vec<BatchVerdict>, Infallible> {
                    panic!("refuted root called checker")
                },
            )
            .unwrap();
        assert!(answers.is_empty());
        assert!(search.exhausted());
        assert!(search.index.get().is_none());
        assert_eq!(search.batch_statistics().pending, 0);
        assert_eq!(search.statistics().search.work, initial);
    }
}

#[test]
fn refuted_root_skips_certificate_preparation() {
    let cancellation = Cancellation::default();
    let mut search =
        StableModels::new(&contradiction(), Limits::default(), cancellation.clone()).unwrap();
    let initial = search.statistics().search.work;
    cancellation.cancel();
    assert!(
        !search
            .enable_class_checking(
                crate::CertificateLimits::default(),
                crate::CertificateOrder::TightFirst
            )
            .unwrap()
    );
    assert!(
        search
            .prepare_tight_certificate(zetesis_ferraris::TightPlanLimits::default())
            .unwrap()
            .is_none()
    );
    assert!(search.certificate.is_none());
    assert_eq!(search.statistics().search.work, initial);
    assert!(search.next().is_none());
    assert!(search.exhausted());
}

#[test]
fn seeded_units_preserve_the_stable_family() {
    // a or b, and :- a. The unique answer is {b}; seeding cuts a.
    let theory = theory(
        2,
        vec![
            Node::falsum(),
            Node::atom(0),
            Node::atom(1),
            Node::or_pair([1, 2]),
            Node::implies(1, 0),
        ],
        vec![3, 4],
    );
    for mut search in routes(&theory, &Cancellation::default()) {
        assert!(!matches!(search.proposer, Proposer::Refuted(_)));
        let initial = search.statistics().regions.unwrap().parity;
        assert!(!initial.root_refuted);
        assert_eq!(initial.units, 1);
        let answers = search.by_ref().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(answers.len(), 1);
        assert_eq!(answers[0].atoms().collect::<Vec<_>>(), vec![1]);
        assert!(search.exhausted());
    }
}

#[test]
fn arithmetic_source_fault_retains_its_typed_cause() {
    use std::error::Error;
    let error = Incomplete::ArithmeticSource(zetesis_ferraris::AdmissionError::Edge);
    assert_eq!(
        error
            .source()
            .unwrap()
            .downcast_ref::<zetesis_ferraris::AdmissionError>(),
        Some(&zetesis_ferraris::AdmissionError::Edge)
    );
}
