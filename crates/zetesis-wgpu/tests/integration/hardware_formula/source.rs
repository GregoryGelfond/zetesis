//! The explicit source constructor retains its context and physical options.

use super::*;
use zetesis_wgpu::GpuDeviceSource;

pub(super) fn qualify(original: &GpuFormulaOracle) {
    let context = original.context();
    let mut default = GpuFormulaOracle::from_source(GpuDeviceSource::Context(context)).unwrap();
    assert!(default.context().same_instance(context));
    assert_eq!(
        default.compiled_profile().kernel_options(),
        GpuKernelOptions::default()
    );
    assert_eq!(default.projection(), GateProjection::default());
    for graph in frozen_theories() {
        compare(&mut default, &graph);
        assert_eq!(
            default.last_batch_stats().unwrap().propagation,
            GpuPropagation::Dense
        );
    }
    for propagation in [GpuPropagation::Dense, GpuPropagation::ChangedDependencies] {
        let options = GpuKernelOptions {
            propagation,
            ..GpuKernelOptions::default()
        };
        let mut configured =
            GpuFormulaOracle::from_source_with_kernel(GpuDeviceSource::Context(context), options)
                .unwrap();
        assert!(configured.context().same_instance(context));
        assert!(
            !configured
                .compiled_profile()
                .same_instance(original.compiled_profile())
        );
        assert_eq!(configured.compiled_profile().kernel_options(), options);
        assert_eq!(configured.projection(), GateProjection::default());
        assert_eq!(
            configured.context().compute_timing(),
            context.compute_timing()
        );
        assert!(configured.last_batch_stats().is_none());
        for graph in frozen_theories() {
            compare(&mut configured, &graph);
            // A graph with no dependent gate needs no incidence index; the
            // existing admission contract retains Dense for that exact shape.
            let has_dependents = (0..graph.view().len()).any(|node| {
                !matches!(
                    graph.view().node(node).unwrap(),
                    NodeView::False | NodeView::Atom(_)
                )
            });
            let expected = if has_dependents {
                propagation
            } else {
                GpuPropagation::Dense
            };
            assert_eq!(configured.last_batch_stats().unwrap().propagation, expected);
        }
    }
}
