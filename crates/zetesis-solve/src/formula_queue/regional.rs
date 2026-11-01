//! Ownership of checked formula prefixes until the oldest slate's exact completion.

use std::mem::size_of;

use zetesis_algebra::{region::Region, words::Words};
use zetesis_ferraris::Interpretation;
use zetesis_stable::{
    BatchVerdict, CompletionExecutor, Incomplete, RegionalOutcome, SlateTicket, StableModels,
};

use crate::{SolveError, formula_execution::Failure};

/// The reserved verdict slots are also present in the pending-row account.
/// Only excess slot capacity and separately owned regional storage add to it.
struct RegionalSlate {
    roots: Vec<(usize, Region)>,
    authorized: Words<u64>,
    conclusions: Vec<BatchVerdict>,
    root_payload: u128,
    extra: u128,
}

pub(crate) struct CheckedRows {
    pub(super) verdicts: Vec<BatchVerdict>,
    regional: Option<RegionalSlate>,
    pub(crate) count: usize,
}

fn space(ceiling: u64, terms: &[u128]) -> Result<u64, Failure> {
    let remaining = terms
        .iter()
        .try_fold(u128::from(ceiling), |left, term| left.checked_sub(*term))
        .ok_or(Failure::Search(Incomplete::PendingBytes))?;
    Ok(u64::try_from(remaining).expect("the remaining u64 allowance fits"))
}

pub(super) fn after_pending(available: u64, pending: u128) -> Result<u64, Failure> {
    space(available, &[pending])
}

fn reserve<T>(values: &mut Vec<T>, n: usize) -> Result<u128, Failure> {
    values
        .try_reserve_exact(n)
        .map_err(|_| Failure::Search(Incomplete::Allocation))?;
    Ok((values.capacity() - n) as u128 * size_of::<T>() as u128)
}

impl CheckedRows {
    pub(super) fn new(count: usize, regional: Option<(u64, u128, u64)>) -> Result<Self, Failure> {
        let mut verdicts = Vec::new();
        let regional = if let Some((limit, pending, resident)) = regional {
            let words = count.div_ceil(64);
            let requested = (size_of::<Self>() - size_of::<Vec<BatchVerdict>>()) as u128
                + count as u128
                    * (size_of::<(usize, Region)>() + size_of::<BatchVerdict>()) as u128
                + words as u128 * size_of::<u64>() as u128;
            let room = space(limit, &[pending, u128::from(resident), requested])?;
            let mut excess = reserve(&mut verdicts, count)?;
            space(room, &[excess])?;
            let mut roots = Vec::new();
            excess += reserve(&mut roots, count)?;
            space(room, &[excess])?;
            let mut authorized = Vec::new();
            excess += reserve(&mut authorized, words)?;
            space(room, &[excess])?;
            authorized.resize(words, 0);
            let mut conclusions = Vec::new();
            excess += reserve(&mut conclusions, count)?;
            space(room, &[excess])?;
            Some(RegionalSlate {
                roots,
                authorized: authorized.into(),
                conclusions,
                root_payload: 0,
                extra: requested + excess,
            })
        } else {
            reserve(&mut verdicts, count)?;
            None
        };
        Ok(Self {
            verdicts,
            regional,
            count,
        })
    }

    pub(crate) fn classified(&mut self, verdict: BatchVerdict) -> Result<(), Failure> {
        if self.verdicts.len() >= self.count {
            return Err(Failure::Run(SolveError::FormulaBatchShape {
                expected: self.count,
                actual: self.verdicts.len().saturating_add(1),
            }));
        }
        self.verdicts.push(verdict);
        Ok(())
    }

    /// Called only in order after the GPU's subject-bound receipt authenticates.
    pub(crate) fn projected(&mut self, root: Region) -> Result<(), Failure> {
        let slot = self.verdicts.len();
        let regional = self
            .regional
            .as_mut()
            .filter(|_| slot < self.count)
            .ok_or(Failure::Run(SolveError::FormulaBatchShape {
                expected: self.count,
                actual: slot.saturating_add(1),
            }))?;
        let payload = root.retained_bytes() - size_of::<Region>() as u128;
        regional.root_payload = regional
            .root_payload
            .checked_add(payload)
            .ok_or(Failure::Search(Incomplete::CounterOverflow))?;
        regional.authorized.insert(slot);
        regional.roots.push((slot, root));
        self.verdicts.push(BatchVerdict::Residual);
        Ok(())
    }

    /// Before a submission, or before its overlap, subtract only owners which
    /// the device plan does not already count. Lookahead admits its own P.
    pub(super) fn remaining(
        &self,
        ceiling: u64,
        pending: u128,
        device: u64,
    ) -> Result<u64, Failure> {
        match &self.regional {
            Some(r) => space(
                ceiling,
                &[pending, r.extra, r.root_payload, u128::from(device)],
            ),
            None => Ok(ceiling),
        }
    }

    pub(super) fn finish(
        self,
        models: &mut StableModels,
        ticket: SlateTicket,
        completion: &mut CompletionExecutor,
    ) -> Result<Vec<Interpretation>, Failure> {
        let Some(mut state) = self.regional else {
            return models
                .commit_slate(ticket, self.verdicts, completion)
                .map_err(Failure::from);
        };
        let pass = models
            .begin_regional_pass(ticket, self.verdicts, state.roots, completion)
            .map_err(Failure::from)?;
        for (slot, outcome) in pass.outcomes() {
            if slot >= self.count || !state.authorized.contains(slot) {
                return Err(Failure::Search(Incomplete::InvalidWitness));
            }
            let whole = match outcome {
                RegionalOutcome::ValidatedCountermodel => BatchVerdict::Refuted,
                RegionalOutcome::NoWitnessInSuppliedRegion => BatchVerdict::NoProperSubset,
            };
            state.conclusions.push(whole);
        }
        pass.finish(state.conclusions).map_err(Failure::from)
    }
}

#[cfg(test)]
mod tests;
