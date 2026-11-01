//! Iterative occurrence traversal of nested conjunctions.
//!
//! Frames borrow the exact admitted owner's children. Every occurrence is read;
//! no seen-node set removes repetitions. The caller interprets each non-And
//! leaf under its own source grammar, with the same storage and work authority.

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

pub(super) struct Conjunction<'a> {
    values: Vec<Frame<'a>>,
}

impl<'a> Conjunction<'a> {
    pub(super) fn new(
        storage: &mut Storage,
        budget: &mut Budget<'_, '_>,
    ) -> Result<Self, BudgetError> {
        Ok(Self {
            values: storage.reserve(0, budget)?,
        })
    }

    pub(super) fn release(self, storage: &mut Storage) -> Result<(), BudgetError> {
        storage.release(self.values)
    }

    pub(super) fn walk(
        &mut self,
        theory: &'a Theory,
        body: usize,
        storage: &mut Storage,
        budget: &mut Budget<'_, '_>,
        visit: impl FnMut(
            usize,
            NodeView<'a>,
            &mut Storage,
            &mut Budget<'_, '_>,
        ) -> Result<bool, BudgetError>,
    ) -> Result<bool, BudgetError> {
        budget.charge(1)?;
        let view = budget.node(theory, body)?;
        self.walk_entered(theory, (body, view), storage, budget, visit)
    }

    /// Continue from the caller's already-charged view of this exact owner and
    /// node. Borrowing the view removes only the repeated entry read; every
    /// child occurrence is still read, in order, through the same budget.
    /// The caller must obtain `view` from `budget.node(theory, body)`.
    pub(super) fn walk_view(
        &mut self,
        theory: &'a Theory,
        entry: (usize, NodeView<'a>),
        storage: &mut Storage,
        budget: &mut Budget<'_, '_>,
        visit: impl FnMut(
            usize,
            NodeView<'a>,
            &mut Storage,
            &mut Budget<'_, '_>,
        ) -> Result<bool, BudgetError>,
    ) -> Result<bool, BudgetError> {
        budget.charge(1)?;
        self.walk_entered(theory, entry, storage, budget, visit)
    }

    fn walk_entered(
        &mut self,
        theory: &'a Theory,
        (mut current, mut view): (usize, NodeView<'a>),
        storage: &mut Storage,
        budget: &mut Budget<'_, '_>,
        mut visit: impl FnMut(
            usize,
            NodeView<'a>,
            &mut Storage,
            &mut Budget<'_, '_>,
        ) -> Result<bool, BudgetError>,
    ) -> Result<bool, BudgetError> {
        loop {
            budget.charge(1)?;
            if let NodeView::And(children) = view {
                budget.charge(1)?;
                if self.values.len() >= theory.nodes().len() {
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
            let Some(next) = self.next(budget)? else {
                budget.charge(1)?;
                return Ok(true);
            };
            current = next;
            view = budget.node(theory, current)?;
        }
    }

    fn next(&mut self, budget: &mut Budget<'_, '_>) -> Result<Option<usize>, BudgetError> {
        loop {
            budget.charge(1)?;
            let Some(frame) = self.values.last_mut() else {
                budget.charge(1)?;
                return Ok(None);
            };
            budget.charge(1)?;
            let Some((child, remaining)) = frame.remaining.split_first() else {
                budget.charge(1)?;
                let _ = self.values.pop();
                continue;
            };
            budget.charge(2)?; // child read and backward-reference comparison
            let child = *child;
            if child >= frame.parent {
                budget.charge(1)?;
                return Err(BudgetError::Source(AdmissionError::Edge));
            }
            budget.charge(4)?; // advance the slice, publish child and result tag
            frame.remaining = remaining;
            return Ok(Some(child));
        }
    }
}
