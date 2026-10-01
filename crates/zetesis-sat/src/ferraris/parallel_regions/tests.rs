//! Fault injection at the production worker accounting and failure boundaries.

use super::*;

#[test]
fn shared_narrowing_never_executes_an_unleased_read() {
    let cancellation =
        Cancellation::with_deadline(std::time::Instant::now() + Duration::from_secs(2)).unwrap();
    let theory = Theory::new(
        1,
        vec![
            zetesis_ferraris::Node::False,
            zetesis_ferraris::Node::Atom(0),
            zetesis_ferraris::Node::Implies(1, 0),
            zetesis_ferraris::Node::Or(1, 2),
        ],
        vec![3],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    let shared = SharedBudget::new(
        crate::SearchLimits {
            max_work: 1,
            ..Default::default()
        },
        SearchStatistics::default(),
    );
    let mut budget = Budget {
        quota: shared.lease(&cancellation),
        // Worker-local work is only a delta; the shared owner has the
        // authoritative one-permit remainder of the whole search.
        limits: crate::SearchLimits::default(),
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let narrower = Narrower::new(&theory);
    let mut region = Region::all_open(theory.atom_count());
    let mut knowledge = CandidateKnowledge::new(narrower.knowledge());
    let mut counts = RegionCounts::default();
    let result = super::super::regions::narrow(
        (&theory, &narrower),
        None,
        &Conditions::<(Theory, Narrower)>::default(),
        &mut region,
        &mut knowledge,
        &mut budget,
        &mut counts,
    );
    assert!(matches!(result, Err(Incomplete::WorkLimit)));
    assert_eq!(
        counts.work, 1,
        "the primitive must stop before a second read"
    );
    assert_eq!(budget.statistics.work, 1);
    drop(budget);
    let mut spent = SearchStatistics::default();
    shared.record(&mut spent);
    assert_eq!(spent.work, 1);
}

#[test]
fn a_panicked_worker_keeps_coverage_incomplete() {
    let cancellation =
        Cancellation::with_deadline(std::time::Instant::now() + Duration::from_secs(2)).unwrap();
    let theory = Theory::new(
        0,
        vec![],
        vec![],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    let limits = Limits::default();
    let mut budget = Budget {
        quota: crate::search::LocalQuota,
        limits: limits.search,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let mut search = ParallelRegions::new(
        &theory,
        NonZeroUsize::new(2).unwrap(),
        limits,
        cancellation.clone(),
        &mut budget,
    )
    .unwrap();
    // Another worker waits for stealable work that never comes (the root is
    // held here, outside the queues). The panic closes the run, and the
    // waiter exits rather than hanging.
    search.started = true;
    let _root = search.shared.take_local(0).unwrap();
    let idle_shared = Arc::clone(&search.shared);
    // The waiter owns the only remaining sender, so the channel disconnects
    // when it exits, letting the coordinator join and report the panic.
    let sender = search.sender.take().unwrap();
    search.handles.push(std::thread::spawn(move || {
        contain_worker(&idle_shared, || worker(&idle_shared, 0, &sender))
    }));
    let failed_shared = Arc::clone(&search.shared);
    search.handles.push(std::thread::spawn(move || {
        contain_worker(&failed_shared, || {
            let mut lease = failed_shared.budget.lease(&failed_shared.cancellation);
            let _reservation = lease.reserve(4).unwrap();
            panic!("injected failure while a worker owns a kernel allowance");
        })
    }));
    assert!(matches!(
        search.propose(None, false, &mut budget),
        Err(Incomplete::WorkerPanicked),
    ));
    assert!(!search.exhausted);
    assert_eq!(budget.statistics.work, 4);
}

#[test]
fn a_refused_certificate_refunds_its_reserved_work() {
    let cancellation =
        Cancellation::with_deadline(std::time::Instant::now() + Duration::from_secs(2)).unwrap();
    let theory = Theory::new(
        1,
        vec![zetesis_ferraris::Node::Atom(0)],
        vec![0],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    let plan = zetesis_ferraris::TightPlan::compile(
        &theory,
        zetesis_ferraris::TightPlanLimits::default(),
        &cancellation,
    )
    .unwrap();
    // Inject a lowered checking-storage allowance after valid construction:
    // the entered kernel refuses before consuming any of its reserved work.
    let certificate = Arc::new(Certification::Tight {
        plan: Arc::new(plan),
        max_bytes: 0,
    });
    let limits = Limits::default();
    let mut budget = Budget {
        quota: crate::search::LocalQuota,
        limits: limits.search,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let mut search = ParallelRegions::new(
        &theory,
        NonZeroUsize::new(2).unwrap(),
        limits,
        cancellation.clone(),
        &mut budget,
    )
    .unwrap();
    assert!(matches!(
        search.propose(Some(&certificate), false, &mut budget),
        Err(Incomplete::Certificate(crate::CertificateError::Tight(
            zetesis_ferraris::TightError::Limit(zetesis_ferraris::TightResource::Bytes),
        ))),
    ));
    let checked = search.merged().certified.unwrap();
    assert_eq!(
        (checked.checks, checked.failed, checked.checking_work),
        (1, 1, 0)
    );
    assert_eq!(budget.statistics.work, search.statistics().counts.work);
    assert!(!search.exhausted);
}

mod coordination_tests;
mod timing_tests;
