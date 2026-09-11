//! Share one optional host-measurement scope across admission and solving.

// ANCHOR: example
use zetesis_cpu::Control;
use zetesis_solve::{
    Backend, PreparedInput, Session, SolveConfig, SolveMeasurements, SolvePhase, WorldViewLimits,
};
use zetesis_themelios::{AdmissionOptions, admit};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let measurements = SolveMeasurements::new(true);
    let admitted = measurements.measure(SolvePhase::AdmissionMaterialization, || {
        admit(
            "a :- not b. b :- not a.".into(),
            AdmissionOptions::default(),
        )
    })?;
    let config = SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        ..SolveConfig::default()
    };
    let family = Session::builder(
        PreparedInput::admitted(&admitted),
        config,
        Control::default(),
    )
    .measurements(&measurements)
    .collect(WorldViewLimits::default())?;
    assert_eq!(family.len(), 2);
    assert!(measurements.snapshot().is_some());
    assert!(SolveMeasurements::new(false).snapshot().is_none());
    Ok(())
}
// ANCHOR_END: example
