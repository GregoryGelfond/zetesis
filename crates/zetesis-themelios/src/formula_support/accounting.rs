//! Transferable source-check history, independent of transient observation.

use std::mem;
use zetesis_cpu::{Cancellation, Stop};

mod precedence;
pub(crate) use precedence::StopPrecedence;

use super::{
    Counters,
    generated::Generated,
    storage::{StorageLease, Workspace},
};

/// All cumulative formula charges and generated-value identity. A terminal
/// reconstruction call is the one exception to cumulation: each call starts
/// from the admission baseline under its own per-answer allowance. The scalar
/// payload budget has its own owner. Neither runtime control nor the local
/// grounding observer belongs to this retained history.
#[derive(Default)]
pub(crate) struct Accounting {
    pub(super) workspace: Workspace,
    pub(crate) work: u64,
    pub(crate) substitutions: u64,
    pub(super) generated_values: Option<Generated>,
    pub(super) allowance: Option<crate::constraint_allowance::Pending>,
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

    #[cfg(test)]
    pub(crate) fn into_accounting(mut self) -> Accounting {
        self.accounting.settle();
        self.accounting
    }
}

/// Accepted source history for a closed continuation that generates no terms.
/// Each new session receives a fresh workspace, never a fresh work allowance;
/// a reconstruction call starts from it with the cursor's per-answer allowance.
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
    /// Publish pending shared charges without resetting accepted local history.
    pub(crate) fn settle(&mut self) {
        if let Some(allowance) = &mut self.allowance {
            allowance.settle();
        }
    }

    /// Inline generated-history storage already represented by its own lease.
    pub(crate) fn leased_header_bytes(&self) -> usize {
        self.generated_values
            .as_ref()
            .map_or(0, |_| size_of::<Generated>())
    }

    /// Retire generated-root membership only for a certified continuation which
    /// cannot construct terms. Its logical usage survives in the baseline.
    /// All other leased owners must have transferred or dropped first.
    #[cfg(test)]
    pub(crate) fn into_flat_baseline(
        mut self,
    ) -> Result<AccountingBaseline, AccountingHandoffError> {
        self.flat_baseline()
    }

    /// Retire generated membership while the controlled scope still owns its
    /// complete receipt, including through terminal admission's final ceilings.
    pub(crate) fn flat_baseline(&mut self) -> Result<AccountingBaseline, AccountingHandoffError> {
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

    /// Run one synchronous operation under its sole control-bearing account.
    /// Entry and exit observe control; at exit only a resource refusal can be
    /// replaced, preserving its source site and original-program envelope.
    /// Completed work is retained on success, refusal and unwind. A stop arriving
    /// after the exit observation belongs to the caller's next observation.
    pub(crate) fn controlled<T, E: StopPrecedence>(
        &mut self,
        cancellation: Option<&Cancellation>,
        observed: super::Work,
        location: crate::ProgramSite,
        action: impl FnOnce(&mut Counters) -> Result<T, E>,
    ) -> Result<T, E> {
        let control = AccountControl::new(cancellation);
        #[cfg(debug_assertions)]
        let _scope = Scope(std::rc::Rc::clone(&control.open));
        let mut counters = Counters::resume(mem::take(self), observed);
        counters.control = Some(control);
        let mut active = Active {
            counters,
            retained: self,
        };
        active
            .counters
            .observe()
            .map_err(|stop| E::from_stop(stop, location, &active.counters.accounting))?;
        let result = action(&mut active.counters);
        match (result, active.counters.observe()) {
            (Ok(value), Ok(())) => Ok(value),
            (Ok(_), Err(stop)) => Err(E::from_stop(stop, location, &active.counters.accounting)),
            (Err(error), Err(stop)) if error.is_resource_refusal() => {
                Err(error.outranked_by(stop, location))
            }
            (Err(error), _) => Err(error),
        }
    }

    /// Create a shared receipt without creating runtime control.
    pub(crate) fn with_allowance(allowance: crate::ConstraintAllowance) -> Self {
        Self {
            allowance: Some(crate::constraint_allowance::Pending::new(allowance)),
            ..Self::default()
        }
    }

    #[cfg(test)]
    fn with_cancellation<T>(
        &mut self,
        cancellation: &Cancellation,
        action: impl FnOnce(&mut Counters) -> T,
    ) -> T {
        let mut active = Active {
            counters: Counters::resume(mem::take(self), super::Work::default())
                .with_cancellation(Some(cancellation)),
            retained: self,
        };
        action(&mut active.counters)
    }
}

