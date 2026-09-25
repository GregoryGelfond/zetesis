//! Transferable source-check history, independent of transient observation.

use std::mem;
use zetesis_cpu::Cancellation;

use super::{
    Counters,
    generated::Generated,
    storage::{StorageLease, Workspace},
};

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
    /// A metadata receipt in this exact operation's sole live workspace.
    pub(crate) fn lease(&self) -> StorageLease {
        self.accounting.workspace.lease()
    }

    /// Current leased capacity, excluding separately measured canonical owners.
    pub(crate) fn workspace_bytes(&self) -> usize {
        self.accounting.workspace.bytes()
    }

    pub(crate) fn into_accounting(self) -> Accounting {
        self.accounting
    }
}

/// Accepted source history for a closed continuation that generates no terms.
/// Each new session receives a fresh workspace, never a fresh work allowance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AccountingBaseline {
    pub(crate) work: u64,
    pub(crate) substitutions: u64,
    pub(crate) generated_values: usize,
}

impl AccountingBaseline {
    pub(crate) fn start(self) -> Accounting {
        Accounting {
            work: self.work,
            substitutions: self.substitutions,
            ..Accounting::default()
        }
    }
}

/// An accounting handoff cannot discard a live owner or shared allowance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AccountingHandoffError {
    SharedAllowance,
    LiveWorkspace { bytes: usize },
}

impl std::fmt::Display for AccountingHandoffError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SharedAllowance => f.write_str("source accounting retains a shared allowance"),
            Self::LiveWorkspace { bytes } => {
                write!(f, "source accounting retains {bytes} workspace bytes")
            }
        }
    }
}
impl std::error::Error for AccountingHandoffError {}

impl Accounting {
    /// Inline generated-history storage already represented by its own lease.
    pub(crate) fn leased_header_bytes(&self) -> usize {
        self.generated_values
            .as_ref()
            .map_or(0, |_| size_of::<Generated>())
    }

    /// Retire generated-root membership only for a certified continuation which
    /// cannot construct terms. Its logical usage survives in the baseline.
    /// All other leased owners must have transferred or dropped first.
    pub(crate) fn into_flat_baseline(
        mut self,
    ) -> Result<AccountingBaseline, AccountingHandoffError> {
        if self.allowance.is_some() {
            return Err(AccountingHandoffError::SharedAllowance);
        }
        let generated_values = self
            .generated_values
            .as_ref()
            .map_or(0, |generated| generated.values.len());
        drop(self.generated_values.take());
        let bytes = self.workspace.bytes();
        if bytes != 0 {
            return Err(AccountingHandoffError::LiveWorkspace { bytes });
        }
        Ok(AccountingBaseline {
            work: self.work,
            substitutions: self.substitutions,
            generated_values,
        })
    }

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
