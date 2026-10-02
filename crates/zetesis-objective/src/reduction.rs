//! Numeric reduction after a caller has established unique-key eligibility.

use crate::evaluate::{Work, final_cost};
use crate::{Error, Limits, ObjectiveProgram, Score, Statistics, Stop};
use zetesis_cpu::Cancellation;

/// A complete numeric score and its actual reduction work. No contribution
/// tuples are retained, and this value establishes no model membership.
#[derive(Debug)]
pub struct CostReduction {
    score: Score,
    work: u64,
}

impl CostReduction {
    /// Transfer the completed fixed-priority score.
    #[must_use]
    pub fn into_score(self) -> Score {
        self.score
    }

    /// Accepted work, including each offered key whether selected or not.
    #[must_use]
    pub const fn work(&self) -> u64 {
        self.work
    }
}

/// Sum already normalized, globally distinct keys at each admitted priority.
/// The caller establishes the keys' identity and eligibility; each iterator
/// item is one key, with `None` denoting an inactive key. Repeated weights remain
/// separate keys. This operation never deduplicates by weight or creates support.
///
/// The callback is invoked once per distinct descending program priority. It
/// must provide each key at that priority exactly once. One work unit admits
/// initialization, each priority, every iterator probe (including exhaustion), and each
/// final checked cost conversion. Costs accumulate in i128, then convert to i64;
/// inactive priority slots and objective presence remain observable.
///
/// # Errors
/// Cancellation, work exhaustion, allocation or numeric overflow returns the
/// accepted work prefix without a partial score. Join/evidence statistics in
/// the error remain zero because this operation performs neither.
pub fn reduce_costs<I: IntoIterator<Item = Option<i32>>>(
    program: &ObjectiveProgram,
    mut keys: impl FnMut(i32) -> I,
    max_work: u64,
    cancellation: &Cancellation,
) -> Result<CostReduction, Error> {
    let mut work = Work {
        limits: Limits {
            max_work,
            ..Limits::default()
        },
        cancellation,
        statistics: Statistics::default(),
        template: None,
    };
    work.tick()?;
    let mut costs = work.reserve(program.priorities().len())?;
    for &priority in program.priorities() {
        work.tick()?;
        let mut total = 0i128;
        let mut keys = keys(priority).into_iter();
        loop {
            work.tick()?;
            let Some(key) = keys.next() else {
                break;
            };
            if let Some(weight) = key {
                total = total
                    .checked_add(i128::from(weight))
                    .ok_or_else(|| work.stop(Stop::ArithmeticOverflow))?;
            }
        }
        work.tick()?;
        costs.push((
            priority,
            final_cost(priority, total).map_err(|kind| work.error(kind))?,
        ));
    }
    work.poll()?;
    Ok(CostReduction {
        score: Score {
            present: program.is_present(),
            costs,
        },
        work: work.statistics.work,
    })
}
