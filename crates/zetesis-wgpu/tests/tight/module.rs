use super::*;
use std::error::Error as _;

#[test]
fn support_shader_validates_without_optional_features() {
    let module = naga::front::wgsl::parse_str(SHADER).expect("support WGSL parses");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("topology and cooperative barriers validate");
    assert_eq!(module.entry_points.len(), 2);
    for (entry, name) in module.entry_points.iter().zip(["check", "check_grouped"]) {
        assert_eq!(entry.name, name);
        assert_eq!(entry.workgroup_size, [64, 1, 1]);
    }
}

#[test]
fn shader_constants_preserve_the_wire_contract() {
    let module = naga::front::wgsl::parse_str(SHADER).unwrap();
    for (name, expected) in [
        ("WORKGROUP_SIZE", 64),
        ("NODE_FALSE", 0),
        ("NODE_ATOM", 1),
        ("NODE_AND", 2),
        ("NODE_OR", 3),
        ("NODE_IMPLIES", 4),
        ("STATUS_STABLE", 0),
        ("STATUS_NOT_MODEL", 1),
        ("STATUS_RESIDUAL", 2),
        ("RESULT_WORDS", 6),
        ("RESULT_MAGIC", 0x5453_5031),
        ("RESULT_GROUPED_MAGIC", 0x5453_4731),
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
    for (name, expected) in [("Params", 32), ("Node", 16), ("Producer", 16)] {
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
    let control = Control::default();
    control.cancel();
    let error = TightGpuError::from(poll(&control).unwrap_err());
    assert_eq!(error, TightGpuError::Stopped(Stop::Cancelled));
    assert_eq!(error.to_string(), "operation cancelled");
    assert_eq!(error.source().unwrap().to_string(), error.to_string());
}

#[test]
fn expired_deadlines_are_not_device_failures() {
    let control = Control::with_deadline(std::time::Instant::now());
    assert_eq!(
        TightGpuError::from(poll(&control).unwrap_err()),
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
