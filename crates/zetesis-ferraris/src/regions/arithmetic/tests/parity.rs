use zetesis_algebra::Meter;

use std::mem::size_of;

use zetesis_algebra::{Cancellation, Stop};

use super::super::{
    budget::{Budget, BudgetError},
    parity::{Prepared, Unit, build_index, prepare},
    rows::Equation,
    storage::Storage,
};

fn equations(atom_count: usize, specifications: &[(usize, bool)]) -> Vec<Equation> {
    specifications
        .iter()
        .map(|&(coefficients, constant)| {
            let mut words = vec![0; atom_count.div_ceil(128) * 2];
            if let Some(first) = words.first_mut() {
                *first = u64::try_from(coefficients).unwrap();
            }
            Equation {
                words: words.into(),
                constant,
            }
        })
        .collect()
}

fn models(atom_count: usize, rows: &[Equation], selected: &[usize]) -> Vec<usize> {
    (0..1usize << atom_count)
        .filter(|&pattern| {
            selected.iter().all(|&row| {
                let true_terms = (0..atom_count)
                    .filter(|&atom| {
                        rows[row].words[atom / 64] & (1 << (atom % 64)) != 0
                            && pattern & (1 << atom) != 0
                    })
                    .count();
                true_terms % 2 == usize::from(rows[row].constant)
            })
        })
        .collect()
}

fn completed(
    atom_count: usize,
    specifications: &[(usize, bool)],
) -> (Vec<Equation>, Prepared, u64, u128) {
    let cancellation = Cancellation::default();
    let mut budget_work = Meter::local(1_000_000);
    let mut budget = Budget::new(&mut budget_work, 1_000_000, usize::MAX, &cancellation);
    let mut storage = Storage::default();
    let mut rows = equations(atom_count, specifications);
    let result = prepare(&mut rows, atom_count, &mut storage, &mut budget).unwrap();
    (rows, result, budget.work(), storage.bytes())
}

