//! The backend line names the search method on a device as it does on the
//! CPU.

use std::num::NonZeroUsize;

use crate::presentation::Diagnostics;
use crate::{ColorMode, ExecutionObservation, ExecutionObserver, Grounder, SearchMethod};

#[test]
fn the_device_formula_line_names_the_search_method() {
    let adapter = zetesis_wgpu::AdapterMetadata {
        name: "test adapter",
        backend: zetesis_wgpu::AdapterBackend::Noop,
        category: zetesis_wgpu::AdapterCategory::Other,
        vendor_id: 0x1234,
        device_id: 0,
        pci_bus_id: None,
        driver: None,
        driver_info: None,
    };
    let mut output = Vec::new();
    Diagnostics::new(&mut output, ColorMode::Never)
        .observe(ExecutionObservation::DeviceFormula {
            adapter,
            projection: zetesis_wgpu::GateProjection::Enumerated,
            grounder: Grounder::Auto,
            search: SearchMethod::Clauses,
            batch_size: NonZeroUsize::new(64).unwrap(),
            completion_workers: NonZeroUsize::new(2).unwrap(),
        })
        .unwrap();
    let line = String::from_utf8(output).unwrap();
    assert!(line.contains("; search: clauses; "), "{line}");
}

#[test]
fn hybrid_device_metadata_preserves_pending_host_acceptance() {
    for terminal in [false, true] {
        for tight in [false, true] {
            let adapter = zetesis_wgpu::AdapterMetadata {
                name: "test adapter",
                backend: zetesis_wgpu::AdapterBackend::Noop,
                category: zetesis_wgpu::AdapterCategory::Other,
                vendor_id: 0x1234,
                device_id: 0,
                pci_bus_id: None,
                driver: None,
                driver_info: None,
            };
            let mut output = Vec::new();
            let mut diagnostics = Diagnostics::new(&mut output, ColorMode::Never);
            diagnostics
                .observe(if terminal {
                    ExecutionObservation::TerminalDefinitions {
                        requested: Grounder::Lazy,
                        base: zetesis_themelios::BaseKind::Hybrid,
                        deferred_templates: 1,
                        streamed: None,
                    }
                } else {
                    ExecutionObservation::HybridGrounding {
                        requested: Grounder::Lazy,
                        streamed_templates: 1,
                        streamed_instances: 2,
                    }
                })
                .unwrap();
            diagnostics
                .observe(if tight {
                    ExecutionObservation::DeviceTight {
                        adapter,
                        grounder: Grounder::Eager,
                        search: SearchMethod::Regions,
                        batch_size: NonZeroUsize::MIN,
                    }
                } else {
                    ExecutionObservation::DeviceFormula {
                        adapter,
                        projection: zetesis_wgpu::GateProjection::Enumerated,
                        grounder: Grounder::Eager,
                        search: SearchMethod::Regions,
                        batch_size: NonZeroUsize::MIN,
                        completion_workers: NonZeroUsize::MIN,
                    }
                })
                .unwrap();
            diagnostics
                .observe(ExecutionObservation::TightMembership)
                .unwrap();
            let text = String::from_utf8(output).unwrap();
            assert!(text.contains("core grounding: eager"), "{text}");
            assert!(!text.contains("over the original theory"), "{text}");
            if terminal {
                assert!(
                    text.contains("original constraints and answer reconstruction: host"),
                    "{text}"
                );
                assert!(
                    text.contains("original constraints and reconstruction still pending"),
                    "{text}"
                );
            } else {
                assert!(text.contains("original constraints: host"), "{text}");
                assert!(
                    text.contains("original constraints still pending"),
                    "{text}"
                );
            }
        }
    }
}
