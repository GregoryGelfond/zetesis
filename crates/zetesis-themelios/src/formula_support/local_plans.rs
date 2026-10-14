//! Immutable local join plans owned by one support snapshot.
//!
//! A key borrows its nonempty IR frame for the snapshot lifetime. Address and
//! length identify that frame; variables and the exact prefix bound pattern
//! cover the remaining planner inputs. Components and row counts belong to
//! this snapshot, and local joins always offer full rows (no delta pivot).
//! Different call sites may share a frame; a different prefix shape uses an
//! owned plan without replacing the first entry. No bindings or rows are kept.
//!
//! Entries are sorted by frame address, length and variable count. Lookup is
//! O(log F + P), insertion O(F + P), for F frames and P prefix slots. Charges
//! count actual comparisons, inspected slots and moved entries, so explicitly
//! bounded planning may depend on heap layout. Plans and join order do not.

mod ir;
pub(crate) use ir::Ir;
#[cfg(test)]
pub(super) mod tests;

use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

use super::{Computation, Context, GroundingWork, StorageLease, Support, order, storage};
use crate::ProgramSite;
use crate::expansion::Budget;
use crate::formula_binding::Binding;
use crate::formula_ir::LiteralIr;
use crate::grounding_observer::Event;
use crate::{ExpansionResource, FormulaFailure};

/// Shared plans are immutable. Only an owned component plan may be re-decided.
pub(super) enum Plan<'a> {
    Owned(order::Plan<'a>),
    Borrowed(&'a order::Plan<'a>),
    Shared(Rc<order::Plan<'a>>),
}
impl<'a> Deref for Plan<'a> {
    type Target = order::Plan<'a>;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Owned(plan) => plan,
            Self::Borrowed(plan) => plan,
            Self::Shared(plan) => plan,
        }
    }
}

pub(super) struct Table<'source> {
    state: RefCell<State<'source>>,
}
struct State<'source> {
    entries: Vec<Entry<'source>>,
    lease: StorageLease,
}
struct Entry<'source> {
    literals: &'source [LiteralIr],
    variables: usize,
    bound: Vec<u8>,
    plan: Rc<order::Plan<'source>>,
}
impl Entry<'_> {
    fn key(&self) -> (usize, usize, usize) {
        key(self.literals, self.variables)
    }
}
fn key(literals: &[LiteralIr], variables: usize) -> (usize, usize, usize) {
    (literals.as_ptr() as usize, literals.len(), variables)
}
fn charge_planning(
    budget: &mut Budget,
    amount: usize,
    location: ProgramSite,
) -> Result<(), FormulaFailure> {
    budget.charge(ExpansionResource::TermWork, amount as u128, location)?;
    Ok(())
}

fn same_binding_pattern(
    entry: &Entry<'_>,
    prefix: &Binding,
    budget: &mut Budget,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    charge_planning(budget, 1, location)?;
    if entry.bound.len() != prefix.len() {
        return Ok(false);
    }
    for (slot, &bound) in entry.bound.iter().enumerate() {
        charge_planning(budget, 1, location)?;
        if (bound != 0) != prefix.is_bound(slot, location)? {
            return Ok(false);
        }
    }
    Ok(true)
}

impl<'source> Table<'source> {
    pub(super) fn new(workspace: &storage::Workspace) -> Self {
        // The inline table/header is already counted by Support's header.
        Self {
            state: RefCell::new(State {
                entries: Vec::new(),
                lease: workspace.lease(),
            }),
        }
    }

    /// None means the frame already has another bound pattern. A failed build
    /// publishes no entry; no cell borrow spans preparation or a live join.
    pub(super) fn plan(
        &self,
        literals: &'source [LiteralIr],
        prefix: &Binding,
        variables: usize,
        support: &Support<'source>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<Rc<order::Plan<'source>>>, FormulaFailure> {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let sought = key(literals, variables);
        let insertion = {
            let state = self.state.borrow();
            let (mut left, mut right) = (0, state.entries.len());
            while left < right {
                charge_planning(budget, 1, location)?;
                let middle = left + (right - left) / 2;
                match state.entries[middle].key().cmp(&sought) {
                    std::cmp::Ordering::Less => left = middle + 1,
                    std::cmp::Ordering::Greater => right = middle,
                    std::cmp::Ordering::Equal => {
                        let entry = &state.entries[middle];
                        if !same_binding_pattern(entry, prefix, budget, location)? {
                            return Ok(None);
                        }
                        counters.record(Event::LocalPlanReuse);
                        #[cfg(test)]
                        tests::hit();
                        return Ok(Some(Rc::clone(&entry.plan)));
                    }
                }
            }
            left
        };
        let plan = order::Plan::new(
            literals,
            prefix,
            variables,
            order::SourceRows {
                relations: support,
                pivot: None,
            },
            budget,
            location,
            Some(&mut |capacity| {
                computation.preparation_capacity(capacity, limits, counters, location)
            }),
        )?;
        // Temporary ownership keeps the finished plan and its bound-pattern
        // copy in the live account while the entry array is reserved.
        let mut pending = computation.lease();
        let plan_bytes = usize::try_from(plan.retained_bytes()).map_err(|_| overflow(location))?;
        pending.observe(plan_bytes, location)?;
        computation.storage_observed(&pending, 0, 0, limits, counters, location)?;
        let mut bound = Vec::new();
        storage::reserve_exact(
            &mut bound,
            prefix.len(),
            &mut pending,
            plan_bytes,
            Context::new(computation, limits, counters, location),
        )?;
        for slot in 0..prefix.len() {
            charge_planning(budget, 1, location)?;
            bound.push(u8::from(prefix.is_bound(slot, location)?));
        }
        // Rc owns one Plan header and its strong/weak counts. No clone of the
        // plan vectors is made, and joins charge only their Rc handle inline.
        let header = shared_header(location)?;
        computation.preparation_capacity(
            order::Capacity::Requested(header as u128),
            limits,
            counters,
            location,
        )?;
        let plan = Rc::new(plan);
        pending.observe(
            pending
                .bytes()
                .checked_add(header)
                .ok_or_else(|| overflow(location))?,
            location,
        )?;
        computation.storage_observed(&pending, pending.bytes(), 0, limits, counters, location)?;
        let mut state = self.state.borrow_mut();
        let State { entries, lease } = &mut *state;
        let other = lease.bytes() - entries.capacity() * size_of::<Entry<'_>>();
        storage::reserve(
            entries,
            1,
            lease,
            other,
            Context::new(computation, limits, counters, location),
        )?;
        charge_planning(budget, entries.len() - insertion, location)?;
        pending.transfer_to(lease, location)?;
        entries.insert(
            insertion,
            Entry {
                literals,
                variables,
                bound,
                plan: Rc::clone(&plan),
            },
        );
        counters.record(Event::LocalPlanPreparation);
        Ok(Some(plan))
    }
}

/// Rc's pinned runtime uses two count words followed by its value. Compose
/// the layouts rather than assuming the value has pointer alignment. Like
/// other workspace owners, this counts requested allocation bytes, not the
/// allocator's private bookkeeping.
fn shared_header(location: ProgramSite) -> Result<usize, FormulaFailure> {
    std::alloc::Layout::new::<[usize; 2]>()
        .extend(std::alloc::Layout::new::<order::Plan<'_>>())
        .map(|(layout, _)| layout.pad_to_align().size())
        .map_err(|_| overflow(location))
}

fn overflow(location: ProgramSite) -> FormulaFailure {
    crate::formula_binding::assignment(
        zetesis_core::catalog::AssignmentError::Storage(zetesis_core::catalog::Error::Overflow),
        location,
    )
}