#[test]
fn every_small_two_row_system_preserves_its_solutions() {
    for atom_count in 0..=3 {
        let possibilities = 1usize << (atom_count + 1);
        for first in 0..possibilities {
            for second in 0..possibilities {
                let specifications = [(first / 2, first % 2 != 0), (second / 2, second % 2 != 0)];
                let original = equations(atom_count, &specifications);
                let expected = models(atom_count, &original, &[0, 1]);
                let (rows, prepared, _, _) = completed(atom_count, &specifications);
                assert_eq!(models(atom_count, &rows, &[0, 1]), expected);
                if prepared.root_refuted {
                    assert!(expected.is_empty());
                } else {
                    assert!(!expected.is_empty());
                    assert_eq!(models(atom_count, &rows, &prepared.basis), expected);
                    for unit in prepared.units {
                        assert!(
                            expected.iter().all(|&pattern| {
                                (pattern & (1 << unit.atom) != 0) == unit.value
                            })
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn forward_elimination_keeps_nonunit_echelon_rows() {
    let (rows, prepared, _, _) = completed(2, &[(3, false), (2, true)]);
    assert!(!prepared.root_refuted);
    assert_eq!(prepared.basis, [0, 1]);
    assert_eq!(rows[0].words[0], 3, "there is no backwards elimination");
    assert_eq!(
        prepared.units,
        [Unit {
            atom: 1,
            value: true
        }]
    );
    assert_eq!(prepared.counts.additions, 0);
    assert_eq!(prepared.counts.rank, 2);
}

#[test]
fn a_derived_contradiction_absorbs_before_the_index() {
    let specifications = [(1, false), (1, true), (2, false)];
    let (_, full, needed, _) = completed(2, &specifications);
    assert!(full.root_refuted);
    assert_eq!(full.counts.additions, 1);
    let cancellation = Cancellation::default();
    let mut budget_work = Meter::local(needed);
    let mut budget = Budget::new(&mut budget_work, needed, usize::MAX, &cancellation);
    let mut storage = Storage::default();
    let mut rows = equations(2, &specifications);
    let prepared = prepare(&mut rows, 2, &mut storage, &mut budget).unwrap();
    assert!(prepared.root_refuted);
    assert!(prepared.index.is_none());
    assert!(prepared.units.is_empty());
    assert_eq!(prepared.counts.incidences, 0);
    assert_eq!(rows[2].words[0], 2);
    assert_eq!(budget.work(), needed);
}

#[test]
fn empty_owner_rows_can_refute_without_coordinates() {
    let (_, prepared, _, _) = completed(0, &[(0, true)]);
    assert!(prepared.root_refuted);
    assert!(prepared.basis.is_empty());
    assert!(prepared.index.is_none());
}

#[test]
fn index_bits_follow_original_atom_and_row_coordinates() {
    for row_count in [0, 1, 63, 64, 65, 127, 128, 129] {
        let supports: Vec<_> = (0..row_count).map(|row| vec![1 << (row % 3), 0]).collect();
        let cancellation = Cancellation::default();
        let mut budget_work = Meter::local(1_000_000);
        let mut budget = Budget::new(&mut budget_work, 1_000_000, usize::MAX, &cancellation);
        let mut storage = Storage::default();
        let index = build_index(
            3,
            supports.iter().map(Vec::as_slice),
            &mut storage,
            &mut budget,
        )
        .unwrap();
        assert_eq!(index.rows, row_count);
        assert_eq!(index.row_words, row_count.div_ceil(128) * 2);
        for atom in 0..3 {
            for row in 0..index.row_words * 64 {
                let present =
                    index.words[atom * index.row_words + row / 64] & (1 << (row % 64)) != 0;
                assert_eq!(present, row < row_count && row % 3 == atom);
            }
        }
        assert_eq!(index.insertions, u64::try_from(row_count).unwrap());
    }
}

#[test]
fn guard_insertions_reuse_the_same_incidence_index() {
    let supports = [vec![0, 0]; 1];
    let cancellation = Cancellation::default();
    let mut budget_work = Meter::local(1_000);
    let mut budget = Budget::new(&mut budget_work, 1_000, usize::MAX, &cancellation);
    let mut storage = Storage::default();
    let mut index = build_index(
        65,
        supports.iter().map(Vec::as_slice),
        &mut storage,
        &mut budget,
    )
    .unwrap();
    let before = budget.work();
    index.insert(64, 0, &mut budget).unwrap();
    index.insert(64, 0, &mut budget).unwrap();
    assert_eq!(budget.work() - before, 10);
    assert_eq!(index.insertions, 2);
    assert_eq!(index.words[64 * index.row_words], 1);
    assert!(
        index.words[..64 * index.row_words]
            .iter()
            .all(|&word| word == 0)
    );
}

#[test]
fn retained_bytes_count_embedded_headers_once() {
    for specifications in [vec![(1, false), (2, true)], vec![(0, true)]] {
        let (_, prepared, _, bytes) = completed(2, &specifications);
        let expected = size_of::<Prepared>() as u128
            + prepared.basis.capacity() as u128 * size_of::<usize>() as u128
            + prepared.units.capacity() as u128 * size_of::<Unit>() as u128
            + prepared.index.as_ref().map_or(0, |index| {
                index.words.capacity() as u128 * size_of::<u64>() as u128
            });
        assert_eq!(prepared.retained_bytes(), expected);
        assert_eq!(bytes, expected);
    }
}

#[test]
fn preparation_work_obeys_the_replacement_bound() {
    for atom_count in 0..=8 {
        let units: Vec<_> = (0..atom_count).map(|atom| (1 << atom, false)).collect();
        let mut redundant = units.clone();
        redundant.insert(0, ((1 << atom_count) - 1, false));
        for specifications in [vec![], vec![(0, true)], units, redundant] {
            let (_, prepared, spent, _) = completed(atom_count, &specifications);
            let n = u64::try_from(atom_count).unwrap();
            let width = n.div_ceil(128) * 2;
            let rows = u64::try_from(specifications.len()).unwrap();
            let counts = &prepared.counts;
            let mut bound = 15
                + 2 * n
                + (rows + counts.additions) * (width + 4)
                + counts.additions * (width + 3)
                + 6 * counts.rank;
            if !prepared.root_refuted {
                let index = 2 * n * (counts.rank.div_ceil(128) * 2)
                    + 6
                    + 3 * counts.rank
                    + counts.rank * width
                    + 7 * counts.incidences;
                bound += counts.rank * (2 * width + 3) + counts.units * (width + 23) + index + 3;
            }
            assert!(
                spent <= bound,
                "atoms={atom_count}, spent={spent}, bound={bound}"
            );
        }
    }
}

#[test]
fn optional_refusals_leave_only_complete_row_additions() {
    let specifications = [(3, true), (1, true), (2, false)];
    let (_, _, needed, _) = completed(2, &specifications);
    let expected = models(2, &equations(2, &specifications), &[0, 1, 2]);
    for limit in 0..needed {
        let cancellation = Cancellation::default();
        let mut budget_work = Meter::local(1_000_000);
        let mut budget = Budget::new(&mut budget_work, limit, usize::MAX, &cancellation);
        let mut storage = Storage::default();
        let mut rows = equations(2, &specifications);
        let error = prepare(&mut rows, 2, &mut storage, &mut budget).unwrap_err();
        assert_eq!(error, BudgetError::WorkDeclined, "limit={limit}");
        assert!(budget.work() <= limit);
        assert_eq!(models(2, &rows, &[0, 1, 2]), expected);
    }
}

#[test]
fn global_work_refusals_keep_their_type_and_prefix() {
    let specifications = [(3, true), (1, true), (2, false)];
    let (_, _, needed, _) = completed(2, &specifications);
    let expected = models(2, &equations(2, &specifications), &[0, 1, 2]);
    for limit in 0..needed {
        let cancellation = Cancellation::default();
        let mut budget_work = Meter::local(limit);
        let mut budget = Budget::new(&mut budget_work, 1_000_000, usize::MAX, &cancellation);
        let mut storage = Storage::default();
        let mut rows = equations(2, &specifications);
        assert_eq!(
            prepare(&mut rows, 2, &mut storage, &mut budget).unwrap_err(),
            BudgetError::Stopped(Stop::WorkLimit),
            "limit={limit}"
        );
        assert!(budget.work() <= limit);
        assert_eq!(models(2, &rows, &[0, 1, 2]), expected);
    }
}

struct Deadline {
    remaining: u64,
    granted: u64,
    refunded: u64,
}

impl zetesis_algebra::Quota for Deadline {
    type Error = Stop;
    fn reserve(&mut self, wanted: u64) -> Result<u64, Stop> {
        if self.remaining == 0 {
            return Err(Stop::Deadline);
        }
        let count = wanted.min(self.remaining);
        self.remaining -= count;
        self.granted += count;
        Ok(count)
    }

    fn refund(&mut self, unspent: u64) {
        self.refunded += unspent;
    }
}

#[test]
fn a_deadline_keeps_its_consumed_prefix_receipt() {
    let cancellation = Cancellation::default();
    let mut quota = Deadline {
        remaining: 40,
        granted: 0,
        refunded: 0,
    };
    let spent;
    {
        let mut budget_work = Meter::reserved(&mut quota);
        let mut budget = Budget::new(&mut budget_work, 1_000, usize::MAX, &cancellation);
        let mut storage = Storage::default();
        let mut rows = equations(2, &[(3, true), (1, true), (2, false)]);
        assert_eq!(
            prepare(&mut rows, 2, &mut storage, &mut budget).unwrap_err(),
            BudgetError::Refused(Stop::Deadline)
        );
        spent = budget.work();
        assert!(spent > 0 && spent <= 40);
        budget.finish();
    }
    assert_eq!(quota.granted - quota.refunded, spent);
}

#[test]
fn cancellation_precedes_optional_preparation_refusal() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut budget_work = Meter::local(0);
    let mut budget = Budget::new(&mut budget_work, 0, 0, &cancellation);
    let mut storage = Storage::default();
    let mut rows = equations(0, &[(0, true)]);
    assert_eq!(
        prepare(&mut rows, 0, &mut storage, &mut budget).unwrap_err(),
        BudgetError::Stopped(Stop::Cancelled)
    );
    assert_eq!(budget.work(), 0);
}

#[test]
fn byte_refusal_preserves_the_header_initialization() {
    let cancellation = Cancellation::default();
    let mut budget_work = Meter::local(1_000);
    let mut budget = Budget::new(&mut budget_work, 1_000, 0, &cancellation);
    let mut storage = Storage::default();
    let mut rows = equations(1, &[(1, true)]);
    assert!(matches!(
        prepare(&mut rows, 1, &mut storage, &mut budget),
        Err(BudgetError::StorageDeclined { .. })
    ));
    assert_eq!(budget.work(), 3);
    assert_eq!(rows[0].words[0], 1);
}

#[test]
fn index_dimension_overflow_is_an_allocation_failure() {
    let supports = vec![Vec::new(); 129];
    let cancellation = Cancellation::default();
    let mut budget_work = Meter::local(0);
    let mut budget = Budget::new(&mut budget_work, 0, usize::MAX, &cancellation);
    let mut storage = Storage::default();
    assert_eq!(
        build_index(
            usize::MAX,
            supports.iter().map(Vec::as_slice),
            &mut storage,
            &mut budget
        )
        .unwrap_err(),
        BudgetError::Stopped(Stop::Allocation)
    );
    assert_eq!(budget.work(), 0);
}
