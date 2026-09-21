//! Live join ownership at the source-coordinator boundary.

use zetesis_core::{AdmissionLimits, AtomPattern, Predicate, Template};

use super::*;

fn pattern(name: &str) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap()
}

#[test]
fn completed_scans_retain_the_join_reservation() {
    let program = Program::new(
        vec![Template::new(
            Some(pattern("goal")),
            ["a", "b", "c"].into_iter().map(pattern).collect(),
            vec![],
            vec![],
            vec![],
        )],
        AdmissionLimits::default(),
    )
    .unwrap();
    let limits = Limits::default();
    let mut state = State::new(1, limits).unwrap();
    state
        .scan(
            &program,
            SourceSelection::Worlds,
            limits,
            &Cancellation::default(),
            &mut Progress::default(),
            &mut evaluate,
        )
        .unwrap();
    let retained = state.world_workspace.as_ref().unwrap().bytes();
    assert_eq!(state.transport.source_mask_bytes, retained);
    let atom = Atom::new(Predicate::new("a", 0).unwrap(), vec![]).unwrap();
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, u64::MAX);
    state.intern(&atom, limits, &mut work).unwrap();
    // Establish the lookup path once; the next probe retains exactly these
    // owners, including the workspace from the completed source scan.
    state.intern(&atom, limits, &mut work).unwrap();
    let identity = std::ptr::from_ref(state.catalog.get(0).unwrap());
    let ceiling = state.transport.fixed_bytes
        + state.transport.payload_bytes
        + size_of::<usize>()
        + usize::try_from(state.catalog.storage_bytes()).unwrap()
        + retained;
    let below = Limits {
        max_host_bytes: ceiling - 1,
        ..limits
    };
    assert!(matches!(
        state.intern(&atom, below, &mut work),
        Err(Stop::Allocation)
    ));
    assert_eq!(state.catalog.len(), 1);
    let exact = Limits {
        max_host_bytes: ceiling,
        ..limits
    };
    assert_eq!(state.intern(&atom, exact, &mut work).unwrap(), 0);
    assert_eq!(std::ptr::from_ref(state.catalog.get(0).unwrap()), identity);
}

#[test]
fn failed_scans_retain_the_join_reservation() {
    let templates = ["a", "b"]
        .into_iter()
        .map(|name| Template::new(Some(pattern(name)), vec![], vec![], vec![], vec![]))
        .collect();
    let program = Program::new(templates, AdmissionLimits::default()).unwrap();
    let limits = Limits {
        max_chunk_rules: 1,
        ..Default::default()
    };
    let mut state = State::new(1, limits).unwrap();
    let failed = state.scan(
        &program,
        SourceSelection::Worlds,
        limits,
        &Cancellation::default(),
        &mut Progress::default(),
        &mut |_| Err::<Vec<u32>, _>(Stop::Cancelled),
    );
    assert!(matches!(
        failed.unwrap_err().cause,
        source::ScanCause::Consumer(Cause::Execution(Stop::Cancelled))
    ));
    let retained = state.world_workspace.as_ref().unwrap().bytes();
    assert_eq!(state.transport.source_mask_bytes, retained);
    assert_eq!(retained, size_of::<u32>());
}

#[test]
fn growing_transport_reserves_live_identity_and_source_storage() {
    let limits = Limits::default();
    let mut state = State::new(1, limits).unwrap();
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, u64::MAX);
    for id in 0..32 {
        let atom = Atom::new(Predicate::new(format!("a{id:02}"), 0).unwrap(), vec![]).unwrap();
        state.intern(&atom, limits, &mut work).unwrap();
    }
    state
        .commit(limits, &cancellation, &mut Progress::default())
        .unwrap();
    state.transport.snapshots[0] = 0x8000_0001;
    state.transport.seeds[0] = 0x0000_0002;
    state.transport.pending[0] = 0x0000_0004;
    let program = Program::new(vec![], AdmissionLimits::default()).unwrap();
    let (snapshot, appender) = prepare_snapshot(
        &mut state.catalog,
        &state.transport,
        &mut state.world_workspace,
        &program,
        SourceSelection::Worlds,
        limits,
        &mut work,
    )
    .unwrap();
    // The actual source view retains all 32 ID slots, two selected membership
    // rows and a root frame. Its atoms remain borrowed from the catalog.
    let source_bytes = snapshot.bytes();
    assert_eq!(source_bytes, 32 * size_of::<usize>() + 3 * size_of::<u32>());
    state.transport.source_mask_bytes = source_bytes;
    let payload = state.transport.payload_bytes + 3;
    let catalog_bytes = appender.storage_bytes();
    let base = state.transport.fixed_bytes - 5 * size_of::<u32>();
    // Three old vectors coexist with three replacements; six new-width vectors
    // conservatively cover that overlap, plus the other independently owned data.
    let ceiling = base
        + 6 * 2 * size_of::<u32>()
        + payload
        + 33 * size_of::<usize>()
        + source_bytes
        + usize::try_from(catalog_bytes).unwrap();
    let below = Limits {
        max_host_bytes: ceiling - 1,
        ..limits
    };
    assert!(matches!(
        state.transport.grow(32, payload, catalog_bytes, below),
        Err(Stop::Allocation)
    ));
    assert_eq!(state.transport.words, 1);
    assert_eq!(state.transport.snapshots, [0x8000_0001]);
    let exact = Limits {
        max_host_bytes: ceiling,
        ..limits
    };
    state
        .transport
        .grow(32, payload, catalog_bytes, exact)
        .unwrap();
    assert_eq!(state.transport.words, 2);
    assert_eq!(state.transport.snapshots, [0x8000_0001, 0]);
    assert_eq!(state.transport.seeds, [2, 0]);
    assert_eq!(state.transport.pending, [4, 0]);
    assert_eq!(appender.len(), 32);
    assert_eq!(snapshot.bytes(), source_bytes);
}

#[test]
fn interrupted_commit_retains_discovered_identity_progress() {
    let limits = Limits::default();
    let mut state = State::new(1, limits).unwrap();
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, u64::MAX);
    let atom = Atom::new(Predicate::new("pending", 0).unwrap(), vec![]).unwrap();
    state.intern(&atom, limits, &mut work).unwrap();
    let mut progress = Progress::default();
    progress.record_source(work.source_statistics(0));
    cancellation.cancel();
    assert!(matches!(
        state.commit(limits, &cancellation, &mut progress),
        Err(Stop::Cancelled)
    ));
    assert_eq!(progress.catalog_atoms, 1);
    assert_eq!(progress.rounds, 0);
    assert_eq!(state.catalog.get(0), Some(&atom));
    assert_eq!(state.catalog.split().0.len(), 0);
}
