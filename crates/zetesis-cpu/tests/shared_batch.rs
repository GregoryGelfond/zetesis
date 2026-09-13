//! Complete occurrence and failure contracts for shared CPU source rounds.

use std::num::NonZeroUsize;
use std::time::Instant;

use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, Model, Predicate, Program, Seed, Template, Term, Value,
};
use zetesis_cpu::lazy::{SourceSelection, shared};
use zetesis_cpu::{BatchError, BatchOracle, Control, Limits, Stop};

fn pattern(name: &str, terms: Vec<Term>) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, terms.len()).unwrap(), terms).unwrap()
}

fn atom(name: &str, values: &[i32]) -> Atom {
    Atom::new(
        Predicate::new(name, values.len()).unwrap(),
        values.iter().copied().map(Value::Number).collect(),
    )
    .unwrap()
}

fn fixture(masks: &[u8]) -> (Program, Vec<Seed>) {
    let mut rules: Vec<_> = (0..4)
        .map(|value| {
            Template::new(
                Some(pattern("d", vec![Term::Constant(Value::Number(value))])),
                vec![],
                vec![],
                vec![],
                vec![],
            )
        })
        .collect();
    let p = pattern("p", vec![Term::Variable(0)]);
    rules.push(Template::new(
        Some(p.clone()),
        vec![pattern("d", vec![Term::Variable(0)])],
        vec![p.clone()],
        vec![],
        vec![],
    ));
    rules.push(Template::new(
        Some(pattern("pair", vec![Term::Variable(0), Term::Variable(1)])),
        vec![p, pattern("p", vec![Term::Variable(1)])],
        vec![],
        vec![],
        vec![],
    ));
    rules.push(Template::new(
        None,
        vec![pattern(
            "pair",
            vec![
                Term::Constant(Value::Number(0)),
                Term::Constant(Value::Number(1)),
            ],
        )],
        vec![],
        vec![],
        vec![],
    ));
    let program = Program::new(rules, AdmissionLimits::default()).unwrap();
    let seeds = masks
        .iter()
        .map(|mask| {
            Seed::new(
                &program,
                (0..4)
                    .filter(|value| mask & (1 << value) != 0)
                    .map(|value| atom("p", &[value])),
            )
            .unwrap()
        })
        .collect();
    (program, seeds)
}

fn pool(capacity: usize) -> BatchOracle {
    BatchOracle::new(
        NonZeroUsize::new(4).unwrap(),
        NonZeroUsize::new(capacity).unwrap(),
    )
    .unwrap()
}

