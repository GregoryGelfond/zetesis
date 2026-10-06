//! Effective execution facts remain separate from requested policy and utilization.

use super::*;
use crate::{Grounder, Oracle, SearchMethod, SourceBatching};

fn workers() -> NonZeroUsize {
    NonZeroUsize::new(14).unwrap()
}

fn formula() -> ExecutionObservation<'static> {
    ExecutionObservation::CpuFormula {
        oracle: Oracle::Auto,
        grounder: Grounder::Auto,
        search: SearchMethod::Regions,
    }
}

#[test]
fn closure_configuration_reports_effective_grounding() {
    for (grounder, expected) in [
        (Grounder::Eager, GroundingDisplay::Eager),
        (Grounder::Lazy, GroundingDisplay::Lazy),
    ] {
        let mut state = Configuration::new(workers());
        let view = state
            .observe(&ExecutionObservation::CpuClosure {
                grounder,
                batching: SourceBatching::Independent,
                workers: workers(),
            })
            .unwrap();
        assert_eq!(view.grounding, expected);
        assert_eq!(view.backend, BackendView::Cpu);
    }
}

#[test]
fn configuration_retains_the_configured_worker_budget() {
    let mut state = Configuration::new(workers());
    let view = state
        .observe(&ExecutionObservation::CpuClosure {
            grounder: Grounder::Lazy,
            batching: SourceBatching::Independent,
            workers: NonZeroUsize::MIN,
        })
        .unwrap();
    assert_eq!(view.workers, workers());
}

#[test]
fn formula_configuration_preserves_composite_grounding() {
    for (event, expected) in [
        (None, GroundingDisplay::Eager),
        (
            Some(ExecutionObservation::HybridGrounding {
                requested: Grounder::Auto,
                streamed_templates: 2,
                streamed_instances: 8,
            }),
            GroundingDisplay::Hybrid,
        ),
        (
            Some(ExecutionObservation::TerminalDefinitions {
                requested: Grounder::Auto,
                base: zetesis_themelios::BaseKind::Eager,
                deferred_templates: 3,
                streamed: None,
            }),
            GroundingDisplay::TerminalDefinitions,
        ),
    ] {
        let mut state = Configuration::new(workers());
        if let Some(event) = event {
            assert!(state.observe(&event).is_none());
        }
        let view = state.observe(&formula()).unwrap();
        assert_eq!(view.grounding, expected);
        assert_eq!(view.backend, BackendView::Cpu);
    }
}

#[test]
fn source_events_wait_for_backend_selection() {
    let mut state = Configuration::new(workers());
    for event in [
        ExecutionObservation::LazyGrounding {
            requested: Grounder::Auto,
        },
        ExecutionObservation::StaticGrounding {
            requested: Grounder::Auto,
            atoms: 4,
            rules: 3,
            limits: zetesis_core::StaticLimits::default(),
        },
    ] {
        assert!(state.observe(&event).is_none());
    }
    assert_eq!(
        state.observe(&formula()).unwrap().grounding,
        GroundingDisplay::Eager
    );
}

#[cfg(feature = "gpu")]
mod device {
    use super::*;

    fn adapter(name: &str) -> zetesis_wgpu::AdapterMetadata<'_> {
        zetesis_wgpu::AdapterMetadata {
            name,
            backend: zetesis_wgpu::AdapterBackend::Metal,
            category: zetesis_wgpu::AdapterCategory::IntegratedGpu,
            vendor_id: 0,
            device_id: 0,
            pci_bus_id: None,
            driver: None,
            driver_info: None,
        }
    }

    #[test]
    fn device_configuration_borrows_the_observed_adapter() {
        let name = String::from("reported adapter");
        let mut state = Configuration::new(workers());
        for (event, completion) in [
            (
                ExecutionObservation::DeviceFormula {
                    adapter: adapter(&name),
                    projection: zetesis_wgpu::GateProjection::Enumerated,
                    grounder: Grounder::Auto,
                    search: SearchMethod::Regions,
                    batch_size: NonZeroUsize::new(32).unwrap(),
                    completion_workers: NonZeroUsize::new(2).unwrap(),
                },
                true,
            ),
            (
                ExecutionObservation::DeviceTight {
                    adapter: adapter(&name),
                    grounder: Grounder::Auto,
                    search: SearchMethod::Regions,
                    batch_size: NonZeroUsize::new(32).unwrap(),
                },
                false,
            ),
        ] {
            let view = state.observe(&event).unwrap();
            assert_eq!(
                view.backend,
                BackendView::Gpu {
                    adapter: &name,
                    api: "Metal",
                    cpu_completion: completion,
                }
            );
            let BackendView::Gpu { adapter, .. } = view.backend else {
                panic!("device observation must report its adapter");
            };
            assert_eq!(adapter.as_ptr(), name.as_ptr());
        }
    }

    #[test]
    fn backend_promotion_reports_the_new_configuration() {
        let mut state = Configuration::new(workers());
        let first = state
            .observe(&ExecutionObservation::CpuClosure {
                grounder: Grounder::Lazy,
                batching: SourceBatching::Independent,
                workers: workers(),
            })
            .unwrap();
        assert_eq!(first.backend, BackendView::Cpu);
        assert_eq!(first.grounding, GroundingDisplay::Lazy);
        let promoted = state
            .observe(&ExecutionObservation::DeviceClosure {
                adapter: adapter("promoted adapter"),
                static_counts: Some((4, 3)),
            })
            .unwrap();
        assert_eq!(
            promoted.backend,
            BackendView::Gpu {
                adapter: "promoted adapter",
                api: "Metal",
                cpu_completion: false,
            }
        );
        assert_eq!(promoted.grounding, GroundingDisplay::Eager);
    }

    #[test]
    fn source_device_closure_reports_lazy_grounding() {
        let mut state = Configuration::new(workers());
        assert!(
            state
                .observe(&ExecutionObservation::LazyDeviceGrounding {
                    requested: Grounder::Lazy,
                })
                .is_none()
        );
        let view = state
            .observe(&ExecutionObservation::DeviceClosure {
                adapter: adapter("lazy adapter"),
                static_counts: None,
            })
            .unwrap();
        assert_eq!(view.grounding, GroundingDisplay::Lazy);
    }
}
