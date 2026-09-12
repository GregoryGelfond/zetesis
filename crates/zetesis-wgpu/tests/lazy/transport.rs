//! Capacity contracts are portable; execution selects a physical API explicitly.

use std::num::NonZeroU32;

use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, Predicate, Program, Seed, Template, Term, Value,
};
use zetesis_cpu::{Control, Limits, Stop, check, lazy};

use crate::{GpuBackendPreference, GpuErrorKind, GpuLimits, GpuOptions, GpuSelection};

use super::{Capacity, GpuLazyOracle, LazyGpuStatistics, Plan, Selection, tests::inspect};

#[test]
fn retained_inputs_count_toward_the_transport_ceiling() {
    inspect(|chunk| {
        let plan = Plan::new(
            NonZeroU32::MIN,
            chunk,
            GpuLimits::default(),
            &wgpu::Limits::default(),
        )
        .unwrap();
        let mut retained = plan.capacity;
        retained.inputs[0] += 64;
        retained.inputs[1] += 128;
        let accounted = retained.accounted(&plan).unwrap();
        assert_eq!(accounted, plan.capacity.accounted(&plan).unwrap() + 192);
        assert!(retained.assess(&plan, accounted).is_reuse());
        assert!(!retained.assess(&plan, accounted - 1).is_reuse());
        // Replacement can use the exact active shape under the tighter limit.
        assert!(plan.capacity.assess(&plan, accounted - 1).is_reuse());
    });
}

#[test]
fn every_input_binding_must_fit_before_reuse() {
    inspect(|chunk| {
        let plan = Plan::new(
            NonZeroU32::MIN,
            chunk,
            GpuLimits::default(),
            &wgpu::Limits::default(),
        )
        .unwrap();
        for index in 0..4 {
            let mut retained = plan.capacity;
            retained.inputs[index] -= 4;
            assert!(!retained.assess(&plan, u64::MAX).is_reuse());
        }
    });
}

#[test]
fn result_shape_changes_require_replacement() {
    inspect(|chunk| {
        let plan = Plan::new(
            NonZeroU32::MIN,
            chunk,
            GpuLimits::default(),
            &wgpu::Limits::default(),
        )
        .unwrap();
        for result in [plan.result_bytes - 4, plan.result_bytes + 4] {
            assert!(
                !Capacity {
                    result,
                    ..plan.capacity
                }
                .assess(&plan, u64::MAX)
                .is_reuse()
            );
        }
    });
}

#[test]
fn inactive_input_capacity_does_not_inflate_uploads() {
    inspect(|chunk| {
        let plan = Plan::new(
            NonZeroU32::MIN,
            chunk,
            GpuLimits::default(),
            &wgpu::Limits::default(),
        )
        .unwrap();
        let mut retained = plan.capacity;
        retained.inputs[1] += 128;
        let reused = LazyGpuStatistics::default()
            .submitted(&plan, Selection::new(Some(retained), &plan, u64::MAX))
            .unwrap();
        assert_eq!(reused.dispatches, 1);
        assert_eq!(reused.transport_allocations, 0);
        assert_eq!(reused.transport_reuses, 1);
        assert_eq!(reused.uploaded_bytes, plan.uploaded_bytes);
        assert_eq!(reused.downloaded_bytes, 0);
        assert_eq!(reused.peak_transport_bytes, retained.bytes().unwrap());
    });
}

#[test]
fn retained_capacity_arithmetic_refuses_overflow() {
    inspect(|chunk| {
        let plan = Plan::new(
            NonZeroU32::MIN,
            chunk,
            GpuLimits::default(),
            &wgpu::Limits::default(),
        )
        .unwrap();
        for retained in [
            Capacity {
                inputs: [u64::MAX; 4],
                ..plan.capacity
            },
            Capacity {
                result: u64::MAX,
                ..plan.capacity
            },
            Capacity {
                // Retained GPU bytes alone fit exactly. The independent active
                // host packing/decode allowance must still overflow the sum.
                inputs: [
                    u64::MAX - super::UNIFORM_BYTES - 2 * plan.result_bytes,
                    0,
                    0,
                    0,
                ],
                ..plan.capacity
            },
        ] {
            assert!(retained.accounted(&plan).is_none());
            assert!(!retained.assess(&plan, u64::MAX).is_reuse());
        }
    });
}

