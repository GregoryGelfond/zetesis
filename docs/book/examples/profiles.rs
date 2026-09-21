//! Reuse an immutable compiled primitive while collecting independent families.

// ANCHOR: example
use zetesis_cpu::Cancellation;
use zetesis_solve::{
    Backend, ExecutionResources, Grounder, Oracle, PreparedInput, Session, SolveConfig,
    WorldViewLimits,
};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};
use zetesis_wgpu::{GpuFormulaProfile, GpuOptions, GpuSelection};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let profile = GpuFormulaProfile::new_selected(GpuOptions::default(), GpuSelection::default())?;
    let resources = ExecutionResources::with_formula_profile(&profile);
    let config = SolveConfig {
        backend: Backend::Gpu,
        grounder: Grounder::Eager,
        oracle: Oracle::Countermodel,
        models: 0,
        ..Default::default()
    };
    for (source, count) in [("a | b.", 2), ("{a; b}. :~ a. [1@0]", 4)] {
        let admitted = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )?;
        let family = Session::builder(
            PreparedInput::formula(&admitted),
            config,
            Cancellation::default(),
        )
        .resources(&resources)
        .collect(WorldViewLimits::default())?;
        assert_eq!(family.len(), count);
    }
    Ok(())
}
// ANCHOR_END: example
