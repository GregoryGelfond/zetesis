//! Actual occurrence masks acquired with the existing topological formula evaluator.

use zetesis_cpu::{Cancellation, Stop};

use super::{
    Error, ErrorKind, Group, GroupRef, Reduction, ReductionLimits, Resource, Statistics, add,
    bytes, storage,
};
use crate::{Interpretation, oracle};

/// Bounds for formula-prefix evaluation and retained tuple occurrence masks.
#[derive(Clone, Copy, Debug)]
pub struct EligibilityLimits {
    /// Original/frozen node and operand visits, plus copied tuple observations.
    pub max_work: u64,
    /// Simultaneous node-truth scratch and output masks; borrowed theory, group,
    /// interpretations, stack and allocator metadata are excluded.
    pub max_bytes: u64,
}
impl Default for EligibilityLimits {
    fn default() -> Self {
        Self {
            max_work: 100_000_000,
            max_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Actual eligibility observations bound to this group and borrowed M/J pair.
/// Constructed only by [`Group::eligibility`]; no aggregate guard is evaluated
/// during acquisition and no field asserts stable-model membership.
#[derive(Debug)]
pub struct Eligibility<'a> {
    group: GroupRef<'a>,
    candidate: &'a Interpretation,
    tested: Option<&'a Interpretation>,
    original: Vec<bool>,
    frozen: Option<Vec<bool>>,
    statistics: Statistics,
}

impl<'a> Eligibility<'a> {
    /// Exact retained operation whose ordered condition IDs supplied the masks.
    #[must_use]
    pub const fn group(&self) -> GroupRef<'a> {
        self.group
    }

    /// Original interpretation M, with the group's immutable theory identity.
    #[must_use]
    pub const fn candidate(&self) -> &'a Interpretation {
        self.candidate
    }

    /// Tested J if frozen eligibility was requested; no subset claim is made.
    #[must_use]
    pub const fn tested(&self) -> Option<&'a Interpretation> {
        self.tested
    }

    /// Actual original eligibility in exact tuple occurrence order.
    #[must_use]
    pub fn original(&self) -> &[bool] {
        &self.original
    }

    /// Actual J-truth of each eligibility formula frozen in M, if requested.
    #[must_use]
    pub fn frozen(&self) -> Option<&[bool]> {
        self.frozen.as_deref()
    }

    /// Completed acquisition accounting, excluding subsequent aggregate reduction.
    #[must_use]
    pub const fn statistics(&self) -> Statistics {
        self.statistics
    }

    /// Reduce the acquired masks and evaluate all group guards under a fresh,
    /// explicit reduction budget. No formula-prefix evaluation is repeated.
    ///
    /// # Errors
    /// Refuses reduction work, checked arithmetic or control.
    pub fn reduce(
        &self,
        limits: ReductionLimits,
        cancellation: &Cancellation,
    ) -> Result<Reduction<'a>, Error> {
        self.group
            .reduce(&self.original, self.frozen.as_deref(), limits, cancellation)
    }
}

impl Group {
    /// Acquire original/frozen formula truth through this canonical group.
    /// # Errors
    /// Refuses theory identity, work, allocation, storage or caller control.
    pub fn eligibility<'a>(
        &'a self,
        candidate: &'a Interpretation,
        tested: Option<&'a Interpretation>,
        limits: EligibilityLimits,
        cancellation: &Cancellation,
    ) -> Result<Eligibility<'a>, Error> {
        self.view()
            .eligibility(candidate, tested, limits, cancellation)
    }
}

