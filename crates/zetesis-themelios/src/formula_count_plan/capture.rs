//! Capture source-established count facts without changing original grounding.

use crate::ProgramSite;
use crate::formula_head_aggregate::Bijection;
use zetesis_ferraris::{AggregateComparison, FormulaView, NodeView as Node, Theory};

use super::{
    CountPlanFailure, CountPlanFailureKind as Fault, CountPlanResource as Resource, Outcome,
    Request, Work, bytes, ceiling,
};

/// Complete source validation supplies a tuple/head bijection certificate.
/// Numeric atom indices retain their local theory coordinate meaning; no source
/// term or atom payload is transferred into this optional planner.
pub(crate) struct Input<'a> {
    pub body: usize,
    pub eligible: &'a [(usize, usize)],
    pub bijection: Bijection,
    pub nodes: FormulaView<'a>,
    pub atom_count: usize,
    pub origins: &'a [ProgramSite],
    pub location: ProgramSite,
}

/// Stack-only interval consequences of the exact source guard evaluations.
/// Disequality supplies no interval premise; no guard is reinterpreted as not.
/// A logical guard excludes the whole group from this numeric certificate.
#[derive(Clone, Copy)]
pub(crate) enum Bounds {
    Numeric { lower: i128, upper: i128 },
    NonNumeric,
}
impl Bounds {
    pub(crate) fn new(members: usize) -> Self {
        Self::Numeric {
            lower: 0,
            upper: members as i128,
        }
    }

    pub(crate) fn exclude_logical_guard(&mut self) {
        *self = Self::NonNumeric;
    }

    pub(crate) fn guard(&mut self, comparison: AggregateComparison, bound: i32) {
        let Self::Numeric { lower, upper } = self else {
            return;
        };
        let bound = i128::from(bound);
        match comparison {
            AggregateComparison::Eq => {
                *lower = (*lower).max(bound);
                *upper = (*upper).min(bound);
            }
            AggregateComparison::Ge => *lower = (*lower).max(bound),
            AggregateComparison::Gt => *lower = (*lower).max(bound + 1),
            AggregateComparison::Le => *upper = (*upper).min(bound),
            AggregateComparison::Lt => *upper = (*upper).min(bound - 1),
            AggregateComparison::Ne => {}
        }
    }
}

/// Owned optional premises prepared before exact guard lowering. Keeping only
/// these compact coordinates releases the source map and its mandatory storage
/// lease at the ordinary lowering boundary. Bounds and roots are attached only
/// after their exact compilation succeeds.
pub(crate) struct Members {
    body: usize,
    unconditional: bool,
    atoms: Vec<usize>,
    origins: Vec<ProgramSite>,
    location: ProgramSite,
}

#[derive(Debug)]
pub(crate) struct Group {
    pub body: usize,
    pub unconditional_members: bool,
    pub members: Vec<usize>,
    pub lower: usize,
    pub upper: usize,
    pub bound_root: usize,
    pub asserted_root: usize,
    pub origins: Vec<ProgramSite>,
    pub location: ProgramSite,
}

pub(crate) struct Collector {
    pub(super) work: Work,
    pub(super) groups: Vec<Group>,
    failure: Option<CountPlanFailure>,
    partition: bool,
    pub(crate) retained: bool,
}
impl Collector {
    pub(crate) fn new(request: Request<'_>, location: ProgramSite) -> Self {
        Self {
            work: Work {
                limits: request.limits,
                cancellation: request.cancellation.clone(),
                statistics: super::CountPlanStatistics::default(),
                location,
            },
            groups: Vec::new(),
            failure: None,
            partition: true,
            retained: false,
        }
    }

    pub(crate) fn objective(
        limits: &crate::FormulaLimits,
        cancellation: &zetesis_cpu::Cancellation,
        location: ProgramSite,
    ) -> Self {
        let mut collector = Self::new(
            Request {
                limits: super::CountPlanLimits {
                    max_groups: limits.theory.max_roots,
                    max_members: limits.theory.max_operands,
                    max_origins: limits.max_origin_locations,
                    max_bytes: u64::try_from(limits.max_support_bytes).unwrap_or(u64::MAX),
                    max_work: limits.max_work,
                    ..super::CountPlanLimits::default()
                },
                cancellation,
            },
            location,
        );
        collector.partition = false;
        collector.retained = true;
        collector
    }

    /// Disequalities provide no interval premise, regardless of their values.
    /// Decline them before staging members, while mandatory lowering still
    /// evaluates every original guard and constructs its exact formula.
    pub(crate) fn interval_guards(
        &mut self,
        guards: &[crate::formula_ir::AggregateGuard],
        location: ProgramSite,
    ) -> bool {
        if self.failure.is_some() {
            return false;
        }
        self.work.location = location;
        for guard in guards {
            if let Err(kind) = self.work.charge(1) {
                self.stop(kind);
                return false;
            }
            if guard.relation != themelios_program::program::Relation::Neq {
                return true;
            }
        }
        false
    }

    pub(crate) fn prepare_group(&mut self, input: &Input<'_>) -> Option<Members> {
        if self.failure.is_some() {
            return None;
        }
        self.work.location = input.location;
        match self.prepare(input) {
            Ok(members) => members,
            Err(kind) => {
                self.stop(kind);
                None
            }
        }
    }

