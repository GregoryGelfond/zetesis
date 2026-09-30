//! Live join ownership at the source-coordinator boundary.

use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, Predicate, Template, Term, Value, ValueNodeRef,
};
use zetesis_test_support::programs::nullary_pattern as pattern;

use super::*;

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
    let mut state = State::new(&program, 1, limits).unwrap();
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
    let identity = state.catalog.get(0).unwrap().predicate().name().as_ptr();
    // Measure the entry's own envelope independently of the transport owner.
    // Even an occupied lookup retains its prepared-result slot while the
    // completed source workspace remains live.
    let bounds = state
        .transport
        .catalog_limits(limits, state.catalog.len())
        .unwrap();
    let entry_bytes = state
        .catalog
        .entry_atom_with(&atom, bounds, || work.tick())
        .unwrap()
        .storage_bytes();
    let ceiling = state.transport.fixed_bytes
        + size_of::<usize>()
        + usize::try_from(entry_bytes - program.shared_vocabulary_bytes()).unwrap()
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
    assert_eq!(
        state.catalog.get(0).unwrap().predicate().name().as_ptr(),
        identity
    );
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
    let mut state = State::new(&program, 1, limits).unwrap();
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
    let program = Program::new(
        (0..33)
            .map(|id| {
                Template::new(
                    Some(pattern(&format!("a{id:02}"))),
                    vec![],
                    vec![],
                    vec![],
                    vec![],
                )
            })
            .collect(),
        AdmissionLimits::default(),
    )
    .unwrap();
    let limits = Limits::default();
    let mut state = State::new(&program, 1, limits).unwrap();
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
    let catalog_bytes = appender.storage_bytes();
    let base = state.transport.fixed_bytes - 5 * size_of::<u32>();
    // Three old vectors coexist with three replacements; six new-width vectors
    // conservatively cover that overlap, plus the other independently owned data.
    let ceiling = base
        + 6 * 2 * size_of::<u32>()
        + 33 * size_of::<usize>()
        + source_bytes
        + usize::try_from(catalog_bytes - program.shared_vocabulary_bytes()).unwrap();
    let below = Limits {
        max_host_bytes: ceiling - 1,
        ..limits
    };
    assert!(matches!(
        state.transport.grow(32, catalog_bytes, below),
        Err(Stop::Allocation)
    ));
    assert_eq!(state.transport.words, 1);
    assert_eq!(state.transport.snapshots, [0x8000_0001]);
    let exact = Limits {
        max_host_bytes: ceiling,
        ..limits
    };
    state.transport.grow(32, catalog_bytes, exact).unwrap();
    assert_eq!(state.transport.words, 2);
    assert_eq!(state.transport.snapshots, [0x8000_0001, 0]);
    assert_eq!(state.transport.seeds, [2, 0]);
    assert_eq!(state.transport.pending, [4, 0]);
    assert_eq!(appender.len(), 32);
    assert_eq!(snapshot.bytes(), source_bytes);
}

#[test]
fn interrupted_commit_retains_discovered_identity_progress() {
    let program = Program::new(
        vec![Template::new(
            Some(pattern("pending")),
            vec![],
            vec![],
            vec![],
            vec![],
        )],
        AdmissionLimits::default(),
    )
    .unwrap();
    let limits = Limits::default();
    let mut state = State::new(&program, 1, limits).unwrap();
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
    assert_eq!(state.catalog.get(0), Some(AtomRef::from(&atom)));
    assert_eq!(state.catalog.split().0.len(), 0);
}

#[test]
fn new_predicate_keys_share_canonical_term_storage() {
    let atom = Atom::new(
        Predicate::new("source", 1).unwrap(),
        vec![Value::String("shared payload".repeat(256))],
    )
    .unwrap();
    let pattern = AtomPattern::new(
        Predicate::new("derived", 1).unwrap(),
        vec![Term::Variable(0)],
    )
    .unwrap();
    let program = Program::new(
        vec![
            Template::new(
                Some(
                    AtomPattern::new(
                        atom.predicate().clone(),
                        atom.values().iter().cloned().map(Term::Constant).collect(),
                    )
                    .unwrap(),
                ),
                vec![],
                vec![],
                vec![],
                vec![],
            ),
            Template::new(
                Some(pattern.clone()),
                vec![AtomPattern::new(atom.predicate().clone(), vec![Term::Variable(0)]).unwrap()],
                vec![],
                vec![],
                vec![],
            ),
        ],
        AdmissionLimits::default(),
    )
    .unwrap();
    let limits = Limits::default();
    let mut state = State::new(&program, 1, limits).unwrap();
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, u64::MAX);
    state.intern(&atom, limits, &mut work).unwrap();
    state
        .commit(limits, &cancellation, &mut Progress::default())
        .unwrap();
    let (committed, mut appender) = state.catalog.split();
    let value = committed.get(0).unwrap().values().at(0).unwrap();
    let frame = [Some(value)];
    let key = pattern.key(frame.as_slice()).unwrap();
    assert_eq!(
        state
            .transport
            .intern_key(&mut appender, key, limits, &mut work)
            .unwrap(),
        1
    );
    let derived = appender.get(1).unwrap().values().at(0).unwrap();
    let (ValueNodeRef::String(original), ValueNodeRef::String(imported)) =
        (value.descriptor(), derived.descriptor())
    else {
        panic!("fixture has a string argument");
    };
    assert!(std::ptr::eq(original, imported));
    let ValueNodeRef::String(program_text) = program.domain().get(0).unwrap().descriptor() else {
        panic!("fixture domain has the string argument");
    };
    assert!(std::ptr::eq(original, program_text));
    assert_eq!(committed.len(), 1);
    assert_eq!(appender.len(), 2);
    assert_eq!(state.transport.snapshots, [0]);
}

