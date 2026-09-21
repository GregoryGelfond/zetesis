//! Transferable source-check history, independent of transient observation.

use std::{collections::BTreeSet, mem};
use zetesis_core::Value;
use zetesis_cpu::Cancellation;

use super::Counters;

/// All cumulative formula charges and generated-value identity. The scalar
/// payload budget has its own owner. Neither runtime control nor the local
/// grounding observer belongs to this retained history.
#[derive(Default)]
pub(crate) struct Accounting {
    pub(crate) work: u64,
    pub(crate) substitutions: u64,
    generated_values: BTreeSet<Value>,
    allowance: Option<crate::ConstraintAllowance>,
}

impl Counters {
    pub(crate) fn into_accounting(self) -> Accounting {
        Accounting {
            work: self.work,
            substitutions: self.substitutions,
            generated_values: self.generated_values,
            allowance: self.allowance,
        }
    }
}

impl Accounting {
    /// Run one synchronous check with supplied cancellation and no grounding observer.
    /// Move the complete history in and back; never clone or reset its values.
    /// The guard restores accepted charges on success, refusal and unwind.
    pub(crate) fn with_cancellation<T>(
        &mut self,
        cancellation: &Cancellation,
        action: impl FnOnce(&mut Counters) -> T,
    ) -> T {
        let Self {
            work,
            substitutions,
            generated_values,
            allowance,
        } = mem::take(self);
        let mut active = Active {
            retained: self,
            counters: Counters {
                work,
                substitutions,
                generated_values,
                allowance,
                cancellation: Some(cancellation.clone()),
                ..Counters::default()
            },
        };
        action(&mut active.counters)
    }
}

struct Active<'a> {
    retained: &'a mut Accounting,
    counters: Counters,
}
impl Drop for Active<'_> {
    fn drop(&mut self) {
        *self.retained = mem::take(&mut self.counters).into_accounting();
    }
}

#[cfg(test)]
mod tests;
