//! Capacity observations are portable shape evidence, not device work.

use crate::{GpuErrorKind, GpuLimits};

use super::{LazyGpuStatistics, Plan, Transition, tests::inspect};

#[test]
fn growth_reasons_identify_undersized_inputs() {
    inspect(|chunk| {
        let plan = Plan::new(chunk, GpuLimits::default(), &wgpu::Limits::default()).unwrap();
        let expected = [
            super::LazyTransportReplacements {
                offsets_growth: 1,
                ..Default::default()
            },
            super::LazyTransportReplacements {
                records_growth: 1,
                ..Default::default()
            },
            super::LazyTransportReplacements {
                snapshots_growth: 1,
                ..Default::default()
            },
            super::LazyTransportReplacements {
                seeds_growth: 1,
                ..Default::default()
            },
        ];
        for (index, reason) in expected.into_iter().enumerate() {
            let mut retained = plan.capacity;
            retained.inputs[index] -= 4;
            let observed = LazyGpuStatistics::default()
                .submitted(&plan, retained.assess(&plan, u64::MAX), plan.capacity)
                .unwrap();
            assert_eq!(observed.transport_replacements, reason);
            assert_eq!(observed.transport_allocations, 1);
        }
    });
}

#[test]
fn replacement_reasons_preserve_simultaneous_causes() {
    inspect(|chunk| {
        let plan = Plan::new(chunk, GpuLimits::default(), &wgpu::Limits::default()).unwrap();
        let mut retained = plan.capacity;
        retained.inputs[0] -= 4;
        retained.inputs[1] += 128;
        retained.result += 4;
        let maximum = plan.capacity.accounted(&plan).unwrap();
        let observed = LazyGpuStatistics::default()
            .submitted(&plan, retained.assess(&plan, maximum), plan.capacity)
            .unwrap();
        assert_eq!(
            observed.transport_replacements,
            super::LazyTransportReplacements {
                offsets_growth: 1,
                result_shape: 1,
                budget: 1,
                ..Default::default()
            }
        );
        // Three causes still describe one submitted allocation, not three.
        assert_eq!(observed.transport_allocations, 1);
    });
}

#[test]
fn retention_overflow_is_distinct_from_a_byte_limit() {
    inspect(|chunk| {
        let plan = Plan::new(chunk, GpuLimits::default(), &wgpu::Limits::default()).unwrap();
        let mut retained = plan.capacity;
        retained.inputs[1] = u64::MAX;
        let observed = LazyGpuStatistics::default()
            .submitted(&plan, retained.assess(&plan, u64::MAX), plan.capacity)
            .unwrap();
        assert_eq!(
            observed.transport_replacements,
            super::LazyTransportReplacements {
                accounting_overflow: 1,
                ..Default::default()
            }
        );
    });
}

#[test]
fn reuse_retains_replacement_observations() {
    inspect(|chunk| {
        let plan = Plan::new(chunk, GpuLimits::default(), &wgpu::Limits::default()).unwrap();
        let first = LazyGpuStatistics::default()
            .submitted(&plan, Transition::Initial, plan.capacity)
            .unwrap();
        assert_eq!(
            first.transport_replacements,
            super::LazyTransportReplacements {
                initial: 1,
                ..Default::default()
            }
        );
        let reused = first
            .submitted(&plan, Transition::Reuse, plan.capacity)
            .unwrap();
        assert_eq!(reused.transport_replacements, first.transport_replacements);
    });
}

#[test]
fn replacement_counter_overflow_preserves_statistics() {
    inspect(|chunk| {
        let plan = Plan::new(chunk, GpuLimits::default(), &wgpu::Limits::default()).unwrap();
        let statistics = LazyGpuStatistics {
            dispatches: 7,
            transport_replacements: super::LazyTransportReplacements {
                initial: u64::MAX,
                ..Default::default()
            },
            ..Default::default()
        };
        let before = statistics;
        assert_eq!(
            statistics
                .submitted(&plan, Transition::Initial, plan.capacity)
                .unwrap_err()
                .kind(),
            GpuErrorKind::Capacity
        );
        assert_eq!(statistics, before);
    });
}

#[test]
fn replacement_sums_refuse_each_field_overflow() {
    use super::LazyTransportReplacements;
    let fields: [fn(&mut LazyTransportReplacements) -> &mut u64; 8] = [
        |value| &mut value.initial,
        |value| &mut value.offsets_growth,
        |value| &mut value.records_growth,
        |value| &mut value.snapshots_growth,
        |value| &mut value.seeds_growth,
        |value| &mut value.result_shape,
        |value| &mut value.budget,
        |value| &mut value.accounting_overflow,
    ];
    for field in fields {
        let mut left = LazyTransportReplacements::default();
        let mut right = LazyTransportReplacements::default();
        *field(&mut left) = u64::MAX;
        *field(&mut right) = 1;
        assert!(left.checked_add(right).is_none());
        assert_eq!(
            left.checked_add(LazyTransportReplacements::default()),
            Some(left)
        );
    }
}