#[test]
fn transport_observation_counters_refuse_overflow() {
    inspect(|chunk| {
        let plan = Plan::new(
            NonZeroU32::MIN,
            chunk,
            GpuLimits::default(),
            &wgpu::Limits::default(),
        )
        .unwrap();
        for (statistics, reused) in [
            (
                LazyGpuStatistics {
                    transport_allocations: u64::MAX,
                    ..Default::default()
                },
                false,
            ),
            (
                LazyGpuStatistics {
                    transport_reuses: u64::MAX,
                    ..Default::default()
                },
                true,
            ),
        ] {
            assert_eq!(
                statistics
                    .submitted(
                        &plan,
                        Selection::new(reused.then_some(plan.capacity), &plan, u64::MAX),
                    )
                    .unwrap_err()
                    .kind(),
                GpuErrorKind::Capacity
            );
        }
    });
}

fn oracle(backend: GpuBackendPreference) -> GpuLazyOracle {
    let oracle = GpuLazyOracle::new_selected(
        GpuOptions::default(),
        GpuSelection {
            backend,
            vendor_id: None,
        },
    )
    .expect("requested physical API is required for this transport qualification");
    let expected = match backend {
        GpuBackendPreference::Metal => "Metal",
        GpuBackendPreference::Vulkan => "Vulkan",
        _ => panic!("transport qualification requires an explicit Metal or Vulkan API"),
    };
    assert_eq!(oracle.info().backend(), expected);
    assert!(oracle.info().is_hardware_gpu());
    eprintln!("lazy transport adapter={:?}", oracle.info());
    oracle
}

fn pattern(name: &str, terms: Vec<Term>) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, terms.len()).unwrap(), terms).unwrap()
}

fn growth_program() -> Program {
    let mut templates: Vec<_> = (0..67)
        .map(|value| {
            Template::new(
                Some(pattern("p", vec![Term::Constant(Value::Number(value))])),
                vec![],
                vec![],
                vec![],
                vec![],
            )
        })
        .collect();
    let variable = || vec![Term::Variable(0)];
    templates.push(Template::new(
        Some(pattern("q", variable())),
        vec![pattern("p", variable())],
        vec![],
        vec![pattern("blocked", variable())],
        vec![],
    ));
    templates.push(Template::new(
        Some(pattern("r", variable())),
        vec![pattern("q", variable())],
        vec![],
        vec![],
        vec![],
    ));
    templates.push(Template::new(
        None,
        vec![
            pattern("q", vec![Term::Constant(Value::Number(0))]),
            pattern("q", vec![Term::Constant(Value::Number(1))]),
        ],
        vec![],
        vec![],
        vec![],
    ));
    Program::new(templates, AdmissionLimits::default()).unwrap()
}

fn growth_seeds(program: &Program) -> Vec<Seed> {
    (0..33)
        .map(|world| {
            let blocked = (world % 3 != 2).then(|| {
                Atom::new(
                    Predicate::new("blocked", 1).unwrap(),
                    vec![Value::Number(world % 3)],
                )
                .unwrap()
            });
            Seed::new(program, blocked).unwrap()
        })
        .collect()
}

fn growth_replacements() -> super::LazyTransportReplacements {
    super::LazyTransportReplacements {
        initial: 1,
        records_growth: 2,
        snapshots_growth: 4,
        seeds_growth: 4,
        result_shape: 4,
        ..Default::default()
    }
}

