//! Optional boundary observation; ordinary admission never reads a clock.

mod profile;

use crate::ProgramSite;

pub(crate) use profile::{Event, Profile, Work};
pub use profile::{GroundingOutcome, GroundingPhase, GroundingWork};

/// Outcome of optional numeric analysis over all prepared producers and compact
/// fact columns. This synchronous borrowed view cannot outlive the attempt.
/// Argument bounds establish neither truth nor answer-set membership.
#[derive(Clone, Copy, Debug)]
pub enum DomainObservation<'a, 'source> {
    /// No domain analysis was requested.
    Disabled,
    /// Actual analysis with its canonical reader, status, producer sites and
    /// logical statistics and diagnostic expression/body census. A stopped
    /// analysis publishes no usable bounds or census. Reports never authorize
    /// pruning, family admission, or answer-set membership.
    Analyzed(crate::NumericAnalysis<'a, 'source>),
}

/// Observe formula materialization or certified exact source evaluation.
///
/// Every `enter` has a matching `exit`, including a failed or unwinding attempt.
/// A boundary event says nothing about semantic completion. Implementations must
/// not panic or affect source semantics. No observer is called by the ordinary
/// admission APIs, and this crate performs no timing itself.
pub trait GroundingObserver {
    /// Begin possible-support completion and formula or source evaluation.
    fn enter(&self);
    /// End the attempted preparation, whether it succeeded or failed.
    fn exit(&self);

    /// Opt into bounded work counters and coarse phase callbacks for this attempt.
    ///
    /// Called once after `enter`. The default retains boundary-only observation.
    /// No clock is read by the frontend. Detailed observation allocates one fixed
    /// counter bank; callback implementations own any further storage or timing.
    fn details_enabled(&self) -> bool {
        false
    }

    /// The certified source partition will ground a base formula and retain
    /// terminal definitions for reconstruction after base membership checks.
    /// Called before that base materialization attempt, including one that later
    /// refuses. This identifies the attempted route, not completed grounding or
    /// reconstructed answers; it introduces no measurement or work charge.
    /// `base` says how the base will be grounded.
    fn terminal_definitions(&self, base: crate::BaseKind) {
        let _ = base;
    }

    /// Request numeric certificates, their census and the domain callback.
    ///
    /// The default preserves existing diagnostic observers. Returning false
    /// skips these diagnostic requests and [`Self::domain_analysis`], including
    /// its [`DomainObservation::Disabled`] observation. It does not disable the shared numeric analysis
    /// or finite-domain guards and static scalar-totality selection requested by
    /// `with_domain_analysis`. Optimizer certificate requests remain independent
    /// of this diagnostic option.
    ///
    /// Sampled when each grounding profile is created, independently of
    /// [`Self::details_enabled`]. An absent observer requests no diagnostics. Diagnostic
    /// work consumes the same cumulative preparation limits as other analysis.
    /// Observers delegating the domain callback can delegate this request too.
    fn domain_diagnostics_enabled(&self) -> bool {
        true
    }

    /// Observe the actual optional domain attempt before final rule guards.
    /// Called synchronously when [`Self::domain_diagnostics_enabled`] permits it,
    /// independently of detailed work-counter opt-in. A prior grounding failure
    /// may prevent the attempt and this callback.
    fn domain_analysis(&self, _observation: DomainObservation<'_, '_>) {}

    /// Begin one coarse phase after its counter bank has been reset.
    ///
    /// Within one grounding attempt, phases are sequential and do not nest.
    /// Rule instantiation emits one
    /// pair per prepared IR rule; the site identifies the original statement and
    /// any actual source coordinate, and may repeat after expansion. Joins,
    /// filtering and formula
    /// emission remain interleaved inside that phase.
    /// Whole-program phases use `None` rather than claiming one program site.
    fn phase_enter(&self, _phase: GroundingPhase, _site: Option<ProgramSite>) {}

    /// End a phase with work performed before success, failure or unwind.
    ///
    /// Counts belong only to this phase, including its failed attempt. Counter
    /// overflow makes that field unavailable and never changes admission or its
    /// resource limits. Callback overhead is not removed from observed durations.
    /// A completed phase does not establish completed grounding or stable models.
    fn phase_exit(
        &self,
        _phase: GroundingPhase,
        _site: Option<ProgramSite>,
        _outcome: GroundingOutcome,
        _work: GroundingWork,
    ) {
    }
}

pub(crate) fn observe<T>(
    observer: Option<&dyn GroundingObserver>,
    action: impl FnOnce() -> T,
) -> T {
    struct Exit<'a>(Option<&'a dyn GroundingObserver>);
    impl Drop for Exit<'_> {
        fn drop(&mut self) {
            if let Some(observer) = self.0 {
                observer.exit();
            }
        }
    }
    if let Some(observer) = observer {
        observer.enter();
    }
    let _exit = Exit(observer);
    action()
}

#[cfg(test)]
mod tests {
    use super::{GroundingObserver, observe};
    use std::cell::Cell;

    struct Observer(Cell<i32>);
    impl GroundingObserver for Observer {
        fn enter(&self) {
            self.0.set(self.0.get() + 1);
        }
        fn exit(&self) {
            self.0.set(self.0.get() - 1);
        }
    }

    #[test]
    fn unwind_restores_the_callers_boundary() {
        let observer = Observer(Cell::new(0));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            observe(Some(&observer), || {
                assert_eq!(observer.0.get(), 1);
                panic!("controlled failure");
            });
        }));
        assert!(result.is_err());
        assert_eq!(observer.0.get(), 0);
        assert_eq!(observe(None, || 7), 7);
    }
}
