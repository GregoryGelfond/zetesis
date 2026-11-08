//! Independent scoped controls for immutable component metadata.

use std::cell::Cell;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(crate) struct Counts {
    pub(crate) preparations: usize,
    pub(crate) cursors: usize,
}

thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(true) };
    static COUNTS: Cell<Counts> = const { Cell::new(Counts { preparations: 0, cursors: 0 }) };
}

pub(crate) fn enabled() -> bool {
    ENABLED.get()
}

pub(super) fn prepared() {
    COUNTS.set(Counts {
        preparations: COUNTS.get().preparations + 1,
        ..COUNTS.get()
    });
}

pub(super) fn reused() {
    COUNTS.set(Counts {
        cursors: COUNTS.get().cursors + 1,
        ..COUNTS.get()
    });
}

pub(crate) fn scoped<T>(enabled: bool, action: impl FnOnce() -> T) -> (T, Counts) {
    struct Restore(bool, Counts);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENABLED.set(self.0);
            COUNTS.set(self.1);
        }
    }
    let restore = Restore(ENABLED.replace(enabled), COUNTS.take());
    let result = action();
    let counts = COUNTS.get();
    drop(restore);
    (result, counts)
}
