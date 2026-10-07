//! Synchronous reference tests control only factorization, never source metadata.
//!
//! The scope is thread-local and restores its predecessor on return or unwind.
//! Unit reference calls do not spawn work; newly spawned threads retain the
//! ordinary production route rather than inheriting a parent's test control.

use std::cell::RefCell;
use zetesis_cpu::Cancellation;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(crate) struct Counts {
    pub(crate) runs: usize,
    pub(crate) witnesses: usize,
    pub(crate) continuations: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Point {
    Run,
    Witness,
    Continuation,
}

struct State {
    enabled: bool,
    counts: Counts,
    cancel: Option<(Point, usize, Cancellation)>,
}
thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State {
        enabled: true,
        counts: Counts { runs: 0, witnesses: 0, continuations: 0 },
        cancel: None,
    }) };
}

struct Restore(State);
impl Drop for Restore {
    fn drop(&mut self) {
        STATE.with(|state| std::mem::swap(&mut *state.borrow_mut(), &mut self.0));
    }
}

pub(crate) fn enabled() -> bool {
    STATE.with(|state| state.borrow().enabled)
}

pub(crate) fn record(point: Point) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let count = match point {
            Point::Run => &mut state.counts.runs,
            Point::Witness => &mut state.counts.witnesses,
            Point::Continuation => &mut state.counts.continuations,
        };
        *count += 1;
        let count = *count;
        if let Some((at, after, cancellation)) = &state.cancel
            && *at == point
            && *after == count
        {
            cancellation.cancel();
        }
    });
}

pub(crate) fn scoped<T>(enabled: bool, action: impl FnOnce() -> T) -> (T, Counts) {
    controlled(enabled, None, action)
}

pub(crate) fn controlled<T>(
    enabled: bool,
    cancel: Option<(Point, usize, Cancellation)>,
    action: impl FnOnce() -> T,
) -> (T, Counts) {
    let previous = STATE.replace(State {
        enabled,
        counts: Counts::default(),
        cancel,
    });
    let _restore = Restore(previous);
    let result = action();
    let counts = STATE.with(|state| state.borrow().counts);
    (result, counts)
}

#[test]
fn reference_control_restores_after_unwind() {
    assert!(enabled());
    let result = std::panic::catch_unwind(|| {
        scoped(false, || {
            assert!(!enabled());
            panic!("test unwind");
        })
    });
    assert!(result.is_err());
    assert!(enabled());
}

#[test]
fn reference_control_does_not_escape_its_thread() {
    scoped(false, || {
        assert!(!enabled());
        assert!(std::thread::spawn(enabled).join().unwrap());
        assert!(!enabled());
    });
    assert!(enabled());
}
