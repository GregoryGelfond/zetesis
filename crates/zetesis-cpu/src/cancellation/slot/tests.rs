//! Window boundaries are checked without relying on thread scheduling.

use std::error::Error;
use std::sync::Barrier;
use std::thread;

use super::*;
use crate::Stop;

#[test]
fn idle_pulls_do_not_cancel_a_later_run() {
    let slot = CancellationSlot::default();
    let handle = slot.clone();
    handle.cancel();
    let run = slot.open(None).unwrap();
    assert_eq!(run.cancellation().poll(), Ok(()));
}

#[test]
fn repeated_pulls_cancel_the_active_tokens() {
    let slot = CancellationSlot::default();
    let run = slot.open(None).unwrap();
    let token = run.cancellation().clone();
    slot.cancel();
    slot.cancel();
    assert_eq!(run.cancellation().poll(), Err(Stop::Cancelled));
    assert_eq!(token.poll(), Err(Stop::Cancelled));
}

#[test]
fn token_cancellation_is_local_to_its_run() {
    let slot = CancellationSlot::default();
    let old = slot.open(None).unwrap();
    let token = old.cancellation().clone();
    let current = slot.open(None).unwrap();
    token.cancel();
    assert_eq!(old.cancellation().poll(), Err(Stop::Cancelled));
    assert_eq!(current.cancellation().poll(), Ok(()));
}

#[test]
fn opening_supersedes_a_leaked_window() {
    let slot = CancellationSlot::default();
    let old = slot.open(None).unwrap();
    let token = old.cancellation().clone();
    // Deliberately leak one small guard: recovery must not depend on its Drop.
    std::mem::forget(old);
    let current = slot.open(None).unwrap();
    assert_eq!(token.poll(), Err(Stop::Cancelled));
    assert_eq!(current.cancellation().poll(), Ok(()));
}

#[test]
fn dropping_an_old_guard_preserves_the_current_run() {
    let slot = CancellationSlot::default();
    let old = slot.open(None).unwrap();
    let current = slot.open(None).unwrap();
    drop(old);
    assert_eq!(current.cancellation().poll(), Ok(()));
    slot.cancel();
    assert_eq!(current.cancellation().poll(), Err(Stop::Cancelled));
}

#[test]
fn token_clones_do_not_keep_a_window_open() {
    let slot = CancellationSlot::default();
    let run = slot.open(None).unwrap();
    let token = run.cancellation().clone();
    drop(run);
    assert_eq!(token.poll(), Err(Stop::Cancelled));
    assert_eq!(slot.state.load(Ordering::Relaxed) & STATUS_MASK, 0);
}

#[test]
fn delayed_idle_pulls_do_not_reach_an_opened_run() {
    let slot = CancellationSlot::default();
    let observed = slot.state.load(Ordering::Relaxed);
    let run = slot.open(None).unwrap();
    slot.cancel_observed(observed);
    assert_eq!(run.cancellation().poll(), Ok(()));
}

#[test]
fn delayed_active_pulls_do_not_reach_a_replacement() {
    let slot = CancellationSlot::default();
    let old = slot.open(None).unwrap();
    let observed = slot.state.load(Ordering::Relaxed);
    drop(old);
    let current = slot.open(None).unwrap();
    // Resume exactly between cancel's load and its one strong exchange.
    slot.cancel_observed(observed);
    assert_eq!(current.cancellation().poll(), Ok(()));
}

#[test]
fn delayed_pulls_do_not_reactivate_a_retired_window() {
    let slot = CancellationSlot::default();
    let run = slot.open(None).unwrap();
    let observed = slot.state.load(Ordering::Relaxed);
    drop(run);
    slot.cancel_observed(observed);
    assert_eq!(slot.state.load(Ordering::Relaxed) & STATUS_MASK, 0);
}

#[test]
fn cancelled_guards_retire_their_window() {
    let slot = CancellationSlot::default();
    let run = slot.open(None).unwrap();
    slot.cancel();
    drop(run);
    assert_eq!(slot.state.load(Ordering::Relaxed) & STATUS_MASK, 0);
}

#[test]
fn clearing_stops_tokens_without_retiring_a_later_run() {
    let slot = CancellationSlot::default();
    let old = slot.open(None).unwrap();
    slot.clear();
    assert_eq!(old.cancellation().poll(), Err(Stop::Cancelled));
    let current = slot.open(None).unwrap();
    drop(old);
    assert_eq!(current.cancellation().poll(), Ok(()));
}

