//! Public controlled doors preserve empty/stop precedence and complete results.

use crate::{
    FormulaLimits, FormulaVerdict, GpuBackendPreference, GpuContext, GpuErrorKind,
    GpuFormulaOracle, GpuLimits, GpuOptions, GpuOracle, GpuSelection,
};
use zetesis_core::{AdmissionLimits, GroundProgram, Program, Seed, StaticLimits};
use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, Node, Theory};

fn cancelled() -> Control {
    let control = Control::default();
    control.cancel();
    control
}

fn controlled_calls(backend: GpuBackendPreference) {
    let context = GpuContext::new_selected(
        GpuOptions::default(),
        GpuSelection {
            backend,
            vendor_id: None,
        },
    )
    .unwrap();
    assert!(context.info().is_hardware_gpu());
    assert_eq!(
        context.info().backend(),
        match backend {
            GpuBackendPreference::Metal => "Metal",
            GpuBackendPreference::Vulkan => "Vulkan",
            _ => panic!("explicit physical API required"),
        }
    );
    eprintln!("controlled calls adapter={:?}", context.info());
    let mut ordinary = GpuOracle::from_context(&context).unwrap();
    let mut formula = GpuFormulaOracle::from_context(&context).unwrap();
    let program = Program::new(vec![], AdmissionLimits::default()).unwrap();
    let ground = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    let seeds = [Seed::new(&program, []).unwrap()];
    let theory = Theory::new(
        1,
        vec![Node::Atom(0)],
        vec![0],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    let candidates = [
        Interpretation::new(&theory, []).unwrap(),
        Interpretation::new(&theory, [0]).unwrap(),
    ];
    for control in [
        cancelled(),
        Control::with_deadline(std::time::Instant::now()),
    ] {
        let expected = control.poll().unwrap_err();
        for empty in [true, false] {
            let ordinary_error = ordinary
                .check_batch_with_control(
                    &ground,
                    if empty { &[] } else { &seeds },
                    GpuLimits::default(),
                    &control,
                )
                .unwrap_err();
            let formula_error = formula
                .propagate_batch_with_control(
                    &theory,
                    if empty { &[] } else { &candidates },
                    FormulaLimits::default(),
                    &control,
                )
                .unwrap_err();
            for error in [ordinary_error, formula_error] {
                assert_eq!(error.kind(), GpuErrorKind::Interrupted);
                assert_eq!(error.interruption(), Some(expected));
            }
            assert_eq!(ordinary.epoch, 0);
            assert!(ordinary.last_batch_stats().is_none());
            assert!(formula.last_batch_stats().is_none());
            context.check_health().unwrap();
        }
    }
    let complete = ordinary
        .check_batch_with_control(&ground, &seeds, GpuLimits::default(), &Control::default())
        .unwrap();
    assert_eq!(complete.len(), 1);
    assert!(complete[0].accepted());
    assert!(complete[0].closure_words().is_empty());
    let checks = formula
        .propagate_batch_with_control(
            &theory,
            &candidates,
            FormulaLimits::default(),
            &Control::default(),
        )
        .unwrap();
    assert_eq!(
        checks
            .iter()
            .map(crate::FormulaCheck::verdict)
            .collect::<Vec<_>>(),
        [FormulaVerdict::NotModel, FormulaVerdict::NoProperSubset]
    );
    entry_precedence(&context, &mut ordinary, &mut formula, &ground, &theory);
}

fn entry_precedence(
    context: &GpuContext,
    ordinary: &mut GpuOracle,
    formula: &mut GpuFormulaOracle,
    ground: &GroundProgram,
    theory: &Theory,
) {
    let lease = context.lease().unwrap();
    assert_eq!(
        ordinary
            .check_batch_with_control(ground, &[], GpuLimits::default(), &cancelled())
            .unwrap_err()
            .kind(),
        GpuErrorKind::Busy
    );
    assert_eq!(
        formula
            .propagate_batch_with_control(theory, &[], FormulaLimits::default(), &cancelled())
            .unwrap_err()
            .kind(),
        GpuErrorKind::Busy
    );
    drop(lease);
    context.invalidate();
    assert_eq!(
        ordinary
            .check_batch_with_control(ground, &[], GpuLimits::default(), &cancelled())
            .unwrap_err()
            .kind(),
        GpuErrorKind::Interrupted
    );
    assert_eq!(
        formula
            .propagate_batch_with_control(theory, &[], FormulaLimits::default(), &cancelled())
            .unwrap_err()
            .kind(),
        GpuErrorKind::Interrupted
    );
    assert_eq!(
        ordinary
            .check_batch(ground, &[], GpuLimits::default())
            .unwrap_err()
            .kind(),
        GpuErrorKind::Device
    );
    assert_eq!(
        formula
            .propagate_batch(theory, &[], FormulaLimits::default())
            .unwrap_err()
            .kind(),
        GpuErrorKind::Device
    );
}

#[test]
#[ignore = "requires an actual physical Metal adapter"]
fn metal_controlled_calls_preserve_stop_identity() {
    controlled_calls(GpuBackendPreference::Metal);
}

#[test]
#[ignore = "requires an actual physical Vulkan adapter"]
fn vulkan_controlled_calls_preserve_stop_identity() {
    controlled_calls(GpuBackendPreference::Vulkan);
}
