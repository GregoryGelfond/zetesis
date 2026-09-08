//! Capacity contracts are portable; execution tests require physical Metal.

use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, Predicate, Program, Seed, Template, Term, Value,
};
use zetesis_cpu::{Control, Limits, Stop, check, lazy};

use crate::{GpuBackendPreference, GpuErrorKind, GpuLimits, GpuOptions, GpuSelection};

use super::{Capacity, GpuLazyOracle, LazyGpuStatistics, Plan, tests::inspect};

#[test]
fn retained_inputs_count_toward_the_transport_ceiling() {
    inspect(|chunk| {
        let plan = Plan::new(chunk, GpuLimits::default(), &wgpu::Limits::default()).unwrap();
        let mut retained = plan.capacity;
        retained.inputs[0] += 64;
        retained.inputs[1] += 128;
        let accounted = retained.accounted(&plan).unwrap();
        assert_eq!(accounted, plan.capacity.accounted(&plan).unwrap() + 192);
        assert!(retained.reusable(&plan, accounted));
        assert!(!retained.reusable(&plan, accounted - 1));
        // Replacement can use the exact active shape under the tighter limit.
        assert!(plan.capacity.reusable(&plan, accounted - 1));
    });
}

#[test]
fn every_input_binding_must_fit_before_reuse() {
    inspect(|chunk| {
        let plan = Plan::new(chunk, GpuLimits::default(), &wgpu::Limits::default()).unwrap();
        for index in 0..4 {
            let mut retained = plan.capacity;
            retained.inputs[index] -= 4;
            assert!(!retained.reusable(&plan, u64::MAX));
        }
    });
}

#[test]
fn result_shape_changes_require_replacement() {
    inspect(|chunk| {
        let plan = Plan::new(chunk, GpuLimits::default(), &wgpu::Limits::default()).unwrap();
        for result in [plan.result_bytes - 4, plan.result_bytes + 4] {
            assert!(
                !Capacity {
                    result,
                    ..plan.capacity
                }
                .reusable(&plan, u64::MAX)
            );
        }
    });
}

#[test]
fn inactive_input_capacity_does_not_inflate_uploads() {
    inspect(|chunk| {
        let plan = Plan::new(chunk, GpuLimits::default(), &wgpu::Limits::default()).unwrap();
        let mut retained = plan.capacity;
        retained.inputs[1] += 128;
        let first = LazyGpuStatistics::default()
            .submitted(&plan, false, retained)
            .unwrap();
        let reused = first.submitted(&plan, true, retained).unwrap();
        assert_eq!(reused.dispatches, 2);
        assert_eq!(reused.transport_allocations, 1);
        assert_eq!(reused.transport_reuses, 1);
        assert_eq!(reused.uploaded_bytes, 2 * plan.uploaded_bytes);
        assert_eq!(reused.downloaded_bytes, 0);
        assert_eq!(reused.peak_transport_bytes, retained.bytes().unwrap());
    });
}

#[test]
fn retained_capacity_arithmetic_refuses_overflow() {
    inspect(|chunk| {
        let plan = Plan::new(chunk, GpuLimits::default(), &wgpu::Limits::default()).unwrap();
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
                inputs: [u64::MAX - 128, 0, 0, 0],
                ..plan.capacity
            },
        ] {
            assert!(retained.accounted(&plan).is_none());
            assert!(!retained.reusable(&plan, u64::MAX));
        }
    });
}

#[test]
fn transport_observation_counters_refuse_overflow() {
    inspect(|chunk| {
        let plan = Plan::new(chunk, GpuLimits::default(), &wgpu::Limits::default()).unwrap();
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
                    .submitted(&plan, reused, plan.capacity)
                    .unwrap_err()
                    .kind(),
                GpuErrorKind::Capacity
            );
        }
    });
}

fn metal() -> GpuLazyOracle {
    let oracle = GpuLazyOracle::new_selected(
        GpuOptions::default(),
        GpuSelection {
            backend: GpuBackendPreference::Metal,
            vendor_id: None,
        },
    )
    .expect("physical Metal is required for this explicit transport qualification");
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

#[test]
fn source_sequence_exercises_transport_resize_boundaries() {
    let program = growth_program();
    let seeds = growth_seeds(&program);
    for selection in [lazy::SourceSelection::Union, lazy::SourceSelection::Worlds] {
        let mut retained: Option<Capacity> = None;
        let mut previous: Option<Capacity> = None;
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
                let plan =
                    Plan::new(chunk, GpuLimits::default(), &wgpu::Limits::default()).unwrap();
                let reused = retained.is_some_and(|capacity| capacity.reusable(&plan, u64::MAX));
                if let Some(previous) = previous {
                    smaller_reused |= reused && plan.capacity.inputs[0] < previous.inputs[0];
                    later_growth |= smaller_reused && plan.result_bytes > previous.result;
                }
                previous = Some(plan.capacity);
                if !reused {
                    retained = Some(plan.capacity);
                }
                lazy::evaluate(chunk)
            },
        )
        .unwrap();
        // This validates only the deterministic source sequence used below.
        // No buffer allocation or device execution is claimed by this control.
        assert!(smaller_reused);
        assert!(later_growth);
    }
}

#[test]
#[ignore = "requires a physical Metal adapter"]
fn metal_lazy_transport_reuse_preserves_round_truth() {
    let program = growth_program();
    let seeds = growth_seeds(&program);
    let mut oracle = metal();
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
                let plan = Plan::new(chunk, GpuLimits::default(), &oracle.runtime.limits).unwrap();
                let reuses = oracle.statistics.transport_reuses;
                let result = oracle.execute(
                    chunk,
                    GpuLimits::default(),
                    &Control::default(),
                    &mut cached,
                )?;
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
fn metal_lazy_transport_refusal_preserves_reuse() {
    let mut oracle = metal();
    let mut ran = false;
    inspect(|chunk| {
        if ran {
            return;
        }
        ran = true;
        let mut cached = None;
        let plan = Plan::new(chunk, GpuLimits::default(), &oracle.runtime.limits).unwrap();
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
    let mut oracle = metal();
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
}