#[test]
fn delayed_retirement_preserves_a_replacement() {
    let slot = CancellationSlot::default();
    let old = slot.open(None).unwrap();
    let observed = slot.state.load(Ordering::Relaxed);
    let current = slot.open(None).unwrap();
    // Resume clear after its load, with the original generation still held.
    retire(&slot.state, observed);
    assert_eq!(old.cancellation().poll(), Err(Stop::Cancelled));
    assert_eq!(current.cancellation().poll(), Ok(()));
}

#[test]
fn slot_cancellation_precedes_an_expired_deadline() {
    let slot = CancellationSlot::default();
    let run = slot.open(Some(Instant::now())).unwrap();
    assert_eq!(run.cancellation().poll(), Err(Stop::Deadline));
    slot.cancel();
    assert_eq!(run.cancellation().poll(), Err(Stop::Cancelled));
}

#[test]
fn pulls_during_timer_setup_are_retained() {
    let slot = CancellationSlot::default();
    let run = slot
        .open_with(Some(Instant::now()), |at| {
            slot.cancel();
            DeadlineOwner::arm(at)
        })
        .unwrap();
    assert_eq!(run.cancellation().poll(), Err(Stop::Cancelled));
}

#[test]
fn a_timer_refusal_retires_its_window() {
    let slot = CancellationSlot::default();
    let error = slot
        .open_with(Some(Instant::now()), |_| {
            Err(io::Error::other("timer refused"))
        })
        .unwrap_err();
    assert_eq!(slot.state.load(Ordering::Relaxed) & STATUS_MASK, 0);
    assert!(matches!(error, CancellationSlotError::Deadline(_)));
    assert_eq!(error.source().unwrap().to_string(), "timer refused");
    assert_eq!(
        error.to_string(),
        "cancellation deadline timer: timer refused"
    );
}

#[test]
fn timer_failure_preserves_a_concurrent_replacement() {
    let slot = CancellationSlot::default();
    let mut replacement = None;
    let error = slot
        .open_with(Some(Instant::now()), |_| {
            replacement = Some(slot.open(None).unwrap());
            Err(io::Error::other("timer refused"))
        })
        .unwrap_err();
    assert!(matches!(error, CancellationSlotError::Deadline(_)));
    assert_eq!(replacement.unwrap().cancellation().poll(), Ok(()));
}

#[test]
fn generation_exhaustion_preserves_the_last_run() {
    let slot = CancellationSlot::default();
    // Position immediately before the last generation. No billions-of-runs
    // loop is needed to establish the checked arithmetic and no-wrap boundary.
    let last_generation = !STATUS_MASK;
    slot.state
        .store(last_generation - GENERATION_STEP, Ordering::Relaxed);
    let last = slot.open(None).unwrap();
    let error = slot.open(None).unwrap_err();
    assert!(matches!(error, CancellationSlotError::GenerationExhausted));
    assert!(error.source().is_none());
    assert_eq!(error.to_string(), "cancellation slot generations exhausted");
    assert_eq!(last.cancellation().poll(), Ok(()));
    slot.cancel();
    assert_eq!(last.cancellation().poll(), Err(Stop::Cancelled));
}

#[test]
fn retirement_does_not_replenish_generations() {
    let slot = CancellationSlot::default();
    slot.state.store(!STATUS_MASK, Ordering::Relaxed);
    slot.clear();
    assert!(matches!(
        slot.open(None),
        Err(CancellationSlotError::GenerationExhausted)
    ));
}

#[test]
fn concurrent_pulls_cancel_one_shared_run() {
    let slot = CancellationSlot::default();
    let run = slot.open(None).unwrap();
    let barrier = Arc::new(Barrier::new(9));
    thread::scope(|scope| {
        for _ in 0..8 {
            let handle = slot.clone();
            let ready = Arc::clone(&barrier);
            scope.spawn(move || {
                ready.wait();
                handle.cancel();
            });
        }
        barrier.wait();
    });
    assert_eq!(run.cancellation().poll(), Err(Stop::Cancelled));
}

#[test]
fn concurrent_openings_leave_one_active_generation() {
    let slot = CancellationSlot::default();
    let barrier = Arc::new(Barrier::new(9));
    let runs = thread::scope(|scope| {
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let handle = slot.clone();
                let ready = Arc::clone(&barrier);
                scope.spawn(move || {
                    ready.wait();
                    handle.open(None).unwrap()
                })
            })
            .collect();
        barrier.wait();
        threads
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        runs.iter()
            .filter(|run| run.cancellation().poll().is_ok())
            .count(),
        1
    );
    slot.cancel();
    assert!(
        runs.iter()
            .all(|run| run.cancellation().poll() == Err(Stop::Cancelled))
    );
}