    fn prepare(&mut self, input: &Input<'_>) -> Result<Option<Members>, Fault> {
        self.work.poll()?;
        if input.body == 0 || input.eligible.is_empty() {
            return Ok(None);
        }
        // Conditional contributions still establish required-head existence:
        // a counted contribution cannot be active without its positive head.
        // Only unconditional eligibility establishes the full interval on true
        // head atoms used by the separate partition consumer.
        let mut unconditional = true;
        for &(_, condition) in input.eligible {
            self.work.charge(1)?;
            unconditional &= condition == 1;
        }
        if !unconditional && !self.retained {
            return Ok(None);
        }
        let next = self
            .work
            .statistics
            .groups
            .checked_add(1)
            .ok_or(Fault::Overflow)?;
        ceiling(next, self.work.limits.max_groups, Resource::Groups)?;
        let count = self
            .work
            .statistics
            .members
            .checked_add(input.eligible.len())
            .ok_or(Fault::Overflow)?;
        ceiling(count, self.work.limits.max_members, Resource::Members)?;
        let origin_count = self
            .work
            .statistics
            .origins
            .checked_add(input.origins.len())
            .ok_or(Fault::Overflow)?;
        ceiling(
            origin_count,
            self.work.limits.max_origins,
            Resource::Origins,
        )?;
        if !input.bijection.accepts_members(input.eligible.len()) {
            return Err(Fault::SourceMapping);
        }
        let mut members = self.work.vector(input.eligible.len())?;
        for &(head, _) in input.eligible {
            self.work.charge(1)?;
            let Ok(Node::Atom(atom)) = input.nodes.node(head) else {
                return Err(Fault::SourceMapping);
            };
            if atom >= input.atom_count {
                return Err(Fault::SourceMapping);
            }
            members.push(atom);
        }
        // Conservative quadratic comparison allowance bounds this small sort;
        // the source group already established uniqueness before this boundary.
        self.work.charge(
            u64::try_from(members.len())
                .map_err(|_| Fault::Overflow)?
                .checked_mul(u64::try_from(members.len()).map_err(|_| Fault::Overflow)?)
                .ok_or(Fault::Overflow)?,
        )?;
        members.sort_unstable();
        if members.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Fault::SourceMapping);
        }
        let mut origins = self.work.vector(input.origins.len())?;
        for &origin in input.origins {
            self.work.charge(1)?;
            origins.push(origin);
        }
        Ok(Some(Members {
            body: input.body,
            unconditional,
            atoms: members,
            origins,
            location: input.location,
        }))
    }

    pub(crate) fn capture_group(
        &mut self,
        members: Members,
        bounds: Bounds,
        bound_root: usize,
        asserted_root: usize,
    ) {
        if self.failure.is_some() {
            return;
        }
        self.work.location = members.location;
        if let Err(kind) = self.capture(members, bounds, bound_root, asserted_root) {
            self.stop(kind);
        }
    }

    fn capture(
        &mut self,
        members: Members,
        bounds: Bounds,
        bound_root: usize,
        asserted_root: usize,
    ) -> Result<(), Fault> {
        self.work.poll()?;
        let Bounds::Numeric { lower, upper } = bounds else {
            return Ok(());
        };
        if (!self.partition || !members.unconditional) && lower <= 0 {
            return Ok(());
        }
        if lower > upper {
            return Ok(());
        }
        let lower = usize::try_from(lower).map_err(|_| Fault::Overflow)?;
        let upper = usize::try_from(upper).map_err(|_| Fault::Overflow)?;
        if lower == 0 && upper == members.atoms.len() {
            return Ok(());
        }
        let groups = self
            .work
            .statistics
            .groups
            .checked_add(1)
            .ok_or(Fault::Overflow)?;
        let count = self
            .work
            .statistics
            .members
            .checked_add(members.atoms.len())
            .ok_or(Fault::Overflow)?;
        let origins = self
            .work
            .statistics
            .origins
            .checked_add(members.origins.len())
            .ok_or(Fault::Overflow)?;
        ceiling(groups, self.work.limits.max_groups, Resource::Groups)?;
        ceiling(count, self.work.limits.max_members, Resource::Members)?;
        ceiling(origins, self.work.limits.max_origins, Resource::Origins)?;
        self.work.statistics.groups = groups;
        self.work.statistics.members = count;
        self.work.statistics.origins = origins;
        self.work.payload(bytes::<Group>(1)?)?;
        self.groups
            .try_reserve_exact(1)
            .map_err(|_| Fault::Stopped(zetesis_cpu::Stop::Allocation))?;
        self.groups.push(Group {
            body: members.body,
            unconditional_members: members.unconditional,
            members: members.atoms,
            lower,
            upper,
            bound_root,
            asserted_root,
            origins: members.origins,
            location: members.location,
        });
        Ok(())
    }

    fn stop(&mut self, kind: Fault) {
        self.failure = Some(self.work.failure(kind));
        self.groups.clear();
    }

    pub(crate) fn finish(mut self, theory: &Theory) -> (Outcome, Option<super::RequiredChoices>) {
        let capture = self.work.statistics;
        let outcome = if self.partition {
            if let Some(error) = self.failure {
                Outcome::Incomplete(error)
            } else {
                match super::derive::plan(&self.groups, theory, &mut self.work) {
                    Ok(Some(plan)) => Outcome::Ready(plan),
                    Ok(None) => Outcome::NoPlan(self.work.statistics),
                    Err(kind) => Outcome::Incomplete(self.work.failure(kind)),
                }
            }
        } else {
            Outcome::NotRequested
        };
        let required = self.retained.then(|| super::RequiredChoices {
            original: theory.clone(),
            groups: self.groups,
            capture,
            failure: self.failure,
        });
        (outcome, required)
    }
}

pub(super) fn group_bytes(groups: &Vec<Group>) -> u128 {
    groups.capacity() as u128 * size_of::<Group>() as u128
        + groups
            .iter()
            .map(|group| {
                group.members.capacity() as u128 * size_of::<usize>() as u128
                    + group.origins.capacity() as u128 * size_of::<ProgramSite>() as u128
            })
            .sum::<u128>()
}