fn limits() -> shared::Limits {
    shared::Limits {
        source: zetesis_cpu::lazy::Limits {
            max_chunk_rules: 3,
            max_chunk_words: 32,
            max_instance_bytes: 1024,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn expected(mask: u8) -> Model {
    let mut atoms: Vec<_> = (0..4).map(|value| atom("d", &[value])).collect();
    for left in 0..4 {
        if mask & (1 << left) == 0 {
            continue;
        }
        atoms.push(atom("p", &[left]));
        for right in 0..4 {
            if mask & (1 << right) != 0 {
                atoms.push(atom("pair", &[left, right]));
            }
        }
    }
    Model::new(atoms)
}

#[test]
fn all_occurrences_match_independent_reduct_closure() {
    let mut masks: Vec<_> = (0..16).rev().collect();
    masks.extend([0, 9, 15, 9]);
    let (program, seeds) = fixture(&masks);
    let pool = pool(seeds.len());
    let independent = pool
        .check_batch(&program, &seeds, Limits::default(), &Control::default())
        .unwrap();
    for selection in [SourceSelection::Union, SourceSelection::Worlds] {
        let batch = pool
            .check_shared(&program, &seeds, limits(), selection, &Control::default())
            .unwrap();
        assert_eq!(batch.checks.len(), masks.len());
        assert_eq!(batch.statistics.worlds.len(), masks.len());
        assert_eq!(batch.statistics.submitted_candidates, masks.len());
        for (((actual, reference), mask), progress) in batch
            .checks
            .iter()
            .zip(&independent)
            .zip(&masks)
            .zip(&batch.statistics.worlds)
        {
            let reference = reference.as_ref().unwrap();
            assert!(actual.program().same_instance(&program));
            assert_eq!(actual.closure(), &expected(*mask));
            assert_eq!(actual.closure(), reference.closure());
            assert_eq!(actual.constraint_violated(), *mask & 3 == 3);
            assert_eq!(actual.accepted(), reference.accepted());
            assert_eq!(actual.seed_mismatch(), reference.seed_mismatch());
            assert!(progress.interruption.is_none());
            assert!(progress.work >= progress.instances);
            assert!(progress.instances > 0);
        }
    }
}

#[test]
fn sparse_worlds_omit_cross_world_source_instances() {
    let (program, seeds) = fixture(&[1, 2, 4, 8, 1]);
    let pool = pool(seeds.len());
    let union = pool
        .check_shared(
            &program,
            &seeds,
            limits(),
            SourceSelection::Union,
            &Control::default(),
        )
        .unwrap();
    let worlds = pool
        .check_shared(
            &program,
            &seeds,
            limits(),
            SourceSelection::Worlds,
            &Control::default(),
        )
        .unwrap();
    assert!(worlds.statistics.source.pruned_prefixes > 0);
    assert!(worlds.statistics.source.instances < union.statistics.source.instances);
    assert_eq!(
        worlds.statistics.worlds.len(),
        union.statistics.worlds.len()
    );
    for (world, union) in worlds
        .statistics
        .worlds
        .iter()
        .zip(&union.statistics.worlds)
    {
        assert!(world.instances < union.instances);
    }
}

#[test]
fn dense_worlds_preserve_the_complete_source_carrier() {
    let (program, seeds) = fixture(&(0..16).collect::<Vec<_>>());
    let pool = pool(seeds.len());
    let union = pool
        .check_shared(
            &program,
            &seeds,
            limits(),
            SourceSelection::Union,
            &Control::default(),
        )
        .unwrap();
    let worlds = pool
        .check_shared(
            &program,
            &seeds,
            limits(),
            SourceSelection::Worlds,
            &Control::default(),
        )
        .unwrap();
    assert_eq!(worlds.statistics.source.pruned_prefixes, 0);
    assert_eq!(
        worlds.statistics.source.instances,
        union.statistics.source.instances
    );
    assert!(worlds.statistics.source.peak_mask_bytes > 0);
}

fn incomplete(result: Result<shared::Batch, shared::Error>) -> shared::Failure {
    let shared::Error::Incomplete(failure) = result.unwrap_err() else {
        panic!("expected incomplete execution, not submission refusal");
    };
    failure
}

#[test]
fn world_work_limits_are_inclusive() {
    let (program, seeds) = fixture(&[1, 6, 12]);
    let pool = pool(seeds.len());
    for selection in [SourceSelection::Union, SourceSelection::Worlds] {
        let complete = pool
            .check_shared(&program, &seeds, limits(), selection, &Control::default())
            .unwrap();
        let work = complete.statistics.worlds[0].work;
        assert!(work > 0);
        let mut exact = limits();
        exact.max_world_work = work;
        assert!(
            pool.check_shared(&program, &seeds, exact, selection, &Control::default())
                .is_ok()
        );
        exact.max_world_work -= 1;
        let failure =
            incomplete(pool.check_shared(&program, &seeds, exact, selection, &Control::default()));
        assert_eq!(
            failure.cause,
            shared::Cause::World {
                index: 0,
                stop: Stop::WorkLimit
            }
        );
        assert_eq!(failure.statistics.worlds.len(), seeds.len());
        for world in &failure.statistics.worlds {
            assert_eq!(world.work, work - 1);
            assert_eq!(world.interruption, Some(Stop::WorkLimit));
        }
    }
}

#[test]
fn source_work_limits_are_collective() {
    let (program, seeds) = fixture(&[1, 2, 4, 8]);
    let pool = pool(seeds.len());
    for selection in [SourceSelection::Union, SourceSelection::Worlds] {
        let complete = pool
            .check_shared(&program, &seeds, limits(), selection, &Control::default())
            .unwrap();
        let work = complete.statistics.source.source_work;
        let mut exact = limits();
        exact.source.max_source_work = work;
        assert!(
            pool.check_shared(&program, &seeds, exact, selection, &Control::default())
                .is_ok()
        );
        exact.source.max_source_work -= 1;
        let failure =
            incomplete(pool.check_shared(&program, &seeds, exact, selection, &Control::default()));
        assert_eq!(failure.cause, shared::Cause::Source(Stop::WorkLimit));
        assert_eq!(failure.statistics.source.source_work, work - 1);
    }
}

#[test]
fn shared_catalog_limits_count_distinct_batch_atoms() {
    let (program, seeds) = fixture(&[1, 2, 4, 8]);
    let pool = pool(seeds.len());
    let selection = SourceSelection::Worlds;
    let complete = pool
        .check_shared(&program, &seeds, limits(), selection, &Control::default())
        .unwrap();
    let mut exact = limits();
    exact.source.max_atoms = complete.statistics.source.catalog_atoms;
    assert!(
        pool.check_shared(&program, &seeds, exact, selection, &Control::default())
            .is_ok()
    );
    exact.source.max_atoms -= 1;
    let failure =
        incomplete(pool.check_shared(&program, &seeds, exact, selection, &Control::default()));
    assert_eq!(failure.cause, shared::Cause::Source(Stop::CarrierLimit));
}

#[test]
fn progress_storage_is_charged_before_allocation() {
    let (program, seeds) = fixture(&[1, 2]);
    let mut limited = limits();
    limited.source.max_host_bytes = size_of::<shared::WorldProgress>() * seeds.len() - 1;
    let failure = incomplete(pool(seeds.len()).check_shared(
        &program,
        &seeds,
        limited,
        SourceSelection::Union,
        &Control::default(),
    ));
    assert_eq!(failure.cause, shared::Cause::Source(Stop::Allocation));
    assert!(failure.statistics.worlds.is_empty());
    assert_eq!(failure.statistics.submitted_candidates, seeds.len());
    assert_eq!(
        failure.statistics.source,
        zetesis_cpu::lazy::Progress::default()
    );
}

#[test]
fn cancelled_submission_retains_no_execution_work() {
    let (program, seeds) = fixture(&[1, 2]);
    let control = Control::default();
    control.cancel();
    let failure = incomplete(pool(seeds.len()).check_shared(
        &program,
        &seeds,
        limits(),
        SourceSelection::Union,
        &control,
    ));
    assert_eq!(failure.cause, shared::Cause::Source(Stop::Cancelled));
    assert!(failure.statistics.worlds.is_empty());
    assert_eq!(
        failure.statistics.source,
        zetesis_cpu::lazy::Progress::default()
    );
}

#[test]
fn expired_deadline_remains_an_incomplete_batch() {
    let (program, seeds) = fixture(&[1, 2]);
    let control = Control::with_deadline(Instant::now());
    let failure = incomplete(pool(seeds.len()).check_shared(
        &program,
        &seeds,
        limits(),
        SourceSelection::Worlds,
        &control,
    ));
    assert_eq!(failure.cause, shared::Cause::Source(Stop::Deadline));
}

#[test]
fn foreign_seed_identity_precedes_execution() {
    let (program, _) = fixture(&[1]);
    let (_, seeds) = fixture(&[1]);
    let failure = incomplete(pool(1).check_shared(
        &program,
        &seeds,
        limits(),
        SourceSelection::Union,
        &Control::default(),
    ));
    assert_eq!(failure.cause, shared::Cause::Source(Stop::WrongProgram));
    assert!(failure.statistics.worlds.is_empty());
}

#[test]
fn pool_capacity_refuses_the_whole_submission() {
    let (program, seeds) = fixture(&[1, 2]);
    let result = pool(1).check_shared(
        &program,
        &seeds,
        limits(),
        SourceSelection::Union,
        &Control::default(),
    );
    assert!(matches!(
        result,
        Err(shared::Error::Admission(BatchError::Capacity {
            limit: 1,
            actual: 2
        }))
    ));
}

#[test]
fn shared_admission_exposes_the_original_capacity_error() {
    use std::error::Error as _;
    let (program, seeds) = fixture(&[1, 2]);
    let error = pool(1)
        .check_shared(
            &program,
            &seeds,
            limits(),
            SourceSelection::Union,
            &Control::default(),
        )
        .unwrap_err();
    let shared::Error::Admission(original) = &error else {
        panic!("capacity must refuse admission, not interrupt a submitted batch")
    };
    assert!(matches!(
        original,
        BatchError::Capacity {
            limit: 1,
            actual: 2
        }
    ));
    let exposed = error
        .source()
        .unwrap()
        .downcast_ref::<BatchError>()
        .unwrap();
    assert!(std::ptr::eq(exposed, original));
    assert!(exposed.source().is_none());
    assert_eq!(error.to_string(), "batch of 2 exceeds capacity 1");
}

fn incomplete_cause(error: &shared::Error) -> &shared::Failure {
    use std::error::Error as _;
    let shared::Error::Incomplete(original) = error else {
        panic!("expected a submitted batch with retained failure evidence")
    };
    let failure = error
        .source()
        .unwrap()
        .downcast_ref::<shared::Failure>()
        .unwrap();
    assert!(std::ptr::eq(failure, original));
    let cause = failure
        .source()
        .unwrap()
        .downcast_ref::<shared::Cause>()
        .unwrap();
    assert!(std::ptr::eq(cause, &failure.cause));
    let stop = match &failure.cause {
        shared::Cause::Source(stop) | shared::Cause::World { stop, .. } => stop,
        shared::Cause::InvalidOutput => panic!("expected an actual source or world stop"),
    };
    let exposed = cause.source().unwrap().downcast_ref::<Stop>().unwrap();
    assert!(std::ptr::eq(exposed, stop));
    assert!(exposed.source().is_none());
    failure
}

#[test]
fn shared_cancellation_exposes_its_source_cause() {
    let (program, seeds) = fixture(&[1, 2]);
    let control = Control::default();
    control.cancel();
    let error = pool(seeds.len())
        .check_shared(&program, &seeds, limits(), SourceSelection::Union, &control)
        .unwrap_err();
    let failure = incomplete_cause(&error);
    assert_eq!(failure.cause, shared::Cause::Source(Stop::Cancelled));
    assert_eq!(failure.statistics.submitted_candidates, seeds.len());
    assert!(failure.statistics.worlds.is_empty());
    assert_eq!(
        failure.statistics.source,
        zetesis_cpu::lazy::Progress::default()
    );
    assert_eq!(error.to_string(), "shared source: operation cancelled");
}

#[test]
fn shared_work_refusal_exposes_its_candidate_occurrence() {
    let (program, seeds) = fixture(&[1, 2]);
    let mut request = limits();
    request.max_world_work = 0;
    let error = pool(seeds.len())
        .check_shared(
            &program,
            &seeds,
            request,
            SourceSelection::Worlds,
            &Control::default(),
        )
        .unwrap_err();
    let failure = incomplete_cause(&error);
    assert_eq!(
        failure.cause,
        shared::Cause::World {
            index: 0,
            stop: Stop::WorkLimit
        }
    );
    assert_eq!(failure.statistics.submitted_candidates, seeds.len());
    assert!(failure.statistics.source.source_work > 0);
    assert_eq!(failure.statistics.worlds.len(), seeds.len());
    for world in &failure.statistics.worlds {
        assert_eq!(world.work, 0);
        assert_eq!(world.instances, 0);
        assert_eq!(world.interruption, Some(Stop::WorkLimit));
    }
    assert_eq!(
        error.to_string(),
        "candidate occurrence 0: oracle work limit reached"
    );
}

#[test]
fn empty_batches_have_no_source_or_world_work() {
    let (program, _) = fixture(&[]);
    let result = pool(1)
        .check_shared(
            &program,
            &[],
            limits(),
            SourceSelection::Union,
            &Control::default(),
        )
        .unwrap();
    assert!(result.checks.is_empty());
    assert!(result.statistics.worlds.is_empty());
    assert_eq!(
        result.statistics.source,
        zetesis_cpu::lazy::Progress::default()
    );
}

#[test]
fn source_candidate_cap_precedes_progress_allocation() {
    let (program, seeds) = fixture(&[1, 2]);
    let mut limited = limits();
    limited.source.max_candidates = 1;
    limited.source.max_host_bytes = 0;
    let error = pool(2)
        .check_shared(
            &program,
            &seeds,
            limited,
            SourceSelection::Union,
            &Control::default(),
        )
        .unwrap_err();
    let shared::Error::Incomplete(failure) = error else {
        panic!("source refusal expected");
    };
    assert_eq!(failure.cause, shared::Cause::Source(Stop::CarrierLimit));
    assert_eq!(failure.statistics.submitted_candidates, 2);
    assert!(failure.statistics.worlds.is_empty());
    assert_eq!(
        failure.statistics.source,
        zetesis_cpu::lazy::Progress::default()
    );
}
