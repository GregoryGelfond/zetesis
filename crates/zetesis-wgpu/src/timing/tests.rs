use super::*;

const WINDOW: Duration = Duration::from_secs(1);

#[test]
fn the_timestamp_tail_is_separate_from_logical_results() {
    let words = [7, 9, 10, 0, 14, 0];
    let (results, tail) = split_readback(&words, true).unwrap();
    assert_eq!(results, &[7, 9]);
    assert_eq!(tail, Some([10, 14]));
}

#[test]
fn an_untimed_readback_has_no_tail() {
    let (results, tail) = split_readback(&[7, 9], false).unwrap();
    assert_eq!(results, &[7, 9]);
    assert_eq!(tail, None);
}

#[test]
fn missing_timestamp_words_are_refused() {
    assert_eq!(
        split_readback(&[0; 3], true).unwrap_err().kind(),
        GpuErrorKind::Readback
    );
}

#[test]
fn a_valid_pair_within_its_window_is_measured() {
    assert_eq!(
        interval(10, 14, [None, None], 2.0, WINDOW),
        Some(Duration::from_nanos(8))
    );
}

#[test]
fn a_zero_begin_is_unavailable_even_on_a_fresh_timer() {
    // An unwritten slot of a fresh query set resolves to zero.
    assert_eq!(interval(0, 14, [None, None], 1.0, WINDOW), None);
}

#[test]
fn all_ones_samples_are_unavailable() {
    // An unbounded window isolates the rule: the end would otherwise be read
    // as an interval of about 584 years.
    assert_eq!(
        interval(10, u64::MAX, [None, None], 1.0, Duration::MAX),
        None
    );
    assert_eq!(
        interval(u64::MAX, 14, [None, None], 1.0, Duration::MAX),
        None
    );
}

#[test]
fn a_value_repeating_its_slot_is_unavailable() {
    assert_eq!(interval(10, 14, [Some(10), None], 1.0, WINDOW), None);
    assert_eq!(interval(10, 14, [None, Some(14)], 1.0, WINDOW), None);
    assert_eq!(
        interval(10, 14, [Some(9), Some(15)], 1.0, WINDOW),
        Some(Duration::from_nanos(4))
    );
}

#[test]
fn an_interval_longer_than_its_window_is_unavailable() {
    let pair = |window| interval(10, 1_010, [None, None], 1.0, window);
    assert_eq!(pair(Duration::from_nanos(999)), None);
    assert_eq!(
        pair(Duration::from_micros(1)),
        Some(Duration::from_micros(1))
    );
}

#[test]
fn reversed_and_equal_readings_are_unavailable() {
    assert_eq!(interval(14, 10, [None, None], 1.0, WINDOW), None);
    assert_eq!(interval(14, 14, [None, None], 1.0, WINDOW), None);
}

#[test]
fn invalid_periods_are_unavailable() {
    for period in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(interval(10, 14, [None, None], period, WINDOW), None);
    }
    // A positive interval below the clock's resolution is not a measured zero.
    assert_eq!(duration(10, 11, 0.25), None);
}

/// One call or operation on an oracle's timer.
#[derive(Clone, Copy, Debug)]
enum Event {
    /// A submitting call that completed.
    Complete,
    /// A submitting call that failed after submission, retiring the timer.
    Fail,
    /// A call that submitted nothing.
    Idle,
    /// A flush that read its pair.
    Flush,
    /// A flush that submitted, then failed, retiring the timer.
    FlushFail,
    /// `clear_residency`, releasing the timer.
    Clear,
}

const EVENTS: [Event; 6] = [
    Event::Complete,
    Event::Fail,
    Event::Idle,
    Event::Flush,
    Event::FlushFail,
    Event::Clear,
];

/// A device's four query slots and the ledger, against a reference that knows
/// which batch is pending. Batch `k` writes `(1000k + 1, 1000k + 1 + k)`, so a
/// measured report names the batch whose pass it measured.
#[derive(Default)]
struct Model {
    ledger: PairLedger,
    slots: [u64; 4],
    pending: Option<u64>,
    completed: Vec<u64>,
    reported: Vec<u64>,
    consumed_after: Vec<bool>,
    batches: u64,
}

impl Model {
    fn tail(&self) -> Option<[u64; 2]> {
        self.ledger.resolving().map(|pair| {
            let begin = pair.begin() as usize;
            [self.slots[begin], self.slots[begin + 1]]
        })
    }

    // A submitting call's pass writes its pair before the same command buffer
    // resolves the pending one, as the device orders them.
    fn submit(&mut self) -> u64 {
        if let Some(pair) = self.ledger.resolving() {
            assert_ne!(
                self.ledger.written(),
                pair,
                "a batch overwrote the pending pair"
            );
        }
        self.batches += 1;
        let batch = self.batches;
        let begin = self.ledger.written().begin() as usize;
        self.slots[begin] = 1_000 * batch + 1;
        self.slots[begin + 1] = 1_000 * batch + 1 + batch;
        batch
    }

