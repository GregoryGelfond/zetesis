//! Capture source-established count facts without changing original grounding.

use std::collections::BTreeMap;

use themelios_base::span::Location;
use zetesis_core::{Atom, Value};
use zetesis_ferraris::{AggregateComparison, Node, Theory};

use super::{
    CountPlanFailure, CountPlanFailureKind as Fault, CountPlanResource as Resource, Outcome,
    Request, Work, bytes, ceiling,
};

/// Values transferred from the complete source tuple/head bijection. Ordinary
/// choices use their full atoms from the admitted atom catalog as keys; numeric
/// atom indices below retain exactly that catalog meaning, never a tuple value.
pub(crate) struct Input<'a> {
    pub body: usize,
    pub eligible: &'a BTreeMap<usize, usize>,
    pub tuple_keys: BTreeMap<Atom, Vec<Value>>,
    pub nodes: &'a [Node],
    pub atoms: &'a [Atom],
    pub bounds: Bounds,
    pub origins: &'a [Location],
    pub location: Location,
}

/// Stack-only interval consequences of the exact source guard evaluations.
/// Disequality supplies no interval premise; no guard is reinterpreted as not.
/// A logical guard excludes the whole group from this numeric certificate.
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

pub(super) struct Group {
    pub body: usize,
    pub members: Vec<usize>,
    pub lower: usize,
    pub upper: usize,
    pub bound_root: usize,
    pub asserted_root: usize,
    pub origins: Vec<Location>,
    pub location: Location,
}

pub(crate) struct Collector {
    pub(super) work: Work,
    pub(super) groups: Vec<Group>,
    failure: Option<CountPlanFailure>,
}
impl Collector {
    pub(crate) fn new(request: Request<'_>, location: Location) -> Self {
        Self {
            work: Work {
                limits: request.limits,
                control: request.control.clone(),
                statistics: super::CountPlanStatistics::default(),
                location,
            },
            groups: Vec::new(),
            failure: None,
        }
    }

    pub(crate) fn capture_group(
        &mut self,
        input: Input<'_>,
        bound_root: usize,
        asserted_root: usize,
    ) {
        if self.failure.is_some() {
            return;
        }
        self.work.location = input.location;
        if let Err(kind) = self.capture(input, bound_root, asserted_root) {
            self.stop(kind);
        }
    }

    fn capture(
        &mut self,
        input: Input<'_>,
        bound_root: usize,
        asserted_root: usize,
    ) -> Result<(), Fault> {
        self.work.poll()?;
        let Bounds::Numeric { lower, upper } = input.bounds else {
            return Ok(());
        };
        // Complete eligibility is inspected before allocating optional descriptors
        // or copying origins. Existing validated keys are transferred, never
        // reconstructed. Possible support is not an eligibility truth certificate.
        for &condition in input.eligible.values() {
            self.work.charge(1)?;
            if condition != 1 {
                return Ok(());
            }
        }
        if input.body == 0 || input.eligible.is_empty() || lower > upper {
            return Ok(());
        }
        let lower = usize::try_from(lower).map_err(|_| Fault::Overflow)?;
        let upper = usize::try_from(upper).map_err(|_| Fault::Overflow)?;
        if lower == 0 && upper == input.eligible.len() {
            return Ok(());
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
        if !input.tuple_keys.is_empty() && input.tuple_keys.len() != input.eligible.len() {
            return Err(Fault::SourceMapping);
        }
        let mut members = self.work.vector(input.eligible.len())?;
        for head in input.eligible.keys() {
            self.work.charge(1)?;
            let Some(Node::Atom(atom)) = input.nodes.get(*head) else {
                return Err(Fault::SourceMapping);
            };
            if input.atoms.get(*atom).is_none() {
                return Err(Fault::SourceMapping);
            }
            members.push(*atom);
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
        self.key_payload(&input.tuple_keys)?;
        let mut origins = self.work.vector(input.origins.len())?;
        for &origin in input.origins {
            self.work.charge(1)?;
            origins.push(origin);
        }
        self.work.statistics.groups = next;
        self.work.statistics.members = count;
        self.work.statistics.origins = origin_count;
        self.work.payload(bytes::<Group>(1)?)?;
        self.groups
            .try_reserve_exact(1)
            .map_err(|_| Fault::Stopped(zetesis_cpu::Stop::Allocation))?;
        // The transferred whole keys established the original group mapping.
        // No synthetic atom-number tuple is published as a source key.
        drop(input.tuple_keys);
        self.groups.push(Group {
            body: input.body,
            members,
            lower,
            upper,
            bound_root,
            asserted_root,
            origins,
            location: input.location,
        });
        Ok(())
    }

    fn key_payload(&mut self, keys: &BTreeMap<Atom, Vec<Value>>) -> Result<(), Fault> {
        for (atom, tuple) in keys {
            self.work.charge(1)?;
            self.work.payload(bytes::<Atom>(1)?)?;
            let name_bytes =
                u64::try_from(atom.predicate().name().len()).map_err(|_| Fault::Overflow)?;
            self.work.charge(name_bytes)?;
            self.work.payload(name_bytes)?;
            self.work.payload(bytes::<Value>(atom.values().len())?)?;
            for value in atom.values() {
                self.value(value)?;
            }
            self.work.payload(bytes::<Value>(tuple.capacity())?)?;
            for value in tuple {
                self.value(value)?;
            }
        }
        Ok(())
    }

    fn value(&mut self, value: &Value) -> Result<(), Fault> {
        let visits = match value {
            Value::Structured(value) => value.nodes().len(),
            _ => 1,
        };
        self.work
            .charge(u64::try_from(visits).map_err(|_| Fault::Overflow)?)?;
        self.work
            .payload(u64::try_from(value.payload_bytes()).map_err(|_| Fault::Overflow)?)
    }

    fn stop(&mut self, kind: Fault) {
        self.failure = Some(self.work.failure(kind));
        self.groups.clear();
    }

    pub(crate) fn finish(mut self, theory: &Theory) -> Outcome {
        if let Some(error) = self.failure {
            return Outcome::Incomplete(error);
        }
        match super::derive::plan(&self.groups, theory, &mut self.work) {
            Ok(Some(plan)) => Outcome::Ready(plan),
            Ok(None) => Outcome::NoPlan(self.work.statistics),
            Err(kind) => Outcome::Incomplete(self.work.failure(kind)),
        }
    }
}
