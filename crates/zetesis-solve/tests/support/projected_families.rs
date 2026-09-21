//! Fixed keys select full members of the independently completed original family.

use std::collections::BTreeSet;
use zetesis_cpu::Cancellation;
use zetesis_solve::{
    AnswerSelection, Backend, Completion, ExecutionResources, PreparedInput, ProjectionLimits,
    Session, SolveConfig, WorldViewLimits,
};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

pub(super) fn check(
    selection: AnswerSelection,
    config: SolveConfig,
    resources: &ExecutionResources,
) {
    // The fixed facts place the changing bit beyond a packed-word boundary.
    let owner = admit_formula(
        "d(1..70).{p;q}.#project d/1.#project p:not absent.#minimize{1@1:not q}.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(owner.projection().atoms().len(), 71);
    let full = Session::builder(
        PreparedInput::formula(&owner),
        SolveConfig {
            backend: Backend::Cpu,
            ..config
        },
        Cancellation::default(),
    )
    .collect(WorldViewLimits::default())
    .unwrap();
    assert_eq!(full.len(), 4);
    let selected: Vec<_> = full
        .answer_sets()
        .iter()
        .filter(|answer| {
            selection == AnswerSelection::All || answer.score().unwrap().costs() == [(1, 0)]
        })
        .collect();
    let key = |answer: &zetesis_solve::AnswerSet| {
        owner
            .projection()
            .atoms()
            .iter()
            .map(|atom| answer.interpretation().contains(atom))
            .collect::<Vec<_>>()
    };
    let expected: BTreeSet<_> = selected.iter().map(|answer| key(answer)).collect();
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        config,
        Cancellation::default(),
    )
    .resources(resources)
    .selection(selection)
    .projected(ProjectionLimits::default())
    .start()
    .unwrap();
    let mut actual = BTreeSet::new();
    for answer in session.by_ref() {
        let answer = answer.unwrap();
        assert!(selected.iter().any(|original| original.interpretation()
            == answer.interpretation()
            && original.score() == answer.score()));
        assert!(actual.insert(key(&answer)));
    }
    assert_eq!(actual, expected);
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    let projection = outcome.projection().unwrap();
    assert!(projection.complete);
    assert_eq!(projection.representatives, expected.len());
    assert_eq!(
        usize::try_from(projection.duplicates).unwrap(),
        selected.len() - expected.len()
    );
    if config.backend == Backend::Metal {
        let execution = outcome.formula_execution().unwrap();
        assert!(execution.adapter.contains("Metal"));
        assert!(execution.gpu_candidates > 0);
        assert!(execution.gpu_work > 0);
        assert_eq!(
            execution.gpu_decided + execution.cpu_residuals,
            execution.gpu_candidates
        );
        assert_eq!(
            (execution.pending_candidates, execution.queued_models),
            (0, 0)
        );
    }
}
