//! Public renderers consume semantic views without CLI parsing or output decoding.

use std::{io, num::NonZeroUsize};

use zetesis_cli::{
    AnswerRenderer, AnswerView, Backend, ColorMode, Completion, HumanRenderer, JsonRenderer,
    PreparedInput, PublicationConfig, PublicationOutcome, PublicationView, RunError,
    SemanticOutcome, SummaryDelivery, publish_prepared,
};
use zetesis_core::{Atom, Model};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_themelios::observation::Symbol;
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

fn config() -> PublicationConfig {
    let mut config = PublicationConfig::default();
    config.solve.backend = Backend::Cpu;
    config.solve.workers = NonZeroUsize::MIN;
    config.solve.models = 0;
    config
}

#[derive(Debug, PartialEq, Eq)]
struct Seen {
    number: usize,
    model: Model,
    shown: Vec<Atom>,
    terms: Vec<Symbol>,
    costs: Option<Vec<(i32, i64)>>,
}

#[derive(Default)]
struct Inspect {
    answers: Vec<Seen>,
    final_semantic: Option<SemanticOutcome>,
}

impl AnswerRenderer for Inspect {
    fn answer(&mut self, view: AnswerView<'_>, _: &Cancellation) -> Result<(), RunError> {
        self.answers.push(Seen {
            number: view.number(),
            model: view.model().model().clone(),
            shown: view.model().shown_atoms().cloned().collect(),
            terms: view.model().shown_terms().to_vec(),
            costs: view.model().score().map(|score| score.costs().to_vec()),
        });
        Ok(())
    }

    fn finish(&mut self, view: PublicationView<'_>) -> Result<SummaryDelivery, RunError> {
        self.final_semantic = view.semantic().cloned();
        Ok(SummaryDelivery::Accepted)
    }
}

fn publish(
    source: &str,
    renderer: &mut impl AnswerRenderer,
) -> Result<PublicationOutcome, zetesis_cli::PublicationFailure> {
    let admitted = admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    publish_prepared(
        PreparedInput::formula(&admitted),
        &config(),
        renderer,
        &mut io::sink(),
        &Cancellation::default(),
    )
}

#[test]
fn custom_view_retains_independent_observation_channels() {
    let mut renderer = Inspect::default();
    let outcome = publish(
        "hidden. p(1). #show p/1. #show p(1). #minimize{2@3,k:hidden}.",
        &mut renderer,
    )
    .unwrap();
    assert_eq!(renderer.answers.len(), 1);
    let answer = &renderer.answers[0];
    assert_eq!(answer.number, 1);
    assert_eq!(answer.model.atoms().len(), 2);
    assert_eq!(answer.shown.len(), 1);
    assert_eq!(answer.shown[0].predicate().name(), "p");
    let [
        Symbol::Function {
            name, arguments, ..
        },
    ] = answer.terms.as_slice()
    else {
        panic!("the typed term channel contains p(1)")
    };
    assert_eq!(name.as_str(), "p");
    assert_eq!(arguments.as_slice(), &[Symbol::Number(1)]);
    assert_eq!(answer.costs.as_deref(), Some([(3, 2)].as_slice()));
    assert!(outcome.semantic().optimum_proved());
    assert_eq!(
        renderer.final_semantic.unwrap().completion(),
        Some(Completion::Exhausted)
    );
}

#[test]
fn equal_displays_do_not_merge_custom_answer_records() {
    let mut renderer = Inspect::default();
    let outcome = publish("{p;q}. #show.", &mut renderer).unwrap();
    assert_eq!(outcome.publication().models(), 4);
    assert_eq!(renderer.answers.len(), 4);
    assert!(
        renderer
            .answers
            .iter()
            .all(|answer| answer.shown.is_empty() && answer.terms.is_empty())
    );
    let mut sizes: Vec<_> = renderer
        .answers
        .iter()
        .map(|answer| answer.model.atoms().len())
        .collect();
    sizes.sort_unstable();
    assert_eq!(sizes, [0, 1, 1, 2]);
}

struct InspectRenderer<R> {
    renderer: R,
    observed: Inspect,
}
impl<R: AnswerRenderer> AnswerRenderer for InspectRenderer<R> {
    fn begin(&mut self) -> Result<(), RunError> {
        self.renderer.begin()
    }
    fn summary_stage(&self) -> zetesis_cli::SummaryStage {
        self.renderer.summary_stage()
    }
    fn answer(
        &mut self,
        view: AnswerView<'_>,
        cancellation: &Cancellation,
    ) -> Result<(), RunError> {
        // The same borrowed view is observed and rendered; no second evaluation.
        self.observed.answer(view, cancellation)?;
        self.renderer.answer(view, cancellation)
    }
    fn finish(&mut self, view: PublicationView<'_>) -> Result<SummaryDelivery, RunError> {
        self.renderer.finish(view)
    }
}

