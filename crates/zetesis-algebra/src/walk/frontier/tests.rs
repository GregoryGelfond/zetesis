//! The generic frontier protocol, independent of solver entry types.

use std::sync::mpsc;
use std::time::Instant;

use super::*;

const WAIT: Duration = Duration::from_secs(2);

#[derive(Debug, PartialEq, Eq)]
struct Packet {
    id: usize,
    bytes: Vec<u8>,
}

impl Payload for Packet {
    fn allocated_bytes(&self) -> u128 {
        self.bytes.capacity() as u128
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Refused {
    Stopped(Stop),
    Injected,
}

impl From<Stop> for Refused {
    fn from(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

type TestFrontier = Frontier<Packet, Refused>;

fn entry() -> Packet {
    packet(0)
}

fn packet(id: usize) -> Packet {
    Packet {
        id,
        bytes: vec![0; id + 1],
    }
}

fn reserve(queue: &mut VecDeque<Packet>, count: usize) -> Result<(), Refused> {
    reserve_entries(queue, count)
}

#[test]
fn a_single_child_publication_grows_the_count_by_one_and_pushes_one_entry() {
    let frontier = TestFrontier::new(2, entry()).unwrap();
    let _held = frontier.take_local(0).unwrap();
    frontier.publish_split(0, [entry()], reserve).unwrap();
    assert_eq!(frontier.termination.unresolved(), 2);
    assert_eq!(frontier.queue(0).len(), 1);
}

#[test]
fn a_resumed_walk_starts_open_at_its_carried_count() {
    let termination = Termination::resuming(5);
    assert_eq!(termination.unresolved(), 5);
    assert!(!termination.has_ended());
}

#[test]
fn a_resolution_to_zero_wakes_the_idle_and_leaves_the_walk_open() {
    let frontier = TestFrontier::new(2, entry()).unwrap();
    let _hand = frontier.take_local(0).unwrap();
    let (done, result) = mpsc::sync_channel(1);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let _ = done.send(frontier.wait_for_work(Duration::from_mins(1), |_| None::<Packet>));
        });
        // The waiter registers under the gate and releases it only by waiting,
        // so the resolution's wake, which takes the gate, reaches it.
        while frontier.termination.idle_waiters.load(Ordering::Relaxed) == 0 {
            std::hint::spin_loop();
        }
        frontier.termination.resolve();
        assert!(
            result.recv_timeout(WAIT).unwrap().is_none(),
            "the waiter was not woken"
        );
    });
    assert!(frontier.termination.is_resolved());
    assert!(!frontier.termination.is_closed());
}

#[test]
fn a_gate_attempt_never_waits_for_a_busy_deque() {
    let frontier = TestFrontier::new(1, entry()).unwrap();
    let busy = frontier.queue(0);
    let started = Instant::now();
    assert_eq!(frontier.try_take_local(0).err(), Some(Busy));
    assert!(started.elapsed() < WAIT);
    drop(busy);
    assert!(matches!(frontier.try_take_local(0), Ok(Some(_))));
}

#[test]
fn a_gate_attempt_tells_a_busy_deque_from_an_empty_one() {
    let frontier = TestFrontier::new(1, entry()).unwrap();
    let _hand = frontier.take_local(0).unwrap();
    let busy = frontier.queue(0);
    assert_eq!(frontier.try_take_local(0).err(), Some(Busy));
    drop(busy);
    assert!(matches!(frontier.try_take_local(0), Ok(None)));
}

#[test]
fn an_unmeasured_frontier_keeps_no_receipts() {
    assert_eq!(TestFrontier::new(1, entry()).unwrap().statistics(), None);
}

#[test]
fn local_removal_is_newest_and_stealing_is_oldest() {
    let frontier = TestFrontier::new(3, packet(0)).unwrap();
    let _hand = frontier.take_local(0).unwrap();
    frontier
        .publish_split(0, [packet(1), packet(2)], reserve)
        .unwrap();
    assert_eq!(frontier.take_local(0).unwrap().id, 2);
    assert_eq!(frontier.steal(1).unwrap().id, 1);
}

#[test]
fn a_refused_split_preserves_queue_count_and_receipts() {
    let frontier = TestFrontier::measured(1, entry()).unwrap();
    let before = frontier.statistics();
    let result = frontier.publish_split(0, [packet(1), packet(2)], |_, count| {
        assert_eq!(count, 2);
        Err(Refused::Injected)
    });
    assert_eq!(result, Err(Refused::Injected));
    assert_eq!(frontier.statistics(), before);
    assert_eq!(frontier.termination.unresolved(), 1);
    assert_eq!(frontier.take_local(0).unwrap().id, 0);
}

#[test]
fn capacity_overflow_preserves_the_typed_allocation_refusal() {
    let frontier = TestFrontier::new(1, entry()).unwrap();
    let result = frontier.publish_split(0, [packet(1)], |queue, _| reserve(queue, usize::MAX));
    assert_eq!(result, Err(Refused::Stopped(Stop::Allocation)));
    assert_eq!(frontier.termination.unresolved(), 1);
    assert_eq!(frontier.queue(0).len(), 1);
}

#[test]
fn idle_cancellation_keeps_its_type_and_poll_cadence() {
    let frontier = TestFrontier::new(1, entry()).unwrap();
    let _hand = frontier.take_local(0).unwrap();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut outside = 0;
    let mut under_gate = 0;
    let mut stopped = None;
    let found = frontier.find_work(
        Duration::ZERO,
        &cancellation,
        |attempt| {
            match attempt {
                Attempt::Outside => outside += 1,
                Attempt::UnderGate => under_gate += 1,
            }
            None::<Packet>
        },
        |error| stopped = Some(error),
    );
    assert!(found.is_none());
    assert_eq!(stopped, Some(Refused::Stopped(Stop::Cancelled)));
    assert_eq!(outside, 16);
    assert_eq!(under_gate, 1);
    assert_eq!(frontier.termination.unresolved(), 1);
    assert!(!frontier.termination.is_closed());
}

#[test]
fn release_drops_owned_storage_but_keeps_receipt_peaks() {
    let frontier = TestFrontier::measured(2, packet(2)).unwrap();
    let _hand = frontier.take_local(0).unwrap();
    let children = [packet(3), packet(4)];
    let payload: usize = children.iter().map(|entry| entry.bytes.capacity()).sum();
    frontier.publish_split(1, children, reserve).unwrap();
    let populated = frontier.statistics().unwrap();
    let headers = size_of::<Vec<Mutex<VecDeque<Packet>>>>()
        + frontier.queues.capacity() * size_of::<Mutex<VecDeque<Packet>>>();
    assert_eq!(populated.regions, 2);
    assert_eq!(
        populated.retained_bytes,
        (headers + populated.capacity * size_of::<Packet>() + payload) as u128
    );
    frontier.release();
    let released = frontier.statistics().unwrap();
    assert_eq!(released.regions, 0);
    assert_eq!(released.capacity, 0);
    assert_eq!(released.retained_bytes, headers as u128);
    assert_eq!(released.peak_regions, populated.peak_regions);
    assert_eq!(released.peak_capacity, populated.peak_capacity);
    assert_eq!(released.peak_retained_bytes, populated.peak_retained_bytes);
}
