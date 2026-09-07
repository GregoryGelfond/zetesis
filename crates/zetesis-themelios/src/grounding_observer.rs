//! Optional boundary observation; ordinary admission never reads a clock.

mod profile;

use themelios_base::span::Location;

pub(crate) use profile::{Event, Profile, Work};
pub use profile::{GroundingOutcome, GroundingPhase, GroundingWork};

/// Observe only complete formula materialization after source preparation.
///
/// Every `enter` has a matching `exit`, including a failed or unwinding attempt.
/// A boundary event says nothing about semantic completion. Implementations must
/// not panic or affect source semantics. No observer is called by the ordinary
/// admission APIs, and this crate performs no timing itself.
pub trait GroundingObserver {
    /// Begin possible-support completion and finite formula instantiation.
    fn enter(&self);
    /// End the attempted materialization, whether it succeeded or failed.
    fn exit(&self);

    /// Opt into bounded work counters and coarse phase callbacks for this attempt.
    ///
    /// Called once after `enter`. The default retains boundary-only observation.
    /// No clock is read by the frontend. Detailed observation allocates one fixed
    /// counter bank; callback implementations own any further storage or timing.
    fn details_enabled(&self) -> bool {
        false
    }

    /// Begin one coarse phase after its counter bank has been reset.
    ///
    /// Within one grounding attempt, phases are sequential and do not nest.
    /// Rule instantiation emits one
    /// pair per prepared IR rule; the location identifies original source context,
    /// but need not be unique after source expansion. Joins, filtering and formula
    /// emission remain interleaved inside that phase.
    /// Whole-program phases use `None` rather than claiming one source location.
    fn phase_enter(&self, _phase: GroundingPhase, _location: Option<Location>) {}

    /// End a phase with work performed before success, failure or unwind.
    ///
    /// Counts belong only to this phase, including its failed attempt. Counter
    /// overflow makes that field unavailable and never changes admission or its
    /// resource limits. Callback overhead is not removed from observed durations.
    /// A completed phase does not establish completed grounding or stable models.
    fn phase_exit(
        &self,
        _phase: GroundingPhase,
        _location: Option<Location>,
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
