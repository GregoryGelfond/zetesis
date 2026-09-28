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
