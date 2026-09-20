//! Fault injection at the production worker accounting and failure boundaries.

use super::*;

#[test]
fn shared_narrowing_never_executes_an_unleased_read() {
    let control =
        Control::with_deadline(std::time::Instant::now() + Duration::from_secs(2)).unwrap();
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
        quota: shared.lease(&control),
        // Worker-local work is only a delta; the shared owner has the
        // authoritative one-permit remainder of the whole search.
        limits: crate::SearchLimits::default(),
        control: &control,
        statistics: SearchStatistics::default(),
    };
    let narrower = Narrower::new(&theory);
    let mut region = Region::all_open(theory.atom_count());
    let mut knowledge = vec![narrower.knowledge()];
    let mut counts = RegionCounts::default();
    let result = super::super::regions::narrow(
        (&theory, &narrower),
        None,
        &[] as &[(Theory, Narrower)],
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
    let control =
        Control::with_deadline(std::time::Instant::now() + Duration::from_secs(2)).unwrap();
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
        control: &control,
        statistics: SearchStatistics::default(),
    };
    let mut search = ParallelRegions::new(
        &theory,
        NonZeroUsize::new(2).unwrap(),
        limits,
        control.clone(),
        &mut budget,
    )
    .unwrap();
    // The failing worker owns the missing frontier; another worker waits
    // for it to offer work. The panic must wake that waiter immediately.
    search.shared.lock().pending.clear();
    search.started = true;
    let idle_shared = Arc::clone(&search.shared);
    let sender = search.sender.take().unwrap();
    search.handles.push(std::thread::spawn(move || {
        contain_worker(&idle_shared, || worker(&idle_shared, &sender))
    }));
    while search.shared.lock().idle == 0 {
        control.poll().unwrap();
        std::thread::yield_now();
    }
    let failed_shared = Arc::clone(&search.shared);
    search.handles.push(std::thread::spawn(move || {
        contain_worker(&failed_shared, || {
            let mut lease = failed_shared.budget.lease(&failed_shared.control);
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
    let control =
        Control::with_deadline(std::time::Instant::now() + Duration::from_secs(2)).unwrap();
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
        &control,
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
        control: &control,
        statistics: SearchStatistics::default(),
    };
    let mut search = ParallelRegions::new(
        &theory,
        NonZeroUsize::new(2).unwrap(),
        limits,
        control.clone(),
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
