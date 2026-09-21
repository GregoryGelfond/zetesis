//! Compare objective selection, original answers and an incomplete collection.

// ANCHOR: example
use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use zetesis_core::{Atom, Model, Predicate};
use zetesis_cpu::Cancellation;
use zetesis_solve::{
    AnswerSelection, Backend, Completion, PreparedInput, Session, SolveConfig, WorldView,
    WorldViewError, WorldViewFailureParts, WorldViewLimits,
};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

fn check_optimum(
    input: PreparedInput<'_>,
    config: SolveConfig,
    expected: Atom,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut selected = Session::new(input, config, Cancellation::default())?;
    let optimum = selected.by_ref().collect::<Result<Vec<_>, _>>()?;
    assert_eq!(optimum.len(), 1);
    assert_eq!(optimum[0].interpretation(), &Model::new([expected]));
    assert_eq!(
        optimum[0].score().expect("active objective").costs(),
        &[(2, 1)]
    );
    let outcome = selected.outcome().expect("finished selection");
    assert_eq!(outcome.selection(), Some(AnswerSelection::Optimal));
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    assert!(outcome.optimum_proved());
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let owner = admit_formula(
        "1{a;b}1. #minimize{1@2,a:a;2@2,b:b}. #show.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )?;
    let input = PreparedInput::formula(&owner);
    let config = SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        batch_size: NonZeroUsize::MIN,
        workers: NonZeroUsize::MIN,
        completion_workers: NonZeroUsize::MIN,
        ..SolveConfig::default()
    };
    let a = Atom::new(Predicate::new("a", 0)?, vec![])?;
    let b = Atom::new(Predicate::new("b", 0)?, vec![])?;
    assert!(!owner.metadata().output().includes(&a));
    assert!(!owner.metadata().output().includes(&b));

    check_optimum(input, config, a.clone())?;

    let mut all = Session::enumerate(input, config, Cancellation::default())?;
    let streamed = all.by_ref().collect::<Result<Vec<_>, _>>()?;
    let full_answers = streamed
        .iter()
        .map(|answer| {
            (
                answer.interpretation().clone(),
                answer.score().expect("active objective").costs().to_vec(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        full_answers,
        BTreeSet::from([
            (Model::new([a]), vec![(2, 1)]),
            (Model::new([b]), vec![(2, 2)]),
        ])
    );
    let outcome = all.outcome().expect("finished enumeration");
    assert_eq!(outcome.selection(), Some(AnswerSelection::All));
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    assert!(!outcome.optimum_proved());

    let family = WorldView::collect(
        input,
        config,
        WorldViewLimits::default(),
        Cancellation::default(),
    )?;
    assert_eq!(family.len(), 2);
    assert_eq!(family.outcome().selection(), Some(AnswerSelection::All));
    assert_eq!(family.outcome().completion(), Some(Completion::Exhausted));
    let collected = family
        .answer_sets()
        .iter()
        .map(|answer| {
            (
                answer.interpretation().clone(),
                answer.score().expect("active objective").costs().to_vec(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(collected, full_answers);

    let failure = WorldView::collect(
        input,
        config,
        WorldViewLimits {
            max_answer_sets: 1,
            ..WorldViewLimits::default()
        },
        Cancellation::default(),
    )
    .expect_err("the original family has two answers");
    let WorldViewFailureParts {
        cause,
        subject,
        answer_sets,
        outcome,
    } = failure.into_parts();
    assert!(matches!(cause, WorldViewError::AnswerSets));
    assert_eq!(answer_sets.len(), 1);
    let retained = &answer_sets[0];
    assert!(full_answers.contains(&(
        retained.interpretation().clone(),
        retained.score().expect("active objective").costs().to_vec(),
    )));
    assert!(retained.subject().same_instance(&subject));
    let outcome = outcome.expect("search started before collection failed");
    assert_eq!(outcome.selection(), Some(AnswerSelection::All));
    assert_eq!(outcome.verified_models(), 2);
    assert_eq!(outcome.completion(), None);
    assert!(!outcome.unsatisfiable());
    Ok(())
}
// ANCHOR_END: example
