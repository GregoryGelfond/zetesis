//! Thread-local witnesses for reuse and the otherwise unreachable guard refusal.

use std::cell::Cell;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Counts {
    pub(super) raises: usize,
    pub(super) formula_attempts: usize,
    pub(super) replays: usize,
}

thread_local! {
    static COUNTS: Cell<Counts> = const { Cell::new(Counts { raises: 0, formula_attempts: 0, replays: 0 }) };
    static REJECT_PROFILE: Cell<bool> = const { Cell::new(false) };
}

pub(super) fn counts() -> Counts {
    COUNTS.with(Cell::get)
}

pub(crate) fn note_raise() {
    COUNTS.with(|value| {
        value.set(Counts {
            raises: value.get().raises + 1,
            ..value.get()
        });
    });
}

pub(crate) fn note_formula_attempt() {
    COUNTS.with(|value| {
        value.set(Counts {
            formula_attempts: value.get().formula_attempts + 1,
            ..value.get()
        });
    });
}

pub(super) fn note_replay() {
    COUNTS.with(|value| {
        value.set(Counts {
            replays: value.get().replays + 1,
            ..value.get()
        });
    });
}

pub(super) fn rejects_recorded_profile() -> bool {
    REJECT_PROFILE.with(Cell::get)
}

pub(super) fn with_rejected_profile<T>(body: impl FnOnce() -> T) -> T {
    struct Reset(bool);
    impl Drop for Reset {
        fn drop(&mut self) {
            REJECT_PROFILE.with(|flag| flag.set(self.0));
        }
    }
    let _reset = Reset(REJECT_PROFILE.with(|flag| flag.replace(true)));
    body()
}
