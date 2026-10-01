//! External preparation shares no candidate truth and preserves admission.

use std::num::NonZeroUsize;

use rayon::prelude::*;
use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, Model, Predicate, Seed, Template, Term, Value,
};

use super::*;
use crate::BatchOracle;

fn oracle() -> BatchOracle {
    BatchOracle::new(
        NonZeroUsize::new(3).unwrap(),
        NonZeroUsize::new(12).unwrap(),
    )
    .unwrap()
}

fn prepared(program: &Program, limits: PreparationLimits) -> Arc<PreparedQueries> {
    Arc::new(PreparedQueries::new(program, limits, &Cancellation::default()).unwrap())
}

#[test]
fn adopted_queries_preserve_retained_answers() {
    let (graph, seeds) = super::super::tests::fixture();
    let prepared = prepared(graph.program(), PreparationLimits::default());
    let mut oracle = oracle();
    let mut retained = Vec::new();
    for order in [[0, 1, 2, 3], [3, 0, 1, 2], [2, 3, 0, 1]] {
        let checks = oracle
            .check_prepared_batch_views(
                &prepared,
                order.into_par_iter().map(|index| seeds[index].view()),
                Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        for (index, check) in order.into_iter().zip(checks) {
            let expected = crate::check(
                graph.program(),
                &seeds[index],
                Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
            retained.push((check.unwrap(), expected));
        }
        let cache = oracle.admission.lock().unwrap();
        assert!(Arc::ptr_eq(cache.prepared.as_ref().unwrap(), &prepared));
        let statistics = cache.statistics().unwrap();
        assert_eq!(statistics.preparation_builds, 0);
        assert_eq!(statistics.preparation_adoptions, 1);
        assert_eq!(statistics.preparation, Some(prepared.statistics()));
        assert_eq!(statistics.active_workspaces, 3);
        drop(cache);
        // Later batches cannot construct queries. The same retained exact owner
        // needs no new preparation work, even under this stricter work ceiling.
        oracle = oracle.with_preparation_limits(PreparationLimits {
            max_work: 0,
            ..PreparationLimits::default()
        });
    }
    for (actual, expected) in retained {
        assert_eq!(actual.closure(), expected.closure());
        assert_eq!(actual.accepted(), expected.accepted());
        assert_eq!(actual.constraint_violated(), expected.constraint_violated());
        assert_eq!(actual.seed_mismatch(), expected.seed_mismatch());
    }
}

#[test]
fn first_adoption_obeys_mandatory_work() {
    let (graph, seeds) = super::super::tests::fixture();
    let prepared = prepared(graph.program(), PreparationLimits::default());
    assert!(prepared.statistics().work > 0);
    let oracle = oracle().with_preparation_limits(PreparationLimits {
        max_work: 0,
        ..PreparationLimits::default()
    });
    assert!(matches!(
        oracle.check_prepared_batch_views(
            &prepared,
            seeds.par_iter().map(Seed::view),
            Limits::default(),
            &Cancellation::default()
        ),
        Err(BatchError::Preparation(Stop::WorkLimit))
    ));
    let statistics = oracle.query_statistics().unwrap();
    assert_eq!(statistics.preparation, None);
    assert_eq!(statistics.preparation_adoptions, 0);
    assert_eq!(statistics.preparation_builds, 0);
    assert_eq!(statistics.active_workspaces, 0);
}

#[test]
fn tighter_construction_bytes_require_fresh_queries() {
    let (graph, seeds) = super::super::tests::fixture();
    let limits = PreparationLimits::default();
    let prepared = prepared(graph.program(), limits);
    let limits = PreparationLimits {
        max_bytes: limits.max_bytes - 1,
        ..limits
    };
    assert!(prepared.statistics().retained_bytes < limits.max_bytes);
    let oracle = oracle().with_preparation_limits(limits);
    let checks = oracle
        .check_prepared_batch_views(
            &prepared,
            seeds.par_iter().map(Seed::view),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(checks.iter().all(Result::is_ok));
    let cache = oracle.admission.lock().unwrap();
    assert!(!Arc::ptr_eq(cache.prepared.as_ref().unwrap(), &prepared));
    assert_eq!(cache.statistics().unwrap().preparation_builds, 1);
    assert_eq!(cache.statistics().unwrap().preparation_adoptions, 0);
}

#[test]
fn incompatible_dense_policy_keeps_ordinary_preparation() {
    let (graph, seeds) = super::super::tests::fixture();
    let prepared = prepared(
        graph.program(),
        PreparationLimits {
            max_dense_atoms: 0,
            ..PreparationLimits::default()
        },
    );
    assert_eq!(prepared.statistics().dense_predicates, 0);
    let oracle = oracle();
    let checks = oracle
        .check_prepared_batch_views(
            &prepared,
            seeds.par_iter().map(Seed::view),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(checks.iter().all(Result::is_ok));
    let cache = oracle.admission.lock().unwrap();
    assert!(!Arc::ptr_eq(cache.prepared.as_ref().unwrap(), &prepared));
    let statistics = cache.statistics().unwrap();
    assert_eq!(statistics.preparation_builds, 1);
    assert_eq!(statistics.preparation_adoptions, 0);
    assert!(statistics.preparation.unwrap().dense_predicates > 0);
}

#[test]
fn cancelled_adoption_retains_no_owner() {
    let (graph, seeds) = super::super::tests::fixture();
    let prepared = prepared(graph.program(), PreparationLimits::default());
    let oracle = oracle();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert!(matches!(
        oracle.check_prepared_batch_views(
            &prepared,
            seeds.par_iter().map(Seed::view),
            Limits::default(),
            &cancellation
        ),
        Err(BatchError::Preparation(Stop::Cancelled))
    ));
    let statistics = oracle.query_statistics().unwrap();
    assert_eq!(statistics.preparation, None);
    assert_eq!(statistics.preparation_adoptions, 0);
    assert_eq!(statistics.active_workspaces, 0);
}

#[test]
fn empty_batches_do_not_adopt_queries() {
    let (graph, _) = super::super::tests::fixture();
    let prepared = prepared(graph.program(), PreparationLimits::default());
    let oracle = oracle();
    let seeds: Vec<Seed> = Vec::new();
    assert!(
        oracle
            .check_prepared_batch_views(
                &prepared,
                seeds.par_iter().map(Seed::view),
                Limits::default(),
                &Cancellation::default()
            )
            .unwrap()
            .is_empty()
    );
    let statistics = oracle.query_statistics().unwrap();
    assert_eq!(statistics.preparation, None);
    assert_eq!(statistics.preparation_adoptions, 0);
    assert_eq!(statistics.retained_bytes, 0);
}

#[test]
fn supplied_queries_reject_foreign_seed_owners() {
    let (graph, _) = super::super::tests::fixture();
    let (foreign, seeds) = super::super::tests::fixture();
    assert!(!graph.program().same_instance(foreign.program()));
    let prepared = prepared(graph.program(), PreparationLimits::default());
    let checks = oracle()
        .check_prepared_batch_views(
            &prepared,
            seeds.par_iter().map(Seed::view),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(
        checks
            .into_iter()
            .all(|check| matches!(check, Err(Stop::WrongProgram)))
    );
}

fn changing_layout_program() -> (Program, Model) {
    let atom = |name: &str, numbers: &[i32]| {
        Atom::new(
            Predicate::new(name, numbers.len()).unwrap(),
            numbers.iter().copied().map(Value::Number).collect(),
        )
        .unwrap()
    };
    let facts = [
        atom("a", &[1, 10]),
        atom("a", &[3, 11]),
        atom("a", &[4, 12]),
        atom("a", &[5, 13]),
        atom("b", &[0]),
        atom("b", &[1]),
        atom("p", &[1]),
    ];
    let mut templates: Vec<_> = facts
        .iter()
        .map(|atom| {
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
            )
        })
        .collect();
    let pattern = |name: &str, terms: Vec<Term>| {
        AtomPattern::new(Predicate::new(name, terms.len()).unwrap(), terms).unwrap()
    };
    let x = Term::Variable(0);
    templates.push(Template::new(
        Some(pattern("p", vec![x.clone()])),
        vec![
            pattern("a", vec![x.clone(), Term::Variable(1)]),
            pattern("b", vec![x]),
        ],
        vec![],
        vec![],
        vec![],
    ));
    (
        Program::new(templates, AdmissionLimits::default()).unwrap(),
        Model::new(facts).unwrap(),
    )
}

#[test]
fn changed_preparation_retires_the_previous_layouts() {
    // At width three a's first argument is unknown and p has axes [0,1];
    // at four p has [1]. Reusing a prior coordinate zero would derive p(0).
    let (program, expected) = changing_layout_program();
    let preparations = [3, 4].map(|max_dense_atoms| {
        prepared(
            &program,
            PreparationLimits {
                max_dense_atoms,
                ..PreparationLimits::default()
            },
        )
    });
    assert!(
        preparations[0]
            .program()
            .same_instance(preparations[1].program())
    );
    let seed = Seed::new(&program, []).unwrap();
    let mut oracle = oracle();
    let mut retained = Vec::new();
    for at in [0, 1, 0, 1] {
        oracle = oracle.with_preparation_limits(PreparationLimits {
            max_dense_atoms: [3, 4][at],
            ..PreparationLimits::default()
        });
        let mut checks = oracle
            .check_prepared_batch_views(
                &preparations[at],
                std::slice::from_ref(&seed).par_iter().map(Seed::view),
                Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        let check = checks.pop().unwrap().unwrap();
        assert!(check.accepted());
        assert_eq!(check.closure(), &expected);
        retained.push(check);
        let cache = oracle.admission.lock().unwrap();
        assert!(Arc::ptr_eq(
            cache.prepared.as_ref().unwrap(),
            &preparations[at]
        ));
        let statistics = cache.statistics().unwrap();
        assert_eq!(statistics.preparation_builds, 0);
        assert_eq!(statistics.reused_workspaces, 0);
    }
    assert_eq!(oracle.query_statistics().unwrap().preparation_adoptions, 4);
    for check in retained {
        assert_eq!(check.closure(), &expected);
    }
}

#[test]
fn external_owner_header_enters_collective_admission() {
    let (graph, seeds) = super::super::tests::fixture();
    let prepared = prepared(graph.program(), PreparationLimits::default());
    let limits = Limits::default();
    let mut cache = Cache::default();
    cache
        .prepare(
            graph.program(),
            Some(&prepared),
            1,
            PreparationLimits::default(),
            usize::MAX,
            &Cancellation::default(),
        )
        .unwrap();
    let workspace = ClosureWorkspace::default().retained_bytes().unwrap();
    let header = size_of::<PreparedQueries>() as u128;
    assert_eq!(
        cache.statistics().unwrap().retained_bytes,
        header + workspace
    );
    let required =
        header + limits.max_closure_bytes as u128 - prepared.statistics().retained_bytes as u128;
    cache
        .admit(1, limits, usize::try_from(required).unwrap())
        .unwrap();
    assert_eq!(cache.statistics().unwrap().reserved_bytes, required);
    let oracle = oracle().with_closure_storage_limit(usize::try_from(required - 1).unwrap());
    assert!(
        matches!(oracle.check_prepared_batch_views(&prepared, seeds[..1].par_iter().map(Seed::view), limits, &Cancellation::default()), Err(BatchError::ClosureStorage { required: actual, .. }) if actual == required)
    );
}

#[test]
fn unavailable_narrowing_uses_mandatory_preparation() {
    let (graph, _) = super::super::tests::fixture();
    let mut candidates = crate::Candidates::new(
        graph.program(),
        crate::CandidateLimits::default(),
        Cancellation::default(),
    );
    candidates.bounded(Limits {
        max_work: 0,
        ..Limits::default()
    });
    let seed = candidates.next_selection().unwrap().unwrap();
    assert_eq!(
        candidates.statistics().narrowing_stop,
        Some(Stop::WorkLimit)
    );
    assert!(candidates.prepared_queries().is_none());
    let oracle = oracle();
    let checks = oracle
        .check_batch_views(
            graph.program(),
            std::slice::from_ref(&seed)
                .par_iter()
                .map(zetesis_core::SeedSelection::view),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(checks[0].is_ok());
    let statistics = oracle.query_statistics().unwrap();
    assert_eq!(statistics.preparation_builds, 1);
    assert_eq!(statistics.preparation_adoptions, 0);
}

#[test]
fn adopted_queries_preserve_candidate_storage_refusal() {
    let (graph, seeds) = super::super::tests::fixture();
    let prepared = prepared(graph.program(), PreparationLimits::default());
    let limits = Limits {
        max_closure_bytes: prepared.statistics().retained_bytes,
        ..Limits::default()
    };
    let oracle = oracle();
    let checks = oracle
        .check_prepared_batch_views(
            &prepared,
            seeds[..1].par_iter().map(Seed::view),
            limits,
            &Cancellation::default(),
        )
        .unwrap();
    assert!(matches!(checks[0], Err(Stop::StorageLimit)));
    let statistics = oracle.query_statistics().unwrap();
    assert_eq!(statistics.preparation_builds, 0);
    assert_eq!(statistics.preparation_adoptions, 1);
    assert_eq!(statistics.active_workspaces, 1);
    let recovered = oracle
        .check_prepared_batch_views(
            &prepared,
            seeds[..1].par_iter().map(Seed::view),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(recovered[0].is_ok());
    assert_eq!(oracle.query_statistics().unwrap().preparation_adoptions, 1);
}
