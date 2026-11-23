use std::cmp::Ordering;
use std::num::NonZeroUsize;

use proptest::prelude::*;
use zetesis_algebra::threads::Pool;

use super::{ContentInterner, ContentOrder, Frozen, Handle, Id, Words, Work, sort_by_content};

fn pool(threads: usize) -> Pool {
    Pool::new(NonZeroUsize::new(threads).unwrap(), |index| {
        format!("intern-test-{index}")
    })
    .unwrap()
}

/// Intern every content, freeze, and return each content's final identity.
fn frozen_ids(interner: &mut ContentInterner<Words>, contents: &[Vec<u32>]) -> Vec<Id> {
    let mut work = Work::default();
    let handles: Vec<Handle> = contents
        .iter()
        .map(|content| interner.intern(content, &mut work).unwrap())
        .collect();
    let generation = interner.freeze(None, &mut work).unwrap();
    handles
        .into_iter()
        .map(|handle| generation.finalize(handle))
        .collect()
}

#[test]
fn freeze_numbers_a_generation_in_content_order() {
    let mut interner = ContentInterner::new(Words);
    let ids = frozen_ids(&mut interner, &[vec![3], vec![1, 9], vec![2], vec![1]]);
    let indices: Vec<usize> = ids.iter().map(|id| id.index()).collect();
    // [1] < [1, 9] < [2] < [3] lexicographically.
    assert_eq!(indices, [3, 1, 2, 0]);
}

#[test]
fn equal_content_interns_to_one_identity() {
    let mut interner = ContentInterner::new(Words);
    let ids = frozen_ids(&mut interner, &[vec![5, 6], vec![5, 6], vec![7]]);
    assert_eq!(ids[0], ids[1]);
    assert_eq!(interner.len(), 2);
}

#[test]
fn a_frozen_lookup_allocates_nothing() {
    let mut interner = ContentInterner::new(Words);
    let contents: Vec<Vec<u32>> = (0..500).map(|value| vec![value, value * 7]).collect();
    let ids = frozen_ids(&mut interner, &contents);
    let bytes = interner.bytes();
    let mut work = Work::default();
    for (content, id) in contents.iter().zip(&ids) {
        assert_eq!(interner.lookup(content, &mut work), Some(*id));
        assert_eq!(interner.intern(content, &mut work), Ok(Handle::Final(*id)));
    }
    assert_eq!(interner.bytes(), bytes);
    assert_eq!(interner.pending_len(), 0);
}

#[test]
fn a_later_generation_follows_every_earlier_identity() {
    let mut interner = ContentInterner::new(Words);
    let first = frozen_ids(&mut interner, &[vec![50], vec![10]]);
    let second = frozen_ids(&mut interner, &[vec![30], vec![5]]);
    assert!(second.iter().all(|id| id.index() >= 2));
    assert_eq!(interner.generations(), 2);
    // Across generations the one comparison reads content, not identities.
    assert_eq!(interner.compare(second[1], first[1]), Ordering::Less);
    assert_eq!(interner.compare(second[0], first[0]), Ordering::Less);
    assert_eq!(interner.compare(second[0], first[1]), Ordering::Greater);
    let mut work = Work::default();
    let ordered: Vec<&[u32]> = interner
        .ordered(&mut work)
        .into_iter()
        .map(|id| interner.content(id))
        .collect();
    assert_eq!(ordered, [&[5][..], &[10], &[30], &[50]]);
}

#[test]
fn an_empty_freeze_starts_no_generation() {
    let mut interner = ContentInterner::new(Words);
    let mut work = Work::default();
    interner.freeze(None, &mut work).unwrap();
    assert_eq!(interner.generations(), 0);
    assert_eq!(work, Work::default());
}

/// Terms `[0, n]` (a number) and `[1, child]` (a successor of an earlier
/// generation's term): content order compares children through the
/// interner's own comparison, so nesting needs no second order.
struct Nested;

impl ContentOrder for Nested {
    fn compare(&self, ids: Frozen<'_>, left: &[u32], right: &[u32]) -> Ordering {
        left[0].cmp(&right[0]).then_with(|| match left[0] {
            0 => left[1].cmp(&right[1]),
            _ => ids.compare(Id::from_word(left[1]), Id::from_word(right[1])),
        })
    }
}

