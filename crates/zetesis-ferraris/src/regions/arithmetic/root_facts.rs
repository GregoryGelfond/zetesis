//! Checked positive consequences of selected original roots.
//!
//! A proposal carries an ordered original-root witness per fresh atom. Replay
//! starts empty and uses only earlier checked facts, so an unsupported cycle
//! cannot certify itself. The original Theory and its reduct are unchanged.

use zetesis_algebra::Quota;

mod prepare;
mod reader;
#[cfg(test)]
mod tests;

use std::mem::size_of;

use crate::{AdmissionError, Theory};
use zetesis_algebra::{Stop, words::Words};

use super::{
    budget::{Budget, BudgetError},
    storage::Storage,
    work::Phase,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Step {
    root: usize,
    atom: usize,
}

/// Only independent replay constructs this exact-owner evidence.
#[derive(Debug)]
pub(super) struct CheckedRootFacts {
    owner: Theory,
    steps: Vec<Step>,
    words: Words<u64>,
}

impl CheckedRootFacts {
    /// The record of a class to which no original root fact belongs: the
    /// owner's identity alone. Admitted to `storage` before it is returned.
    pub(super) fn none<Q: Quota + ?Sized>(
        theory: &Theory,
        storage: &mut Storage,
        budget: &mut Budget<'_, '_, Q>,
    ) -> Result<Self, BudgetError<Q::Error>> {
        storage.admit_record(size_of::<Self>() as u128, budget)?;
        budget.charge(1)?;
        Ok(Self {
            owner: theory.clone(),
            steps: Vec::new(),
            words: Words::new(Vec::new()),
        })
    }

    pub(super) fn owner(&self) -> &Theory {
        &self.owner
    }

    pub(super) fn belongs_to(&self, theory: &Theory) -> bool {
        self.owner.same_instance(theory)
    }

    /// Consumers authenticate the owner once before interpreting coordinates.
    pub(super) fn contains<Q: Quota + ?Sized>(
        &self,
        atom: usize,
        budget: &mut Budget<'_, '_, Q>,
    ) -> Result<bool, BudgetError<Q::Error>> {
        membership(&self.owner, &self.words, atom, budget)
    }

    pub(super) fn retained_bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + self.steps.capacity() as u128 * size_of::<Step>() as u128
            + self.words.capacity() as u128 * size_of::<u64>() as u128
    }
}

/// Prepare selected-source witnesses and independently replay them. A refusal
/// returns no evidence prefix. The caller discards its preparation on failure.
pub(super) fn build<Q: Quota + ?Sized>(
    theory: &Theory,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<CheckedRootFacts, BudgetError<Q::Error>> {
    let steps = budget.in_phase(Phase::Recognition, |budget| {
        prepare::propose(theory, storage, budget)
    })?;
    let words = budget.in_phase(Phase::Provenance, |budget| {
        replay(theory, theory, &steps, storage, budget)
    })?;
    let Some(words) = words else {
        budget.in_phase(Phase::Provenance, |budget| budget.charge(1))?;
        return Err(BudgetError::Stopped(Stop::InvalidProgram));
    };
    budget.in_phase(Phase::Recognition, |budget| {
        storage.admit_record(
            (size_of::<CheckedRootFacts>() - size_of::<Vec<Step>>() - size_of::<Vec<u64>>())
                as u128,
            budget,
        )?;
        budget.charge(2)?; // exact owner and successful result tag
        Ok(CheckedRootFacts {
            owner: theory.clone(),
            steps,
            words,
        })
    })
}

/// Check actual original witnesses, never a proposed final mask or graph.
/// A well-formed but unsupported/false witness rejects the proposal; a bad
/// source coordinate keeps the canonical typed admission error.
fn replay<Q: Quota + ?Sized>(
    theory: &Theory,
    owner: &Theory,
    steps: &[Step],
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<Words<u64>>, BudgetError<Q::Error>> {
    budget.charge(1)?;
    if !theory.same_instance(owner) {
        budget.charge(1)?;
        return Ok(None);
    }
    let mut words =
        Words::new(storage.filled(theory.atom_count().div_ceil(64), 0_u64, 1, budget)?);
    for step in steps {
        budget.charge(3)?; // occurrence, root and claimed head
        let Some(rule) = reader::rule(theory, step.root, budget)? else {
            storage.release(words.into_storage())?;
            budget.charge(1)?;
            return Ok(None);
        };
        budget.charge(1)?;
        if rule.head != step.atom || membership(theory, &words, step.atom, budget)? {
            storage.release(words.into_storage())?;
            budget.charge(1)?;
            return Ok(None);
        }
        let ready = reader::premises(theory, rule.body, storage, budget, |atom, _, budget| {
            membership(theory, &words, atom, budget)
        })?;
        budget.charge(1)?;
        if !ready {
            storage.release(words.into_storage())?;
            budget.charge(1)?;
            return Ok(None);
        }
        budget.charge(4)?; // divide, remainder, shift and OR (including its load/store)
        words.insert(step.atom);
    }
    budget.charge(1)?;
    Ok(Some(words))
}

fn membership<Q: Quota + ?Sized>(
    theory: &Theory,
    words: &[u64],
    atom: usize,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<bool, BudgetError<Q::Error>> {
    budget.charge(1)?;
    if atom >= theory.atom_count() {
        budget.charge(1)?;
        return Err(BudgetError::Source(AdmissionError::Atom));
    }
    budget.charge(5)?; // divide, remainder, shift, AND and truth test
    Ok(Words::<u64, _>::new(words).contains(atom))
}
