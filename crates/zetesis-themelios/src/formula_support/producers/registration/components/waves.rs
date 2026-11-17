//! Conservative independence of contiguous component-ready producers.
//!
//! This private, test-only prototype does not execute producers concurrently.
//! It certifies only that each selected producer reads published prerequisites
//! and that its complete component footprint is independent of every other
//! selected producer. An executor additionally needs an admitted operator and
//! private proposal/value storage: this result never permits concurrent mutation
//! of the shared term catalog, support appender or formula builder.
//!
//! The existing ready queue is not an independence certificate. In particular,
//! it can contain a whole negative chain whose later producers read earlier
//! producers' pending output. Such readers remain singleton serial waves here.
//! Ordered queue prefixes preserve the existing publication and readiness order;
//! this module neither advances producer states nor creates a support round.

mod tests;

use std::cell::Ref;
use std::mem::size_of;
use std::num::NonZeroUsize;
use std::ops::Range;

use zetesis_core::TemplateComponentsRef;

use super::{Counters, FormulaFailure, FormulaLimits, Memory, Producer, ProgramSite};
use super::{Registration, Schedule, State, lookup, negated, resolve};

#[derive(Debug)]
struct Footprint {
    reads: Range<usize>,
    writes: Range<usize>,
    complete: bool,
}

/// Complete read/write components for exactly one registered schedule.
///
/// `resolve` already reads all nested rich frames and every head of one
/// producer. Certified negative reads are added explicitly: their omission
/// from ordinary positive delta scheduling is not an independence proof.
struct Inventory<'registration, 'source> {
    schedule: &'registration Schedule<'source>,
    footprints: Vec<Footprint>,
    components: Vec<usize>,
    bytes: usize,
}

/// A borrowed contiguous queue prefix. The queue cannot advance while this
/// borrow is live. Width greater than one establishes component independence,
/// not executor eligibility or completed output. Drop before serially taking
/// the same entries or committing an independently qualified execution.
struct Wave<'schedule> {
    producers: Ref<'schedule, [usize]>,
}

impl<'registration, 'source> Inventory<'registration, 'source> {
    /// Build from the actual schedule, declining when it has no component
    /// inventory. An unresolved component is retained as incomplete evidence,
    /// never erased from an otherwise trusted footprint.
    ///
    /// The caller includes all other live owners in `external`, including the
    /// registration and catalog. Memory counts header, retained capacities and
    /// peak scratch through the existing allocator. Work charges resolution,
    /// every inspected literal and every copied component. Failure publishes
    /// no inventory and retains performed charges.
    fn prepare(
        registration: &'registration Registration<'source>,
        view: TemplateComponentsRef<'_>,
        external: u128,
        (limits, counters, location): (&FormulaLimits, &mut Counters, ProgramSite),
    ) -> Result<Option<Self>, FormulaFailure> {
        counters.work(limits, location)?;
        let Some(schedule) = registration.schedule.as_deref() else {
            return Ok(None);
        };
        let mut memory = Memory::new(size_of::<Self>(), external, limits, counters, location);
        memory.add(0)?;
        let mut footprints = Vec::new();
        let mut components = Vec::new();
        let mut reads = Vec::new();
        let mut writes = Vec::new();
        memory.reserve(&mut footprints, registration.producers.len())?;
        for (index, producer) in registration.producers.iter().enumerate() {
            counters.work(limits, location)?;
            let rule = &registration.rules[index];
            reads.clear();
            writes.clear();
            let mut complete = resolve(
                registration,
                producer,
                rule,
                &schedule.nodes,
                view,
                (&mut reads, &mut writes, &mut memory),
                (limits, counters, rule.location),
            )?;
            if matches!(producer, Producer::Certified(_)) {
                for literal in &rule.body {
                    counters.work(limits, rule.location)?;
                    let Some(pattern) = negated(literal) else {
                        continue;
                    };
                    let predicate = pattern
                        .get(view, limits, counters, rule.location)?
                        .predicate();
                    let component = lookup(
                        schedule.nodes.len(),
                        |position| schedule.nodes[position].signature,
                        predicate,
                        limits,
                        counters,
                        rule.location,
                    )?
                    .and_then(|position| schedule.nodes[position].component);
                    if let Some(component) = component {
                        memory.reserve(&mut reads, 1)?;
                        reads.push(component);
                    } else {
                        complete = false;
                    }
                }
            }
            complete &= !writes.is_empty();
            let reads = append(
                &mut components,
                &reads,
                &mut memory,
                (limits, counters, rule.location),
            )?;
            let writes = append(
                &mut components,
                &writes,
                &mut memory,
                (limits, counters, rule.location),
            )?;
            footprints.push(Footprint {
                reads,
                writes,
                complete,
            });
        }
        memory.release((reads.capacity() + writes.capacity()) * size_of::<usize>());
        Ok(Some(Self {
            schedule,
            footprints,
            components,
            bytes: memory.bytes,
        }))
    }

