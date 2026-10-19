//! Conditional absence witnessed by complete incoming producer coverage.
//!
//! A completed original founded pass supplies candidates for an unsupported
//! set U; it is not itself a reusable reason. This collector reconstructs a
//! false body for every producer of every atom in U. Positive atoms in U are
//! false after removing U. Other positive atoms require an explicit original
//! cut literal; an exact negated atom requires an explicit held literal.
//! Conjunction needs one false operand and disjunction needs every operand.
//! Other implications and aggregates cannot provide a witness. In particular,
//! a cached false-node bit, a held cohead, or a candidate-only restriction is
//! never a premise. Ordinary and choice producers have the same coverage duty.
//!
//! Let B be the returned condition and M a stable interpretation satisfying B.
//! For J = M minus U, every producer with a head in U has a false body reduct
//! in J by these witnesses. Producers without such heads retain their heads;
//! an unselected atomic choice retains its true negative alternative. Hence J
//! models the full reduct. Stability implies J = M, so M contains no atom in U.
//! This is a conditional stable consequence, not a classical implication.
//!
//! Growing U preserves every earlier body witness. Every newly added atom's
//! complete incoming row must nevertheless be checked before publication. An
//! AND alternative rolls back both pending atoms and literals when it fails;
//! producer rows are processed only outside that Boolean traversal. The first
//! locally successful alternative is retained. If its later producer coverage
//! fails, capture declines instead of exploring combinations of alternatives.
//!
//! Storage consists of bounded atom/literal lists and an explicit Boolean
//! frame stack, not a full-theory mask or another dependency graph. Every
//! producer/body visit, list comparison, frame step and canonicalization step
//! spends existing Work. Local visit/storage/list bounds decline capture;
//! allocation, quota and cancellation failures preserve their original Stop.
//! The caller invokes this immediately after a successful exact-owner original
//! pass, before publishing any of its cuts, using the same original masks.
//! Scratch preparation ownership alone does not certify pass completion.

use super::{
    Narrower, NodeView, Producers, Scratch, Theory, Work, bit, chain_position, flag_words,
};
use std::mem::size_of;
use zetesis_cpu::Stop;

#[cfg(test)]
mod tests;

/// Complete original coverage and the immediately completed founded pass.
#[derive(Clone, Copy)]
pub(in crate::regions) struct Input<'a> {
    pub(in crate::regions) theory: &'a Theory,
    pub(in crate::regions) index: &'a Narrower,
    pub(in crate::regions) producers: &'a Producers,
    pub(in crate::regions) completed: &'a Scratch,
    pub(in crate::regions) atom_sure: &'a [u64],
    pub(in crate::regions) atom_never: &'a [u64],
}

/// Optional capture bounds, independent of the caller's global Work quota.
#[derive(Clone, Copy)]
pub(in crate::regions) struct Limits {
    pub(in crate::regions) atoms: usize,
    pub(in crate::regions) literals: usize,
    pub(in crate::regions) visits: u64,
    pub(in crate::regions) bytes: u128,
}

/// Sorted, duplicate-free condition B and atoms U absent whenever B holds.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::regions) struct Explanation {
    pub(in crate::regions) conditions: Vec<(usize, bool)>,
    pub(in crate::regions) unsupported: Vec<usize>,
}

impl Explanation {
    /// Owned buffers; the caller accounts for its containing row/header.
    pub(in crate::regions) fn retained_bytes(&self) -> u128 {
        capacity_bytes(&self.conditions) + capacity_bytes(&self.unsupported)
    }
}

/// Capture one directly missing held head before any founded cuts are published.
/// A local refusal returns None and has no semantic effect. The peak includes
/// the local collector header and simultaneous buffers even on failed capture.
pub(in crate::regions) fn collect(
    input: Input<'_>,
    start: usize,
    limits: Limits,
    work: &mut Work<'_>,
    peak_bytes: &mut u128,
) -> Result<Option<Explanation>, Stop> {
    let mut collector = Collector {
        input,
        limits,
        result: Explanation {
            conditions: Vec::new(),
            unsupported: Vec::new(),
        },
        frames: Vec::new(),
        meter: Meter {
            work,
            remaining: limits.visits,
        },
        peak_bytes,
    };
    match collector.run(start) {
        Ok(()) => Ok(Some(collector.result)),
        Err(Failure::Declined) => Ok(None),
        Err(Failure::Stopped(stop)) => Err(stop),
    }
}

enum Failure {
    Declined,
    Stopped(Stop),
}