#[test]
fn the_initial_host_ceiling_charges_new_metadata_not_the_shared_input_base() {
    let program = Program::new(
        vec![Template::new(
            Some(
                AtomPattern::new(
                    Predicate::new("large_input", 1).unwrap(),
                    vec![Term::Constant(Value::String("shared input".repeat(32_768)))],
                )
                .unwrap(),
            ),
            vec![],
            vec![],
            vec![],
            vec![],
        )],
        AdmissionLimits::default(),
    )
    .unwrap();
    let limits = Limits {
        max_chunk_rules: 1,
        max_chunk_words: RECORD_HEADER_WORDS,
        max_instance_bytes: 0,
        ..Limits::default()
    };
    let state = State::new(&program, 1, limits).unwrap();
    let shared = program.shared_vocabulary_bytes();
    assert_eq!(state.catalog.shared_vocabulary_bytes(), shared);
    let local = state.catalog.storage_bytes() - shared;
    let required = usize::try_from(state.transport.external_bytes(0).unwrap() + local).unwrap();
    assert!((required as u128) < shared);
    assert!(state.catalog.is_empty());
    let exact = Limits {
        max_host_bytes: required,
        ..limits
    };
    let admitted = State::new(&program, 1, exact).unwrap();
    assert_eq!(
        admitted
            .transport
            .catalog_limits(exact, 0)
            .unwrap()
            .max_bytes,
        admitted.catalog.storage_bytes(),
    );
    assert_eq!(
        admitted
            .transport
            .source_bytes(exact, admitted.catalog.storage_bytes(), 0)
            .unwrap(),
        0,
    );
    assert!(matches!(
        State::new(
            &program,
            1,
            Limits {
                max_host_bytes: required - 1,
                ..limits
            }
        ),
        Err(Stop::Allocation),
    ));
}

#[test]
fn batches_share_the_vocabulary_but_keep_independent_discovery_and_truth() {
    let program = Program::new(
        ["a", "b"]
            .into_iter()
            .map(|name| Template::new(Some(pattern(name)), vec![], vec![], vec![], vec![]))
            .collect(),
        AdmissionLimits::default(),
    )
    .unwrap();
    let limits = Limits::default();
    let mut first = State::new(&program, 1, limits).unwrap();
    let mut second = State::new(&program, 1, limits).unwrap();
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, u64::MAX);
    let a = Atom::new(Predicate::new("a", 0).unwrap(), vec![]).unwrap();
    let b = Atom::new(Predicate::new("b", 0).unwrap(), vec![]).unwrap();
    assert_eq!(first.intern(&a, limits, &mut work).unwrap(), 0);
    assert!(second.catalog.is_empty());
    assert_eq!(second.intern(&b, limits, &mut work).unwrap(), 0);
    assert_eq!(first.intern(&b, limits, &mut work).unwrap(), 1);
    assert_eq!(second.intern(&a, limits, &mut work).unwrap(), 1);
    assert_eq!(
        first.catalog.get(0).unwrap(),
        second.catalog.get(1).unwrap()
    );
    assert_eq!(
        first.catalog.get(0).unwrap().predicate().name().as_ptr(),
        second.catalog.get(1).unwrap().predicate().name().as_ptr(),
    );
    assert_eq!(first.transport.snapshots, [0]);
    assert_eq!(second.transport.snapshots, [0]);
    first.transport.snapshots[0] = 1;
    assert_eq!(second.transport.snapshots, [0]);
}

#[test]
fn empty_catalog_preparation_has_an_inclusive_host_cap() {
    let program = Program::new(vec![], AdmissionLimits::default()).unwrap();
    let limits = Limits {
        max_atoms: 1,
        max_chunk_rules: 1,
        max_chunk_words: RECORD_HEADER_WORDS,
        max_instance_bytes: 0,
        ..Limits::default()
    };
    let state = State::new(&program, 33, limits).unwrap();
    let local =
        usize::try_from(state.catalog.storage_bytes() - program.shared_vocabulary_bytes()).unwrap();
    let masks = 2 * size_of::<u32>();
    // The empty ordered-ID vector header and root-mask workspace are disjoint
    // preparation stages. Catalog publication is tested at the public boundary.
    let required = state.transport.fixed_bytes + local + size_of::<Vec<usize>>().max(masks);
    let cancellation = Cancellation::default();
    let exact = Limits {
        max_host_bytes: required,
        ..limits
    };
    let mut state = State::new(&program, 33, exact).unwrap();
    let mut work = Work::source(&cancellation, u64::MAX);
    let (snapshot, _) = prepare_snapshot(
        &mut state.catalog,
        &state.transport,
        &mut state.world_workspace,
        &program,
        SourceSelection::Worlds,
        exact,
        &mut work,
    )
    .unwrap();
    assert_eq!(snapshot.bytes(), masks);
    assert_eq!(work.source_statistics(0).mask_bytes, masks);
    drop(snapshot);

    let below = Limits {
        max_host_bytes: required - 1,
        ..limits
    };
    let mut state = State::new(&program, 33, below).unwrap();
    let mut work = Work::source(&cancellation, u64::MAX);
    assert!(matches!(
        prepare_snapshot(
            &mut state.catalog,
            &state.transport,
            &mut state.world_workspace,
            &program,
            SourceSelection::Worlds,
            below,
            &mut work,
        ),
        Err(Stop::Allocation),
    ));
    assert_eq!(work.source_statistics(0).mask_bytes, 0);
    assert_eq!(work.source_statistics(0).mask_words, 0);
}
