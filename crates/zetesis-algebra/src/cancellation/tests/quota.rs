use super::super::*;
use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Error {
    Storage { required: u128, limit: u128 },
    Source(usize),
    Control(Stop),
}

impl Refusal for Error {
    fn is_resource_refusal(&self) -> bool {
        match self {
            Self::Storage { .. } => true,
            Self::Source(_) => false,
            Self::Control(stop) => stop.is_resource_refusal(),
        }
    }

    fn from_stop(stop: Stop) -> Self {
        Self::Control(stop)
    }
}

#[derive(Default)]
struct Account {
    grants: VecDeque<Result<u64, Error>>,
    requests: Vec<u64>,
    refunds: Vec<u64>,
}

impl Account {
    fn new(grants: impl IntoIterator<Item = Result<u64, Error>>) -> Self {
        Self {
            grants: grants.into_iter().collect(),
            ..Self::default()
        }
    }
}

impl Quota for Account {
    type Error = Error;

    fn reserve(&mut self, wanted: u64) -> Result<u64, Error> {
        self.requests.push(wanted);
        self.grants
            .pop_front()
            .expect("every reservation is specified")
    }

    fn refund(&mut self, unspent: u64) {
        self.refunds.push(unspent);
    }
}

#[test]
fn partial_topups_retain_the_exact_refusal_and_refund_only_unused_grants() {
    let error = Error::Storage {
        required: 4097,
        limit: 4096,
    };
    let mut account = Account::new([Ok(5), Ok(2), Err(error)]);
    let mut meter = Meter::reserved(&mut account);
    for _ in 0..3 {
        meter.tick().unwrap();
    }
    let mut executed = false;
    assert_eq!(
        meter.bounded(8, || {
            executed = true;
            ((), 8)
        }),
        Err(Failure::Refused(error))
    );
    assert!(!executed);
    assert_eq!((meter.charged(), meter.remaining()), (3, 4));
    meter.refund();
    assert_eq!((meter.charged(), meter.remaining()), (3, 0));
    assert_eq!(account.requests, [256, 256, 256]);
    assert_eq!(account.refunds, [4]);
}

#[test]
fn bounded_work_counts_actual_failure_work_without_charging_the_bound() {
    let mut account = Account::new([Ok(256), Ok(256)]);
    let mut meter = Meter::reserved(&mut account);
    let failure = Error::Source(17);
    assert_eq!(
        meter.bounded(200, || (Err::<(), _>(failure), 7)),
        Ok(Err(failure))
    );
    assert_eq!((meter.charged(), meter.remaining()), (7, 249));
    // The shortage is one, but the unchanged top-up asks for a full batch.
    assert_eq!(meter.bounded(250, || (23, 3)), Ok(23));
    assert_eq!((meter.charged(), meter.remaining()), (10, 502));
    meter.refund();
    assert_eq!(account.requests, [256, 256]);
    assert_eq!(account.refunds, [502]);
}

#[test]
fn every_small_refusal_retains_the_independently_counted_read_prefix() {
    for ceiling in 0..600_u64 {
        let grants = (0..ceiling.div_ceil(256))
            .map(|batch| Ok(256.min(ceiling - batch * 256)))
            .chain([Err(Error::Source(91))]);
        let mut account = Account::new(grants);
        let mut meter = Meter::reserved(&mut account);
        let mut reads = Vec::new();
        let refusal = loop {
            match meter.tick() {
                Ok(()) => reads.push(reads.len()),
                Err(error) => break error,
            }
        };
        assert_eq!(refusal, Failure::Refused(Error::Source(91)));
        assert_eq!(u64::try_from(reads.len()).unwrap(), ceiling);
        assert!(reads.into_iter().eq(0..usize::try_from(ceiling).unwrap()));
        assert_eq!((meter.charged(), meter.remaining()), (ceiling, 0));
        meter.refund();
        assert_eq!(account.refunds, [0]);
        assert!(account.requests.iter().all(|&request| request == 256));
    }
}

#[test]
fn optional_exhaustion_precedes_the_global_reservation() {
    let mut account = Account::new([Err(Error::Source(8))]);
    let mut meter = Meter::reserved(&mut account);
    let mut visits = 0;
    assert_eq!(meter.tick_within(&mut visits), Ok(false));
    visits = 1;
    assert_eq!(
        meter.tick_within(&mut visits),
        Err(Failure::Refused(Error::Source(8)))
    );
    assert_eq!(visits, 1);
    assert_eq!(meter.charged(), 0);
    assert_eq!(account.requests, [256]);
}

#[test]
fn optional_visits_decrement_only_after_the_global_read_is_admitted() {
    let mut account = Account::new([Ok(2)]);
    let mut meter = Meter::reserved(&mut account);
    let mut visits = 1;
    assert_eq!(meter.tick_within(&mut visits), Ok(true));
    assert_eq!(visits, 0);
    assert_eq!(meter.tick_within(&mut visits), Ok(false));
    assert_eq!((meter.charged(), meter.remaining()), (1, 1));
    meter.refund();
    assert_eq!(account.refunds, [1]);
}

struct PollingAccount<'a> {
    cancellation: &'a Cancellation,
    reservations: usize,
    refunded: u64,
}

impl Quota for PollingAccount<'_> {
    type Error = Stop;

    fn reserve(&mut self, wanted: u64) -> Result<u64, Stop> {
        self.reservations += 1;
        self.cancellation.poll()?;
        Ok(wanted)
    }

    fn refund(&mut self, unspent: u64) {
        self.refunded += unspent;
    }
}

