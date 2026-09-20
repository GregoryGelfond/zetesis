//! Finite instrumented schedules retain configuration positions before execution.
use std::num::NonZeroUsize;
use zetesis_validation::performance::{
    Phase,
    matrix::{Plan, Producer, Suite},
};
use zetesis_validation::selected::{Backend, Grounder, NativeExecution};

fn plan(repetitions: usize) -> Plan {
    let profiles = [
        (Backend::Cpu, Grounder::Eager),
        (Backend::Cpu, Grounder::Lazy),
        (Backend::Metal, Grounder::Eager),
        (Backend::Metal, Grounder::Lazy),
    ]
    .map(|(backend, grounder)| NativeExecution {
        backend,
        grounder,
        ..Default::default()
    })
    .to_vec();
    Plan::new(
        Suite::Corpus,
        profiles,
        NonZeroUsize::new(1).unwrap(),
        3,
        repetitions,
    )
    .unwrap()
}
#[test]
fn every_profile_occupies_every_timed_position() {
    let slots = plan(20).slots(94).unwrap();
    for case in 0..94 {
        let timed: Vec<_> = slots
            .iter()
            .filter(|s| s.case == case && s.phase == Phase::Timed)
            .collect();
        for position in 0..5 {
            for producer in [
                Producer::Reference,
                Producer::Native { profile: 0 },
                Producer::Native { profile: 1 },
                Producer::Native { profile: 2 },
                Producer::Native { profile: 3 },
            ] {
                assert_eq!(
                    timed
                        .chunks_exact(5)
                        .filter(|round| round[position].producer == producer)
                        .count(),
                    4
                );
            }
        }
    }
}
#[test]
fn memory_rounds_follow_the_timed_rounds() {
    let slots = plan(2).with_memory(3).unwrap().slots(4).unwrap();
    let memory: Vec<_> = slots
        .iter()
        .enumerate()
        .filter(|(_, slot)| slot.phase == Phase::Memory)
        .collect();
    // Three rounds of four cases and five producers.
    assert_eq!(memory.len(), 3 * 4 * 5);
    let last_timed = slots
        .iter()
        .rposition(|slot| slot.phase == Phase::Timed)
        .unwrap();
    assert!(memory.iter().all(|(index, _)| *index > last_timed));
    for case in 0..4 {
        for profile in 0..4 {
            assert_eq!(
                memory
                    .iter()
                    .filter(|(_, slot)| slot.case == case
                        && slot.producer == Producer::Native { profile })
                    .count(),
                3
            );
        }
    }
    assert!(plan(2).with_memory(42).is_err());
    assert_eq!(plan(2).memory_runs(), 0);
}

#[test]
fn reference_census_precedes_native_census() {
    let slots = plan(1).slots(6).unwrap();
    assert!(
        slots[..30]
            .iter()
            .all(|slot| slot.phase == Phase::Qualification)
    );
    assert!(
        slots[..30]
            .chunks_exact(5)
            .all(|row| row[0].producer == Producer::Reference)
    );
    assert_eq!(slots.len(), 6 * 5 * 5);
}
#[test]
fn automatic_grounding_is_an_admitted_profile() {
    // The request records `auto`; the observation retains the mode taken, so
    // an automatic cell cannot be read as an explicit eager or lazy one.
    let profile = NativeExecution {
        grounder: Grounder::Auto,
        ..Default::default()
    };
    let plan = Plan::new(
        Suite::Corpus,
        vec![profile],
        NonZeroUsize::new(1).unwrap(),
        0,
        1,
    )
    .unwrap();
    assert_eq!(plan.profiles()[0].grounder, Grounder::Auto);
}
#[test]
fn matrix_population_has_finite_construction_bounds() {
    let workers = NonZeroUsize::new(1).unwrap();
    for profiles in [vec![], vec![NativeExecution::default(); 9]] {
        assert!(Plan::new(Suite::Baseline, profiles, workers, 0, 1).is_err());
    }
    let too_many_workers = NativeExecution {
        workers: NonZeroUsize::new(257).unwrap(),
        ..Default::default()
    };
    assert!(Plan::new(Suite::Queens, vec![too_many_workers], workers, 0, 1).is_err());
    assert!(plan(1).slots(0).is_err());
    assert!(plan(1).slots(95).is_err());
}
