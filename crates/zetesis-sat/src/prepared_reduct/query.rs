//! Per-worker parameter, truth and search ownership for one immutable query.

use std::{
    mem::size_of,
    sync::{Arc, Weak},
};

use zetesis_ferraris::{
    EvaluationError, EvaluationLimits, EvaluationWorkspace, FormulaEvaluation, Interpretation,
};

use super::{PreparedReduct, bound};
use crate::timing::{self, Phase};
use crate::{
    Check, Control, Incomplete, Limits, Literal, SearchStatistics, Statistics, ferraris, search,
};
use search::{Budget, LocalQuota, Quota, increment};

/// Cumulative actual persistent-reduct activity within an enumeration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReductStatistics {
    /// One attempted cold construction, including a failed prefix. Absence means
    /// no residual has requested a prepared owner. Work is a subset of SAT work.
    pub preparation: Option<super::ReductPreparationStatistics>,
    /// Actual original-node/root evaluation work of entered prepared queries.
    /// Proposal and independent returned-witness evaluations are separate calls.
    pub original_work: u64,
    /// Parameter-copy work, included in cumulative SAT work.
    pub parameter_work: u64,
    /// Largest actual single-worker retained capacity observed on query return,
    /// including failure. Shared preparation and temporary witness vectors are
    /// excluded; this is not the aggregate completion peak or process RSS.
    pub peak_workspace_bytes: u128,
    /// What the proper-subset queries walked as region trees, under the
    /// regions method; zero under the clause kernel.
    pub regions: crate::RegionCounts,
}

/// One returned attempt, including work and retained storage after a refusal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReductQueryStatistics {
    /// Existing search and prepared-reduct accounting for this attempt.
    pub statistics: Statistics,
    /// Workspace header and actual retained vector capacities on return.
    /// This includes capacity grown by an incomplete attempt. Prepared query,
    /// inputs, returned witness, transient verification/output vectors, allocator
    /// metadata, allocation overlap and thread stacks are excluded.
    pub retained_bytes: u128,
}

/// Reusable worker allocations and unconditional consequences of one exact CNF.
///
/// Before each entered subset search, original truth is recomputed, parameters
/// are replaced, and candidate search truth and decisions are reset. A complete
/// watch index and the CNF's unconditional unit-propagation closure are retained
/// only for the exact prepared owner. No candidate-derived assignment is reused.
/// Workspaces can move between owners and invalidate the index on every change.
/// The weak identity retains no theory/CNF payload; its allocation control block
/// remains outside named vector capacity, like the prepared owner's Arc metadata.
#[derive(Debug, Default)]
pub struct ReductWorkspace {
    evaluation: EvaluationWorkspace,
    parameters: Vec<Literal>,
    search: search::PreparedWorkspace,
    owner: Weak<super::Data>,
}

impl ReductWorkspace {
    /// Evaluate the candidate's original truth into this workspace: the
    /// truth as `evaluate_truth` gives it, under this workspace's ceiling
    /// and with its evaluation workspace.
    pub(crate) fn evaluate<'a>(
        &'a mut self,
        candidate: &'a Interpretation,
        limits: Limits,
        control: &Control,
        statistics: &mut Statistics,
    ) -> Result<(FormulaEvaluation<'a>, u128), Incomplete> {
        let max_bytes = limits.max_reduct_bytes;
        bound(self.retained_bytes(), max_bytes)?;
        let other_bytes = self.retained_bytes() - self.evaluation.retained_bytes();
        evaluate_truth(
            &mut self.evaluation,
            other_bytes,
            max_bytes,
            candidate,
            limits,
            control,
            statistics,
        )
    }

    pub(crate) fn reserve(
        &mut self,
        prepared: &PreparedReduct,
        max_bytes: u64,
        control: &Control,
    ) -> Result<(), Incomplete> {
        control.poll()?;
        let (variables, clauses) = prepared.cnf_shape();
        let count = prepared.parameter_count();
        let evaluation = self.evaluation.retained_bytes().max(
            size_of::<EvaluationWorkspace>() as u128
                + prepared.theory().nodes().len() as u128 * size_of::<bool>() as u128,
        );
        let other = header_bytes()
            + self.parameters.capacity().max(count) as u128 * size_of::<Literal>() as u128
            + self.search.required_bytes(variables, clauses);
        bound(other + evaluation, max_bytes)?;
        let available = usize::try_from(u128::from(max_bytes) - other).unwrap_or(usize::MAX);
        self.evaluation
            .reserve(prepared.theory(), available, control)
            .map_err(|error| match error {
                EvaluationError::Stopped(stop) => Incomplete::from(stop),
                EvaluationError::Storage { required, .. } => Incomplete::ReductStorage {
                    required: other + required,
                    limit: u128::from(max_bytes),
                },
            })?;
        reserve_query(
            &mut self.search,
            &mut self.parameters,
            &prepared.0,
            header_bytes() + self.evaluation.retained_bytes(),
            max_bytes,
        )
    }

    /// Actual retained worker storage, with the exclusions in the query receipt.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        header_bytes()
            + self.evaluation.retained_bytes()
            + self.parameters.capacity() as u128 * size_of::<Literal>() as u128
            + self.search.retained_bytes()
    }
}

