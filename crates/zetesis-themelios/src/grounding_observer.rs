//! Optional boundary observation; ordinary admission never reads a clock.

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
