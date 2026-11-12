//! Synchronous reference control and actual grouped-route observations.
use std::cell::RefCell;
use zetesis_algebra::Cancellation;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(super) struct Counts {
    pub(super) accepted: usize,
    pub(super) first_rows: usize,
    pub(super) second_rows: usize,
    pub(super) groups: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Point {
    Accepted,
    FirstRow,
    SecondRow,
    Group,
}

struct State {
    enabled: bool,
    counts: Counts,
    cancel: Option<(Point, Cancellation)>,
}
thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State {
        enabled: true,
        counts: Counts { accepted: 0, first_rows: 0, second_rows: 0, groups: 0 },
        cancel: None,
    }) };
}
struct Restore(State);
impl Drop for Restore {
    fn drop(&mut self) {
        STATE.with(|state| std::mem::swap(&mut *state.borrow_mut(), &mut self.0));
    }
}
pub(super) fn enabled() -> bool {
    STATE.with(|state| state.borrow().enabled)
}
pub(super) fn record(point: Point) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        match point {
            Point::Accepted => state.counts.accepted += 1,
            Point::FirstRow => state.counts.first_rows += 1,
            Point::SecondRow => state.counts.second_rows += 1,
            Point::Group => state.counts.groups += 1,
        }
        if let Some((at, cancellation)) = &state.cancel
            && *at == point
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
    cancel: Option<(Point, Cancellation)>,
    action: impl FnOnce() -> T,
) -> (T, Counts) {
    let old = STATE.replace(State {
        enabled,
        counts: Counts::default(),
        cancel,
    });
    let restore = Restore(old);
    let result = action();
    let counts = STATE.with(|state| state.borrow().counts);
    drop(restore);
    (result, counts)
}
