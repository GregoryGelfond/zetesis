//! Replace answer presentation without parsing arguments or collecting a family.

// ANCHOR: example
use std::{io, num::NonZeroUsize};
use zetesis_cli::{
    AnswerRenderer, AnswerView, Backend, Completion, PreparedInput, PublicationConfig,
    PublicationView, RunError, SummaryDelivery, publish_prepared,
};
use zetesis_cpu::Cancellation;
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

#[derive(Default)]
struct CountViews {
    records: usize,
    completion: Option<Completion>,
}

impl AnswerRenderer for CountViews {
    fn answer(&mut self, answer: AnswerView<'_>, _: &Cancellation) -> Result<(), RunError> {
        // Full identity includes hidden; #show selects a separate atom channel
        // and evaluates item(X) into a separate typed term channel.
        let view = answer.model();
        assert_eq!(view.model().atoms().len(), 2);
        assert!(
            view.model()
                .atoms()
                .iter()
                .any(|atom| atom.predicate().name() == "hidden")
        );
        assert_eq!(view.shown_atoms().count(), 1);
        assert_eq!(view.shown_terms().len(), 1);
        assert!(view.score().is_none());
        self.records = answer.number();
        Ok(())
    }

    fn finish(&mut self, result: PublicationView<'_>) -> Result<SummaryDelivery, RunError> {
        self.completion = result
            .semantic()
            .and_then(zetesis_cli::SemanticOutcome::completion);
        // This consumer accepts typed evidence; it need not write a text record.
        Ok(SummaryDelivery::Accepted)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = admit_formula(
        "1{p(1);p(2)}1. hidden. #show p/1. #show item(X):p(X).".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )?;
    let config = PublicationConfig {
        solve: zetesis_cli::SolveConfig {
            backend: Backend::Cpu,
            workers: NonZeroUsize::MIN,
            models: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut renderer = CountViews::default();
    let outcome = publish_prepared(
        PreparedInput::formula(&input),
        &config,
        &mut renderer,
        &mut io::sink(),
        &Cancellation::default(),
    )?;
    assert_eq!(renderer.records, 2);
    assert_eq!(renderer.completion, Some(Completion::Exhausted));
    assert_eq!(outcome.publication().models(), 2);
    assert!(outcome.publication().summary());
    Ok(())
}
// ANCHOR_END: example
