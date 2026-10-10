//! Rejected core proposals cannot establish an incumbent or restrict candidates.

use super::{FormulaSession, Input};
use crate::execution_observation::{Ignore, Observer};
use crate::formula_execution::{Failure, MembershipExecution};
use crate::phase_timing::Recorder;
use crate::{
    AnswerSelection, Backend, ExecutionObservation, ExecutionObserver, FormulaExecutionStatistics,
    SearchMethod, SolveConfig,
};
use std::convert::Infallible;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::Interpretation;
use zetesis_sat::{Incomplete, StableModels};
use zetesis_themelios::{
    AdmissionOptions, ConstraintCheckLimits, ConstraintVerdict, ExpansionLimits, FormulaLimits,
    prepare_formula,
};

/// A fixed order of independently checked core interpretations. Ending this
/// controlled prefix reports an interruption, never fabricated exhaustion.
struct OrderedAnswers(std::vec::IntoIter<Interpretation>);

impl MembershipExecution for OrderedAnswers {
    fn next(
        &mut self,
        _: &mut StableModels,
        _: &SolveConfig,
        _: &Cancellation,
        _: &Recorder,
    ) -> Option<Result<Interpretation, Failure>> {
        Some(self.0.next().ok_or(Failure::Search(Incomplete::WorkLimit)))
    }

    fn statistics(&self, _: &StableModels) -> Option<FormulaExecutionStatistics> {
        None
    }
}

#[derive(Default)]
struct BoundScores(Vec<Vec<(i32, i64)>>);

impl ExecutionObserver for BoundScores {
    type Error = Infallible;

    fn observe(&mut self, observation: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        if let ExecutionObservation::ObjectiveBound { costs, .. } = observation {
            self.0.push(costs.to_vec());
        }
        Ok(())
    }
}

#[test]
fn a_rejected_cheap_proposal_cannot_tighten_the_bound() {
    let owner = prepare_formula(
        include_str!("../../../tests/fixtures/hybrid-objectives/cheap-invalid.lp").into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_hybrid()
    .unwrap();
    let p = owner
        .atom_catalog()
        .atoms()
        .iter()
        .position(|atom| atom.predicate().name() == "p")
        .unwrap();
    let cancellation = Cancellation::default();
    // Put the invalid zero-cost core answer first, regardless of native search
    // ordering. Both proposals have real reference core-membership decisions.
    let proposals = vec![
        Interpretation::new(owner.core_theory(), []).unwrap(),
        Interpretation::new(owner.core_theory(), [p]).unwrap(),
    ];
    for proposal in &proposals {
        assert!(
            zetesis_ferraris::check(
                owner.core_theory(),
                proposal,
                zetesis_ferraris::Limits::default(),
                &cancellation,
            )
            .unwrap()
            .accepted(),
        );
    }
    let config = SolveConfig {
        backend: Backend::Cpu,
        search: SearchMethod::Clauses,
        workers: std::num::NonZeroUsize::MIN,
        models: 0,
        ..SolveConfig::default()
    };
    let phases = Recorder::new(false);
    let mut session = FormulaSession::with_selection(
        Input {
            theory: owner.core_theory(),
            atoms: owner.atom_catalog(),
            objectives: owner.objectives(),
            required_choices: owner.core().required_choices(),
            gate_atoms: 0,
            keyed_constraints: 0,
            key_analysis: zetesis_themelios::KeyAnalysis::Complete,
            certificate_order: zetesis_sat::CertificateOrder::TightFirst,
        },
        OrderedAnswers(proposals.into_iter()),
        &config,
        &mut Ignore,
        &cancellation,
        &phases,
        AnswerSelection::Optimal,
    );
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut rejected = 0;
    let mut observed = BoundScores::default();
    let answer = session
        .next_with_acceptance(
            &config,
            &mut Observer(&mut observed),
            &cancellation,
            &phases,
            &mut |model| {
                let accepted = matches!(
                    checker.check(model, &cancellation).unwrap(),
                    ConstraintVerdict::Satisfied
                );
                rejected += usize::from(!accepted);
                Ok(accepted)
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(rejected, 1);
    assert_eq!(answer.1.unwrap().costs(), [(0, 1)]);
    assert_eq!(observed.0, [vec![(0, 1)]]);
    assert_eq!(session.outcome(&phases).scored_models(), 1);
    assert_eq!(session.outcome(&phases).retained_models(), 1);
    assert!(session.next_retained().is_none());
}