#[test]
fn default_renderers_consume_the_same_typed_views() {
    let mut human = InspectRenderer {
        renderer: HumanRenderer::new(Vec::new(), ColorMode::Never, 8192),
        observed: Inspect::default(),
    };
    let mut json = InspectRenderer {
        renderer: JsonRenderer::new(Vec::new(), 8192, 1024),
        observed: Inspect::default(),
    };
    let source = "{p;q}. #show p/0. #show f(1):p.";
    publish(source, &mut human).unwrap();
    publish(source, &mut json).unwrap();
    assert_eq!(human.observed.answers, json.observed.answers);
    assert!(!human.renderer.into_inner().is_empty());
    let encoded: serde_json::Value = serde_json::from_slice(&json.renderer.into_inner()).unwrap();
    assert_eq!(encoded["models"].as_array().unwrap().len(), 4);
}

struct RefuseAnswer(Stop);
impl AnswerRenderer for RefuseAnswer {
    fn answer(&mut self, _: AnswerView<'_>, _: &Cancellation) -> Result<(), RunError> {
        Err(RunError::PublicationStopped(self.0))
    }
    fn finish(&mut self, _: PublicationView<'_>) -> Result<SummaryDelivery, RunError> {
        Ok(SummaryDelivery::Accepted)
    }
}

#[test]
fn custom_publication_stop_retains_the_proved_optimum() {
    let outcome = publish("p. #minimize{1:p}.", &mut RefuseAnswer(Stop::Cancelled)).unwrap();
    assert!(matches!(outcome, PublicationOutcome::Stopped(_)));
    assert!(outcome.semantic().optimum_proved());
    assert_eq!(outcome.semantic().completion(), Some(Completion::Exhausted));
    assert_eq!(outcome.publication().models(), 0);
}

struct FailAfterOne(usize);
impl AnswerRenderer for FailAfterOne {
    fn answer(&mut self, _: AnswerView<'_>, _: &Cancellation) -> Result<(), RunError> {
        self.0 += 1;
        if self.0 == 2 {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "custom sink closed").into());
        }
        Ok(())
    }
    fn finish(&mut self, _: PublicationView<'_>) -> Result<SummaryDelivery, RunError> {
        Ok(SummaryDelivery::Omitted)
    }
}

#[test]
fn custom_writer_failure_acknowledges_only_complete_records() {
    let failure = publish("{p;q}.", &mut FailAfterOne(0)).unwrap_err();
    assert!(
        matches!(*failure.cause, RunError::Output(ref error) if error.kind()==io::ErrorKind::BrokenPipe)
    );
    assert_eq!(failure.publication().unwrap().models(), 1);
    assert!(failure.semantic().unwrap().verified_models() >= 2);
    assert!(!failure.semantic().unwrap().unsatisfiable());
}

#[test]
fn each_json_document_owns_its_atom_indices() {
    let mut renderer = JsonRenderer::new(Vec::new(), 8192, 1024);
    publish("p.", &mut renderer).unwrap();
    publish("p.", &mut renderer).unwrap();
    let bytes = renderer.into_inner();
    let documents: Vec<serde_json::Value> = serde_json::Deserializer::from_slice(&bytes)
        .into_iter()
        .map(Result::unwrap)
        .collect();
    assert_eq!(documents.len(), 2);
    for document in documents {
        assert_eq!(
            document["models"][0]["model"]["atoms"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            document["models"][0]["model"]["full_model"],
            serde_json::json!([0])
        );
    }
}

#[test]
fn atom_only_human_view_needs_no_observation_work() {
    let admitted = admit_formula(
        "p.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let mut configured = config();
    configured.observations.max_work = 0;
    let mut renderer = HumanRenderer::new(Vec::new(), ColorMode::Never, 8192);
    let outcome = publish_prepared(
        PreparedInput::formula(&admitted),
        &configured,
        &mut renderer,
        &mut io::sink(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(outcome.publication().models(), 1);
    assert!(renderer.into_inner().starts_with(b"Answer: 1\np\n"));
}

#[test]
fn renderer_state_cannot_reschedule_terminal_publication() {
    struct ChangingStage {
        finishes: usize,
    }
    impl AnswerRenderer for ChangingStage {
        fn summary_stage(&self) -> zetesis_cli::SummaryStage {
            if self.finishes == 0 {
                zetesis_cli::SummaryStage::SearchFinished
            } else {
                zetesis_cli::SummaryStage::Finalized
            }
        }
        fn answer(&mut self, _: AnswerView<'_>, _: &Cancellation) -> Result<(), RunError> {
            Ok(())
        }
        fn finish(&mut self, _: PublicationView<'_>) -> Result<SummaryDelivery, RunError> {
            self.finishes += 1;
            Ok(SummaryDelivery::Accepted)
        }
    }
    let mut renderer = ChangingStage { finishes: 0 };
    publish("p.", &mut renderer).unwrap();
    assert_eq!(renderer.finishes, 1);
}