impl<'a> GroupRef<'a> {
    /// Acquire actual original and optional frozen eligibility without lowering
    /// this aggregate or changing the retained theory.
    ///
    /// Each requested phase evaluates the complete existing formula prefix, then
    /// copies each tuple's condition value. Repeated condition IDs remain distinct
    /// tuple occurrences. Frozen evaluation uses original node truth from M and
    /// atom truth from J. J need not be a subset of M; minimality is a separate
    /// solver obligation. All prefix work is included in this acquisition's
    /// budget, separate from reduction and guard evaluation.
    ///
    /// # Errors
    /// Refuses foreign M/J identity, storage, work, cancellation or deadline.
    /// No partial observation record escapes an interrupted acquisition.
    pub fn eligibility(
        self,
        candidate: &'a Interpretation,
        tested: Option<&'a Interpretation>,
        limits: EligibilityLimits,
        cancellation: &Cancellation,
    ) -> Result<Eligibility<'a>, Error> {
        let mut statistics = Statistics::default();
        let result = self.acquire(candidate, tested, limits, cancellation, &mut statistics);
        result
            .map(|(original, frozen)| Eligibility {
                group: self,
                candidate,
                tested,
                original,
                frozen,
                statistics,
            })
            .map_err(|kind| Error { kind, statistics })
    }

    fn acquire(
        &self,
        candidate: &Interpretation,
        tested: Option<&Interpretation>,
        limits: EligibilityLimits,
        cancellation: &Cancellation,
        statistics: &mut Statistics,
    ) -> Result<(Vec<bool>, Option<Vec<bool>>), ErrorKind> {
        cancellation.poll().map_err(ErrorKind::Stopped)?;
        if !self.data.theory.same_instance(candidate.theory())
            || tested.is_some_and(|tested| !self.data.theory.same_instance(tested.theory()))
        {
            return Err(ErrorKind::WrongTheory);
        }
        let phases = if tested.is_some() { 2 } else { 1 };
        statistics.resident_bytes = bytes::<bool>(self.data.tuples.len())?
            .checked_mul(phases)
            .ok_or(ErrorKind::Overflow)?;
        statistics.peak_bytes = add(
            statistics.resident_bytes,
            bytes::<bool>(self.data.theory.nodes().len())?
                .checked_mul(phases)
                .ok_or(ErrorKind::Overflow)?,
        )?;
        if statistics.peak_bytes > limits.max_bytes {
            return Err(ErrorKind::Limit(Resource::Bytes));
        }
        let mut work = oracle::Work {
            limits: oracle::Limits {
                max_work: limits.max_work,
                max_subsets: 0,
            },
            cancellation,
            statistics: oracle::Statistics::default(),
        };
        let result = observations(*self, candidate, tested, &mut work);
        statistics.work = work.statistics.work;
        let observations = result?;
        cancellation.poll().map_err(ErrorKind::Stopped)?;
        Ok(observations)
    }
}

fn observations(
    group: GroupRef<'_>,
    candidate: &Interpretation,
    tested: Option<&Interpretation>,
    work: &mut oracle::Work<'_>,
) -> Result<(Vec<bool>, Option<Vec<bool>>), ErrorKind> {
    let mut original_nodes = storage(group.data.theory.nodes().len())?;
    oracle::evaluate(
        &group.data.theory,
        candidate,
        None,
        &mut original_nodes,
        work,
    )
    .map_err(stopped)?;
    let original = project(group, &original_nodes, work)?;
    let frozen = if let Some(tested) = tested {
        let mut frozen_nodes = storage(group.data.theory.nodes().len())?;
        oracle::evaluate(
            &group.data.theory,
            tested,
            Some(&original_nodes),
            &mut frozen_nodes,
            work,
        )
        .map_err(stopped)?;
        Some(project(group, &frozen_nodes, work)?)
    } else {
        None
    };
    Ok((original, frozen))
}

fn project(
    group: GroupRef<'_>,
    nodes: &[bool],
    work: &mut oracle::Work<'_>,
) -> Result<Vec<bool>, ErrorKind> {
    let mut mask = storage(group.data.tuples.len())?;
    for tuple in &group.data.tuples {
        work.tick().map_err(stopped)?;
        mask.push(nodes[tuple.condition]);
    }
    Ok(mask)
}

fn stopped(stop: Stop) -> ErrorKind {
    match stop {
        Stop::WorkLimit => ErrorKind::Limit(Resource::Work),
        _ => ErrorKind::Stopped(stop),
    }
}
