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
fn bitwise_transfer_preserves_the_shader_scaffold() {
    let baseline = include_str!("../../formula.wgsl");
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
