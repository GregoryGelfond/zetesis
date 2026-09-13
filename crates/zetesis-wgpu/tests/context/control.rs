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
    settled_stops(&context, &mut ordinary, &mut formula);
    entry_precedence(&context, &mut ordinary, &mut formula, &ground, &theory);
}

fn settled_stops(context: &GpuContext, ordinary: &mut GpuOracle, formula: &mut GpuFormulaOracle) {
    let program = Program::new(vec![], AdmissionLimits::default()).unwrap();
    let ground = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    let seeds = [Seed::new(&program, []).unwrap()];
    let theory = Theory::new(
        0,
        vec![],
        vec![],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    let candidates = [Interpretation::new(&theory, []).unwrap()];
    for stop in [zetesis_cpu::Stop::Cancelled, zetesis_cpu::Stop::Deadline] {
        for boundary in [2, 3] {
            settled_read(&ordinary.runtime, stop, boundary);
            context.check_health().unwrap();
            assert!(ordinary.context().same_instance(context));
            assert!(formula.context().same_instance(context));
            let checks = ordinary
                .check_batch(&ground, &seeds, GpuLimits::default())
                .unwrap();
            assert_eq!(checks.len(), 1);
            assert!(checks[0].accepted());
            let checks = formula
                .propagate_batch(&theory, &candidates, FormulaLimits::default())
                .unwrap();
            assert_eq!(checks.len(), 1);
            assert_eq!(checks[0].verdict(), FormulaVerdict::NoProperSubset);
        }
    }
}

fn settled_read(runtime: &crate::runtime::Runtime, stop: zetesis_cpu::Stop, boundary: usize) {
    // A real submission/map is settled before deterministic control injection.
    // Boundary 2 is after polling, boundary 3 after decoding. No clock race or
    // test-only production hook decides when a shader is likely to have ended.
    let _lease = runtime.context.lease().unwrap();
    let scopes = crate::runtime::ErrorScopes::new(runtime.device());
    let source = crate::runtime::initialized(
        runtime.device(),
        "settled stop source",
        &[42],
        wgpu::BufferUsages::COPY_SRC,
    );
    let readback = crate::runtime::buffer(
        runtime.device(),
        "settled stop readback",
        4,
        wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
    );
    let mut encoder = runtime
        .device()
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("settled stop copy"),
        });
    encoder.copy_buffer_to_buffer(&source, 0, &readback, 0, 4);
    let submission = runtime.queue().submit([encoder.finish()]);
    let mut controls = 0;
    let mut decoded = false;
    let result = crate::runtime::read_polled(
        runtime.device(),
        &readback,
        submission,
        std::time::Duration::from_secs(10),
        || {
            controls += 1;
            if controls == boundary {
                Err(crate::GpuError::interrupted(stop))
            } else {
                Ok(())
            }
        },
        |words| {
            assert_eq!(words, &[42]);
            decoded = true;
            Ok(())
        },
    );
    assert_eq!(decoded, boundary == 3);
    assert!(!result.is_ok());
    let error = runtime.complete(scopes, Ok(result)).unwrap_err();
    assert_eq!(error.interruption(), Some(stop));
    assert_eq!(error.kind(), GpuErrorKind::Interrupted);
    runtime.check_health().unwrap();
    println!(
        "settled readback stop={stop:?} boundary={boundary} decoded={decoded} context=reusable"
    );
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