fn join_program(width: i32) -> zetesis_core::Program {
    use zetesis_core::{AdmissionLimits, AtomPattern, Predicate, Program, Template, Term, Value};
    let pattern = |name, terms: Vec<Term>| {
        AtomPattern::new(Predicate::new(name, terms.len()).unwrap(), terms).unwrap()
    };
    let selected = |value| pattern("pick", vec![Term::Constant(Value::Number(value))]);
    let mut templates = Vec::new();
    for value in 0..width {
        for name in ["pick", "a", "b", "c"] {
            templates.push(Template::new(
                Some(pattern(name, vec![Term::Constant(Value::Number(value))])),
                vec![],
                vec![selected(value)],
                vec![],
                vec![],
            ));
        }
    }
    templates.push(Template::new(
        Some(pattern("triple", (0..3).map(Term::Variable).collect())),
        ["a", "b", "c"]
            .into_iter()
            .enumerate()
            .map(|(slot, name)| pattern(name, vec![Term::Variable(slot)]))
            .collect(),
        vec![],
        vec![],
        vec![],
    ));
    templates.push(Template::new(
        None,
        vec![pattern("triple", vec![Term::Constant(Value::Number(0)); 3])],
        vec![],
        vec![],
        vec![],
    ));
    Program::new(templates, AdmissionLimits::default()).unwrap()
}

fn join_seeds(
    program: &zetesis_core::Program,
    width: i32,
    worlds: i32,
    dense: bool,
) -> Vec<zetesis_core::Seed> {
    use zetesis_core::{Atom, Predicate, Seed, Value};
    (0..worlds)
        .map(|world| {
            Seed::new(
                program,
                (0..width)
                    .filter(|value| dense || *value == world % width)
                    .map(|value| {
                        Atom::new(
                            Predicate::new("pick", 1).unwrap(),
                            vec![Value::Number(value)],
                        )
                        .unwrap()
                    }),
            )
            .unwrap()
        })
        .collect()
}

#[test]
fn three_way_joins_expose_retention_causes() {
    use super::Capacity;
    use zetesis_cpu::{Control, lazy};
    let mut causes = super::LazyTransportReplacements::default();
    let mut submissions = 0;
    let mut allocations = 0;
    for dense in [false, true] {
        for width in [4, 8] {
            let program = join_program(width);
            for worlds in [1, 32, 128] {
                let seeds = join_seeds(&program, width, worlds, dense);
                for selection in [lazy::SourceSelection::Union, lazy::SourceSelection::Worlds] {
                    let mut retained: Option<Capacity> = None;
                    let mut statistics = LazyGpuStatistics::default();
                    let batch = lazy::check_with_source(
                        &program,
                        &seeds,
                        lazy::Limits {
                            max_chunk_rules: 256,
                            max_source_work: 100_000_000,
                            ..Default::default()
                        },
                        selection,
                        &Control::default(),
                        |chunk| {
                            let plan =
                                Plan::new(chunk, GpuLimits::default(), &wgpu::Limits::default())
                                    .unwrap();
                            let transition = retained.map_or(Transition::Initial, |capacity| {
                                capacity.assess(&plan, u64::MAX)
                            });
                            if !transition.is_reuse() {
                                retained = Some(plan.capacity);
                            }
                            statistics = statistics
                                .submitted(&plan, transition, retained.unwrap())
                                .unwrap();
                            lazy::evaluate(chunk)
                        },
                    )
                    .unwrap();
                    assert_eq!(batch.progress.chunks, statistics.dispatches);
                    causes = causes
                        .checked_add(statistics.transport_replacements)
                        .unwrap();
                    submissions += statistics.dispatches;
                    allocations += statistics.transport_allocations;
                    assert_eq!(statistics.transport_replacements.initial, 1);
                    assert_eq!(statistics.transport_replacements.budget, 0);
                    assert_eq!(statistics.transport_replacements.accounting_overflow, 0);
                    eprintln!(
                        "join shape trace dense={dense} width={width} worlds={worlds} selection={selection:?} statistics={statistics:?}"
                    );
                }
            }
        }
    }
    // The complete fixed family matches the old ABBA structural work: eight
    // seven-dispatch schedules and sixteen three-dispatch schedules. Causes
    // overlap, so 48+64+36+36+36 does not count allocations.
    assert_eq!(submissions, 104);
    assert_eq!(allocations, 96);
    assert_eq!(
        causes,
        super::LazyTransportReplacements {
            initial: 24,
            offsets_growth: 48,
            records_growth: 64,
            snapshots_growth: 36,
            seeds_growth: 36,
            result_shape: 36,
            ..Default::default()
        }
    );
}
