//! The selected original-root grammar, shared by preparation and replay.
//!
//! The proposer stores premises; the checker revisits their original source.
//! Sharing this decoder does not let either consumer supply the other's truth.

use crate::{AdmissionError, NodeView, Theory};

use super::super::{
    budget::{Budget, BudgetError},
    conjunction::Conjunction,
    source,
    storage::Storage,
};

pub(super) struct Rule {
    pub head: usize,
    pub body: Option<usize>,
}

pub(super) fn rule(
    theory: &Theory,
    ordinal: usize,
    budget: &mut Budget<'_, '_>,
) -> Result<Option<Rule>, BudgetError> {
    budget.charge(2)?;
    let root = *theory
        .roots()
        .get(ordinal)
        .ok_or(BudgetError::Source(AdmissionError::Root))?;
    let view = budget.node(theory, root)?;
    budget.charge(1)?;
    let (head, body) = match view {
        NodeView::Atom(atom) => (atom, None),
        NodeView::Implies(body, head) => {
            let head = budget.node(theory, head)?;
            budget.charge(1)?;
            let NodeView::Atom(atom) = head else {
                budget.charge(1)?;
                return Ok(None);
            };
            (atom, Some(body))
        }
        _ => {
            budget.charge(1)?;
            return Ok(None);
        }
    };
    budget.charge(1)?;
    if head >= theory.atom_count() {
        budget.charge(1)?;
        return Err(BudgetError::Source(AdmissionError::Atom));
    }
    budget.charge(3 + u64::from(body.is_some()))?; // head, body tag/coordinate and result tag
    Ok(Some(Rule { head, body }))
}

/// Visit every positive premise occurrence; exact TRUE contributes no atom.
/// Unsupported leaves stop this source. A caller may also stop on a false
/// premise during replay. The walker is released on every returned path.
pub(super) fn premises(
    theory: &Theory,
    body: Option<usize>,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_>,
    mut visit: impl FnMut(usize, &mut Storage, &mut Budget<'_, '_>) -> Result<bool, BudgetError>,
) -> Result<bool, BudgetError> {
    budget.charge(1)?;
    let Some(body) = body else {
        budget.charge(1)?;
        return Ok(true);
    };
    let mut conjunction = Conjunction::new(storage, budget)?;
    let result = conjunction.walk(theory, body, storage, budget, |_, view, storage, budget| {
        budget.charge(1)?;
        if let NodeView::Atom(atom) = view {
            budget.charge(1)?;
            if atom >= theory.atom_count() {
                budget.charge(1)?;
                return Err(BudgetError::Source(AdmissionError::Atom));
            }
            visit(atom, storage, budget)
        } else {
            source::canonical_truth(theory, view, budget)
        }
    });
    let released = conjunction.release(storage);
    result.and_then(|value| released.map(|()| value))
}