fn execute_inspected(
    oracle: &mut GpuLazyOracle,
    chunk: &lazy::Chunk<'_>,
    limits: GpuLimits,
    cached: &mut Option<super::Transport>,
) -> Result<Vec<u32>, crate::GpuError> {
    let plan = Plan::new(NonZeroU32::MIN, chunk, limits, oracle.runtime.limits())?;
    let selected = Selection::new(
        cached.as_ref().map(|value| value.capacity),
        &plan,
        limits.max_batch_bytes,
    );
    let mask = [
        selected.retention.uniform,
        selected.retention.inputs[0],
        selected.retention.inputs[1],
        selected.retention.inputs[2],
        selected.retention.inputs[3],
        selected.retention.result,
        selected.retention.result,
    ];
    // Clone only handles that the admitted selection will keep. Rejected
    // handles must have no test-owned reference extending their lifetime.
    let retained: [Option<wgpu::Buffer>; 7] = std::array::from_fn(|index| {
        mask[index].then(|| cached.as_ref().unwrap().buffers()[index].clone())
    });
    let before = oracle.statistics();
    let expected = lazy::evaluate(chunk).unwrap();
    let actual = oracle.execute(chunk, limits, &Control::default(), cached)?;
    assert_eq!(actual, expected);
    let transport = cached.as_ref().unwrap();
    assert!(transport.capacity.accounted(&plan).unwrap() <= limits.max_batch_bytes);
    let expected_sizes = [
        super::UNIFORM_BYTES,
        selected.capacity.inputs[0],
        selected.capacity.inputs[1],
        selected.capacity.inputs[2],
        selected.capacity.inputs[3],
        plan.result_bytes,
        plan.result_bytes,
    ];
    for (index, buffer) in transport.buffers().into_iter().enumerate() {
        assert_eq!(buffer.size(), expected_sizes[index]);
        if let Some(previous) = &retained[index] {
            assert_eq!(buffer, previous);
        }
    }
    let bindings = |usage: super::LazyTransportUsage| {
        [
            usage.uniform,
            usage.offsets,
            usage.records,
            usage.snapshots,
            usage.seeds,
            usage.output,
            usage.readback,
        ]
    };
    for (index, (old, new)) in bindings(before.transport_usage)
        .into_iter()
        .zip(bindings(oracle.statistics.transport_usage))
        .enumerate()
    {
        assert_eq!(new.allocations, old.allocations + u64::from(!mask[index]));
        assert_eq!(new.reuses, old.reuses + u64::from(mask[index]));
        assert_eq!(new.allocations + new.reuses, oracle.statistics.dispatches);
    }
    assert_eq!(
        oracle.statistics.transport_usage.output,
        oracle.statistics.transport_usage.readback
    );
    Ok(actual)
}

#[test]
fn source_sequence_exercises_transport_resize_boundaries() {
    let program = growth_program();
    let seeds = growth_seeds(&program);
    for selection in [lazy::SourceSelection::Union, lazy::SourceSelection::Worlds] {
        let mut retained: Option<Capacity> = None;
        let mut previous: Option<Capacity> = None;
        let mut statistics = LazyGpuStatistics::default();
        let mut smaller_reused = false;
        let mut later_growth = false;
        lazy::check_with_source(
            &program,
            &seeds,
            lazy::Limits {
                max_chunk_rules: 8,
                ..Default::default()
            },
            selection,
            &Control::default(),
            |chunk| {
                let plan = Plan::new(
                    NonZeroU32::MIN,
                    chunk,
                    GpuLimits::default(),
                    &wgpu::Limits::default(),
                )
                .unwrap();
                let selected = Selection::new(retained, &plan, u64::MAX);
                let reused = selected.transition.is_reuse();
                if let Some(previous) = previous {
                    smaller_reused |= reused && plan.capacity.inputs[0] < previous.inputs[0];
                    later_growth |= smaller_reused && plan.result_bytes > previous.result;
                }
                previous = Some(plan.capacity);
                retained = Some(selected.capacity);
                statistics = statistics.submitted(&plan, selected).unwrap();
                lazy::evaluate(chunk)
            },
        )
        .unwrap();
        // This validates only the deterministic source sequence used below.
        // No buffer allocation or device execution is claimed by this control.
        assert!(smaller_reused);
        assert!(later_growth);
        assert_eq!(statistics.transport_replacements, growth_replacements());
        eprintln!("portable shape trace selection={selection:?} statistics={statistics:?}");
    }
}

#[test]
#[ignore = "requires a physical Metal adapter"]
fn metal_lazy_transport_reuse_preserves_round_truth() {
    qualify_lazy_transport_reuse_preserves_round_truth(GpuBackendPreference::Metal);
}

#[test]
#[ignore = "requires a physical Vulkan adapter"]
fn vulkan_lazy_transport_reuse_preserves_round_truth() {
    qualify_lazy_transport_reuse_preserves_round_truth(GpuBackendPreference::Vulkan);
}

