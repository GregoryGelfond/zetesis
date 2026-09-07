//! An explicitly sized, reused CPU pool for exact candidate membership batches.

use std::num::NonZeroUsize;

use rayon::prelude::*;
use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, Theory};

use crate::FormulaBenchmarkError;
use crate::formula_completion::{Membership, native_with_control};
use crate::formula_fixtures::reserve;

pub(super) struct FormulaPool {
    pool: rayon::ThreadPool,
    max_candidates: usize,
}

impl FormulaPool {
    pub(super) fn new(
        workers: NonZeroUsize,
        max_candidates: usize,
    ) -> Result<Self, FormulaBenchmarkError> {
        if workers.get() > 64 || !(1..=4096).contains(&max_candidates) {
            return Err(FormulaBenchmarkError::Dimensions);
        }
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers.get())
            .thread_name(|index| format!("zetesis-formula-{index}"))
            .build()
            .map_err(FormulaBenchmarkError::Pool)?;
        Ok(Self {
            pool,
            max_candidates,
        })
    }

    pub(super) fn workers(&self) -> usize {
        self.pool.current_num_threads()
    }

    // Each candidate retains the scalar search and verification work ceilings.
    // All jobs join before an ordered result or error is returned. A worker
    // failure neither publishes a partial vector nor cancels the caller's control.
    pub(super) fn check_batch(
        &self,
        theory: &Theory,
        candidates: &[Interpretation],
        work: u64,
        control: &Control,
    ) -> Result<Vec<Membership>, FormulaBenchmarkError> {
        poll(control)?;
        if candidates.len() > self.max_candidates {
            return Err(FormulaBenchmarkError::Dimensions);
        }
        for candidate in candidates {
            poll(control)?;
            if !theory.same_instance(candidate.theory()) {
                return Err(FormulaBenchmarkError::Incomplete(
                    zetesis_sat::Incomplete::WrongTheory,
                ));
            }
        }
        // Indexed collection preserves input order and reuses this fallibly
        // reserved capacity. Pool creation is excluded from batch measurements;
        // dispatch, per-candidate checking and these output allocations are included.
        let mut outcomes = reserve(candidates.len())?;
        self.pool.install(|| {
            candidates
                .par_iter()
                .map(|candidate| native_with_control(theory, candidate, work, control))
                .collect_into_vec(&mut outcomes);
        });
        poll(control)?;
        let mut completed = reserve(candidates.len())?;
        for outcome in outcomes {
            poll(control)?;
            completed.push(outcome?);
        }
        Ok(completed)
    }
}

fn poll(control: &Control) -> Result<(), FormulaBenchmarkError> {
    control
        .poll()
        .map_err(|error| FormulaBenchmarkError::Incomplete(error.into()))
}

#[cfg(test)]
#[path = "../tests/support/formula_parallel.rs"]
mod tests;
