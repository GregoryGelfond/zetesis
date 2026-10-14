//! The scoped reference changes ordinary emission/capture only. The existing
//! producer algorithm and its accounting remain independently controllable.
use std::cell::{Cell, RefCell};

#[derive(Default)]
pub(super) struct Visits {
    pub(super) literals: Vec<(usize, usize, bool)>,
}
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(true) };
    static VISITS: RefCell<Visits> = RefCell::new(Visits::default());
}
pub(in crate::formula_ground) fn enabled() -> bool {
    ENABLED.get()
}
pub(super) fn record(literal: usize, depth: usize, reused: bool) {
    VISITS.with_borrow_mut(|visits| visits.literals.push((literal, depth, reused)));
}
pub(super) fn scoped<T>(enabled: bool, action: impl FnOnce() -> T) -> (T, Visits) {
    struct Restore(bool, Visits);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENABLED.set(self.0);
            VISITS.with_borrow_mut(|visits| *visits = std::mem::take(&mut self.1));
        }
    }
    let restore = Restore(ENABLED.replace(enabled), VISITS.take());
    let result = action();
    let visits = VISITS.take();
    drop(restore);
    (result, visits)
}
