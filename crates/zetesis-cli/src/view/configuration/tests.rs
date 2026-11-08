//! Effective execution facts remain separate from requested policy and utilization.

use super::*;
use crate::{Grounder, Oracle};

fn workers() -> NonZeroUsize {
    NonZeroUsize::new(14).unwrap()
}

fn formula() -> ExecutionObservation<'static> {
    ExecutionObservation::CpuFormula {
        oracle: Oracle::Auto,
        grounder: Grounder::Auto,
    }
}

#[test]
fn configuration_retains_the_configured_worker_budget() {
    let mut state = Configuration::new(workers());
    // An observed utilization does not replace the configured capacity.
    assert!(
        state
            .observe(&ExecutionObservation::ParallelProposals {
                workers: NonZeroUsize::MIN,
            })
            .is_none()
    );
    let view = state.observe(&formula()).unwrap();
    assert_eq!(view.workers, workers());
}

#[test]
fn formula_configuration_preserves_composite_grounding() {
    for (event, expected) in [
        (None, GroundingDisplay::Eager),
        (
            Some(ExecutionObservation::HybridGrounding {
                requested: Grounder::Auto,
                streamed: zetesis_solve::StreamedConstraints {
                    templates: 2,
                    instances: 8,
                    ..Default::default()
                },
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
                    batch_size: NonZeroUsize::new(32).unwrap(),
                    completion_workers: NonZeroUsize::new(2).unwrap(),
                },
                true,
            ),
            (
                ExecutionObservation::DeviceTight {
                    adapter: adapter(&name),
                    grounder: Grounder::Auto,
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
        let first = state.observe(&formula()).unwrap();
        assert_eq!(first.backend, BackendView::Cpu);
        assert_eq!(first.grounding, GroundingDisplay::Eager);
        let promoted = state
            .observe(&ExecutionObservation::DeviceFormula {
                adapter: adapter("promoted adapter"),
                projection: zetesis_wgpu::GateProjection::Enumerated,
                grounder: Grounder::Auto,
                batch_size: NonZeroUsize::new(32).unwrap(),
                completion_workers: NonZeroUsize::new(2).unwrap(),
            })
            .unwrap();
        assert_eq!(
            promoted.backend,
            BackendView::Gpu {
                adapter: "promoted adapter",
                api: "Metal",
                cpu_completion: true,
            }
        );
        assert_eq!(promoted.grounding, GroundingDisplay::Eager);
    }
}