/// The only production constructor is the settled entry above. This capability
/// may move within its scope but must not be used after that scope exits.
pub(super) struct AccountControl {
    cancellation: Option<Cancellation>,
    #[cfg(debug_assertions)]
    open: std::rc::Rc<std::cell::Cell<bool>>,
    #[cfg(test)]
    every_charge: bool,
}
impl AccountControl {
    fn new(cancellation: Option<&Cancellation>) -> Self {
        Self {
            cancellation: cancellation.cloned(),
            #[cfg(debug_assertions)]
            open: std::rc::Rc::new(std::cell::Cell::new(true)),
            #[cfg(test)]
            every_charge: false,
        }
    }

    pub(super) fn cancellation(&self) -> Option<&Cancellation> {
        self.cancellation.as_ref()
    }

    pub(super) fn observe(&self) -> Result<(), Stop> {
        #[cfg(test)]
        OBSERVATIONS.set(OBSERVATIONS.get() + 1);
        #[cfg(debug_assertions)]
        assert!(
            self.open.get(),
            "controlled counters escaped their accounting scope"
        );
        self.cancellation
            .as_ref()
            .map_or(Ok(()), Cancellation::poll)
    }

    /// Observe a crossed boundary of the retained work account. A zero-sized
    /// charge does not observe cancellation; explicit entry/exit polls do.
    #[inline]
    pub(super) fn observe_charge(&self, charged: u64, amount: u64) -> Result<(), Stop> {
        #[cfg(test)]
        if amount != 0 && (self.every_charge || EVERY_CHARGE.get()) {
            return self.observe();
        }
        if zetesis_cpu::poll_due(charged, amount) {
            self.observe()
        } else {
            Ok(())
        }
    }
}

#[cfg(debug_assertions)]
struct Scope(std::rc::Rc<std::cell::Cell<bool>>);
#[cfg(debug_assertions)]
impl Drop for Scope {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

impl Counters {
    pub(super) fn observe(&self) -> Result<(), Stop> {
        self.control
            .as_ref()
            .map_or(Ok(()), AccountControl::observe)
    }

    /// Observe immediately without admitting or counting work.
    pub(crate) fn poll(&self, location: crate::ProgramSite) -> Result<(), crate::FormulaFailure> {
        self.observe()
            .map_err(|reason| super::interrupted(reason, location))
    }

    /// Retain work already completed by a nested operation under its supplied
    /// allowance. Count before observing: interruption cannot refund that work.
    pub(crate) fn absorb(
        &mut self,
        amount: u128,
        limits: &crate::FormulaLimits,
        location: crate::ProgramSite,
    ) -> Result<(), crate::FormulaFailure> {
        let before = self.accounting.work;
        super::ceiling(
            crate::FormulaResource::Work,
            u128::from(before).saturating_add(amount),
            u128::from(limits.max_work),
            location,
        )?;
        let amount = u64::try_from(amount).expect("absorbed work fits its supplied allowance");
        self.count_work(amount);
        if let Some(control) = &self.control {
            control
                .observe_charge(before, amount)
                .map_err(|reason| super::interrupted(reason, location))?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn attach_test_control(&mut self, cancellation: &Cancellation) {
        let mut control = AccountControl::new(Some(cancellation));
        control.every_charge = true;
        self.control = Some(control);
    }

    #[cfg(test)]
    pub(crate) fn detach_test_control(&mut self) {
        self.control = None;
    }

    #[cfg(test)]
    pub(crate) fn with_cancellation(mut self, cancellation: Option<&Cancellation>) -> Self {
        match cancellation {
            Some(cancellation) => self.attach_test_control(cancellation),
            None => self.detach_test_control(),
        }
        self
    }

    #[cfg(test)]
    pub(crate) fn with_allowance(
        allowance: crate::ConstraintAllowance,
        cancellation: &Cancellation,
    ) -> Self {
        Self::resume(
            Accounting::with_allowance(allowance),
            super::Work::default(),
        )
        .with_cancellation(Some(cancellation))
    }
}

#[cfg(test)]
thread_local! { static OBSERVATIONS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
thread_local! { static EVERY_CHARGE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
#[cfg(test)]
pub(crate) fn observe_every_charge<T>(action: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            EVERY_CHARGE.set(self.0);
        }
    }
    let _restore = Restore(EVERY_CHARGE.replace(true));
    action()
}

struct Active<'a> {
    retained: &'a mut Accounting,
    counters: Counters,
}
impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.counters.accounting.settle();
        mem::swap(self.retained, &mut self.counters.accounting);
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod cadence_tests;
