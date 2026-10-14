//! Anchored facts keep source order and all source-scan failure contracts.

use zetesis_core::{
    AdmissionLimits, Atom, AtomCatalog, FactTables, Predicate, Template, ValueLimits,
};
use zetesis_test_support::programs::{atom, fact, fact_tables, nullary, pattern, program};

use super::*;

#[derive(Debug, PartialEq, Eq)]
struct Offered {
    head: Option<Atom>,
    positive: Vec<Atom>,
    gate_true: Vec<Atom>,
    gate_false: Vec<Atom>,
}

impl Offered {
    fn from(instance: &Instance<'_>) -> Self {
        let owned = |key: AtomKey<'_>| key.to_atom(ValueLimits::default()).unwrap();
        Self {
            head: instance.head().map(owned),
            positive: instance.positive().iter().map(owned).collect(),
            gate_true: instance.gate_true().iter().map(owned).collect(),
            gate_false: instance.gate_false().iter().map(owned).collect(),
        }
    }
}

fn truth() -> Model {
    Model::new([
        atom("seed", vec![fact_tables::structured()]),
        atom("joined", vec![fact_tables::structured()]),
    ])
    .unwrap()
}

fn collect(
    program: &Program,
    limits: ScanLimits,
) -> (Vec<Offered>, Result<ScanStatistics, ScanFailure<Stop>>) {
    let mut offered = Vec::new();
    let result = scan(
        program,
        &truth(),
        limits,
        &Cancellation::default(),
        |instance| {
            offered.push(Offered::from(&instance));
            Ok(())
        },
    );
    (offered, result)
}

#[test]
fn table_facts_preserve_ordered_instances_and_work() {
    let [explicit, table] = fact_tables::interleaved();
    assert_eq!(table.templates().len(), 2);
    assert_eq!(table.facts().len(), 4);
    let (expected, expected_result) = collect(&explicit, ScanLimits::default());
    let (actual, actual_result) = collect(&table, ScanLimits::default());
    assert_eq!(actual, expected);
    assert_eq!(actual_result.unwrap(), expected_result.unwrap());
    let heads = [
        Some(atom("seed", vec![fact_tables::structured()])),
        Some(atom("joined", vec![fact_tables::structured()])),
        Some(nullary("ready")),
        Some(atom("seed", vec![fact_tables::structured()])),
        None,
        Some(nullary("tail")),
    ];
    assert_eq!(actual.len(), heads.len());
    for (offered, head) in actual.iter().zip(heads) {
        assert_eq!(offered.head, head);
    }
    assert_eq!(
        actual[1].positive,
        [atom("seed", vec![fact_tables::structured()])]
    );
    assert_eq!(
        actual[1].gate_false,
        [atom("blocked", vec![fact_tables::structured()])]
    );
    assert_eq!(actual[4].gate_false, [nullary("permit")]);
}

#[test]
fn every_work_cutoff_preserves_the_same_fact_prefix() {
    let [explicit, table] = fact_tables::interleaved();
    let (_, completed) = collect(&explicit, ScanLimits::default());
    let completed = completed.unwrap();
    for cutoff in 0..completed.work {
        let limits = ScanLimits {
            max_work: cutoff,
            ..ScanLimits::default()
        };
        let (expected, expected_result) = collect(&explicit, limits);
        let (actual, actual_result) = collect(&table, limits);
        let expected_failure = expected_result.unwrap_err();
        let actual_failure = actual_result.unwrap_err();
        assert_eq!(actual, expected, "work cutoff {cutoff}");
        assert_eq!(actual_failure.statistics, expected_failure.statistics);
        assert!(matches!(
            actual_failure.cause,
            ScanCause::Source(Stop::WorkLimit)
        ));
        assert!(matches!(
            expected_failure.cause,
            ScanCause::Source(Stop::WorkLimit)
        ));
    }
    let (_, exact) = collect(
        &table,
        ScanLimits {
            max_work: completed.work,
            ..ScanLimits::default()
        },
    );
    assert_eq!(exact.unwrap(), completed);
}

