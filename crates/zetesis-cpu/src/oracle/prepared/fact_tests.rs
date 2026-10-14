//! Explicit facts share immutable storage without supplying candidate truth.

use super::*;
use crate::oracle::{Gates, Schedule, restrictions};
use crate::{CandidateRestrictionLimits, Limits};
use zetesis_core::{
    AdmissionLimits, Atom, FactTables, Predicate, Program, Seed, Template, Term, Value,
};
use zetesis_test_support::programs::{atom, fact, fact_tables, nullary, pattern};
use zetesis_test_support::thresholds::first_success;

fn gate_atoms(mask: u8) -> Vec<Atom> {
    [
        (1, atom("blocked", vec![fact_tables::structured()])),
        (2, nullary("permit")),
    ]
    .into_iter()
    .filter_map(|(bit, atom)| (mask & bit != 0).then_some(atom))
    .collect()
}

fn run(
    prepared: &PreparedQueries,
    seed: SeedView<'_>,
    workspace: &mut ClosureWorkspace,
    schedule: Schedule,
) -> Check {
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, Limits::default().max_work);
    work.limits = Limits::default();
    prepared
        .check_scheduled(seed, workspace, schedule, &mut work)
        .unwrap()
}

#[test]
fn table_bootstrap_matches_explicit_frozen_reducts() {
    let [explicit, table] = fact_tables::interleaved();
    assert_eq!(table.templates().len(), 2);
    assert_eq!(table.facts().len(), 4);
    for max_dense_atoms in [0, 64] {
        let limits = PreparationLimits {
            max_dense_atoms,
            ..PreparationLimits::default()
        };
        let reference = PreparedQueries::new(&explicit, limits, &Cancellation::default()).unwrap();
        let prepared = PreparedQueries::new(&table, limits, &Cancellation::default()).unwrap();
        assert_eq!(
            prepared.statistics().dense_predicates,
            reference.statistics().dense_predicates
        );
        // Empty inferred bounds can have zero-position dense layouts even at
        // ceiling zero. The actual nonempty seed relation witnesses the route.
        let predicate = table.facts().at(0).unwrap().atom().predicate();
        assert_eq!(
            prepared.layouts.slot(predicate).is_some(),
            max_dense_atoms > 0
        );
        let mut workspace = ClosureWorkspace::default();
        let mut retained = Vec::new();
        for mask in [0, 3, 1, 2, 0] {
            let seed = Seed::new(&table, gate_atoms(mask)).unwrap();
            let reference_seed = Seed::new(&explicit, gate_atoms(mask)).unwrap();
            for schedule in [Schedule::Delta, Schedule::Full] {
                let actual = run(&prepared, seed.view(), &mut workspace, schedule);
                let expected = run(
                    &reference,
                    reference_seed.view(),
                    &mut ClosureWorkspace::default(),
                    schedule,
                );
                assert_eq!(actual.closure(), expected.closure());
                assert_eq!(actual.constraint_violated(), expected.constraint_violated());
                assert_eq!(actual.seed_mismatch(), expected.seed_mismatch());
                assert_eq!(actual.accepted(), expected.accepted());
                assert_eq!(actual.statistics().bindings, expected.statistics().bindings);
                retained.push((actual, expected));
            }
        }
        for (actual, expected) in retained {
            assert_eq!(actual.closure(), expected.closure());
        }
    }
}

#[test]
fn stored_facts_start_with_empty_discovery() {
    let [_, table] = fact_tables::nullary_duplicates();
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, Limits::default().max_work);
    work.limits = Limits::default();
    let mut workspace = ClosureWorkspace::default();
    workspace.catalogs.bind_program(&table, &mut work).unwrap();
    assert_eq!(workspace.catalogs.len(), 0);
    assert_eq!(workspace.catalogs.discovered(), 0);
    assert_eq!(
        workspace.shared_fact_bytes(),
        table.fact_catalog().unwrap().storage_bytes()
            - size_of::<zetesis_core::atom_interner::ClosedCatalog>() as u128
            - table.shared_vocabulary_bytes()
    );
    let seed = Seed::new(&table, []).unwrap();
    let prepared =
        PreparedQueries::new(&table, PreparationLimits::default(), &cancellation).unwrap();
    let check = prepared
        .check_view(
            seed.view(),
            &mut workspace,
            Limits::default(),
            &cancellation,
        )
        .unwrap();
    assert!(check.accepted());
    assert_eq!(check.closure().atoms().len(), 1);
    assert_eq!(check.closure().atoms().first().unwrap(), nullary("ready"));
    assert_eq!(check.statistics().bindings, 3);
}

