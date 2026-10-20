//! Thread-local reference and interruption controls for this optional route.

use std::cell::RefCell;
use zetesis_cpu::Cancellation;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(super) struct Counts {
    pub(super) heads: usize,
    pub(super) certified: usize,
    pub(super) witnesses: usize,
    pub(super) emissions: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::formula_factor) enum Point {
    Head,
    Certified,
    Witness,
    Emission,
}

struct State {
    enabled: bool,
    counts: Counts,
    cancel: Option<(Point, usize, Cancellation)>,
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State {
        enabled: true,
        counts: Counts { heads: 0, certified: 0, witnesses: 0, emissions: 0 },
        cancel: None,
    }) };
}

struct Restore(State);
impl Drop for Restore {
    fn drop(&mut self) {
        STATE.with(|state| std::mem::swap(&mut *state.borrow_mut(), &mut self.0));
    }
}

pub(in crate::formula_factor) fn enabled() -> bool {
    STATE.with(|state| state.borrow().enabled)
}

pub(in crate::formula_factor) fn record(point: Point) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let count = match point {
            Point::Head => &mut state.counts.heads,
            Point::Certified => &mut state.counts.certified,
            Point::Witness => &mut state.counts.witnesses,
            Point::Emission => &mut state.counts.emissions,
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

pub(super) fn scoped<T>(enabled: bool, action: impl FnOnce() -> T) -> (T, Counts) {
    controlled(enabled, None, action)
}

pub(super) fn controlled<T>(
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
