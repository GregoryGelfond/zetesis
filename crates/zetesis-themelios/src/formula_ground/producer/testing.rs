//! Synchronous controls select only the producer witness representation.
//!
//! Scoped state restores on return or unwind. Child threads retain the normal
//! production route. Counters name attempted materialization calls; Publish
//! records a completed normal root and its producer metadata.

use std::cell::RefCell;

use zetesis_cpu::Cancellation;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(crate) struct Counts {
    pub(crate) runs: usize,
    pub(crate) witnesses: usize,
    pub(crate) atoms: usize,
    pub(crate) groups: usize,
    pub(crate) lookups: usize,
    pub(crate) reused: usize,
    pub(crate) published: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Point {
    Run,
    Witness,
    Atom,
    Group,
    Lookup,
    Reuse,
    Publish,
}

struct State {
    enabled: bool,
    reuse: bool,
    counts: Counts,
    cancel: Option<(Point, usize, Cancellation)>,
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State {
        enabled: true,
        reuse: true,
        counts: Counts { runs: 0, witnesses: 0, atoms: 0, groups: 0, lookups: 0, reused: 0, published: 0 },
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

pub(crate) fn reuse_enabled() -> bool {
    STATE.with(|state| state.borrow().reuse)
}

/// Compare with the previous producer algorithm independently of the general
/// producer-versus-scalar semantic control. Nested controls keep this policy.
pub(crate) fn sharing<T>(reuse: bool, action: impl FnOnce() -> T) -> T {
    let previous = STATE.replace(State {
        enabled: enabled(),
        reuse,
        counts: Counts::default(),
        cancel: None,
    });
    let _restore = Restore(previous);
    action()
}

pub(crate) fn record(point: Point) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let count = match point {
            Point::Run => &mut state.counts.runs,
            Point::Witness => &mut state.counts.witnesses,
            Point::Atom => &mut state.counts.atoms,
            Point::Group => &mut state.counts.groups,
            Point::Lookup => &mut state.counts.lookups,
            Point::Reuse => &mut state.counts.reused,
            Point::Publish => &mut state.counts.published,
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
    let reuse = reuse_enabled();
    let previous = STATE.replace(State {
        enabled,
        reuse,
        counts: Counts::default(),
        cancel,
    });
    let _restore = Restore(previous);
    let result = action();
    (result, STATE.with(|state| state.borrow().counts))
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
fn reference_control_stays_on_its_thread() {
    scoped(false, || {
        assert!(!enabled());
        assert!(std::thread::spawn(enabled).join().unwrap());
    });
    assert!(enabled());
}