#[test]
fn reserved_ticks_observe_control_only_at_the_existing_refill_boundary() {
    let cancellation = Cancellation::default();
    let mut account = PollingAccount {
        cancellation: &cancellation,
        reservations: 0,
        refunded: 0,
    };
    let mut meter = Meter::reserved(&mut account);
    meter.tick().unwrap();
    cancellation.cancel();
    for _ in 1..256 {
        meter.tick().unwrap();
    }
    assert_eq!(meter.tick(), Err(Failure::Refused(Stop::Cancelled)));
    assert_eq!(meter.charged(), 256);
    meter.refund();
    assert_eq!(account.reservations, 2);
    assert_eq!(account.refunded, 0);
}

#[test]
fn local_work_preserves_the_unpolled_ceiling_and_bounded_prefix() {
    let mut meter = Meter::local(3);
    assert_eq!(meter.tick(), Ok(()));
    assert_eq!(meter.bounded(2, || (11, 1)), Ok(11));
    assert_eq!(
        meter.bounded(2, || -> ((), u64) { panic!("refused before execution") }),
        Err(Failure::Stopped(Stop::WorkLimit))
    );
    assert_eq!((meter.charged(), meter.remaining()), (2, 1));
    assert_eq!(meter.tick(), Ok(()));
    assert_eq!(meter.tick(), Err(Failure::Stopped(Stop::WorkLimit)));
    meter.refund();
    assert_eq!((meter.charged(), meter.remaining()), (3, 0));
}

#[test]
fn zero_grants_and_invalid_bounded_grants_preserve_distinct_local_stops() {
    let mut zero = Account::new([Ok(0)]);
    assert_eq!(
        Meter::reserved(&mut zero).tick(),
        Err(Failure::Stopped(Stop::WorkLimit))
    );
    assert!(zero.refunds.is_empty());
    let mut excessive = Account::new([Ok(257)]);
    let mut meter = Meter::reserved(&mut excessive);
    assert_eq!(
        meter.bounded(1, || ((), 1)),
        Err(Failure::Stopped(Stop::InvalidProgram))
    );
    assert_eq!((meter.charged(), meter.remaining()), (0, 0));
    assert_eq!(excessive.refunds, [257]);
}

#[test]
fn refund_preserves_the_count_for_the_next_attempt_delta() {
    let mut account = Account::new([Ok(8), Ok(8)]);
    let mut meter = Meter::reserved(&mut account);
    for _ in 0..3 {
        meter.tick().unwrap();
    }
    meter.refund();
    let before = meter.charged();
    meter.tick().unwrap();
    meter.refund();
    assert_eq!(meter.charged() - before, 1);
    assert_eq!(meter.charged(), 4);
    assert_eq!(account.refunds, [5, 7]);
}

#[test]
fn nested_failure_classification_preserves_the_origin() {
    let source = Failure::Refused(Failure::Refused(Error::Source(27)));
    assert!(!source.is_resource_refusal());
    assert!(
        Failure::Refused(Failure::Refused(Error::Storage {
            required: 8,
            limit: 7
        }))
        .is_resource_refusal()
    );
    assert!(Failure::<Error>::Stopped(Stop::Allocation).is_resource_refusal());
    assert!(!Failure::<Error>::Stopped(Stop::WrongProgram).is_resource_refusal());
    assert_eq!(
        Failure::Refused(Stop::InvalidProgram).into_stop(),
        Stop::InvalidProgram
    );
}

#[test]
fn the_existing_exit_settles_only_typed_resource_failures() {
    for result in [
        Ok(19),
        Err(Error::Source(6)),
        Err(Error::Storage {
            required: 3,
            limit: 2,
        }),
    ] {
        let cancellation = Cancellation::default();
        let attempt = cancellation.metered(5, |meter| {
            meter
                .charge()
                .map_err(|error| Failure::<Error>::Stopped(error.into_stop()))?;
            cancellation.cancel();
            result.map_err(Failure::Refused)
        });
        let expected = match result {
            Err(Error::Storage { .. }) => Err(Failure::Stopped(Stop::Cancelled)),
            other => other.map_err(Failure::Refused),
        };
        assert_eq!(attempt.result, expected);
        assert_eq!(attempt.charged, 1);
    }
}

#[test]
fn a_panicking_bounded_operation_does_not_refund_unknown_work() {
    let mut account = Account::new([Ok(256)]);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut meter = Meter::reserved(&mut account);
        meter.bounded(10, || -> ((), u64) {
            panic!("no returned actual-work receipt")
        })
    }));
    assert!(result.is_err());
    assert_eq!(account.requests, [256]);
    assert!(account.refunds.is_empty());
}

#[test]
fn a_bounded_overflow_refuses_before_requesting_or_executing() {
    let mut account = Account::default();
    let mut meter = Meter::reserved(&mut account);
    meter.count.value = u64::MAX;
    meter.limit = u64::MAX;
    assert_eq!(
        meter.bounded(1, || ((), 1)),
        Err(Failure::Stopped(Stop::WorkLimit))
    );
    assert_eq!(meter.charged(), u64::MAX);
    assert!(account.requests.is_empty());
}

#[test]
fn bounded_topups_request_only_representable_grants_at_the_counter_limit() {
    let mut account = Account::new([Ok(4)]);
    let mut meter = Meter::reserved(&mut account);
    meter.count.value = u64::MAX - 4;
    meter.limit = u64::MAX - 4;
    assert_eq!(meter.bounded(4, || ((), 4)), Ok(()));
    assert_eq!((meter.charged(), meter.remaining()), (u64::MAX, 0));
    assert_eq!(
        meter.bounded(1, || ((), 1)),
        Err(Failure::Stopped(Stop::WorkLimit))
    );
    assert_eq!(account.requests, [4]);
    assert!(account.refunds.is_empty());
}