    // The next timed batch starts a fresh timer and query set, whose unwritten
    // slots resolve to zero.
    fn retire(&mut self) {
        self.ledger = PairLedger::default();
        self.slots = [0; 4];
    }

    fn expect(&mut self, report: Option<BatchComputeTime>, measured: bool) {
        match self.pending.take() {
            Some(batch) => {
                let expected = if measured {
                    BatchComputeTime::Measured(Duration::from_nanos(batch))
                } else {
                    BatchComputeTime::Unavailable
                };
                assert_eq!(report, Some(expected), "batch {batch}");
                self.reported.push(batch);
            }
            None => assert_eq!(report, None),
        }
    }

    fn apply(&mut self, event: Event) {
        if !matches!(event, Event::Idle) {
            self.consumed_after
                .iter_mut()
                .for_each(|consumed| *consumed = true);
        }
        match event {
            Event::Complete => {
                let batch = self.submit();
                let timing = BatchTiming {
                    tail: Some(self.tail().unwrap_or_default()),
                    window: Some(WINDOW),
                };
                let report = self.ledger.completed(&timing, 1.0);
                self.expect(report, true);
                self.pending = Some(batch);
                self.completed.push(batch);
                self.consumed_after.push(false);
            }
            Event::Fail => {
                self.submit();
                let report = self.ledger.abandon();
                self.expect(report, false);
                self.retire();
            }
            Event::Idle => {}
            Event::Flush => {
                // An oracle without a pending batch returns before submitting.
                if let Some(tail) = self.tail() {
                    let report = self.ledger.flushed(tail, 1.0);
                    self.expect(report, true);
                }
            }
            Event::FlushFail => {
                if self.ledger.pending() {
                    let report = self.ledger.abandon();
                    self.expect(report, false);
                    self.retire();
                }
            }
            Event::Clear => {
                let report = self.ledger.abandon();
                self.expect(report, false);
                self.retire();
            }
        }
    }
}

#[test]
fn each_completed_batch_is_reported_once_from_its_own_pair() {
    let mut sequences = vec![Vec::new()];
    for _ in 0..6 {
        sequences = sequences
            .into_iter()
            .flat_map(|sequence: Vec<Event>| {
                EVENTS.into_iter().map(move |event| {
                    let mut next = sequence.clone();
                    next.push(event);
                    next
                })
            })
            .collect();
        for sequence in &sequences {
            let mut model = Model::default();
            for event in sequence {
                model.apply(*event);
            }
            let mut reported = model.reported.clone();
            reported.sort_unstable();
            reported.dedup();
            assert_eq!(reported.len(), model.reported.len(), "{sequence:?}");
            for (batch, consumed) in model.completed.iter().zip(&model.consumed_after) {
                let times = model.reported.iter().filter(|seen| *seen == batch).count();
                // Exactly once when a submitting call, a flush or a release
                // follows its completion; otherwise it is still pending.
                assert_eq!(times, usize::from(*consumed), "{sequence:?}");
            }
            assert_eq!(
                model.ledger.pending(),
                model.pending.is_some(),
                "{sequence:?}"
            );
        }
    }
}

#[test]
fn a_pair_still_holding_its_last_read_values_is_unavailable() {
    let mut ledger = PairLedger::default();
    let read = |tail| BatchTiming {
        tail: Some(tail),
        window: Some(WINDOW),
    };
    // Batch 1 writes the first pair; batch 2 resolves it and writes the second;
    // batch 3 resolves that and writes the first pair again.
    assert_eq!(ledger.completed(&read([0, 0]), 1.0), None);
    assert_eq!(
        ledger.completed(&read([11, 15]), 1.0),
        Some(BatchComputeTime::Measured(Duration::from_nanos(4)))
    );
    assert_eq!(
        ledger.completed(&read([21, 26]), 1.0),
        Some(BatchComputeTime::Measured(Duration::from_nanos(5)))
    );
    // Batch 4 resolves the first pair, which batch 3 did not rewrite.
    assert_eq!(
        ledger.completed(&read([11, 15]), 1.0),
        Some(BatchComputeTime::Unavailable)
    );
}

#[test]
fn a_completed_call_without_a_window_admits_no_interval() {
    let mut ledger = PairLedger::default();
    let unobserved = BatchTiming {
        tail: None,
        window: None,
    };
    assert_eq!(ledger.completed(&unobserved, 1.0), None);
    assert_eq!(
        ledger.flushed([10, 14], 1.0),
        Some(BatchComputeTime::Unavailable)
    );
}