impl PreparedReduct {
    /// Decide original membership using this immutable proper-subset query.
    ///
    /// Original satisfaction is evaluated once and supplies authenticated truth
    /// for implication parameters. A returned subset is still independently
    /// checked against the original frozen reduct. This operation preserves
    /// `Limits`' separate search and per-verification work ceilings. Preparation
    /// work is reported only by this owner's construction receipt.
    ///
    /// `Limits::max_reduct_bytes` bounds the retained worker storage named by the receipt,
    /// including capacity from earlier calls. Requested final vector slots are
    /// checked before growth and actual capacities afterward; allocation overlap
    /// and witness/output transients are not included. Those finite transients
    /// retain the existing admitted formula/search shape bounds. This is not an
    /// allocator quota or process RSS limit. A failure may retain larger capacity
    /// but never establishes stability or publishes an unchecked witness.
    /// The prepared CNF's admission limits were applied at construction; the
    /// `admission` field of the per-query `limits` does not readmit that owner.
    #[must_use]
    pub fn check(
        &self,
        candidate: &Interpretation,
        workspace: &mut ReductWorkspace,
        limits: Limits,
        control: &Control,
    ) -> (Check, ReductQueryStatistics) {
        let mut budget = Budget {
            quota: LocalQuota,
            limits: limits.search,
            control,
            statistics: SearchStatistics::default(),
        };
        let mut receipt = ReductQueryStatistics::default();
        let result = self.check_with(
            candidate,
            workspace,
            limits,
            &mut budget,
            &mut receipt.statistics,
        );
        receipt.statistics.search = budget.statistics;
        receipt.retained_bytes = workspace.retained_bytes();
        (result.unwrap_or_else(Check::Inconclusive), receipt)
    }

    pub(crate) fn check_with(
        &self,
        candidate: &Interpretation,
        workspace: &mut ReductWorkspace,
        limits: Limits,
        budget: &mut Budget<'_, impl Quota>,
        statistics: &mut Statistics,
    ) -> Result<Check, Incomplete> {
        let result = self.check_into(candidate, workspace, limits, budget, statistics);
        statistics.reduct.peak_workspace_bytes = statistics
            .reduct
            .peak_workspace_bytes
            .max(workspace.retained_bytes());
        result
    }

    fn check_into(
        &self,
        candidate: &Interpretation,
        workspace: &mut ReductWorkspace,
        limits: Limits,
        budget: &mut Budget<'_, impl Quota>,
        statistics: &mut Statistics,
    ) -> Result<Check, Incomplete> {
        budget.control.poll()?;
        let max_bytes = limits.max_reduct_bytes;
        if !self.theory().same_instance(candidate.theory()) {
            return Err(Incomplete::WrongTheory);
        }
        bound(workspace.retained_bytes(), max_bytes)?;
        let other_bytes = workspace.retained_bytes() - workspace.evaluation.retained_bytes();
        let (truth, evaluated_bytes) = evaluate_truth(
            &mut workspace.evaluation,
            other_bytes,
            max_bytes,
            candidate,
            limits,
            budget.control,
            statistics,
        )?;
        if !truth.is_model() {
            return Ok(Check::NotModel);
        }
        let started = timing::start(statistics.phase_timings.as_ref());
        let result = (|| {
            let base = header_bytes() + evaluated_bytes;
            let prepared = &self.0;
            reserve_query(
                &mut workspace.search,
                &mut workspace.parameters,
                prepared,
                base,
                max_bytes,
            )?;
            let before = budget.statistics.work;
            let parameters = self.parameters(&truth, &mut workspace.parameters, budget);
            statistics.reduct.parameter_work = statistics
                .reduct
                .parameter_work
                .checked_add(budget.statistics.work - before)
                .ok_or(Incomplete::CounterOverflow)?;
            parameters?;
            increment(&mut statistics.countermodel_queries)?;
            let owner = Arc::downgrade(prepared);
            if !workspace.owner.ptr_eq(&owner) {
                workspace.search.invalidate();
                workspace.owner = owner;
            }
            let result = workspace.search.query(
                &prepared.cnf,
                &prepared.units,
                prepared.has_empty_clause,
                &workspace.parameters,
                budget,
            );
            ferraris::checked_reduct_result(
                self.theory(),
                candidate,
                limits,
                budget,
                statistics,
                result,
            )
        })();
        timing::finish(&mut statistics.phase_timings, Phase::Reduct, started);
        result
    }