fn qualify_lazy_transport_reuse_preserves_round_truth(backend: GpuBackendPreference) {
    let program = growth_program();
    let seeds = growth_seeds(&program);
    let mut oracle = oracle(backend);
    for selection in [lazy::SourceSelection::Union, lazy::SourceSelection::Worlds] {
        oracle.statistics = LazyGpuStatistics::default();
        let mut cached = None;
        let mut previous: Option<Capacity> = None;
        let mut smaller_reused = false;
        let mut later_growth = false;
        let batch = lazy::check_with_source(
            &program,
            &seeds,
            lazy::Limits {
                max_chunk_rules: 8,
                ..Default::default()
            },
            selection,
            &Control::default(),
            |chunk| {
                let plan = Plan::new(
                    NonZeroU32::MIN,
                    chunk,
                    GpuLimits::default(),
                    oracle.runtime.limits(),
                )
                .unwrap();
                let reuses = oracle.statistics.transport_reuses;
                let result =
                    execute_inspected(&mut oracle, chunk, GpuLimits::default(), &mut cached)?;
                if let Some(previous) = previous {
                    smaller_reused |= plan.capacity.inputs[0] < previous.inputs[0]
                        && oracle.statistics.transport_reuses > reuses;
                    later_growth |= smaller_reused && plan.result_bytes > previous.result;
                }
                previous = Some(plan.capacity);
                Ok::<_, crate::GpuError>(result)
            },
        )
        .unwrap();
        assert!(
            smaller_reused,
            "must reuse a larger allocation for a smaller chunk"
        );
        assert!(
            later_growth,
            "must subsequently replace the transport on catalog growth"
        );
        assert_eq!(batch.checks.len(), seeds.len());
        for (actual, seed) in batch.checks.iter().zip(&seeds) {
            let expected = check(&program, seed, Limits::default(), &Control::default()).unwrap();
            assert_eq!(actual.closure(), expected.closure());
            assert_eq!(actual.constraint_violated(), expected.constraint_violated());
            assert_eq!(actual.seed_mismatch(), expected.seed_mismatch());
            assert_eq!(actual.accepted(), expected.accepted());
        }
        assert_eq!(
            oracle.statistics.transport_replacements,
            growth_replacements()
        );
        assert!(oracle.statistics.transport_reuses > oracle.statistics.transport_allocations);
        assert_eq!(
            oracle.statistics.dispatches,
            oracle.statistics.transport_reuses + oracle.statistics.transport_allocations
        );
        eprintln!(
            "selection={selection:?} transport={:?}",
            oracle.statistics()
        );
        // The public boundary owns and releases its own cache on each exit.
        drop(cached);
        let again = oracle
            .check_batch_with_source(
                &program,
                &seeds,
                lazy::Limits {
                    max_chunk_rules: 8,
                    ..Default::default()
                },
                GpuLimits::default(),
                selection,
                &Control::default(),
            )
            .unwrap();
        assert_eq!(again.progress.chunks, oracle.statistics.dispatches);
        assert!(oracle.statistics.transport_allocations > 0);
    }
}

#[test]
#[ignore = "requires a physical Metal adapter"]
fn metal_input_slack_preserves_exact_admission() {
    qualify_input_slack_preserves_exact_admission(GpuBackendPreference::Metal);
}

#[test]
#[ignore = "requires a physical Vulkan adapter"]
fn vulkan_input_slack_preserves_exact_admission() {
    qualify_input_slack_preserves_exact_admission(GpuBackendPreference::Vulkan);
}

fn qualify_input_slack_preserves_exact_admission(backend: GpuBackendPreference) {
    let program = growth_program();
    let seeds = growth_seeds(&program);
    let mut oracle = oracle(backend);
    for selection in [lazy::SourceSelection::Union, lazy::SourceSelection::Worlds] {
        oracle.statistics = LazyGpuStatistics::default();
        let mut cached = None;
        let batch = lazy::check_with_source(
            &program,
            &seeds,
            lazy::Limits {
                max_chunk_rules: 8,
                ..Default::default()
            },
            selection,
            &Control::default(),
            |chunk| {
                let plan = Plan::new(
                    NonZeroU32::MIN,
                    chunk,
                    GpuLimits::default(),
                    oracle.runtime.limits(),
                )
                .unwrap();
                let maximum = plan.capacity.accounted(&plan).unwrap();
                execute_inspected(
                    &mut oracle,
                    chunk,
                    GpuLimits {
                        max_batch_bytes: maximum,
                        ..GpuLimits::default()
                    },
                    &mut cached,
                )
            },
        )
        .unwrap();
        assert_eq!(batch.checks.len(), seeds.len());
        assert!(oracle.statistics.transport_usage.budget_releases > 0);
        assert_eq!(oracle.statistics.transport_usage.uniform.allocations, 1);
        assert_eq!(oracle.statistics.dispatches, batch.progress.chunks);
        assert_eq!(
            oracle
                .statistics
                .transport_usage
                .accounting_overflow_releases,
            0
        );
    }
}

#[test]
#[ignore = "requires a physical Metal adapter"]
fn metal_lazy_transport_refusal_preserves_reuse() {
    qualify_lazy_transport_refusal_preserves_reuse(GpuBackendPreference::Metal);
}

