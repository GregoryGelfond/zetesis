mod copying;

use std::mem::size_of;

use zetesis_algebra::Refusal;

use super::*;

#[test]
fn counted_intersection_counts_the_common_positions() {
    let left = [1 | (1 << 63), 1 | (1 << 7)];
    let right = [1 << 63, 1 | (1 << 8)];
    let mut work = Meter::local(u64::MAX);
    // The sets share exactly positions 63 and 64, across the word boundary.
    assert_eq!(counted_intersection(&left, &right, &mut work), Ok(2));
    assert_eq!(work.charged(), 7);
}

#[test]
fn an_empty_intersection_only_initializes_the_accumulator() {
    let mut work = Meter::local(1);
    assert_eq!(counted_intersection(&[], &[], &mut work), Ok(0));
    assert_eq!(work.charged(), 1);
}

#[test]
fn intersection_refusal_keeps_the_admitted_primitive_prefix() {
    let left = [u64::MAX, u64::MAX];
    let right = [1, 1 << 63];
    // The two-unit popcount/addition is atomic; an unused last permit is not
    // reported as performed work. AND is separately admitted before it.
    for (limit, prefix) in [(0, 0), (1, 1), (2, 2), (3, 2), (4, 4), (5, 5), (6, 5)] {
        let mut work = Meter::local(limit);
        assert_eq!(
            counted_intersection(&left, &right, &mut work),
            Err(Failure::Stopped(Stop::WorkLimit))
        );
        assert_eq!(work.charged(), prefix, "limit {limit}");
        assert_eq!(left, [u64::MAX, u64::MAX]);
        assert_eq!(right, [1, 1 << 63]);
    }
}

#[test]
fn initialized_word_copies_stop_before_the_refused_write() {
    for limit in 0..=3 {
        let mut target = Words::new([9, 9, 9]);
        let mut work = Meter::local(limit);
        let result = copy_words(&mut target, &[1, 2, 3], &mut work);
        assert_eq!(
            result,
            if limit == 3 {
                Ok(())
            } else {
                Err(Failure::Stopped(Stop::WorkLimit))
            }
        );
        let expected: Vec<_> = (0..3)
            .map(|index| if index < limit { index + 1 } else { 9 })
            .collect();
        assert_eq!(&*target, expected.as_slice());
        assert_eq!(work.charged(), limit);
    }
}

#[test]
fn preparing_shorter_words_reuses_real_capacity() {
    let mut target = Words::new(vec![7, 8, 9]);
    let pointer = target.as_ptr();
    let capacity = target.capacity();
    let mut work = Meter::local(u64::MAX);
    prepare_words(&mut target, 2, &mut work).unwrap();
    assert_eq!(&*target, &[0, 0]);
    assert_eq!(target.as_ptr(), pointer);
    assert_eq!(target.capacity(), capacity);
    assert_eq!(work.charged(), 5); // capacity test, two writes, truncate test/length
}

#[test]
fn impossible_word_capacity_preserves_the_old_buffer() {
    let mut target = Words::new(vec![7]);
    let pointer = target.as_ptr();
    let mut work = Meter::local(u64::MAX);
    assert_eq!(
        prepare_words(&mut target, usize::MAX, &mut work),
        Err(Failure::Stopped(Stop::Allocation))
    );
    assert_eq!(&*target, &[7]);
    assert_eq!(target.as_ptr(), pointer);
    assert_eq!(work.charged(), 4); // capacity condition and empty temporary header
}

#[test]
fn scratch_bytes_count_actual_payload_capacities_once() {
    let scratch = ArithmeticScratch {
        touched: Words::new(Vec::with_capacity(3)),
        open: Words::new(Vec::with_capacity(5)),
        consequences: Vec::with_capacity(7),
    };
    let payload = (scratch.touched.capacity() as u128 + scratch.open.capacity() as u128)
        * size_of::<u64>() as u128
        + scratch.consequences.capacity() as u128 * size_of::<(usize, bool)>() as u128;
    assert_eq!(scratch.allocated_bytes(), payload);
    assert_eq!(
        size_of::<ArithmeticScratch>(),
        2 * size_of::<Words<u64>>() + size_of::<Vec<(usize, bool)>>()
    );
}

#[test]
fn entry_cancellation_invalidates_copy_without_semantic_work() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut destination = Some(9);
    let mut work = Meter::local(0);
    let result = copy_attempt(
        &1,
        &mut destination,
        &cancellation,
        &mut work,
        |_| panic!("cancelled copy must not initialize"),
        |_, _, _| panic!("cancelled copy must not read or copy payload"),
    );
    assert_eq!(result, Err(Failure::Stopped(Stop::Cancelled)));
    assert_eq!(destination, None);
    assert_eq!(work.charged(), 0);
}

#[test]
fn every_copy_refusal_invalidates_the_destination() {
    for limit in 0..=4 {
        let source = 23;
        let mut destination = None;
        let mut work = Meter::local(limit);
        let result = copy_attempt(
            &source,
            &mut destination,
            &Cancellation::default(),
            &mut work,
            |work| {
                charge(work, 1)?;
                Ok(0)
            },
            |target, source, work| {
                charge(work, 1)?;
                *target = *source;
                Ok(())
            },
        );
        assert_eq!(source, 23);
        assert_eq!(work.charged(), limit);
        if limit == 4 {
            assert_eq!(result, Ok(()));
            assert_eq!(destination, Some(23));
        } else {
            assert_eq!(result, Err(Failure::Stopped(Stop::WorkLimit)));
            assert_eq!(destination, None);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Denied {
    Ticket(u8),
    Control(Stop),
}

impl Refusal for Denied {
    fn is_resource_refusal(&self) -> bool {
        match self {
            Self::Ticket(_) => true,
            Self::Control(stop) => stop.is_resource_refusal(),
        }
    }
    fn from_stop(stop: Stop) -> Self {
        Self::Control(stop)
    }
}

struct Reject;
impl Quota for Reject {
    type Error = Denied;
    fn reserve(&mut self, _: u64) -> Result<u64, Denied> {
        Err(Denied::Ticket(19))
    }
    fn refund(&mut self, _: u64) {
        panic!("copy helper must not settle the outer account");
    }
}

#[test]
fn copy_preserves_the_callers_typed_refusal() {
    let mut quota = Reject;
    let mut work = Meter::reserved(&mut quota);
    let mut destination = Some(9);
    let result = copy_attempt(
        &1,
        &mut destination,
        &Cancellation::default(),
        &mut work,
        |_| panic!("first admission refused"),
        |_, _, _| panic!("first admission refused"),
    );
    assert_eq!(result, Err(Failure::Refused(Denied::Ticket(19))));
    assert_eq!(destination, None);
    assert_eq!(work.charged(), 0);
}

#[test]
fn a_retained_copy_target_skips_empty_record_initialization() {
    let mut destination = Some(9);
    let mut work = Meter::local(2);
    let result = copy_attempt(
        &23,
        &mut destination,
        &Cancellation::default(),
        &mut work,
        |_| panic!("an existing target is reused"),
        |target, source, work| {
            charge(work, 1)?;
            *target = *source;
            Ok(())
        },
    );
    assert_eq!(result, Ok(()));
    assert_eq!(destination, Some(23));
    assert_eq!(work.charged(), 2); // presence test and one initialized field copy
}
