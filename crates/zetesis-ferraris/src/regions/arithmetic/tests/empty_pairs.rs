//! The completed empty source relation still checks every mandatory domain.

use zetesis_algebra::{Cancellation, Meter, Stop};

use super::super::{
    budget::{Budget, BudgetError},
    images::{kernel, pairs},
    storage::Storage,
};
use crate::regions::adjacency::Adjacency;
use crate::{
    Narrowing, NodeView, Region, Theory,
    test_support::{constructed_theory, push},
};

#[derive(Debug)]
struct Closed {
    result: Result<Narrowing, BudgetError>,
    work: u64,
    peak: u128,
    rounds: u64,
    cuts: u64,
    region: Region,
}

fn close(groups: &[Vec<u64>], region: &Region, indexed: bool, maximum: u64) -> Closed {
    let relation = indexed.then(|| Adjacency::build(region.len(), std::iter::empty()).unwrap());
    let mut region = region.clone();
    let mut meter = Meter::local(u64::MAX);
    let cancellation = Cancellation::default();
    let (result, work, peak, rounds, cuts) = {
        let mut budget = Budget::new(&mut meter, maximum, usize::MAX, &cancellation);
        let mut storage = Storage::default();
        let mut counts = kernel::Counts::default();
        let result = kernel::close(
            &groups.iter().map(Vec::as_slice),
            relation.as_ref(),
            &mut region,
            &mut storage,
            &mut budget,
            &mut counts,
        );
        assert_eq!(storage.bytes(), 0);
        let observations = (
            result,
            budget.work(),
            budget.peak_bytes(),
            counts.rounds,
            counts.cuts,
        );
        budget.finish();
        observations
    };
    assert_eq!(meter.charged(), work);
    Closed {
        result,
        work,
        peak,
        rounds,
        cuts,
        region,
    }
}

fn one_word_region(mut code: usize) -> Region {
    let mut region = Region::all_open(3);
    for atom in 0..3 {
        match code % 3 {
            1 => assert!(region.hold(atom)),
            2 => assert!(region.cut(atom)),
            _ => {}
        }
        code /= 3;
    }
    region
}

#[test]
fn empty_relation_agrees_with_the_indexed_kernel_on_every_small_region() {
    for code in 0..27 {
        let region = one_word_region(code);
        for members in 0_u64..8 {
            let groups = [vec![members]];
            let empty = close(&groups, &region, false, u64::MAX);
            let indexed = close(&groups, &region, true, u64::MAX);
            assert_eq!(empty.result, indexed.result);
            assert_eq!(empty.region, indexed.region);
            assert_eq!(empty.region, region);
            assert_eq!((empty.rounds, empty.cuts), (indexed.rounds, indexed.cuts));
            assert_eq!(empty.peak, 0);
            assert!(empty.work < indexed.work);
            let expected = if members & !region.decision_words().1[0] == 0 {
                Narrowing::Refuted
            } else {
                Narrowing::Fixed { changed: false }
            };
            assert_eq!(empty.result, Ok(expected));
        }
    }
    let empty = close(&[], &Region::all_open(0), false, u64::MAX);
    assert_eq!(empty.result, Ok(Narrowing::Fixed { changed: false }));
    assert_eq!(
        (empty.work, empty.rounds, empty.cuts, empty.peak),
        (5, 1, 0, 0)
    );
}

#[test]
fn empty_relation_checks_later_groups_and_word_boundaries() {
    for atoms in [63, 64, 65, 129] {
        let mut region = Region::all_open(atoms);
        let mut group = vec![0; region.mask_words()];
        group[(atoms - 1) / 64] = 1_u64 << ((atoms - 1) % 64);
        let groups = [group.clone(), group];
        let possible = close(&groups, &region, false, u64::MAX);
        assert_eq!(possible.result, Ok(Narrowing::Fixed { changed: false }));
        assert_eq!(possible.work, 5 + 2 * (1 + 3 * region.mask_words() as u64));
        assert!(region.cut(atoms - 1));
        let empty = close(&groups, &region, false, u64::MAX);
        assert_eq!(empty.result, Ok(Narrowing::Refuted));
        assert_eq!(empty.work, 4 + 3 * region.mask_words() as u64);
        assert_eq!((empty.rounds, empty.cuts), (0, 0));
    }
    let region = Region::all_open(3);
    let later_empty = close(&[vec![1], vec![0]], &region, false, u64::MAX);
    assert_eq!(later_empty.result, Ok(Narrowing::Refuted));
    assert_eq!(
        (later_empty.work, later_empty.rounds, later_empty.cuts),
        (11, 0, 0)
    );
}