#[test]
#[ignore = "requires a physical Vulkan adapter"]
fn vulkan_lazy_transport_refusal_preserves_reuse() {
    qualify_lazy_transport_refusal_preserves_reuse(GpuBackendPreference::Vulkan);
}

fn qualify_lazy_transport_refusal_preserves_reuse(backend: GpuBackendPreference) {
    let mut oracle = oracle(backend);
    let mut ran = false;
    inspect(|chunk| {
        if ran {
            return;
        }
        ran = true;
        let mut cached = None;
        let plan = Plan::new(
            NonZeroU32::MIN,
            chunk,
            GpuLimits::default(),
            oracle.runtime.limits(),
        )
        .unwrap();
        let exact = plan.capacity.accounted(&plan).unwrap();
        let expected = lazy::evaluate(chunk).unwrap();
        assert_eq!(
            oracle
                .execute(
                    chunk,
                    GpuLimits::default(),
                    &Control::default(),
                    &mut cached
                )
                .unwrap(),
            expected
        );
        let before = oracle.statistics();
        assert_eq!(
            oracle
                .execute(
                    chunk,
                    GpuLimits {
                        max_batch_bytes: exact - 1,
                        ..Default::default()
                    },
                    &Control::default(),
                    &mut cached,
                )
                .unwrap_err()
                .kind(),
            GpuErrorKind::Capacity
        );
        assert_eq!(oracle.statistics(), before);
        assert!(cached.is_some());
        assert_eq!(
            oracle
                .execute(
                    chunk,
                    GpuLimits {
                        max_batch_bytes: exact,
                        ..Default::default()
                    },
                    &Control::default(),
                    &mut cached,
                )
                .unwrap(),
            expected
        );
        assert_eq!(oracle.statistics.transport_allocations, 1);
        assert_eq!(oracle.statistics.transport_reuses, 1);
    });
    assert!(ran);
}

#[test]
#[ignore = "requires a physical Metal adapter"]
fn metal_lazy_transport_cancelled_read_discards_capacity() {
    qualify_lazy_transport_cancelled_read_discards_capacity(GpuBackendPreference::Metal);
}

#[test]
#[ignore = "requires a physical Vulkan adapter"]
fn vulkan_lazy_transport_cancelled_read_discards_capacity() {
    qualify_lazy_transport_cancelled_read_discards_capacity(GpuBackendPreference::Vulkan);
}

fn qualify_lazy_transport_cancelled_read_discards_capacity(backend: GpuBackendPreference) {
    let mut oracle = oracle(backend);
    let mut ran = false;
    inspect(|chunk| {
        if ran {
            return;
        }
        ran = true;
        let mut cached = None;
        oracle
            .execute(
                chunk,
                GpuLimits::default(),
                &Control::default(),
                &mut cached,
            )
            .unwrap();
        let completed = oracle.statistics();
        // Inject cancellation at the device-read boundary. The public source
        // coordinator normally polls earlier, but cancellation may race submit.
        let cancelled = Control::default();
        cancelled.cancel();
        let error = oracle
            .execute(chunk, GpuLimits::default(), &cancelled, &mut cached)
            .unwrap_err();
        assert_eq!(error.interruption, Some(Stop::Cancelled));
        assert!(cached.is_none());
        assert_eq!(oracle.statistics.dispatches, completed.dispatches + 1);
        assert_eq!(oracle.statistics.transport_reuses, 1);
        assert_eq!(
            oracle.statistics.downloaded_bytes,
            completed.downloaded_bytes
        );
        assert_eq!(
            oracle
                .execute(
                    chunk,
                    GpuLimits::default(),
                    &Control::default(),
                    &mut cached
                )
                .unwrap_err()
                .kind(),
            GpuErrorKind::Device
        );
    });
    assert!(ran);
    qualify_cancelled_replacement(backend);
}

