//! The assembled production sources retain the independently checked interface.

use super::GateProjection;
use std::borrow::Cow;

#[test]
fn default_shader_remains_the_borrowed_baseline() {
    assert!(
        matches!(GateProjection::Enumerated.shader().unwrap(), Cow::Borrowed(source)
        if source == include_str!("../../formula.wgsl"))
    );
}

#[test]
fn bitwise_shader_matches_the_retained_transfer() {
    assert_eq!(
        GateProjection::Bitwise.shader().unwrap(),
        include_str!("../../../../../experiments/gate-transfer/generated/proposed-formula.wgsl")
    );
}

#[test]
fn projection_variants_preserve_the_device_interface() {
    let mut interfaces = Vec::new();
    for projection in GateProjection::ALL {
        let source = projection.shader().unwrap();
        let module = naga::front::wgsl::parse_str(&source).expect("WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("portable shader validation");
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
