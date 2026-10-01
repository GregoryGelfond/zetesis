//! Monotone producer unions over bounded source-derived intersections.

use std::collections::{BTreeMap, BTreeSet};

use themelios_program::program::{Program, Statement};
use themelios_program::provenance::WithProvenance;
use themelios_program::symbol::Symbol;

use crate::limits::check;
use crate::{
    Analysis, Context, Domain, Limits, Resource, Statistics, Status, Stop, UnknownReason, Widening,
};

pub(super) enum Transfer<'p> {
    Value(&'p Symbol),
    Intersection(Vec<usize>),
    Unknown(Widening),
}
pub(super) struct Producer<'p> {
    pub target: usize,
    pub transfer: Transfer<'p>,
    pub source: &'p WithProvenance<Statement>,
}
pub(super) enum Failure {
    Stop(Stop),
    Unknown(UnknownReason),
}
impl From<Stop> for Failure {
    fn from(value: Stop) -> Self {
        Self::Stop(value)
    }
}
pub(super) struct Engine<'p> {
    pub result: Analysis<'p>,
    pub limits: Limits,
    pub producers: Vec<Producer<'p>>,
}

/// Infer conservative argument bounds without altering or reinterpreting the
/// supplied program. No strings, source loader, or normalization context is used.
///
/// A global resource stop or unresolved source context clears every finite result.
/// Unsupported local producers and finite-width overflow instead widen their
/// affected argument. All callers must inspect [`Analysis::status`].
#[must_use]
pub fn analyze(program: &Program, limits: Limits) -> Analysis<'_> {
    let mut engine = Engine {
        result: Analysis {
            program,
            signatures: BTreeMap::new(),
            arguments: Vec::new(),
            status: Status::FixedPoint,
            context: None,
            statistics: Statistics::default(),
        },
        limits,
        producers: Vec::new(),
    };
    if let Err(error) = engine.compile().and_then(|()| engine.run()) {
        engine.result.status = match error {
            Failure::Stop(stop) => Status::Stopped(stop),
            Failure::Unknown(reason) => Status::Unknown(reason),
        };
        engine.result.signatures.clear();
        engine.result.arguments.clear();
    } else {
        engine.result.context = None;
    }
    engine.result
}

