//! Portable declared-interface contracts for the maintained formula oracle.
//!
//! These checks inspect shader declarations. Physical qualification covers
//! integration with host buffers and dispatch.

fn module() -> naga::Module {
    let module = naga::front::wgsl::parse_str(include_str!("../src/formula.wgsl"))
        .expect("production WGSL parses");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("production WGSL validates");
    module
}

#[test]
fn formula_preserves_the_declared_bindings() {
    use naga::{AddressSpace, StorageAccess};
    let module = module();
    let bindings: Vec<_> = module
        .global_variables
        .iter()
        .filter_map(|(_, variable)| {
            variable.binding.map(|binding| {
                (
                    variable.name.as_deref().unwrap(),
                    binding.group,
                    binding.binding,
                    variable.space,
                )
            })
        })
        .collect();
    let read = AddressSpace::Storage {
        access: StorageAccess::LOAD,
    };
    let write = AddressSpace::Storage {
        access: StorageAccess::LOAD | StorageAccess::STORE,
    };
    assert_eq!(
        bindings,
        [
            ("params", 0, 0, AddressSpace::Uniform),
            ("nodes", 0, 1, read),
            ("roots", 0, 2, read),
            ("seeds", 0, 3, read),
            ("frozen", 0, 4, write),
            ("domains", 0, 5, write),
            ("results", 0, 6, write),
        ]
    );
}

#[test]
fn formula_parameters_keep_the_packed_word_layout() {
    let module = module();
    let (_, parameters) = module
        .types
        .iter()
        .find(|(_, ty)| ty.name.as_deref() == Some("Params"))
        .unwrap();
    let naga::TypeInner::Struct { members, span } = &parameters.inner else {
        panic!("parameters are a uniform structure");
    };
    assert_eq!(*span, 48);
    let fields = [
        "atoms",
        "nodes",
        "roots",
        "variables",
        "words",
        "worlds",
        "max_rounds",
        "max_work",
        "setup_work",
        "sweep_work",
        "epoch",
        "levels",
    ];
    assert_eq!(members.len(), fields.len());
    for (index, (member, name)) in members.iter().zip(fields).enumerate() {
        assert_eq!(member.name.as_deref(), Some(name));
        assert_eq!(member.offset, u32::try_from(index * 4).unwrap());
        assert!(matches!(
            module.types[member.ty].inner,
            naga::TypeInner::Scalar(naga::Scalar {
                kind: naga::ScalarKind::Uint,
                width: 4
            })
        ));
    }
}

#[test]
fn formula_preserves_the_declared_compute_entry() {
    let module = module();
    assert_eq!(module.entry_points.len(), 1);
    let entry = &module.entry_points[0];
    assert_eq!(entry.name, "propagate");
    assert_eq!(entry.stage, naga::ShaderStage::Compute);
    assert_eq!(entry.workgroup_size, [64, 1, 1]);
}