#[test]
fn refusing_a_fact_consumer_never_completes_the_scan() {
    let [_, table] = fact_tables::interleaved();
    let (expected, _) = collect(&table, ScanLimits::default());
    for cutoff in 0..expected.len() {
        let mut offered = Vec::new();
        let failure = scan(
            &table,
            &truth(),
            ScanLimits::default(),
            &Cancellation::default(),
            |instance| {
                offered.push(Offered::from(&instance));
                if offered.len() == cutoff + 1 {
                    Err(cutoff)
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert!(matches!(failure.cause, ScanCause::Consumer(actual) if actual == cutoff));
        assert_eq!(offered, expected[..=cutoff]);
        assert_eq!(
            failure.statistics.bindings,
            u64::try_from(cutoff + 1).unwrap()
        );
    }
}

#[test]
fn table_facts_obey_instance_and_scan_storage_limits() {
    let [_, table] = fact_tables::nullary_duplicates();
    for (limits, expected) in [
        (
            ScanLimits {
                max_instance_atoms: 0,
                ..ScanLimits::default()
            },
            Stop::CarrierLimit,
        ),
        (
            ScanLimits {
                max_instance_bytes: 0,
                ..ScanLimits::default()
            },
            Stop::Allocation,
        ),
        (
            ScanLimits {
                max_scan_bytes: 0,
                ..ScanLimits::default()
            },
            Stop::StorageLimit,
        ),
    ] {
        let failure = scan(
            &table,
            &Model::default(),
            limits,
            &Cancellation::default(),
            |_| -> Result<(), Stop> { panic!("a refused fact cannot be offered") },
        )
        .unwrap_err();
        assert!(matches!(failure.cause, ScanCause::Source(actual) if actual == expected));
        assert_eq!(failure.statistics.bindings, 0);
    }
}

#[test]
fn cancellation_after_the_final_fact_prevents_completion() {
    let [_, table] = fact_tables::nullary_duplicates();
    let cancellation = Cancellation::default();
    let mut offered = 0;
    let failure = scan(
        &table,
        &Model::default(),
        ScanLimits::default(),
        &cancellation,
        |_| {
            offered += 1;
            if offered == 3 {
                cancellation.cancel();
            }
            Ok::<(), Stop>(())
        },
    )
    .unwrap_err();
    assert!(matches!(failure.cause, ScanCause::Source(Stop::Cancelled)));
    assert_eq!(offered, 3);
    assert_eq!(failure.statistics.bindings, 3);
}

#[test]
fn a_world_pruned_join_does_not_suppress_the_next_fact() {
    let constraint = Template::new(
        None,
        vec![pattern("a", vec![]), pattern("b", vec![])],
        vec![],
        vec![],
        vec![],
    );
    let explicit = program(vec![constraint.clone(), fact("ready", vec![])]);
    let mut facts = FactTables::default();
    facts
        .push_row(1, Predicate::new("ready", 0).unwrap(), vec![])
        .unwrap();
    let table = Program::new(vec![constraint], facts, AdmissionLimits::default()).unwrap();
    for source in [explicit, table] {
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, u64::MAX);
        let catalog = AtomCatalog::new(vec![nullary("a"), nullary("b")]).unwrap();
        // Each positive atom occurs somewhere, but no world contains both.
        let masks = [1, 2];
        let workspace = worlds::Workspace::new(&source, 2, usize::MAX, &mut work).unwrap();
        let rows = worlds::Rows::select(
            catalog.atoms(),
            vec![0, 1],
            &masks,
            1,
            2,
            usize::MAX,
            &mut work,
        )
        .unwrap();
        let mut snapshot = workspace
            .snapshot(rows, &masks, 1, usize::MAX, &mut work)
            .unwrap();
        let mut offered = Vec::new();
        let statistics = scan_worlds(
            &source,
            &mut snapshot,
            ScanLimits::default(),
            &mut work,
            |instance, _| {
                offered.push(Offered::from(&instance));
                Ok::<(), Stop>(())
            },
        )
        .unwrap();
        assert_eq!(statistics.pruned_prefixes, 1);
        assert_eq!(statistics.bindings, 1);
        assert_eq!(offered.len(), 1);
        assert_eq!(offered[0].head, Some(nullary("ready")));
        assert!(offered[0].positive.is_empty());
    }
}
