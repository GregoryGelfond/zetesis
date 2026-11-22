//! Switch row recording off to compare replay with retention's own join.
use std::cell::Cell;

thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(true) };
    static REPLAYED: Cell<u64> = const { Cell::new(0) };
}

/// Count one template whose instances were replayed.
pub(super) fn replayed() {
    REPLAYED.set(REPLAYED.get() + 1);
}

pub(in crate::formula_ground) fn enabled() -> bool {
    ENABLED.get()
}

/// Run `action` with recording switched as asked; also return how many
/// templates it replayed.
pub(super) fn scoped<T>(enabled: bool, action: impl FnOnce() -> T) -> (T, u64) {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENABLED.set(self.0);
        }
    }
    let _restore = Restore(ENABLED.replace(enabled));
    let before = REPLAYED.get();
    let result = action();
    (result, REPLAYED.get() - before)
}
