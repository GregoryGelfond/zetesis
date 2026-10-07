//! Finite instrumented schedules retain configuration positions before execution.
use std::num::NonZeroUsize;
use zetesis_validation::performance::{
    Phase,
    matrix::{Plan, Producer, ReferencePolicy, Suite},
};
use zetesis_validation::selected::{Backend, Grounder, NativeExecution};

fn plan(repetitions: usize) -> Plan {
    let profiles = [
        (Backend::Cpu, Grounder::Eager),
        (Backend::Cpu, Grounder::Lazy),
        (
            Backend::Gpu(Some(zetesis_backend::GpuApi::Metal)),
            Grounder::Eager,
        ),
        (
            Backend::Gpu(Some(zetesis_backend::GpuApi::Metal)),
            Grounder::Lazy,
        ),
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
    let slots = plan(20)
        .slots(94, Some(ReferencePolicy::AllPhases))
        .unwrap();
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
    let slots = plan(2)
        .with_memory(3)
        .unwrap()
        .slots(4, Some(ReferencePolicy::AllPhases))
        .unwrap();
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
    let slots = plan(1).slots(6, Some(ReferencePolicy::AllPhases)).unwrap();
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
fn qualification_only_omits_reference_measurements() {
    let plan = plan(4).with_memory(2).unwrap();
    let slots = plan
        .slots(2, Some(ReferencePolicy::QualificationOnly))
        .unwrap();
    let reference: Vec<_> = slots
        .iter()
        .filter(|slot| slot.producer == Producer::Reference)
        .collect();
    assert_eq!(reference.len(), 2);
    assert!(
        reference
            .iter()
            .all(|slot| slot.phase == Phase::Qualification)
    );
    assert_eq!(slots.len(), 2 * (5 + 4 * (3 + 4 + 2)));
    for case in 0..2 {
        for profile in 0..4 {
            assert_eq!(
                slots
                    .iter()
                    .filter(
                        |slot| slot.case == case && slot.producer == Producer::Native { profile }
                    )
                    .count(),
                10
            );
        }
    }
}

#[test]
fn qualification_only_balances_native_positions() {
    let slots = plan(8)
        .slots(3, Some(ReferencePolicy::QualificationOnly))
        .unwrap();
    for case in 0..3 {
        let timed: Vec<_> = slots
            .iter()
            .filter(|slot| slot.case == case && slot.phase == Phase::Timed)
            .collect();
        for position in 0..4 {
            for profile in 0..4 {
                assert_eq!(
                    timed
                        .chunks_exact(4)
                        .filter(|round| round[position].producer == Producer::Native { profile })
                        .count(),
                    2
                );
            }
        }
    }
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
        threads: NonZeroUsize::new(257).unwrap(),
        ..Default::default()
    };
    assert!(Plan::new(Suite::Queens, vec![too_many_workers], workers, 0, 1).is_err());
    assert!(plan(1).slots(0, None).is_err());
    assert!(plan(1).slots(95, None).is_err());
}

#[test]
fn a_selection_refuses_empty_duplicate_and_escaping_paths() {
    for paths in [
        vec![],
        vec!["a.lp".to_owned(), "a.lp".to_owned()],
        vec!["../a.lp".to_owned()],
        vec![String::new()],
    ] {
        assert!(plan(1).with_cases(paths).is_err());
    }
    let selected = plan(1).with_cases(vec!["a.lp".to_owned()]).unwrap();
    assert_eq!(selected.selection(), Some(&["a.lp".to_owned()][..]));
}

#[test]
fn qualification_plans_schedule_one_census_without_measurements() {
    let profiles = zetesis_validation::performance::scalability::profiles(None);
    let plan = Plan::qualification(Suite::Scalability, profiles, NonZeroUsize::MIN).unwrap();
    let slots = plan
        .slots(10, Some(ReferencePolicy::QualificationOnly))
        .unwrap();
    assert_eq!(slots.len(), 60);
    for (case, census) in slots.chunks_exact(6).enumerate() {
        assert!(census.iter().all(|slot| slot.case == case
            && slot.phase == Phase::Qualification
            && slot.round == 0));
        assert_eq!(census[0].producer, Producer::Reference);
        for (profile, slot) in census[1..].iter().enumerate() {
            assert_eq!(slot.producer, Producer::Native { profile });
        }
    }
    let encoded = serde_json::to_value(&plan).unwrap();
    assert_eq!(encoded["warmups"], 0);
    assert_eq!(encoded["repetitions"], 0);
    assert_eq!(encoded["memory_runs"], 0);
    assert!(plan.clone().with_memory(0).is_ok());
    assert!(plan.with_memory(1).is_err());
}

#[test]
fn qualification_and_measurement_share_profile_bounds() {
    let one = NonZeroUsize::MIN;
    let excessive = NonZeroUsize::new(257).unwrap();
    for (profiles, reference) in [
        (vec![], one),
        (vec![NativeExecution::default(); 9], one),
        (
            vec![NativeExecution {
                threads: excessive,
                ..NativeExecution::default()
            }],
            one,
        ),
        (vec![NativeExecution::default()], excessive),
    ] {
        assert!(Plan::qualification(Suite::Scalability, profiles.clone(), reference).is_err());
        assert!(Plan::new(Suite::Scalability, profiles, reference, 0, 1).is_err());
    }
}