#[test]
fn table_only_checks_obey_independent_limits() {
    let [_, table] = fact_tables::nullary_duplicates();
    let seed = Seed::new(&table, []).unwrap();
    let cancellation = Cancellation::default();
    let prepared =
        PreparedQueries::new(&table, PreparationLimits::default(), &cancellation).unwrap();
    assert!(matches!(
        prepared.check_view(
            seed.view(),
            &mut ClosureWorkspace::default(),
            Limits {
                max_derived_atoms: 0,
                ..Limits::default()
            },
            &cancellation
        ),
        Err(Stop::DerivedAtomLimit)
    ));
    let attempt = |max_closure_bytes| {
        prepared.check_view(
            seed.view(),
            &mut ClosureWorkspace::default(),
            Limits {
                max_closure_bytes,
                ..Limits::default()
            },
            &cancellation,
        )
    };
    let least = first_success(Limits::default().max_closure_bytes, |limit| {
        attempt(limit).is_ok()
    });
    assert!(attempt(least).unwrap().accepted());
    assert!(matches!(attempt(least - 1), Err(Stop::StorageLimit)));
    cancellation.cancel();
    assert!(matches!(
        prepared.check_view(
            seed.view(),
            &mut ClosureWorkspace::default(),
            Limits::default(),
            &cancellation
        ),
        Err(Stop::Cancelled)
    ));
}

#[test]
fn table_heads_keep_prepared_dense_coordinates() {
    let [_, table] = fact_tables::interleaved();
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, Limits::default().max_work);
    work.limits = Limits::default();
    let prepared =
        PreparedQueries::new(&table, PreparationLimits::default(), &cancellation).unwrap();
    for fact in table.facts() {
        let slot = prepared.layouts.slot(fact.atom().predicate()).unwrap();
        assert!(
            prepared
                .heads
                .fact_position(fact.index(), slot)
                .unwrap()
                .is_some()
        );
    }
    let seed = Seed::new(&table, []).unwrap();
    let closure = prepared
        .closure_with(
            Gates::Frozen(seed.view()),
            &mut ClosureWorkspace::default(),
            Schedule::Delta,
            &mut work,
        )
        .unwrap();
    assert!(
        closure
            .atoms
            .atoms()
            .iter()
            .any(|atom| atom == nullary("ready"))
    );
}

#[test]
fn restrictions_keep_original_fact_occurrence_accounting() {
    let constant = Value::Number(7);
    let constraint = Template::new(
        None,
        vec![pattern("seed", vec![Term::Constant(constant.clone())])],
        vec![pattern("permit", vec![])],
        vec![],
        vec![],
    );
    let explicit = Program::new(
        vec![fact("seed", vec![Term::Constant(constant.clone())]); 2]
            .into_iter()
            .chain([constraint.clone()])
            .collect(),
        FactTables::default(),
        AdmissionLimits::default(),
    )
    .unwrap();
    let mut facts = FactTables::default();
    for _ in 0..2 {
        facts
            .push_row(
                0,
                Predicate::new("seed", 1).unwrap(),
                vec![constant.clone()],
            )
            .unwrap();
    }
    let table = Program::new(vec![constraint], facts, AdmissionLimits::default()).unwrap();
    let cancellation = Cancellation::default();
    let expected = restrictions::Restrictions::compile(
        &explicit,
        CandidateRestrictionLimits::default(),
        &cancellation,
    )
    .result
    .unwrap();
    let actual = restrictions::Restrictions::compile(
        &table,
        CandidateRestrictionLimits::default(),
        &cancellation,
    )
    .result
    .unwrap();
    assert_eq!(actual.atoms, expected.atoms);
    assert_eq!(actual.conjunctions(), expected.conjunctions());
    assert_eq!(actual.conjunctions(), 1);
    assert_eq!(actual.atoms, 3);
    let limit = expected.atoms - 1;
    let narrow = CandidateRestrictionLimits {
        max_atoms: limit,
        ..CandidateRestrictionLimits::default()
    };
    assert!(
        restrictions::Restrictions::compile(&explicit, narrow, &cancellation)
            .result
            .is_err()
    );
    assert!(
        restrictions::Restrictions::compile(&table, narrow, &cancellation)
            .result
            .is_err()
    );
}