    /// Return the longest safe contiguous prefix, bounded by `maximum`, of
    /// the existing unconsumed ready queue. Missing evidence produces a serial
    /// singleton, never a skipped producer. Empty means the queue is drained.
    ///
    /// One unit admits each queue/state/footprint visit, each prerequisite read
    /// and each left-component visit and compared component pair. There is no allocation.
    /// A refused check leaves the queue and every producer state untouched.
    fn next(
        &self,
        maximum: NonZeroUsize,
        (limits, counters, location): (&FormulaLimits, &mut Counters, ProgramSite),
    ) -> Result<Option<Wave<'_>>, FormulaFailure> {
        counters.work(limits, location)?;
        let queue = self.schedule.ready.borrow();
        let start = self.schedule.taken.get();
        let Some(&first) = queue.get(start) else {
            return Ok(None);
        };
        let published = self.schedule.reached.borrow();
        let mut end = start + 1;
        if self.published(first, &published, (limits, counters, location))? {
            while end < queue.len() && end - start < maximum.get() {
                counters.work(limits, location)?;
                let candidate = queue[end];
                if !self.published(candidate, &published, (limits, counters, location))? {
                    break;
                }
                let mut independent = true;
                for &earlier in &queue[start..end] {
                    counters.work(limits, location)?;
                    if !self.independent(earlier, candidate, (limits, counters, location))? {
                        independent = false;
                        break;
                    }
                }
                if !independent {
                    break;
                }
                end += 1;
            }
        }
        Ok(Some(Wave {
            producers: Ref::map(queue, |queue| &queue[start..end]),
        }))
    }

    fn published(
        &self,
        producer: usize,
        published: &[bool],
        (limits, counters, location): (&FormulaLimits, &mut Counters, ProgramSite),
    ) -> Result<bool, FormulaFailure> {
        counters.work(limits, location)?;
        let Some(footprint) = self.footprints.get(producer) else {
            return Ok(false);
        };
        if !footprint.complete
            || self
                .schedule
                .state
                .get(producer)
                .is_none_or(|state| state.get() != State::Ready)
        {
            return Ok(false);
        }
        for &read in &self.components[footprint.reads.clone()] {
            counters.work(limits, location)?;
            if published.get(read).copied() != Some(true) {
                return Ok(false);
            }
        }
        // Recursive producers remain serial even if a damaged caller supplied
        // a published bit for their own write component.
        disjoint(
            &self.components[footprint.reads.clone()],
            &self.components[footprint.writes.clone()],
            (limits, counters, location),
        )
    }

    fn independent(
        &self,
        left: usize,
        right: usize,
        (limits, counters, location): (&FormulaLimits, &mut Counters, ProgramSite),
    ) -> Result<bool, FormulaFailure> {
        if left == right {
            return Ok(false);
        }
        let left = &self.footprints[left];
        let right = &self.footprints[right];
        for (first, second) in [
            (&left.writes, &right.writes),
            (&left.writes, &right.reads),
            (&left.reads, &right.writes),
        ] {
            if !disjoint(
                &self.components[first.clone()],
                &self.components[second.clone()],
                (limits, counters, location),
            )? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn append(
    target: &mut Vec<usize>,
    source: &[usize],
    memory: &mut Memory<'_>,
    (limits, counters, location): (&FormulaLimits, &mut Counters, ProgramSite),
) -> Result<Range<usize>, FormulaFailure> {
    memory.reserve(target, source.len())?;
    let start = target.len();
    for &component in source {
        counters.work(limits, location)?;
        target.push(component);
    }
    Ok(start..target.len())
}

fn disjoint(
    first: &[usize],
    second: &[usize],
    (limits, counters, location): (&FormulaLimits, &mut Counters, ProgramSite),
) -> Result<bool, FormulaFailure> {
    for &left in first {
        counters.work(limits, location)?;
        for &right in second {
            counters.work(limits, location)?;
            if left == right {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
