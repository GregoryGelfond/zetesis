use std::cell::RefCell;
use std::rc::Rc;

use super::*;

/// The original implementation, retained as the independently stepped trace.
fn scalar<Q: Quota + ?Sized, S: AsRef<[u64]> + AsMut<[u64]>>(
    target: &mut Words<u64, S>,
    source: &[u64],
    work: &mut Meter<'_, Q>,
) -> Result<(), Failure<Q::Error>> {
    assert_eq!(target.len(), source.len(), "snapshot word shapes agree");
    for (target, source) in target.iter_mut().zip(source) {
        charge(work, 1)?;
        *target = *source;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum Copying {
    Scalar,
    Covered,
}

impl Copying {
    fn run<Q: Quota + ?Sized, S: AsRef<[u64]> + AsMut<[u64]>>(
        self,
        target: &mut Words<u64, S>,
        source: &[u64],
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        match self {
            Self::Scalar => scalar(target, source, work),
            Self::Covered => copy_words(target, source, work),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Shared,
    Mutable,
    Replenish(u64, u64, u64),
    Reserve(u64),
    Poll,
    Grant(u64),
    Refund(u64),
}

type Events = Rc<RefCell<Vec<Event>>>;

struct Backing {
    words: Vec<u64>,
    shared: usize,
    mutable: usize,
    events: Events,
    cancel_on_mutable: Option<Cancellation>,
}

impl AsRef<[u64]> for Backing {
    fn as_ref(&self) -> &[u64] {
        self.events.borrow_mut().push(Event::Shared);
        &self.words[..self.shared]
    }
}

impl AsMut<[u64]> for Backing {
    fn as_mut(&mut self) -> &mut [u64] {
        self.events.borrow_mut().push(Event::Mutable);
        if let Some(cancellation) = &self.cancel_on_mutable {
            cancellation.cancel();
        }
        &mut self.words[..self.mutable]
    }
}

#[derive(Clone, Copy, Debug)]
enum Fault {
    None,
    Refuse(usize),
    Cancel(usize),
    Deadline(usize),
}

struct Account {
    grant: u64,
    remaining: u64,
    calls: usize,
    fault: Fault,
    cancellation: Cancellation,
    events: Events,
}

impl Quota for Account {
    type Error = Denied;

    fn reserve(&mut self, wanted: u64) -> Result<u64, Denied> {
        self.events.borrow_mut().push(Event::Reserve(wanted));
        let call = self.calls;
        self.calls += 1;
        if matches!(self.fault, Fault::Cancel(at) if at == call) {
            self.cancellation.cancel();
        }
        self.events.borrow_mut().push(Event::Poll);
        self.cancellation.poll().map_err(Denied::Control)?;
        match self.fault {
            Fault::Refuse(at) if at == call => return Err(Denied::Ticket(19)),
            // A deterministic caller-owned deadline observation, without a timer.
            Fault::Deadline(at) if at == call => return Err(Denied::Control(Stop::Deadline)),
            _ => {}
        }
        let granted = self.grant.min(wanted).min(self.remaining);
        self.remaining -= granted;
        self.events.borrow_mut().push(Event::Grant(granted));
        Ok(granted)
    }

    fn refund(&mut self, unused: u64) {
        self.events.borrow_mut().push(Event::Refund(unused));
        self.remaining += unused;
    }
}

/// Observe the minimum without reimplementing Quota's default replenishment.
struct Recorded(Account);

impl Quota for Recorded {
    type Error = Denied;

    fn reserve(&mut self, wanted: u64) -> Result<u64, Denied> {
        self.0.reserve(wanted)
    }

    fn refund(&mut self, unused: u64) {
        self.0.refund(unused);
    }

    fn replenish(
        &mut self,
        unused: &mut u64,
        minimum: u64,
        capacity: u64,
    ) -> Result<(), Failure<Denied>> {
        self.0
            .events
            .borrow_mut()
            .push(Event::Replenish(*unused, minimum, capacity));
        self.0.replenish(unused, minimum, capacity)
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Trace {
    result: Result<(), Failure<Denied>>,
    words: Vec<u64>,
    charged: u64,
    unused_before_refund: u64,
    account_remaining: u64,
    events: Vec<Event>,
}

fn traced(
    copying: Copying,
    length: usize,
    grant: u64,
    limit: u64,
    prime: bool,
    fault: Fault,
) -> Trace {
    let events = Events::default();
    let mut account = Recorded(Account {
        grant,
        remaining: limit,
        calls: 0,
        fault,
        cancellation: Cancellation::default(),
        events: Rc::clone(&events),
    });
    let mut target = Words::new(Backing {
        words: vec![u64::MAX; length],
        shared: length,
        mutable: length,
        events: Rc::clone(&events),
        cancel_on_mutable: None,
    });
    let source: Vec<_> = (0..length)
        .map(|index| u64::try_from(index).unwrap())
        .collect();
    let (result, charged, unused_before_refund) = {
        let mut work = Meter::reserved(&mut account);
        let result = charge(&mut work, u64::from(prime))
            .and_then(|()| copying.run(&mut target, &source, &mut work));
        let charged = work.charged();
        let unused = work.remaining();
        work.refund();
        (result, charged, unused)
    };
    let recorded = events.borrow().clone();
    Trace {
        result,
        words: target.into_storage().words,
        charged,
        unused_before_refund,
        account_remaining: account.0.remaining,
        events: recorded,
    }
}

#[test]
fn covered_word_copy_preserves_every_local_cutoff_and_written_prefix() {
    for length in [0, 1, 2, 3, 7, 255, 256, 257, 513] {
        let source: Vec<_> = (0..length)
            .map(|index| u64::try_from(index).unwrap())
            .collect();
        for limit in 0..=u64::try_from(length).unwrap() + 1 {
            let mut target = Words::new(vec![u64::MAX; length]);
            let mut reference = target.clone();
            let mut work = Meter::local(limit);
            let mut prior = Meter::local(limit);
            assert_eq!(
                copy_words(&mut target, &source, &mut work),
                scalar(&mut reference, &source, &mut prior)
            );
            assert_eq!(&*target, &*reference);
            assert_eq!(work.charged(), prior.charged());
            let copied = length.min(usize::try_from(limit).unwrap());
            assert_eq!(&target[..copied], &source[..copied]);
            assert!(target[copied..].iter().all(|word| *word == u64::MAX));
            assert_eq!(work.charged(), u64::try_from(copied).unwrap());
        }
    }
}

#[test]
fn covered_word_copy_preserves_grants_minima_polls_and_refunds() {
    for grant in [1, 7, 256] {
        for length in [0, 1, 3, 17, 257, 513] {
            for prime in [false, true] {
                for limit in 0..=u64::try_from(length).unwrap() + 2 {
                    let reference =
                        traced(Copying::Scalar, length, grant, limit, prime, Fault::None);
                    let actual = traced(Copying::Covered, length, grant, limit, prime, Fault::None);
                    assert_eq!(
                        actual, reference,
                        "grant {grant}, length {length}, limit {limit}, prime {prime}"
                    );
                    assert!(actual.events.iter().all(
                        |event| !matches!(event, Event::Replenish(_, minimum, _) if *minimum != 1)
                    ));
                }
            }
        }
    }
}

#[test]
fn covered_word_copy_preserves_each_refill_refusal_and_control_stop() {
    for grant in [1, 7, 256] {
        let length = 2 * usize::try_from(grant).unwrap() + 3;
        for prime in [false, true] {
            let reserves = (u64::try_from(length).unwrap() + u64::from(prime)).div_ceil(grant);
            for at in 0..=usize::try_from(reserves).unwrap() {
                for fault in [Fault::Refuse(at), Fault::Cancel(at), Fault::Deadline(at)] {
                    assert_eq!(
                        traced(Copying::Covered, length, grant, 1024, prime, fault),
                        traced(Copying::Scalar, length, grant, 1024, prime, fault),
                        "grant {grant}, prime {prime}, fault {fault:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn covered_word_copy_reaches_the_same_representable_count_boundary() {
    let source = [1, 2, 3, 4, 5];
    let mut target = Words::new([9; 5]);
    let mut work = Meter::local(u64::MAX);
    work.absorb(u64::MAX - 3).unwrap();
    assert_eq!(
        copy_words(&mut target, &source, &mut work),
        Err(Failure::Stopped(Stop::WorkLimit))
    );
    assert_eq!(&*target, &[1, 2, 3, 9, 9]);
    assert_eq!(work.charged(), u64::MAX);
    assert_eq!(work.remaining(), 0);
    let mut reference = Words::new([9; 5]);
    let mut prior = Meter::local(u64::MAX);
    prior.absorb(u64::MAX - 3).unwrap();
    assert_eq!(
        scalar(&mut reference, &source, &mut prior),
        Err(Failure::Stopped(Stop::WorkLimit))
    );
    assert_eq!(&*target, &*reference);
    assert_eq!(work.charged(), prior.charged());
}

#[test]
fn covered_word_copy_acquires_generic_mutable_storage_once_and_keeps_zip_extent() {
    for (shared, mutable) in [(0, 0), (0, 3), (3, 0), (3, 2), (3, 3), (3, 5)] {
        let events = Events::default();
        let mut target = Words::new(Backing {
            words: vec![9; shared.max(mutable)],
            shared,
            mutable,
            events: Rc::clone(&events),
            cancel_on_mutable: None,
        });
        let mut work = Meter::local(u64::MAX);
        assert_eq!(copy_words(&mut target, &vec![1; shared], &mut work), Ok(()));
        let copied = shared.min(mutable);
        assert_eq!(work.charged(), u64::try_from(copied).unwrap());
        assert_eq!(&target.storage().words[..copied], vec![1; copied]);
        assert!(
            target.storage().words[copied..]
                .iter()
                .all(|word| *word == 9)
        );
        assert_eq!(*events.borrow(), [Event::Shared, Event::Mutable]);
    }
}

#[test]
fn generic_mutable_acquisition_precedes_the_first_copy_admission() {
    let events = Events::default();
    let cancellation = Cancellation::default();
    let mut target = Words::new(Backing {
        words: vec![9; 3],
        shared: 3,
        mutable: 3,
        events: Rc::clone(&events),
        cancel_on_mutable: Some(cancellation.clone()),
    });
    let mut account = Recorded(Account {
        grant: 256,
        remaining: 256,
        calls: 0,
        fault: Fault::None,
        cancellation,
        events: Rc::clone(&events),
    });
    let mut work = Meter::reserved(&mut account);
    assert_eq!(
        copy_words(&mut target, &[1, 2, 3], &mut work),
        Err(Failure::Refused(Denied::Control(Stop::Cancelled)))
    );
    assert_eq!(work.charged(), 0);
    assert_eq!(target.storage().words, [9; 3]);
    assert_eq!(
        *events.borrow(),
        [
            Event::Shared,
            Event::Mutable,
            Event::Replenish(0, 1, u64::MAX),
            Event::Reserve(256),
            Event::Poll
        ]
    );
}

#[test]
fn cancellation_after_a_grant_is_observed_at_the_original_next_refill() {
    let events = Events::default();
    let cancellation = Cancellation::default();
    let mut target = Words::new(Backing {
        words: vec![9; 8],
        shared: 8,
        mutable: 8,
        events: Rc::clone(&events),
        cancel_on_mutable: Some(cancellation.clone()),
    });
    let mut account = Recorded(Account {
        grant: 7,
        remaining: 256,
        calls: 0,
        fault: Fault::None,
        cancellation,
        events: Rc::clone(&events),
    });
    let mut work = Meter::reserved(&mut account);
    charge(&mut work, 1).unwrap();
    assert_eq!(work.remaining(), 6);
    assert_eq!(
        copy_words(&mut target, &[1; 8], &mut work),
        Err(Failure::Refused(Denied::Control(Stop::Cancelled)))
    );
    assert_eq!(target.storage().words, [1, 1, 1, 1, 1, 1, 9, 9]);
    assert_eq!(work.charged(), 7);
    assert_eq!(work.remaining(), 0);
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|event| **event == Event::Poll)
            .count(),
        2
    );
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|event| **event == Event::Mutable)
            .count(),
        1
    );
}

#[test]
fn mismatched_word_shape_panics_before_mutable_storage_or_admission() {
    let events = Events::default();
    let mut target = Words::new(Backing {
        words: vec![9; 3],
        shared: 3,
        mutable: 3,
        events: Rc::clone(&events),
        cancel_on_mutable: None,
    });
    let mut work = Meter::local(0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        copy_words(&mut target, &[1, 2], &mut work)
    }));
    assert!(result.is_err());
    assert_eq!(work.charged(), 0);
    assert_eq!(*events.borrow(), [Event::Shared]);
    assert_eq!(target.storage().words, [9; 3]);
}