    fn parameters(
        &self,
        truth: &FormulaEvaluation<'_>,
        parameters: &mut Vec<Literal>,
        budget: &mut Budget<'_, impl Quota>,
    ) -> Result<(), Incomplete> {
        // Both the capability and this query carry the exact original owner.
        // The capability cannot be constructed from externally supplied bits.
        if !self.theory().same_instance(truth.theory()) || !truth.is_model() {
            return Err(Incomplete::WrongTheory);
        }
        parameters.clear();
        let atoms = self.theory().atom_count();
        for atom in 0..atoms {
            budget.tick()?;
            parameters.push(Literal::new(
                atoms + atom,
                truth.interpretation().contains(atom),
            ));
        }
        for (offset, &node) in self.0.implications.iter().enumerate() {
            budget.tick()?;
            let value = truth.node_truth(node).ok_or(Incomplete::InvalidWitness)?;
            parameters.push(Literal::new(2 * atoms + offset, value));
        }
        Ok(())
    }
}

/// Evaluate the candidate's original truth into the evaluation workspace
/// under the workspace's retained-storage ceiling, charging the work to the
/// reduct receipts. The truth is the frozen mask of the candidate's reduct;
/// the second value is the evaluation's retained storage.
fn evaluate_truth<'a>(
    evaluation: &'a mut EvaluationWorkspace,
    other_bytes: u128,
    max_bytes: u64,
    candidate: &'a Interpretation,
    limits: Limits,
    control: &Control,
    statistics: &mut Statistics,
) -> Result<(FormulaEvaluation<'a>, u128), Incomplete> {
    let evaluation_limit =
        usize::try_from(u128::from(max_bytes) - other_bytes).unwrap_or(usize::MAX);
    let started = timing::start(statistics.phase_timings.as_ref());
    let evaluated = evaluation.evaluate(
        candidate,
        EvaluationLimits {
            max_work: limits.max_verification_work,
            max_bytes: evaluation_limit,
        },
        control,
    );
    timing::finish(
        &mut statistics.phase_timings,
        Phase::OriginalValidation,
        started,
    );
    statistics.reduct.original_work = statistics
        .reduct
        .original_work
        .checked_add(evaluated.work)
        .ok_or(Incomplete::CounterOverflow)?;
    let truth = evaluated.result.map_err(|error| match error {
        EvaluationError::Stopped(stop) => Incomplete::from(stop),
        EvaluationError::Storage { required, .. } => Incomplete::ReductStorage {
            required: other_bytes + required,
            limit: u128::from(max_bytes),
        },
    })?;
    Ok((truth, evaluated.retained_bytes))
}

fn header_bytes() -> u128 {
    // Nested workspace methods already include their headers.
    (size_of::<ReductWorkspace>()
        - size_of::<EvaluationWorkspace>()
        - size_of::<search::PreparedWorkspace>()) as u128
}

fn reserve_query(
    search: &mut search::PreparedWorkspace,
    parameters: &mut Vec<Literal>,
    prepared: &super::Data,
    base: u128,
    max_bytes: u64,
) -> Result<(), Incomplete> {
    let count = prepared
        .theory
        .atom_count()
        .checked_add(prepared.implications.len())
        .ok_or(Incomplete::CounterOverflow)?;
    let required = base
        + parameters.capacity().max(count) as u128 * size_of::<Literal>() as u128
        + search.required_bytes(prepared.cnf.variables(), prepared.cnf.clauses().len());
    bound(required, max_bytes)?;
    parameters
        .try_reserve_exact(count.saturating_sub(parameters.len()))
        .map_err(|_| Incomplete::Allocation)?;
    bound(
        base + parameters.capacity() as u128 * size_of::<Literal>() as u128
            + search.required_bytes(prepared.cnf.variables(), prepared.cnf.clauses().len()),
        max_bytes,
    )?;
    search.reserve(prepared.cnf.variables(), prepared.cnf.clauses().len())?;
    bound(
        base + parameters.capacity() as u128 * size_of::<Literal>() as u128
            + search.retained_bytes(),
        max_bytes,
    )
}

#[cfg(test)]
#[path = "../../tests/support/prepared_identity.rs"]
mod tests;
