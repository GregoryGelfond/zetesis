//! Reuse one device across independent native-program sessions.

// ANCHOR: example
use zetesis_core::{AdmissionLimits, Atom, AtomPattern, Model, Predicate, Program, Template};
use zetesis_cpu::Cancellation;
use zetesis_solve::{
    AnswerSelection, Backend, Completion, ExecutionResources, Grounder, PreparedInput, Session,
    SolveConfig, Subject,
};
use zetesis_wgpu::{GpuContext, GpuOptions, GpuSelection};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = GpuContext::new_selected(GpuOptions::default(), GpuSelection::default())?;
    let resources = ExecutionResources::with_gpu(&context);
    // The resource owner retains the device; it is independent of this handle.
    drop(context);

    for name in ["ready", "available"] {
        let predicate = Predicate::new(name, 0)?;
        let fact = Template::new(
            Some(AtomPattern::new(predicate.clone(), vec![])?),
            vec![],
            vec![],
            vec![],
            vec![],
        );
        let program = Program::new(vec![fact], AdmissionLimits::default())?;
        let config = SolveConfig {
            backend: Backend::Gpu,
            grounder: Grounder::Lazy,
            models: 0,
            ..Default::default()
        };
        let mut session = Session::builder(
            PreparedInput::program(&program),
            config,
            Cancellation::default(),
        )
        .selection(AnswerSelection::All)
        .resources(&resources)
        .start()?;
        let answer = session.next().unwrap()?;
        assert_eq!(
            answer.interpretation(),
            &Model::new([Atom::new(predicate, vec![])?])
        );
        assert!(
            answer
                .subject()
                .same_instance(&Subject::Program(program.clone()))
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
