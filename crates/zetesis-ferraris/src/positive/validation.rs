//! Exact original truth after positive closure, under the same finite budget.

use std::mem::size_of;

use super::{Budget, PositiveError, PositivePlan, PositiveResource};
use crate::{EvaluationError, EvaluationLimits, EvaluationWorkspace, Interpretation, NodeView};
use zetesis_cpu::Stop;

pub(super) fn complete(
    least: &Interpretation,
    budget: &mut Budget<'_>,
) -> Result<Option<usize>, PositiveError> {
    budget.cancellation.poll()?;
    // The CSR and its construction vectors have been dropped. Only the final
    // plan header and its interpretation words overlap this evaluation owner.
    let base = size_of::<PositivePlan>() as u128
        + least.words.capacity() as u128 * size_of::<u64>() as u128;
    let mut workspace = EvaluationWorkspace::default();
    budget.observe_bytes(base + workspace.retained_bytes())?;
    let available = usize::try_from(budget.limits.max_bytes as u128 - base)
        .map_err(|_| PositiveError::Overflow)?;
    let attempt = workspace.evaluate(
        least,
        EvaluationLimits {
            max_work: budget.limits.max_work - budget.statistics.work,
            max_bytes: available,
        },
        budget.cancellation,
    );
    // Evaluation admits at most its supplied remaining work. Preserve its
    // original error even if observed allocation slack also exceeds the cap.
    budget.statistics.work = budget
        .statistics
        .work
        .checked_add(attempt.work)
        .ok_or(PositiveError::Overflow)?;
    let capacity = budget.observe_bytes(base + attempt.retained_bytes);
    let truth = attempt.result.map_err(|error| match error {
        EvaluationError::Stopped(Stop::WorkLimit) => PositiveError::Limit {
            resource: PositiveResource::Work,
            observed: u128::from(budget.statistics.work) + 1,
            limit: u128::from(budget.limits.max_work),
        },
        EvaluationError::Stopped(stop) => PositiveError::Stopped(stop),
        EvaluationError::Storage { required, .. } => PositiveError::Limit {
            resource: PositiveResource::Bytes,
            observed: base + required,
            limit: budget.limits.max_bytes as u128,
        },
    })?;
    capacity?;
    let failed = truth.failed_root();
    if let Some(root) = failed {
        let constraint = match least
            .theory()
            .view()
            .node(root)
            .map_err(|_| Stop::InvalidProgram)?
        {
            NodeView::False => true,
            NodeView::Implies(_, head) => {
                least
                    .theory()
                    .view()
                    .node(head)
                    .map_err(|_| Stop::InvalidProgram)?
                    == NodeView::False
            }
            NodeView::Atom(_) | NodeView::And(..) | NodeView::Or(..) => false,
        };
        if !constraint {
            return Err(PositiveError::InvalidClosure { root });
        }
    }
    budget.cancellation.poll()?;
    Ok(failed)
}
