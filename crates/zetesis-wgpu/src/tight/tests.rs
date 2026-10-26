use super::*;
use std::error::Error as _;

#[test]
fn support_shader_validates_without_optional_features() {
    let module = naga::front::wgsl::parse_str(SHADER).expect("support WGSL parses");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("topology and cooperative barriers validate");
    let (module, _) = naga::back::pipeline_constants::process_overrides(
        &module,
        &info,
        None,
        &naga::back::PipelineConstants::default(),
    )
    .unwrap();
    assert_eq!(module.entry_points.len(), 4);
    for (entry, name) in module.entry_points.iter().zip([
        "check",
        "check_grouped",
        "check_dense",
        "check_grouped_dense",
    ]) {
        assert_eq!(entry.name, name);
        assert_eq!(entry.workgroup_size, [64, 1, 1]);
    }
}

/// The module as pipeline creation compiles it for `entry_point`: reduced to
/// that entry point and what it reaches, with its workgroup size overridden.
fn reduced_to(entry_point: &str) -> naga::Module {
    let module = naga::front::wgsl::parse_str(SHADER).unwrap();
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap();
    let constants = naga::back::PipelineConstants::from([("WORKGROUP_SIZE".to_owned(), 64.0)]);
    naga::back::pipeline_constants::process_overrides(
        &module,
        &info,
        Some((naga::ShaderStage::Compute, entry_point)),
        &constants,
    )
    .unwrap()
    .0
    .into_owned()
}

/// The update route's own globals: the state at binding 8 and the counters,
/// path maxima and decisions only its phases use.
const UPDATE_GLOBALS: [&str; 9] = [
    "state",
    "changed_atoms",
    "performed",
    "evaluated",
    "phase_path",
    "entry_route",
    "chosen_route",
    "summary_flag",
    "verdict_counts",
];

fn global_names(module: &naga::Module) -> Vec<String> {
    module
        .global_variables
        .iter()
        .filter_map(|(_, global)| global.name.clone())
        .collect()
}

/// A theory whose route is declined for its residency runs a kernel that
/// carries none of the route: no binding 8, so its pipeline binds no state, no
/// update counter or decision, and no update function.
#[test]
fn dense_entry_points_carry_no_update_route() {
    for entry_point in ["check_dense", "check_grouped_dense"] {
        let module = reduced_to(entry_point);
        let globals = global_names(&module);
        for name in UPDATE_GLOBALS {
            assert!(
                !globals.iter().any(|global| global == name),
                "{entry_point} keeps {name}"
            );
        }
        assert!(
            module.global_variables.iter().all(|(_, global)| global
                .binding
                .as_ref()
                .is_none_or(|binding| binding.binding != 8)),
            "{entry_point} binds the update state"
        );
        let functions: Vec<_> = module
            .functions
            .iter()
            .filter_map(|(_, function)| function.name.clone())
            .collect();
        for name in ["update_slot", "summarize", "end_phase", "retain_for_update"] {
            assert!(
                !functions.iter().any(|function| function == name),
                "{entry_point} keeps {name}"
            );
        }
    }
}

/// The full entry points keep the route: the reduction that strips it from the
/// dense twins leaves every update global in place here.
#[test]
fn full_entry_points_carry_the_update_route() {
    for entry_point in ["check", "check_grouped"] {
        let globals = global_names(&reduced_to(entry_point));
        for name in UPDATE_GLOBALS {
            assert!(
                globals.iter().any(|global| global == name),
                "{entry_point} lacks {name}"
            );
        }
    }
}

#[test]
fn shader_constants_preserve_the_wire_contract() {
    let module = naga::front::wgsl::parse_str(SHADER).unwrap();
    let (_, width) = module
        .overrides
        .iter()
        .find(|(_, value)| value.name.as_deref() == Some("WORKGROUP_SIZE"))
        .unwrap();
    assert!(matches!(
        module.global_expressions[width.init.unwrap()],
        naga::Expression::Literal(naga::Literal::U32(64))
    ));
    for (name, expected) in [
        ("NODE_FALSE", 0),
        ("NODE_ATOM", 1),
        ("NODE_AND", 2),
        ("NODE_OR", 3),
        ("NODE_IMPLIES", 4),
        ("NODE_ALL", 5),
        ("NODE_ANY", 6),
        ("STATUS_STABLE", 0),
        ("STATUS_NOT_MODEL", 1),
        ("STATUS_RESIDUAL", 2),
        ("RESULT_WORDS", 9),
        ("RESULT_MAGIC", 0x5453_5031),
        ("RESULT_GROUPED_MAGIC", 0x5453_4731),
        ("ROUTE_COMPLETE", super::update::ROUTE_COMPLETE),
        ("ROUTE_DENSE", super::update::ROUTE_DENSE),
        ("ROUTE_UPDATE", super::update::ROUTE_UPDATE),
        (
            "ROUTE_BEYOND_THRESHOLD",
            super::update::ROUTE_BEYOND_THRESHOLD,
        ),
        ("SEEDED", super::update::SEEDED),
        ("READER_NODE", super::update::READER_NODE >> 30),
        ("READER_PRODUCER", super::update::READER_PRODUCER >> 30),
        ("SUMMARY_WORDS", super::update::SUMMARY_WORDS),
    ] {
        let (_, constant) = module
            .constants
            .iter()
            .find(|(_, constant)| constant.name.as_deref() == Some(name))
            .unwrap();
        assert!(
            matches!(module.global_expressions[constant.init],naga::Expression::Literal(naga::Literal::U32(actual)) if actual==expected),
            "{name}"
        );
    }
}

#[test]
fn shader_structs_match_host_buffer_strides() {
    let module = naga::front::wgsl::parse_str(SHADER).unwrap();
    for (name, expected) in [("Params", 64), ("Node", 16), ("Producer", 16), ("Slot", 36)] {
        let (_, ty) = module
            .types
            .iter()
            .find(|(_, ty)| ty.name.as_deref() == Some(name))
            .unwrap();
        assert!(
            matches!(ty.inner,naga::TypeInner::Struct{span,..} if span==expected),
            "{name}"
        );
    }
}

#[test]
fn control_failures_keep_their_typed_reason() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let error = TightGpuError::from(poll(&cancellation).unwrap_err());
    assert_eq!(error, TightGpuError::Stopped(Stop::Cancelled));
    assert_eq!(error.to_string(), "operation cancelled");
    assert_eq!(error.source().unwrap().to_string(), error.to_string());
}

#[test]
fn expired_deadlines_are_not_device_failures() {
    let cancellation = Cancellation::with_deadline(std::time::Instant::now()).unwrap();
    assert_eq!(
        TightGpuError::from(poll(&cancellation).unwrap_err()),
        TightGpuError::Stopped(Stop::Deadline)
    );
}

#[test]
fn backend_failures_retain_their_class() {
    let gpu = GpuError::new(crate::GpuErrorKind::Readback, "bad witness");
    let error = TightGpuError::from(gpu.clone());
    assert_eq!(error, TightGpuError::Gpu(gpu.clone()));
    assert_eq!(error.to_string(), gpu.to_string());
    assert_eq!(error.source().unwrap().to_string(), gpu.to_string());
}