#[test]
fn empty_relation_keeps_exact_work_prefixes_without_mutating_the_root() {
    let region = Region::all_open(3);
    let groups = [vec![1], vec![2]];
    let full = close(&groups, &region, false, u64::MAX);
    assert_eq!(full.work, 13);
    for maximum in 0..=full.work {
        let attempt = close(&groups, &region, false, maximum);
        assert_eq!(attempt.region, region);
        assert_eq!((attempt.peak, attempt.cuts), (0, 0));
        if maximum == full.work {
            assert_eq!(attempt.result, full.result);
            assert_eq!((attempt.work, attempt.rounds), (13, 1));
        } else {
            assert_eq!(attempt.result, Err(BudgetError::WorkDeclined));
            // The final return is one atomic two-operation group.
            assert_eq!(attempt.work, if maximum == 12 { 11 } else { maximum });
            assert_eq!(attempt.rounds, u64::from(maximum >= 11));
        }
    }
}

#[test]
fn empty_relation_preserves_cancellation_and_global_refusal() {
    for cancelled in [false, true] {
        let cancellation = Cancellation::default();
        if cancelled {
            cancellation.cancel();
        }
        let mut meter = Meter::local(0);
        let mut budget = Budget::new(&mut meter, 0, 0, &cancellation);
        let mut region = Region::all_open(3);
        let original = region.clone();
        let mut counts = kernel::Counts::default();
        let groups = [vec![1_u64]];
        assert_eq!(
            kernel::close(
                &groups.iter().map(Vec::as_slice),
                None,
                &mut region,
                &mut Storage::default(),
                &mut budget,
                &mut counts
            ),
            Err(BudgetError::Stopped(if cancelled {
                Stop::Cancelled
            } else {
                Stop::WorkLimit
            }))
        );
        assert_eq!((budget.work(), counts.rounds, counts.cuts), (0, 0, 0));
        assert_eq!(region, original);
    }
}

#[derive(Debug)]
struct Read {
    result: Result<(bool, usize), BudgetError>,
    work: u64,
    peak: u128,
    retained: u128,
}

fn read(owner: &Theory, maximum: u64, bytes: usize) -> Read {
    let cancellation = Cancellation::default();
    let mut meter = Meter::local(u64::MAX);
    let (result, work, peak, retained) = {
        let mut budget = Budget::new(&mut meter, maximum, bytes, &cancellation);
        let mut storage = Storage::default();
        let result = pairs::read(owner, &mut storage, &mut budget)
            .map(|(pairs, count)| (pairs.is_none(), count));
        let observations = (result, budget.work(), budget.peak_bytes(), storage.bytes());
        budget.finish();
        observations
    };
    assert_eq!(meter.charged(), work);
    Read {
        result,
        work,
        peak,
        retained,
    }
}

#[test]
fn completed_empty_scan_has_no_atom_sized_index_and_exact_owner_boundaries() {
    let bytes = size_of::<Option<Adjacency>>();
    for atoms in [0, 3, 65, 129] {
        let owner = constructed_theory(atoms, |_| vec![]);
        let full = read(&owner, u64::MAX, usize::MAX);
        assert_eq!(full.result, Ok((true, 0)));
        assert_eq!(
            (full.work, full.peak, full.retained),
            (5, bytes as u128, bytes as u128)
        );
        assert_eq!(read(&owner, 5, bytes).result, full.result);
        for maximum in 0..5 {
            let attempt = read(&owner, maximum, usize::MAX);
            assert_eq!(attempt.result, Err(BudgetError::WorkDeclined));
            assert_eq!(attempt.work, if maximum < 3 { 0 } else { maximum });
        }
        let short = read(&owner, u64::MAX, bytes - 1);
        assert_eq!(
            short.result,
            Err(BudgetError::StorageDeclined {
                required: bytes as u128,
                limit: bytes - 1
            })
        );
        assert_eq!(short.work, 4);
    }
}

#[test]
fn empty_and_nonempty_pair_owners_release_their_exact_storage() {
    for with_pair in [false, true] {
        let owner = constructed_theory(3, |nodes| {
            if !with_pair {
                return vec![];
            }
            let falsehood = push(nodes, NodeView::False);
            let body = push(nodes, NodeView::And(&[1, 0]));
            vec![push(nodes, NodeView::Implies(body, falsehood))]
        });
        let cancellation = Cancellation::default();
        let mut meter = Meter::local(u64::MAX);
        let mut budget = Budget::new(&mut meter, u64::MAX, usize::MAX, &cancellation);
        let mut storage = Storage::default();
        let (index, count) = pairs::read(&owner, &mut storage, &mut budget).unwrap();
        assert_eq!(count, usize::from(with_pair));
        assert_eq!(index.is_some(), with_pair);
        if let Some(index) = &index {
            assert_eq!(index[0], [1]);
            assert_eq!(index[1], [0]);
            assert!(index[2].is_empty());
        } else {
            assert_eq!(storage.bytes(), size_of::<Option<Adjacency>>() as u128);
        }
        let before = budget.work();
        pairs::release(index, &mut storage, &mut budget).unwrap();
        assert_eq!(storage.bytes(), 0);
        assert_eq!(budget.work() - before, 1);
    }
}
