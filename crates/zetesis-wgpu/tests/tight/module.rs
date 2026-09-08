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
    assert_eq!(module.entry_points.len(), 1);
    assert_eq!(module.entry_points[0].name, "check");
    assert_eq!(module.entry_points[0].workgroup_size, [64, 1, 1]);
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
