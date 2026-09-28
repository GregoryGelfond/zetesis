pub(in crate::aggregate) mod fixtures;
mod packing;
mod preparation;

#[test]
fn aggregate_shader_uses_portable_integer_capabilities() {
    let module = naga::front::wgsl::parse_str(super::SHADER).expect("aggregate shader parses");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("aggregate shader validates without optional features");
    assert_eq!(module.entry_points.len(), 1);
    assert_eq!(module.entry_points[0].workgroup_size, [64, 1, 1]);
}