fn qualify_cancelled_replacement(backend: GpuBackendPreference) {
    let program = growth_program();
    let seeds = growth_seeds(&program);
    let mut oracle = oracle(backend);
    let mut cached = None;
    let mut cancelled_replacement = false;
    let failure = lazy::check_with_source(
        &program,
        &seeds,
        lazy::Limits {
            max_chunk_rules: 8,
            ..Default::default()
        },
        lazy::SourceSelection::Worlds,
        &Control::default(),
        |chunk| {
            let plan = Plan::new(
                NonZeroU32::MIN,
                chunk,
                GpuLimits::default(),
                oracle.runtime.limits(),
            )
            .unwrap();
            let selected = Selection::new(
                cached
                    .as_ref()
                    .map(|value: &super::Transport| value.capacity),
                &plan,
                u64::MAX,
            );
            if !selected.retention.uniform || selected.transition.is_reuse() {
                return execute_inspected(&mut oracle, chunk, GpuLimits::default(), &mut cached);
            }
            cancelled_replacement = true;
            let previous = oracle.statistics();
            let cancelled = Control::default();
            cancelled.cancel();
            let error = oracle
                .execute(chunk, GpuLimits::default(), &cancelled, &mut cached)
                .unwrap_err();
            assert_eq!(error.interruption, Some(Stop::Cancelled));
            assert!(cached.is_none());
            assert_eq!(oracle.statistics.dispatches, previous.dispatches + 1);
            assert_eq!(
                oracle.statistics.transport_allocations,
                previous.transport_allocations + 1
            );
            assert_eq!(
                oracle.statistics.transport_usage,
                previous.transport_usage.record(selected).unwrap()
            );
            assert_eq!(
                oracle.statistics.downloaded_bytes,
                previous.downloaded_bytes
            );
            assert_eq!(
                oracle
                    .execute(
                        chunk,
                        GpuLimits::default(),
                        &Control::default(),
                        &mut cached
                    )
                    .unwrap_err()
                    .kind(),
                GpuErrorKind::Device
            );
            Err(error)
        },
    )
    .unwrap_err();
    assert!(cancelled_replacement);
    assert!(matches!(failure.cause, lazy::Cause::Execution(_)));
}

fn qualify_immutable_uploads(backend: GpuBackendPreference) {
    let mut oracle = oracle(backend);
    let fact = |name: &str| {
        Template::new(
            Some(AtomPattern::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap()),
            vec![],
            vec![],
            vec![],
            vec![],
        )
    };
    let program = Program::new(vec![fact("a"), fact("b")], AdmissionLimits::default()).unwrap();
    let seeds = [Seed::new(&program, []).unwrap()];
    for selection in [lazy::SourceSelection::Union, lazy::SourceSelection::Worlds] {
        // A fresh public batch must upload again, even on the same oracle and
        // with identical layout/round numbers. Four chunks span two rounds.
        let mut cached = None;
        oracle.statistics = LazyGpuStatistics::default();
        let mut sizes = Vec::new();
        let batch = lazy::check_with_source(
            &program,
            &seeds,
            lazy::Limits {
                max_chunk_rules: 1,
                ..Default::default()
            },
            selection,
            &Control::default(),
            |chunk| {
                let before = oracle.statistics.uploaded_bytes;
                let actual = oracle.execute(
                    chunk,
                    GpuLimits::default(),
                    &Control::default(),
                    &mut cached,
                )?;
                assert_eq!(actual, lazy::evaluate(chunk).unwrap());
                sizes.push(oracle.statistics.uploaded_bytes - before);
                Ok::<_, crate::GpuError>(actual)
            },
        )
        .unwrap();
        assert_eq!(sizes, [44, 36, 40, 36]);
        assert_eq!(oracle.statistics.uploaded_bytes, 156);
        assert_eq!(oracle.statistics.dispatches, 4);
        assert_eq!(batch.progress.rounds, 2);
        assert_eq!(
            batch.checks[0]
                .closure()
                .atoms()
                .iter()
                .map(|atom| atom.predicate().name())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        // Actual public entry owns and resets its transport; it has a default
        // larger chunk, so two rounds upload both facts together: 64 then60.
        let public = oracle
            .check_batch_with_source(
                &program,
                &seeds,
                lazy::Limits::default(),
                GpuLimits::default(),
                selection,
                &Control::default(),
            )
            .unwrap();
        assert_eq!(public.checks[0].closure(), batch.checks[0].closure());
        assert_eq!(oracle.statistics.uploaded_bytes, 124);
        assert_eq!(oracle.statistics.transport_allocations, 1);
    }
    println!("lazy upload bytes per split chunk=[44,36,40,36]; total=156; public batch reset=124");
}

#[test]
#[ignore = "requires actual Metal; checks immutable input upload reuse"]
fn metal_lazy_uploads_reuse_only_current_batch_inputs() {
    qualify_immutable_uploads(GpuBackendPreference::Metal);
}

#[test]
#[ignore = "requires actual Vulkan; checks immutable input upload reuse"]
fn vulkan_lazy_uploads_reuse_only_current_batch_inputs() {
    qualify_immutable_uploads(GpuBackendPreference::Vulkan);
}
