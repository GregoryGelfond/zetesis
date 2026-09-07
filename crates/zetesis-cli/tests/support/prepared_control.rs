//! Cancellation after admission must retain the already established subject.

use clap::Parser;
use zetesis_cpu::Control;
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

#[test]
fn stopped_formula_adapter_retains_admitted_subject() {
    let admitted = admit_formula(
        "a | b.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let control = Control::default();
    control.cancel();
    let options = crate::Options::try_parse_from(["zetesis", "--backend", "cpu"]).unwrap();
    let progress = super::run_formula(
        super::Input {
            theory: admitted.theory(),
            atoms: admitted.atoms(),
            objectives: admitted.objectives(),
            observations: admitted.metadata().observations(),
            gate_atoms: 0,
        },
        admitted.metadata().output(),
        &options,
        &mut std::io::sink(),
        &mut crate::presentation::Diagnostics::new(std::io::sink(), crate::ColorMode::Never),
        &control,
        &crate::phase_timing::Recorder::new(false),
    )
    .unwrap();
    let semantic = progress.semantic.unwrap();
    let crate::Subject::Theory(theory) = semantic.subject().unwrap() else {
        panic!("original theory")
    };
    assert!(theory.same_instance(admitted.theory()));
    assert_eq!(semantic.completion(), Some(crate::Completion::Interrupted));
    assert_eq!(semantic.verified_models(), 0);
}
