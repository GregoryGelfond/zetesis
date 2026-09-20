//! A bounded external reference executor using only public library operations.

// ANCHOR: example
use std::convert::Infallible;

use zetesis_cpu::{Control, Stop};
use zetesis_ferraris::{Limits, Theory, Verdict};
use zetesis_solve::{
    BatchExecutor, BatchResult, BatchVerdict, CandidateBatch, ExecutorCapabilities, ExecutorError,
    ExecutorFailure, MembershipPlan,
};

struct ReferenceExecutor {
    theory: Option<Theory>,
    limits: Limits,
}

impl BatchExecutor for ReferenceExecutor {
    type Error = Infallible;

    fn capabilities(&self) -> ExecutorCapabilities {
        ExecutorCapabilities::General
    }

    fn prepare(
        &mut self,
        plan: MembershipPlan<'_>,
        control: &Control,
    ) -> Result<(), ExecutorFailure<Self::Error>> {
        control.poll()?;
        let MembershipPlan::General(theory) = plan else {
            return Err(ExecutorFailure::Unsupported);
        };
        self.theory = Some(theory.clone());
        Ok(())
    }

    fn check<'a>(
        &mut self,
        batch: CandidateBatch<'a>,
        control: &Control,
    ) -> Result<BatchResult<'a>, ExecutorFailure<Self::Error>> {
        if !self
            .theory
            .as_ref()
            .is_some_and(|theory| theory.same_instance(batch.theory()))
        {
            return Err(ExecutorError::ForeignBatch.into());
        }
        let mut verdicts = Vec::new();
        verdicts
            .try_reserve_exact(batch.candidates().len())
            .map_err(|_| Stop::Allocation)?;
        for candidate in batch.candidates() {
            let check = zetesis_ferraris::check(batch.theory(), candidate, self.limits, control)?;
            verdicts.push(match check.verdict() {
                Verdict::Stable => BatchVerdict::NoProperSubset,
                Verdict::NonMinimal { .. } => BatchVerdict::Refuted,
                Verdict::NotModel { .. } => BatchVerdict::NotModel,
            });
        }
        Ok(batch.finish(verdicts)?)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use zetesis_solve::{Completion, PreparedInput, Session, SolveConfig, WorldViewLimits};
    use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

    let admitted = admit_formula(
        "{a;b}.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )?;
    let executor = ReferenceExecutor {
        theory: None,
        limits: Limits {
            max_work: 1_000_000,
            max_subsets: 1_024,
        },
    };
    let family = Session::builder(
        PreparedInput::formula(&admitted),
        SolveConfig {
            models: 0,
            ..Default::default()
        },
        Control::default(),
    )
    .executor(executor)
    .collect(WorldViewLimits::default())?;
    assert_eq!(family.len(), 4);
    assert_eq!(family.outcome().completion(), Some(Completion::Exhausted));
    let receipt = family.outcome().batch_execution().unwrap();
    assert_eq!(receipt.batches.committed, 4);
    assert_eq!(receipt.batches.pending, 0);
    Ok(())
}
// ANCHOR_END: example