#[test]
fn nested_content_orders_by_its_children_order() {
    let mut interner = ContentInterner::new(Nested);
    let mut work = Work::default();
    let numbers: Vec<Handle> = [9, 4]
        .iter()
        .map(|&n| interner.intern(&[0, n], &mut work).unwrap())
        .collect();
    let generation = interner.freeze(None, &mut work).unwrap();
    let (nine, four) = (
        generation.finalize(numbers[0]),
        generation.finalize(numbers[1]),
    );
    let successors: Vec<Handle> = [nine, four]
        .iter()
        .map(|child| interner.intern(&[1, child.word()], &mut work).unwrap())
        .collect();
    let generation = interner.freeze(None, &mut work).unwrap();
    let (s9, s4) = (
        generation.finalize(successors[0]),
        generation.finalize(successors[1]),
    );
    // s(4) < s(9) because 4 < 9, though s(9) was interned first.
    assert_eq!(interner.compare(s4, s9), Ordering::Less);
    assert!(s4.index() < s9.index());
}

/// The same multiset of contents, interned by `threads` workers in a
/// per-worker shuffled order, then frozen on the same pool.
fn concurrent_run(threads: usize, contents: &[Vec<u32>]) -> (Vec<Id>, Work) {
    let pool = pool(threads);
    let mut interner = ContentInterner::new(Words);
    let works = std::sync::Mutex::new(Vec::new());
    pool.scope(|scope| {
        for worker in 0..threads {
            let interner = &interner;
            let works = &works;
            scope.spawn(move |_| {
                let mut work = Work::default();
                // Every worker interns every content, in its own rotation.
                let rotate = worker * contents.len() / threads;
                for index in 0..contents.len() {
                    let content = &contents[(index + rotate) % contents.len()];
                    interner.intern(content, &mut work).unwrap();
                }
                works.lock().unwrap().push(work);
            });
        }
    });
    let mut freeze = Work::default();
    interner.freeze(Some(&pool), &mut freeze).unwrap();
    let mut work = Work::default();
    let ids = contents
        .iter()
        .map(|content| interner.lookup(content, &mut work).unwrap())
        .collect();
    (ids, freeze)
}

#[test]
fn identities_are_the_same_at_one_two_four_and_fourteen_threads() {
    let contents: Vec<Vec<u32>> = (0..5_000_u32)
        .map(|value| vec![value.wrapping_mul(2_654_435_761) % 1_000, value % 7, value])
        .collect();
    let (expected, expected_work) = concurrent_run(1, &contents);
    for threads in [2, 4, 14] {
        let (ids, work) = concurrent_run(threads, &contents);
        assert_eq!(ids, expected, "{threads} threads");
        // The freeze's comparisons and writes do not depend on the workers.
        assert_eq!(work, expected_work, "{threads} threads");
    }
}

#[test]
fn intern_work_totals_do_not_depend_on_the_interning_order() {
    let contents: Vec<Vec<u32>> = (0..300).map(|value| vec![value % 40, value]).collect();
    let tally = |order: &[usize]| {
        let interner = ContentInterner::new(Words);
        let mut work = Work::default();
        for &index in order {
            interner.intern(&contents[index], &mut work).unwrap();
        }
        work
    };
    let forward: Vec<usize> = (0..contents.len()).collect();
    let backward: Vec<usize> = forward.iter().rev().copied().collect();
    let (a, b) = (tally(&forward), tally(&backward));
    assert_eq!(
        (a.hashed, a.probes, a.written),
        (b.hashed, b.probes, b.written)
    );
}

#[test]
fn the_block_sort_counts_the_same_comparisons_on_any_pool() {
    let items: Vec<u32> = (0..20_000_u32)
        .map(|value| value.wrapping_mul(2_654_435_761))
        .collect();
    let mut serial = items.clone();
    let count = sort_by_content(&mut serial, None, u32::cmp);
    let mut expected = items.clone();
    expected.sort_unstable();
    assert_eq!(serial, expected);
    for threads in [2, 4, 14] {
        let mut parallel = items.clone();
        assert_eq!(
            sort_by_content(&mut parallel, Some(&pool(threads)), u32::cmp),
            count
        );
        assert_eq!(parallel, expected);
    }
}

proptest! {
    /// The Lean law `IdOrderEqCompare`: within one generation, identity order
    /// is content order.
    #[test]
    fn identity_order_is_content_order_within_a_generation(
        contents in proptest::collection::vec(proptest::collection::vec(0_u32..6, 0..4), 1..60)
    ) {
        let mut interner = ContentInterner::new(Words);
        let ids = frozen_ids(&mut interner, &contents);
        for (a, left) in ids.iter().zip(&contents) {
            for (b, right) in ids.iter().zip(&contents) {
                prop_assert_eq!(a.index().cmp(&b.index()), left.cmp(right));
                prop_assert_eq!(interner.compare(*a, *b), left.cmp(right));
            }
        }
    }
}

