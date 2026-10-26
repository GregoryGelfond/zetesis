//! Every kernel translates to SPIR-V as wgpu's Vulkan backend translates it.
//!
//! wgpu-hal's Vulkan device validates a kernel's WGSL, resolves the overridable
//! constants of the one entry point a pipeline compiles, and writes SPIR-V with
//! its device's options. The physical Vulkan qualification runs on a Linux
//! host; this check establishes on any host that no kernel, entry point,
//! workgroup size or optional-feature specialization depends on a construct the
//! SPIR-V writer refuses. It does not establish that a driver accepts the words
//! written, which only the physical qualification can.

use naga::back::spv;

use crate::GpuWorkgroupSize;
use crate::runtime::DeviceProfile;

/// The first word of every SPIR-V module.
const MAGIC_NUMBER: u32 = 0x0723_0203;

/// The optional device features a kernel's source specializes on, in every
/// combination the runtime can grant.
const FEATURE_SETS: [wgpu::Features; 4] = [
    wgpu::Features::empty(),
    wgpu::Features::SUBGROUP,
    wgpu::Features::SHADER_INT64,
    wgpu::Features::SUBGROUP.union(wgpu::Features::SHADER_INT64),
];

/// Translate every entry point of `profile`, as its runtime would compile it,
/// for every feature set, workgroup size, SPIR-V version and zero-fill mode
/// wgpu-hal's Vulkan backend can choose, and require each translation to be
/// written. Panics, naming the combination, at the first one refused.
pub(crate) fn assert_profile_translates(profile: &DeviceProfile) {
    let entry_points: Vec<_> = std::iter::once(profile.entry_point)
        .chain(profile.dense_entry_point)
        .collect();
    // A sized kernel compiles each size the runtime can dispatch; a fixed
    // kernel's pipeline resolves no workgroup-size constant.
    let sizes: Vec<Option<u32>> = match profile.kernel.options() {
        Some(_) => GpuWorkgroupSize::ALL
            .iter()
            .map(|size| Some(size.invocations()))
            .collect(),
        None => vec![None],
    };
    for features in FEATURE_SETS {
        let source = profile
            .kernel
            .source(profile.shader.clone(), features)
            .unwrap_or_else(|error| panic!("{} with {features:?}: {error}", profile.shader_label));
        assert_source_translates(
            profile.shader_label,
            &source,
            features,
            &entry_points,
            &sizes,
        );
    }
}

fn assert_source_translates(
    label: &str,
    source: &str,
    features: wgpu::Features,
    entry_points: &[&str],
    sizes: &[Option<u32>],
) {
    let module = naga::front::wgsl::parse_str(source)
        .unwrap_or_else(|error| panic!("{label}: {}", error.emit_to_string(source)));
    let mut capabilities = naga::valid::Capabilities::empty();
    if features.contains(wgpu::Features::SUBGROUP) {
        capabilities |= naga::valid::Capabilities::SUBGROUP;
    }
    if features.contains(wgpu::Features::SHADER_INT64) {
        capabilities |= naga::valid::Capabilities::SHADER_INT64;
    }
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), capabilities)
        .validate(&module)
        .unwrap_or_else(|error| panic!("{label} with {features:?}: {error}"));
    // wgpu-hal targets SPIR-V 1.0 on Vulkan 1.0 and 1.6 from Vulkan 1.3, the
    // ends of its range. Subgroup operations need SPIR-V 1.3, which it targets
    // from Vulkan 1.1, the first version that grants them.
    let versions: &[(u8, u8)] = if features.contains(wgpu::Features::SUBGROUP) {
        &[(1, 3), (1, 6)]
    } else {
        &[(1, 0), (1, 6)]
    };
    for &entry_point in entry_points {
        for &size in sizes {
            let mut constants = naga::back::PipelineConstants::default();
            if let Some(invocations) = size {
                constants.insert("WORKGROUP_SIZE".to_owned(), f64::from(invocations));
            }
            let (resolved, resolved_info) = naga::back::pipeline_constants::process_overrides(
                &module,
                &info,
                Some((naga::ShaderStage::Compute, entry_point)),
                &constants,
            )
            .unwrap_or_else(|error| panic!("{label} {entry_point} at {size:?}: {error}"));
            for &lang_version in versions {
                // Every pipeline keeps WebGPU's zero-fill of workgroup memory
                // (`runtime::ZERO_FILL_WORKGROUP_MEMORY`); wgpu-hal writes it
                // natively where the device can, and as a polyfill otherwise.
                for zero_fill in [
                    spv::ZeroInitializeWorkgroupMemoryMode::Native,
                    spv::ZeroInitializeWorkgroupMemoryMode::Polyfill,
                ] {
                    // The writer may declare any capability. That a device
                    // grants the ones a kernel needs is checked above, by
                    // validating with the capabilities of the features that
                    // select the kernel's specialization.
                    let options = spv::Options {
                        lang_version,
                        zero_initialize_workgroup_memory: zero_fill,
                        force_loop_bounding: true,
                        emit_int_div_checks: true,
                        ..spv::Options::default()
                    };
                    let pipeline = spv::PipelineOptions {
                        entry_point: entry_point.to_owned(),
                        shader_stage: naga::ShaderStage::Compute,
                    };
                    let words =
                        spv::write_vec(&resolved, &resolved_info, &options, Some(&pipeline))
                            .unwrap_or_else(|error| {
                                panic!(
                                    "{label} {entry_point} with {features:?} at {size:?}, SPIR-V \
                                     {lang_version:?}, {zero_fill:?}: {error}"
                                )
                            });
                    assert_eq!(words.first(), Some(&MAGIC_NUMBER), "{label} {entry_point}");
                }
            }
        }
    }
}
