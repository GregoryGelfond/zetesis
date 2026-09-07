//! Portable validation of an unapplied shader experiment; no device claim.

#[test]
fn proposed_gate_transfer_validates_with_the_pinned_shader_frontend() {
    let original = include_str!("../src/formula.wgsl");
    let proposed =
        include_str!("../../../experiments/gate-transfer/generated/proposed-formula.wgsl");
    let mut interfaces = Vec::new();
    for source in [original, proposed] {
        let module = naga::front::wgsl::parse_str(source).expect("WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("portable WGSL validation");
        let bindings: Vec<_> = module
            .global_variables
            .iter()
            .map(|(_, variable)| (variable.name.clone(), variable.space, variable.binding))
            .collect();
        let entry_points: Vec<_> = module
            .entry_points
            .iter()
            .map(|entry| (entry.name.clone(), entry.stage, entry.workgroup_size))
            .collect();
        interfaces.push((bindings, entry_points));
    }
    assert_eq!(
        interfaces[0], interfaces[1],
        "experimental transfer retains the host interface"
    );
}