impl<'p> Engine<'p> {
    pub fn work(&mut self) -> Result<(), Stop> {
        self.steps(1)
    }
    fn steps(&mut self, count: usize) -> Result<(), Stop> {
        let stats = &mut self.result.statistics;
        let observed = u128::from(stats.work) + count as u128;
        check(Resource::Work, observed, u128::from(self.limits.max_work))?;
        stats.work = u64::try_from(observed).expect("bounded by u64 ceiling");
        Ok(())
    }
    pub fn bytes(&mut self, bytes: usize) -> Result<(), Stop> {
        let stats = &mut self.result.statistics;
        let observed = u128::from(stats.inspected_bytes) + bytes as u128;
        check(
            Resource::InspectedBytes,
            observed,
            u128::from(self.limits.max_inspected_bytes),
        )?;
        stats.inspected_bytes = u64::try_from(observed).expect("bounded by u64 ceiling");
        Ok(())
    }
    pub fn links(&mut self, count: usize) -> Result<(), Stop> {
        let stats = &mut self.result.statistics;
        let observed = stats.links as u128 + count as u128;
        check(Resource::Links, observed, self.limits.max_links as u128)?;
        stats.links = usize::try_from(observed).expect("bounded by usize ceiling");
        Ok(())
    }
    fn widen(&mut self, target: usize, reason: Widening) -> bool {
        let argument = &mut self.result.arguments[target];
        let Domain::Finite(values) = &argument.domain else {
            return false;
        };
        self.result.statistics.value_entries -= values.len();
        self.result.statistics.widened += 1;
        argument.domain = Domain::Unknown;
        argument.widening = Some(reason);
        true
    }
    fn merge(&mut self, target: usize, value: &'p Symbol) -> Result<bool, Stop> {
        self.work()?;
        let Domain::Finite(values) = &mut self.result.arguments[target].domain else {
            return Ok(false);
        };
        if values.contains(value) {
            return Ok(false);
        }
        if values.len() >= self.limits.max_values_per_argument {
            return Ok(self.widen(target, Widening::ValueWidth));
        }
        check(
            Resource::ValueEntries,
            self.result.statistics.value_entries as u128 + 1,
            self.limits.max_value_entries as u128,
        )?;
        values.insert(value);
        self.result.statistics.value_entries += 1;
        Ok(true)
    }
    fn run(&mut self) -> Result<(), Failure> {
        if self.producers.is_empty() {
            return Ok(());
        }
        loop {
            check(
                Resource::Rounds,
                u128::from(self.result.statistics.rounds) + 1,
                u128::from(self.limits.max_rounds),
            )?;
            self.result.statistics.rounds += 1;
            let mut changed = false;
            for index in 0..self.producers.len() {
                self.result.context = Some(Context::Statement(self.producers[index].source));
                self.work()?;
                let target = self.producers[index].target;
                match &self.producers[index].transfer {
                    Transfer::Value(value) => changed |= self.merge(target, value)?,
                    Transfer::Unknown(reason) => changed |= self.widen(target, *reason),
                    Transfer::Intersection(_) => changed |= self.intersection(index)?,
                }
            }
            if !changed {
                return Ok(());
            }
        }
    }
    // A producer binds its head variable at every listed positive body column.
    // Unknown is top: a finite column still bounds the conjunction. Growing an
    // input can only grow this intersection, so unioning the result into the
    // target preserves the inflationary fixed-point iteration, including cycles.
    fn intersection(&mut self, producer: usize) -> Result<bool, Stop> {
        let Transfer::Intersection(inputs) = &self.producers[producer].transfer else {
            unreachable!("intersection transfer")
        };
        let length = inputs.len();
        let target = self.producers[producer].target;
        let mut smallest: Option<(usize, usize, usize)> = None;
        for input in 0..length {
            self.work()?;
            let Transfer::Intersection(inputs) = &self.producers[producer].transfer else {
                unreachable!("immutable transfer kind")
            };
            let from = inputs[input];
            if let Domain::Finite(values) = &self.result.arguments[from].domain
                && smallest.is_none_or(|(_, _, count)| values.len() < count)
            {
                smallest = Some((input, from, values.len()));
            }
        }
        let Some((selected, from, count)) = smallest else {
            return Ok(self.widen(target, Widening::Dependency));
        };
        // Snapshot only borrowed references, before the target can change. It
        // may itself be an input. Admit every copy before allocating scratch;
        // its length is at most one finite argument's admitted width.
        self.steps(count)?;
        let Domain::Finite(values) = &self.result.arguments[from].domain else {
            unreachable!("work charging does not change domains")
        };
        let snapshot = values.iter().copied().collect::<Vec<_>>();
        let mut changed = false;
        for value in snapshot {
            let mut permitted = true;
            // The selected occurrence needs no visit: snapshot iteration
            // already establishes membership. Every other occurrence is
            // charged before inspecting it, including duplicates of the source.
            for input in (0..selected).chain(selected + 1..length) {
                self.work()?;
                let Transfer::Intersection(inputs) = &self.producers[producer].transfer else {
                    unreachable!("immutable transfer kind")
                };
                let other = inputs[input];
                // A repeated source occurrence needs no additional set lookup.
                if other == from {
                    continue;
                }
                if !self.result.arguments[other].domain.permits(value) {
                    permitted = false;
                    break;
                }
            }
            if permitted {
                changed |= self.merge(target, value)?;
            }
        }
        Ok(changed)
    }
    pub fn symbol(&mut self, symbol: &'p Symbol) -> Result<bool, Stop> {
        if self.limits.max_symbol_nodes == 0 || self.limits.max_symbol_depth == 0 {
            return Ok(false);
        }
        let mut stack = vec![(symbol, 1_usize)];
        let mut nodes = 0_usize;
        let mut bytes = 0_u128;
        while let Some((symbol, depth)) = stack.pop() {
            self.work()?;
            nodes += 1;
            let payload = match symbol {
                Symbol::String(value) => value.len(),
                Symbol::Function { name, .. } => name.as_str().len(),
                _ => 0,
            };
            bytes += payload as u128;
            if bytes > u128::from(self.limits.max_symbol_bytes) {
                return Ok(false);
            }
            self.bytes(payload)?;
            let children = symbol.arguments();
            if nodes as u128 + stack.len() as u128 + children.len() as u128
                > self.limits.max_symbol_nodes as u128
                || (!children.is_empty() && depth >= self.limits.max_symbol_depth)
            {
                return Ok(false);
            }
            stack.extend(children.iter().map(|child| (child, depth + 1)));
        }
        Ok(true)
    }
}

pub(super) fn empty_argument<'p>() -> crate::Argument<'p> {
    crate::Argument {
        domain: Domain::Finite(BTreeSet::new()),
        producers: Vec::new(),
        widening: None,
    }
}