/// The fixed-block sort and Rayon's parallel sort on the same counted pool
/// order identically; only the fixed-block sort's count is split-invariant.
#[test]
fn block_sort_agrees_with_parallel_sort() {
    use rayon::slice::ParallelSliceMut;
    let items: Vec<u64> = (0..1_u64 << 12)
        .map(|value| value.wrapping_mul(0x9e37_79b9_7f4a_7c15))
        .collect();
    for threads in [1, 4, 14] {
        let pool = pool(threads);
        let mut block = items.clone();
        sort_by_content(&mut block, Some(&pool), u64::cmp);
        let mut parallel = items.clone();
        pool.install(|| parallel.par_sort_by(u64::cmp));
        assert_eq!(block, parallel);
    }
}

#[test]
fn generation_remaps_keep_their_capacity_owner_after_the_interner_drops() {
    let storage = super::Storage::new(usize::MAX).unwrap();
    let header = storage.current_bytes();
    let mut interner = ContentInterner::in_storage(Words, storage.clone()).unwrap();
    let mut work = Work::default();
    for value in 0..4096 {
        interner.intern(&[value; 8], &mut work).unwrap();
    }
    let generation = interner.freeze(None, &mut work).unwrap();
    assert_eq!(
        storage.current_bytes(),
        header + interner.bytes() + generation.storage_bytes()
    );
    let remap = generation.storage_bytes();
    assert!(remap >= 4 * 4096);
    drop(interner);
    assert_eq!(storage.current_bytes(), header + remap);
    drop(generation);
    assert_eq!(storage.current_bytes(), header);
}

fn bounded_generation(limit: usize, threads: usize) -> (Result<(), super::Refusal>, u128, Work) {
    let storage = super::Storage::new(limit).unwrap();
    let mut work = Work::default();
    let result = (|| {
        let mut interner = ContentInterner::in_storage(Words, storage.clone())?;
        for value in 0..4096 {
            interner.intern(&[value; 8], &mut work)?;
        }
        let pool = pool(threads);
        interner.freeze(Some(&pool), &mut work)?;
        Ok(())
    })();
    (result, storage.peak_bytes(), work)
}

#[test]
fn a_generation_accepts_its_exact_capacity_peak_on_every_pool() {
    let (result, peak, expected_work) = bounded_generation(usize::MAX, 1);
    assert!(result.is_ok());
    let peak = usize::try_from(peak).unwrap();
    for threads in [1, 2, 3, 4, 14] {
        let (result, actual_peak, work) = bounded_generation(peak, threads);
        assert!(result.is_ok(), "threads={threads}: {result:?}");
        assert_eq!(actual_peak, peak as u128);
        assert_eq!(work, expected_work);
        let (result, _, _) = bounded_generation(peak - 1, threads);
        assert!(matches!(result, Err(super::Refusal::Storage { .. })));
    }
}

#[test]
fn a_refused_freeze_retains_pending_content_and_completed_work() {
    let (_, peak, _) = bounded_generation(usize::MAX, 1);
    let storage = super::Storage::new(usize::try_from(peak).unwrap() - 1).unwrap();
    let header = storage.current_bytes();
    let mut interner = ContentInterner::in_storage(Words, storage.clone()).unwrap();
    let mut work = Work::default();
    let mut handles = Vec::new();
    for value in 0..4096 {
        handles.push(interner.intern(&[value; 8], &mut work).unwrap());
    }
    let before = work;
    assert!(matches!(
        interner.freeze(None, &mut work),
        Err(super::Refusal::Storage { .. })
    ));
    assert_eq!(interner.pending_len(), 4096);
    assert_eq!(interner.len(), 0);
    assert!(work.total() > before.total());
    assert_eq!(storage.current_bytes(), header + interner.bytes());
    for (value, handle) in handles.into_iter().enumerate() {
        let Handle::Pending(pending) = handle else {
            panic!("pending before freeze");
        };
        assert!(interner.pending_content(pending, |content| {
            content == [u32::try_from(value).unwrap(); 8]
        }));
    }
}

#[test]
fn a_storage_refused_generation_has_an_exact_prefix_on_every_pool() {
    let (_, peak, complete) = bounded_generation(usize::MAX, 1);
    let maximum = usize::try_from(peak).unwrap();
    let reference = bounded_generation(maximum - 1, 1);
    assert!(matches!(reference.0, Err(super::Refusal::Storage { .. })));
    assert!(reference.2.total() > 0);
    assert!(reference.2.total() <= complete.total());
    for threads in [1, 2, 3, 4, 14] {
        let refused = bounded_generation(maximum - 1, threads);
        assert_eq!(refused, reference, "threads={threads}");
        // Retry with a fresh bounded owner; no failed generation can poison
        // the next attempt or consume another owner's allowance.
        let (result, observed, work) = bounded_generation(maximum, threads);
        assert_eq!(result, Ok(()), "threads={threads}");
        assert_eq!(observed, peak, "threads={threads}");
        assert_eq!(work, complete, "threads={threads}");
    }
}
