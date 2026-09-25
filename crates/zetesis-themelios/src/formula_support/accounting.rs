//! Transferable source-check history, independent of transient observation.

use std::mem;
use zetesis_cpu::Cancellation;

use super::{Counters, generated::Generated, storage::Workspace};

/// All cumulative formula charges and generated-value identity. The scalar
/// payload budget has its own owner. Neither runtime control nor the local
/// grounding observer belongs to this retained history.
#[derive(Default)]
pub(crate) struct Accounting {
    pub(super) workspace: Workspace,
    pub(crate) work: u64,
    pub(crate) substitutions: u64,
    pub(super) generated_values: Option<Generated>,
    pub(super) allowance: Option<crate::ConstraintAllowance>,
}

impl Counters {
    pub(crate) fn into_accounting(self) -> Accounting {
        self.accounting
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
        let mut active = Active {
            counters: Counters {
                accounting: mem::take(self),
                cancellation: Some(cancellation.clone()),
                observed: super::Work::default(),
            },
            retained: self,
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
        mem::swap(self.retained, &mut self.counters.accounting);
    }
}

#[cfg(test)]
mod tests;
