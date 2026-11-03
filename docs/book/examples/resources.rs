//! Reuse one device across independent formula sessions.

// ANCHOR: example
use zetesis_algebra::Cancellation;
use zetesis_core::{Atom, Model, Predicate};
use zetesis_solve::{
    AnswerSelection, Backend, Completion, ExecutionResources, Original, PreparedInput, Session,
    SolveConfig,
};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};
use zetesis_wgpu::{GpuContext, GpuOptions, GpuSelection};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = GpuContext::new_selected(GpuOptions::default(), GpuSelection::default())?;
    let resources = ExecutionResources::with_gpu(&context);
    // The resource owner retains the device; it is independent of this handle.
    drop(context);

    for name in ["ready", "available"] {
        let program = admit_formula(
            format!("{name}."),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )?;
        let config = SolveConfig {
            backend: Backend::Gpu(None),
            models: 0,
            ..Default::default()
        };
        let mut session = Session::builder(
            PreparedInput::formula(&program),
            config,
            Cancellation::default(),
        )
        .selection(AnswerSelection::All)
        .resources(&resources)
        .start()?;
        let answer = session.next().unwrap()?;
        assert_eq!(
            answer.interpretation(),
            &Model::new([Atom::new(Predicate::new(name, 0)?, vec![])?])?
        );
        assert!(
            answer
                .original()
                .same_instance(&Original::Theory(program.theory().clone()))
        );
        assert!(session.next().is_none());
        assert_eq!(
            session.outcome().unwrap().completion(),
            Some(Completion::Exhausted)
        );
    }
    Ok(())
}
// ANCHOR_END: example
