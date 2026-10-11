//! Actual occurrence masks acquired with the existing topological formula evaluator.

use zetesis_cpu::{Cancellation, Stop};

use super::{
    Error, ErrorKind, Group, GroupRef, Reduction, ReductionLimits, Resource, Statistics, add, bytes,
};
use crate::extremum_cut::ExtremaScratch;
use crate::{Interpretation, Theory, oracle};

/// Bounds for formula-prefix evaluation and retained tuple occurrence masks.
#[derive(Clone, Copy, Debug)]
pub struct EligibilityLimits {
    /// Original/frozen node and operand visits, ordinal-cache initialization and
    /// packed-validity resets, plus copied tuple observations.
    pub max_work: u64,
    /// Simultaneous node truth, ordinal scratch and output masks; borrowed theory, group,
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
        statistics.peak_bytes = add(
            statistics.peak_bytes,
            u64::try_from(ExtremaScratch::default().required_bytes(&self.data.theory))
                .map_err(|_| ErrorKind::Overflow)?,
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
        let result = observations(
            *self,
            candidate,
            tested,
            limits.max_bytes,
            statistics,
            &mut work,
        );
        statistics.work = work.statistics.work;
        let observations = result?;
        cancellation.poll().map_err(ErrorKind::Stopped)?;
        Ok(observations)
    }
}

/// The full requested envelope is checked before this owner reserves anything.
/// Actual capacity is observed after each reservation on success and refusal.
#[derive(Default)]
struct Observations {
    original_nodes: Vec<bool>,
    frozen_nodes: Vec<bool>,
    original: Vec<bool>,
    frozen: Vec<bool>,
    extrema: ExtremaScratch,
}

impl Observations {
    fn bytes(&self) -> u128 {
        self.original_nodes.capacity() as u128
            + self.frozen_nodes.capacity() as u128
            + self.original.capacity() as u128
            + self.frozen.capacity() as u128
            + self.extrema.retained_bytes()
    }

    fn observe(
        &self,
        reservation: Result<(), ErrorKind>,
        limit: u64,
        statistics: &mut Statistics,
    ) -> Result<(), ErrorKind> {
        observe_capacity(self.bytes(), reservation, limit, statistics)
    }

    fn reserve(
        &mut self,
        theory: &Theory,
        tuples: usize,
        frozen: bool,
        limit: u64,
        statistics: &mut Statistics,
    ) -> Result<(), ErrorKind> {
        let result = reserve_mask(&mut self.original_nodes, theory.nodes().len());
        self.observe(result, limit, statistics)?;
        let base = self.bytes() - self.extrema.retained_bytes();
        self.extrema
            .reserve_observed(theory, |actual| {
                observe_capacity(base + actual, Ok(()), limit, statistics).map_err(Observed)
            })
            .map_err(|error| error.0)?;
        let result = reserve_mask(&mut self.original, tuples);
        self.observe(result, limit, statistics)?;
        if frozen {
            let result = reserve_mask(&mut self.frozen_nodes, theory.nodes().len());
            self.observe(result, limit, statistics)?;
            let result = reserve_mask(&mut self.frozen, tuples);
            self.observe(result, limit, statistics)?;
        }
        Ok(())
    }
}

// Adapt the native group's existing error vocabulary to generic scratch control.
struct Observed(ErrorKind);
impl From<Stop> for Observed {
    fn from(stop: Stop) -> Self {
        Self(stopped(stop))
    }
}

fn observe_capacity(
    bytes: u128,
    reservation: Result<(), ErrorKind>,
    limit: u64,
    statistics: &mut Statistics,
) -> Result<(), ErrorKind> {
    let actual = u64::try_from(bytes).map_err(|_| ErrorKind::Overflow);
    if let Ok(actual) = &actual {
        statistics.storage_peak_bytes = statistics.storage_peak_bytes.max(*actual);
    }
    // Preserve the original allocation failure if observation also refuses.
    reservation?;
    if actual? > limit {
        return Err(ErrorKind::Limit(Resource::Bytes));
    }
    Ok(())
}

fn reserve_mask(target: &mut Vec<bool>, count: usize) -> Result<(), ErrorKind> {
    target
        .try_reserve_exact(count)
        .map_err(|_| ErrorKind::Stopped(Stop::Allocation))
}

fn observations(
    group: GroupRef<'_>,
    candidate: &Interpretation,
    tested: Option<&Interpretation>,
    limit: u64,
    statistics: &mut Statistics,
    work: &mut oracle::Work<'_>,
) -> Result<(Vec<bool>, Option<Vec<bool>>), ErrorKind> {
    let mut buffers = Observations::default();
    buffers.reserve(
        &group.data.theory,
        group.data.tuples.len(),
        tested.is_some(),
        limit,
        statistics,
    )?;
    oracle::evaluate(
        &group.data.theory,
        candidate,
        None,
        &mut buffers.original_nodes,
        &mut buffers.extrema,
        work,
    )
    .map_err(stopped)?;
    project(group, &buffers.original_nodes, &mut buffers.original, work)?;
    if let Some(tested) = tested {
        oracle::evaluate(
            &group.data.theory,
            tested,
            Some(&buffers.original_nodes),
            &mut buffers.frozen_nodes,
            &mut buffers.extrema,
            work,
        )
        .map_err(stopped)?;
        project(group, &buffers.frozen_nodes, &mut buffers.frozen, work)?;
    }
    statistics.storage_bytes = add(
        bytes::<bool>(buffers.original.capacity())?,
        bytes::<bool>(buffers.frozen.capacity())?,
    )?;
    Ok((buffers.original, tested.map(|_| buffers.frozen)))
}

fn project(
    group: GroupRef<'_>,
    nodes: &[bool],
    mask: &mut Vec<bool>,
    work: &mut oracle::Work<'_>,
) -> Result<(), ErrorKind> {
    for tuple in &group.data.tuples {
        work.tick().map_err(stopped)?;
        mask.push(nodes[tuple.condition]);
    }
    Ok(())
}

fn stopped(stop: Stop) -> ErrorKind {
    match stop {
        Stop::WorkLimit => ErrorKind::Limit(Resource::Work),
        _ => ErrorKind::Stopped(stop),
    }
}