impl From<Stop> for Failure {
    fn from(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

struct Meter<'w, 'q> {
    work: &'w mut Work<'q>,
    remaining: u64,
}

impl Meter<'_, '_> {
    fn tick(&mut self) -> Result<(), Failure> {
        if self.remaining == 0 {
            return Err(Failure::Declined);
        }
        self.work.tick()?;
        self.remaining -= 1;
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Mark {
    atoms: usize,
    literals: usize,
}

#[derive(Clone, Copy)]
struct Frame {
    chain: usize,
    next: usize,
    mark: Mark,
    disjunction: bool,
}

enum BodyStep {
    Descend(usize),
    Done(bool),
}

struct Collector<'a, 'w, 'q> {
    input: Input<'a>,
    limits: Limits,
    result: Explanation,
    frames: Vec<Frame>,
    meter: Meter<'w, 'q>,
    peak_bytes: &'w mut u128,
}

impl Collector<'_, '_, '_> {
    fn bytes(&self) -> u128 {
        size_of::<Self>() as u128 + self.result.retained_bytes() + capacity_bytes(&self.frames)
    }

    fn run(&mut self, start: usize) -> Result<(), Failure> {
        *self.peak_bytes = (*self.peak_bytes).max(self.bytes());
        if self.bytes() > self.limits.bytes {
            return Err(Failure::Declined);
        }
        self.validate(start)?;
        self.add_atom(start)?;
        let mut next = 0;
        let producers = self.input.producers;
        while next < self.result.unsupported.len() {
            self.meter.tick()?;
            let atom = self.result.unsupported[next];
            next += 1;
            for &row in &producers.by_head[atom] {
                self.meter.tick()?;
                let Some(body) = producers.rules[row].body else {
                    return Err(Failure::Declined);
                };
                if !self.body(body)? {
                    return Err(Failure::Declined);
                }
            }
        }
        canonicalize(&mut self.result.conditions, &mut self.meter)?;
        canonicalize(&mut self.result.unsupported, &mut self.meter)
    }

    fn validate(&mut self, start: usize) -> Result<(), Failure> {
        self.meter.tick()?;
        let input = self.input;
        if !input
            .completed
            .owner
            .as_ref()
            .is_some_and(|owner| owner.same_instance(input.theory))
        {
            return Err(Failure::Declined);
        }
        let atoms = input.theory.atom_count();
        let words = flag_words(atoms);
        if start >= atoms
            || input.atom_sure.len() != words
            || input.atom_never.len() != words
            || input.completed.atoms.len() != words
            || input.completed.cyclic_atoms.len() != words
            || input.producers.by_head.len() != atoms
            || input.index.chain_of.len() != input.theory.nodes().len()
        {
            return Err(Stop::InvalidProgram.into());
        }
        self.meter.tick()?;
        if !bit(input.atom_sure, start) || bit(input.atom_never, start) || !self.missing(start) {
            return Err(Failure::Declined);
        }
        Ok(())
    }

    fn missing(&self, atom: usize) -> bool {
        bit(&self.input.completed.cyclic_atoms, atom) && !self.input.completed.contains(atom)
    }

    fn add_atom(&mut self, atom: usize) -> Result<(), Failure> {
        for &present in &self.result.unsupported {
            self.meter.tick()?;
            if present == atom {
                return Ok(());
            }
        }
        if self.result.unsupported.len() == self.limits.atoms {
            return Err(Failure::Declined);
        }
        self.meter.tick()?;
        let other = self.bytes() - capacity_bytes(&self.result.unsupported);
        reserve_one(
            &mut self.result.unsupported,
            other,
            self.limits.bytes,
            self.peak_bytes,
        )?;
        self.result.unsupported.push(atom);
        Ok(())
    }

    fn add_condition(&mut self, atom: usize, held: bool) -> Result<(), Failure> {
        for &(present, value) in &self.result.conditions {
            self.meter.tick()?;
            if present == atom {
                return if value == held {
                    Ok(())
                } else {
                    Err(Failure::Declined)
                };
            }
        }
        if self.result.conditions.len() == self.limits.literals {
            return Err(Failure::Declined);
        }
        self.meter.tick()?;
        let other = self.bytes() - capacity_bytes(&self.result.conditions);
        reserve_one(
            &mut self.result.conditions,
            other,
            self.limits.bytes,
            self.peak_bytes,
        )?;
        self.result.conditions.push((atom, held));
        Ok(())
    }

    fn body(&mut self, root: usize) -> Result<bool, Failure> {
        let mut node = root;
        loop {
            match self.enter(node)? {
                BodyStep::Descend(child) => node = child,
                BodyStep::Done(found) => match self.resume(found)? {
                    BodyStep::Descend(child) => node = child,
                    BodyStep::Done(found) => return Ok(found),
                },
            }
        }
    }

    fn enter(&mut self, node: usize) -> Result<BodyStep, Failure> {
        self.meter.tick()?;
        let view = self.input.theory.view();
        let found = match view.node(node).map_err(|_| Stop::InvalidProgram)? {
            NodeView::False => true,
            NodeView::Atom(atom) => self.positive(atom)?,
            NodeView::Implies(left, right) => self.negative(left, right)?,
            NodeView::And(_) | NodeView::Or(_) => return self.chain(node),
            NodeView::Threshold(_) | NodeView::ExtremumCut(_) => false,
        };
        Ok(BodyStep::Done(found))
    }

    fn positive(&mut self, atom: usize) -> Result<bool, Failure> {
        self.meter.tick()?;
        if bit(self.input.atom_never, atom) {
            if bit(self.input.atom_sure, atom) {
                return Err(Failure::Declined);
            }
            self.add_condition(atom, false)?;
        } else if self.missing(atom) {
            self.add_atom(atom)?;
        } else {
            return Ok(false);
        }
        Ok(true)
    }

    fn negative(&mut self, left: usize, right: usize) -> Result<bool, Failure> {
        let view = self.input.theory.view();
        self.meter.tick()?;
        if !matches!(
            view.node(right).map_err(|_| Stop::InvalidProgram)?,
            NodeView::False
        ) {
            return Ok(false);
        }
        self.meter.tick()?;
        let NodeView::Atom(atom) = view.node(left).map_err(|_| Stop::InvalidProgram)? else {
            return Ok(false);
        };
        self.meter.tick()?;
        if !bit(self.input.atom_sure, atom) {
            return Ok(false);
        }
        if bit(self.input.atom_never, atom) {
            return Err(Failure::Declined);
        }
        self.add_condition(atom, true)?;
        Ok(true)
    }

    fn chain(&mut self, node: usize) -> Result<BodyStep, Failure> {
        self.meter.tick()?;
        let index = self.input.index;
        let chain = index.chain_of[node].ok_or(Stop::InvalidProgram)?;
        let chain = chain_position(chain);
        let row = &index.chains[chain];
        let Some(&first) = row.leaves(&index.extrema).first() else {
            return Ok(BodyStep::Done(row.disjunction));
        };
        let frame = Frame {
            chain,
            next: 1,
            mark: Mark {
                atoms: self.result.unsupported.len(),
                literals: self.result.conditions.len(),
            },
            disjunction: row.disjunction,
        };
        let other = self.bytes() - capacity_bytes(&self.frames);
        reserve_one(&mut self.frames, other, self.limits.bytes, self.peak_bytes)?;
        self.frames.push(frame);
        Ok(BodyStep::Descend(first))
    }

    fn resume(&mut self, mut found: bool) -> Result<BodyStep, Failure> {
        while let Some(mut frame) = self.frames.pop() {
            self.meter.tick()?;
            if !found {
                self.result.unsupported.truncate(frame.mark.atoms);
                self.result.conditions.truncate(frame.mark.literals);
            }
            // One false conjunct or one unexplainable disjunct decides this
            // frame. Otherwise advance in the existing flattened operand row.
            if found != frame.disjunction {
                continue;
            }
            self.meter.tick()?;
            let index = self.input.index;
            let children = index.chains[frame.chain].leaves(&index.extrema);
            if let Some(&child) = children.get(frame.next) {
                frame.next += 1;
                self.frames.push(frame);
                return Ok(BodyStep::Descend(child));
            }
            found = frame.disjunction;
        }
        Ok(BodyStep::Done(found))
    }
}

fn capacity_bytes<T>(values: &Vec<T>) -> u128 {
    values.capacity() as u128 * size_of::<T>() as u128
}

fn reserve_one<T>(
    values: &mut Vec<T>,
    other_bytes: u128,
    limit: u128,
    peak: &mut u128,
) -> Result<(), Failure> {
    if values.len() == values.capacity() {
        let requested = (values.len() as u128 + 1) * size_of::<T>() as u128;
        if other_bytes.saturating_add(requested) > limit {
            return Err(Failure::Declined);
        }
        values.try_reserve_exact(1).map_err(|_| Stop::Allocation)?;
        let actual = other_bytes.saturating_add(capacity_bytes(values));
        *peak = (*peak).max(actual);
        if actual > limit {
            return Err(Failure::Declined);
        }
    }
    Ok(())
}

fn canonicalize<T: Ord>(values: &mut [T], meter: &mut Meter<'_, '_>) -> Result<(), Failure> {
    for next in 1..values.len() {
        let mut cursor = next;
        while cursor > 0 {
            meter.tick()?;
            if values[cursor - 1] <= values[cursor] {
                break;
            }
            values.swap(cursor - 1, cursor);
            cursor -= 1;
        }
    }
    Ok(())
}
