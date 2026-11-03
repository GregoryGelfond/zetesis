//! The whole-kernel transaction retains native receipts and guard ownership.

use std::cell::RefCell;

use super::super::{Failure, Meter, Reservation, Stop};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Run(u64),
    Commit(u64),
    Drop(Option<u64>),
}

struct Guard<'a> {
    granted: u64,
    used: Option<u64>,
    events: &'a RefCell<Vec<Event>>,
}

impl<'a> Guard<'a> {
    fn new(granted: u64, events: &'a RefCell<Vec<Event>>) -> Self {
        Self {
            granted,
            used: None,
            events,
        }
    }
}

impl Reservation for Guard<'_> {
    fn allowance(&self) -> u64 {
        self.granted
    }

    fn commit(mut self, used: u64) {
        self.used = Some(used);
        self.events.borrow_mut().push(Event::Commit(used));
    }
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        self.events.borrow_mut().push(Event::Drop(self.used));
    }
}

#[test]
fn full_partial_and_zero_allowances_return_native_results_after_settlement() {
    for (granted, used) in [(10, 3), (7, 5), (0, 0), (u64::MAX, u64::MAX)] {
        for native in [Ok(()), Err(43)] {
            let events = RefCell::new(Vec::new());
            let result = Meter::under_reservation(Guard::new(granted, &events), |allowance| {
                events.borrow_mut().push(Event::Run(allowance));
                Ok::<_, Stop>((native, used))
            });
            assert_eq!(result, Ok((native, used)));
            assert_eq!(
                *events.borrow(),
                [
                    Event::Run(granted),
                    Event::Commit(used),
                    Event::Drop(Some(used))
                ]
            );
        }
    }
}

#[test]
fn an_outer_refusal_retains_its_type_and_drops_without_a_receipt() {
    let events = RefCell::new(Vec::new());
    let result = Meter::under_reservation(Guard::new(7, &events), |allowance| {
        events.borrow_mut().push(Event::Run(allowance));
        Err::<((), u64), _>(Stop::WorkLimit)
    });
    assert_eq!(result, Err(Failure::Refused(Stop::WorkLimit)));
    assert_eq!(*events.borrow(), [Event::Run(7), Event::Drop(None)]);
}

#[test]
fn an_excessive_receipt_never_commits() {
    for granted in [0, 7, u64::MAX - 1] {
        let events = RefCell::new(Vec::new());
        let result = Meter::under_reservation(Guard::new(granted, &events), |allowance| {
            events.borrow_mut().push(Event::Run(allowance));
            Ok::<_, Stop>(((), granted + 1))
        });
        assert_eq!(result, Err(Failure::Stopped(Stop::InvalidProgram)));
        assert_eq!(*events.borrow(), [Event::Run(granted), Event::Drop(None)]);
    }
}

#[test]
fn an_unwinding_callback_drops_the_admitted_guard_without_committing() {
    let events = RefCell::new(Vec::new());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        Meter::under_reservation(
            Guard::new(7, &events),
            |allowance| -> Result<((), u64), Stop> {
                events.borrow_mut().push(Event::Run(allowance));
                panic!("kernel returned no receipt");
            },
        )
    }));
    assert!(result.is_err());
    assert_eq!(*events.borrow(), [Event::Run(7), Event::Drop(None)]);
}
