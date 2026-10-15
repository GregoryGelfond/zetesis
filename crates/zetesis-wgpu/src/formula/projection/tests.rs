//! The assembled production sources retain the independently checked interface.

use super::GateProjection;
use crate::kernel::Kernel;
use crate::{GpuKernelOptions, GpuWorkgroupSize};
use std::borrow::Cow;

#[test]
fn default_shader_remains_the_borrowed_baseline() {
    assert!(
        matches!(GateProjection::Enumerated.shader().unwrap(), Cow::Borrowed(source)
        if source == concat!(include_str!("../../formula.wgsl"), include_str!("../../formula_graph/numeric.wgsl"), include_str!("../numeric.wgsl"), include_str!("../subset.wgsl")))
    );
}

#[test]
fn bitwise_transfer_preserves_the_shader_scaffold() {
    let baseline = concat!(
        include_str!("../../formula.wgsl"),
        include_str!("../../formula_graph/numeric.wgsl"),
        include_str!("../numeric.wgsl"),
        include_str!("../subset.wgsl")
    );
    let (prefix, transfer) = baseline.split_once("fn gate(").unwrap();
    let suffix = &transfer[transfer.find("fn finish(").unwrap()..];
    let assembled = GateProjection::Bitwise.shader().unwrap();
    let replacement = assembled
        .strip_prefix(prefix)
        .unwrap()
        .strip_suffix(suffix)
        .unwrap();
    assert_eq!(replacement, include_str!("../bitwise.wgsl"));
}

#[test]
fn projection_variants_preserve_the_device_interface() {
    let mut interfaces = Vec::new();
    for projection in GateProjection::ALL {
        let source = projection.shader().unwrap();
        let module = naga::front::wgsl::parse_str(&source).expect("WGSL parses");
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("portable shader validation");
        let (module, _) = naga::back::pipeline_constants::process_overrides(
            &module,
            &info,
            None,
            &naga::back::PipelineConstants::default(),
        )
        .unwrap();
        assert_eq!(module.entry_points.len(), 1);
        assert_eq!(module.entry_points[0].workgroup_size, [64, 1, 1]);
        let mut workgroup_bytes = 0;
        for (_, variable) in module.global_variables.iter() {
            if variable.space == naga::AddressSpace::WorkGroup {
                let (naga::TypeInner::Atomic(scalar) | naga::TypeInner::Scalar(scalar)) =
                    module.types[variable.ty].inner
                else {
                    panic!("formula workgroup storage consists only of scalar words");
                };
                assert_eq!(scalar.width, 4);
                workgroup_bytes += u32::from(scalar.width);
            }
        }
        assert_eq!(workgroup_bytes, 24);
        let bindings: Vec<_> = module
            .global_variables
            .iter()
            .map(|(_, variable)| (variable.name.clone(), variable.space, variable.binding))
            .collect();
        let entries: Vec<_> = module
            .entry_points
            .iter()
            .map(|entry| (entry.name.clone(), entry.stage, entry.workgroup_size))
            .collect();
        interfaces.push((bindings, entries));
    }
    assert_eq!(interfaces[0], interfaces[1]);
}

#[test]
fn optional_instructions_require_granted_features() {
    for projection in GateProjection::ALL {
        for features in [
            wgpu::Features::empty(),
            wgpu::Features::SUBGROUP,
            wgpu::Features::SHADER_INT64,
            wgpu::Features::SUBGROUP | wgpu::Features::SHADER_INT64,
        ] {
            let source = Kernel::Formula(GpuKernelOptions::default())
                .source(projection.shader().unwrap(), features)
                .unwrap();
            let module = naga::front::wgsl::parse_str(&source)
                .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&source)));
            let mut capabilities = naga::valid::Capabilities::empty();
            if features.contains(wgpu::Features::SUBGROUP) {
                capabilities |= naga::valid::Capabilities::SUBGROUP;
            }
            if features.contains(wgpu::Features::SHADER_INT64) {
                capabilities |= naga::valid::Capabilities::SHADER_INT64;
            }
            let info =
                naga::valid::Validator::new(naga::valid::ValidationFlags::all(), capabilities)
                    .validate(&module)
                    .unwrap();
            for width in [GpuWorkgroupSize::Threads32, GpuWorkgroupSize::Threads64] {
                let constants = [("WORKGROUP_SIZE".to_owned(), f64::from(width.invocations()))]
                    .into_iter()
                    .collect();
                let (compiled, _) = naga::back::pipeline_constants::process_overrides(
                    &module, &info, None, &constants,
                )
                .unwrap();
                assert_eq!(
                    compiled.entry_points[0].workgroup_size,
                    [width.invocations(), 1, 1]
                );
            }
            if !features.is_empty() {
                assert!(
                    naga::valid::Validator::new(
                        naga::valid::ValidationFlags::all(),
                        naga::valid::Capabilities::empty(),
                    )
                    .validate(&module)
                    .is_err()
                );
            }
        }
    }
}

#[test]
fn portable_selection_ignores_optional_device_features() {
    let kernel = Kernel::Formula(GpuKernelOptions {
        optional_features: false,
        ..Default::default()
    });
    let source = kernel
        .source(
            GateProjection::Enumerated.shader().unwrap(),
            wgpu::Features::all(),
        )
        .unwrap();
    assert!(matches!(source, Cow::Borrowed(_)));
}
