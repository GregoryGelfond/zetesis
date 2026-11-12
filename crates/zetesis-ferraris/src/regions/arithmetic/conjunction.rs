//! Iterative occurrence traversal of nested conjunctions.
//!
//! Frames borrow the exact admitted owner's children. Every occurrence is read;
//! no seen-node set removes repetitions. The caller interprets each non-And
//! leaf under its own source grammar, with the same storage and work authority.
//! The outer frame borrows its child slice locally; only nested frames need
//! a vector. Both owners are accounted while live and advance identically.

use zetesis_algebra::{Quota, Stop};

use crate::{AdmissionError, NodeView, Theory};

use super::{
    budget::{Budget, BudgetError},
    storage::Storage,
};

#[derive(Clone, Copy)]
struct Frame<'a> {
    parent: usize,
    remaining: &'a [usize],
}

impl Frame<'_> {
    fn next<Q: Quota + ?Sized>(
        &mut self,
        budget: &mut Budget<'_, '_, Q>,
    ) -> Result<Option<usize>, BudgetError<Q::Error>> {
        budget.charge(1)?;
        let Some((child, remaining)) = self.remaining.split_first() else {
            budget.charge(1)?;
            return Ok(None);
        };
        budget.charge(2)?; // child read and backward-reference comparison
        let child = *child;
        if child >= self.parent {
            budget.charge(1)?;
            return Err(BudgetError::Source(AdmissionError::Edge));
        }
        budget.charge(4)?; // advance the slice, publish child and result tag
        self.remaining = remaining;
        Ok(Some(child))
    }
}

pub(super) struct Conjunction<'a> {
    values: Vec<Frame<'a>>,
}

impl<'a> Conjunction<'a> {
    pub(super) fn new<Q: Quota + ?Sized>(
        storage: &mut Storage,
        budget: &mut Budget<'_, '_, Q>,
    ) -> Result<Self, BudgetError<Q::Error>> {
        Ok(Self {
            values: storage.reserve(0, budget)?,
        })
    }

    pub(super) fn release(self, storage: &mut Storage) -> Result<(), Stop> {
        storage.release(self.values)
    }

    pub(super) fn walk<Q: Quota + ?Sized>(
        &mut self,
        theory: &'a Theory,
        body: usize,
        storage: &mut Storage,
        budget: &mut Budget<'_, '_, Q>,
        visit: impl FnMut(
            usize,
            NodeView<'a>,
            &mut Storage,
            &mut Budget<'_, '_, Q>,
        ) -> Result<bool, BudgetError<Q::Error>>,
    ) -> Result<bool, BudgetError<Q::Error>> {
        budget.charge(1)?;
        let view = budget.node(theory, body)?;
        self.walk_entered(theory, (body, view), storage, budget, visit)
    }

    /// Continue from the caller's already-charged view of this exact owner and
    /// node. Borrowing the view removes only the repeated entry read; every
    /// child occurrence is still read, in order, through the same budget.
    /// The caller must obtain `view` from `budget.node(theory, body)`.
    pub(super) fn walk_view<Q: Quota + ?Sized>(
        &mut self,
        theory: &'a Theory,
        entry: (usize, NodeView<'a>),
        storage: &mut Storage,
        budget: &mut Budget<'_, '_, Q>,
        visit: impl FnMut(
            usize,
            NodeView<'a>,
            &mut Storage,
            &mut Budget<'_, '_, Q>,
        ) -> Result<bool, BudgetError<Q::Error>>,
    ) -> Result<bool, BudgetError<Q::Error>> {
        // A walk borrowed from an earlier read begins no work once stopped.
        budget.poll()?;
        budget.charge(1)?;
        self.walk_entered(theory, entry, storage, budget, visit)
    }

    fn walk_entered<Q: Quota + ?Sized>(
        &mut self,
        theory: &'a Theory,
        (current, view): (usize, NodeView<'a>),
        storage: &mut Storage,
        budget: &mut Budget<'_, '_, Q>,
        mut visit: impl FnMut(
            usize,
            NodeView<'a>,
            &mut Storage,
            &mut Budget<'_, '_, Q>,
        ) -> Result<bool, BudgetError<Q::Error>>,
    ) -> Result<bool, BudgetError<Q::Error>> {
        budget.charge(1)?;
        let NodeView::And(children) = view else {
            let accepted = visit(current, view, storage, budget)?;
            budget.charge(1)?;
            return Ok(accepted);
        };
        budget.charge(1)?;
        if self.values.len() >= theory.nodes().len() {
            budget.charge(1)?;
            return Err(BudgetError::Source(AdmissionError::Edge));
        }
        storage.with_record(size_of::<Frame<'_>>() as u128, budget, |storage, budget| {
            budget.charge(3)?; // parent and borrowed slice's two words
            let mut outer = Frame {
                parent: current,
                remaining: children,
            };
            self.walk_children(theory, &mut outer, storage, budget, visit)
        })
    }

    /// The local frame is the bottom of the old stack; every retained vector
    /// frame is above it. Exhausting a frame discards no unvisited occurrence.
    /// A complete walk empties the vector, so its capacity can be reused. A
    /// rejected or failed walk returns to its owner for release, never resume.
    fn walk_children<Q: Quota + ?Sized>(
        &mut self,
        theory: &'a Theory,
        outer: &mut Frame<'a>,
        storage: &mut Storage,
        budget: &mut Budget<'_, '_, Q>,
        mut visit: impl FnMut(
            usize,
            NodeView<'a>,
            &mut Storage,
            &mut Budget<'_, '_, Q>,
        ) -> Result<bool, BudgetError<Q::Error>>,
    ) -> Result<bool, BudgetError<Q::Error>> {
        loop {
            let Some(current) = self.next(outer, budget)? else {
                budget.charge(1)?;
                return Ok(true);
            };
            let view = budget.node(theory, current)?;
            budget.charge(1)?;
            if let NodeView::And(children) = view {
                budget.charge(2)?; // include the local frame, then compare depth
                if self.values.len().saturating_add(1) >= theory.nodes().len() {
                    budget.charge(1)?;
                    return Err(BudgetError::Source(AdmissionError::Edge));
                }
                storage.push(
                    &mut self.values,
                    Frame {
                        parent: current,
                        remaining: children,
                    },
                    3,
                    budget,
                )?;
            } else if !visit(current, view, storage, budget)? {
                budget.charge(1)?;
                return Ok(false);
            }
        }
    }

    fn next<Q: Quota + ?Sized>(
        &mut self,
        outer: &mut Frame<'a>,
        budget: &mut Budget<'_, '_, Q>,
    ) -> Result<Option<usize>, BudgetError<Q::Error>> {
        loop {
            budget.charge(1)?;
            let frame = self.values.last_mut().unwrap_or(outer);
            if let Some(child) = frame.next(budget)? {
                return Ok(Some(child));
            }
            budget.charge(1)?; // pop a nested frame, or finish the local frame
            if self.values.pop().is_none() {
                return Ok(None);
            }
        }
    }
}
