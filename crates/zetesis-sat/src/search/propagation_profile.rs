//! Test-only bounded work counters; there is no clock or production observer.
//! The watch-trace tests read them to charge, under their historical fixtures'
//! cost map, the positions the current replacement search no longer scans.

use std::cell::Cell;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Counts {
    pub(super) binary_attempts: u64,
    pub(super) ternary_elided_positions: u64,
}

thread_local! {
    // Fixed counters per test thread. Each trace resets them before its finite
    // source query; no candidates, source values or results are cached here.
    static COUNTS: Cell<Counts> = const { Cell::new(Counts {
        binary_attempts: 0,
        ternary_elided_positions: 0,
    }) };
}

pub(super) fn ternary_inspection(watches: [usize; 2], available: impl Fn(usize) -> bool) {
    // Independently replay the former ascending scan, including its watched
    // positions. The production specialization always performs one test.
    let mut inspections = 0;
    for position in 0..3 {
        inspections += 1;
        if !watches.contains(&position) && available(position) {
            break;
        }
    }
    COUNTS.with(|cell| {
        let mut counts = cell.get();
        counts.ternary_elided_positions = counts
            .ternary_elided_positions
            .checked_add(inspections - 1)
            .expect("bounded profile count");
        cell.set(counts);
    });
}

pub(super) fn reset() {
    COUNTS.set(Counts::default());
}

pub(super) fn snapshot() -> Counts {
    COUNTS.get()
}

pub(super) fn binary_attempt() {
    COUNTS.with(|cell| {
        let mut counts = cell.get();
        counts.binary_attempts = counts
            .binary_attempts
            .checked_add(1)
            .expect("bounded profile count");
        cell.set(counts);
    });
}
