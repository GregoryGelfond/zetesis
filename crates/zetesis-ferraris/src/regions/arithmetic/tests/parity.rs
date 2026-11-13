use zetesis_algebra::Meter;

use std::mem::size_of;

use zetesis_algebra::{Cancellation, Stop};

use super::super::{
    budget::{Budget, BudgetError},
    parity::{Prepared, build_index, prepare, prepare_after_units},
    rows::{Equation, Unit},
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

/// The units' vector in `storage`'s ledger, as recognition leaves it.
fn direct(
    units: &[(usize, bool)],
    storage: &mut Storage,
    budget: &mut Budget<'_, '_>,
) -> Vec<Unit> {
    let mut direct = storage.reserve(0, budget).unwrap();
    for &(atom, value) in units {
        storage
            .push(&mut direct, Unit { atom, value }, 2, budget)
            .unwrap();
    }
    direct
}

/// The rows after preparation following `units`, the preparation, the work
/// the preparation alone spent and the ledger's live bytes.
fn completed_after(
    atom_count: usize,
    units: &[(usize, bool)],
    specifications: &[(usize, bool)],
) -> (Vec<Equation>, Prepared, u64, u128) {
    completed_after_rows(atom_count, units, equations(atom_count, specifications))
}

/// As [`completed_after`], for rows given whole.
fn completed_after_rows(
    atom_count: usize,
    units: &[(usize, bool)],
    mut rows: Vec<Equation>,
) -> (Vec<Equation>, Prepared, u64, u128) {
    let cancellation = Cancellation::default();
    let mut budget_work = Meter::local(100_000_000);
    let mut budget = Budget::new(&mut budget_work, 100_000_000, usize::MAX, &cancellation);
    let mut storage = Storage::default();
    let direct = direct(units, &mut storage, &mut budget);
    let before = budget.work();
    let result =
        prepare_after_units(direct, &mut rows, atom_count, &mut storage, &mut budget).unwrap();
    (rows, result, budget.work() - before, storage.bytes())
}

/// The row over `atoms` with `constant`, in the width of `atom_count` atoms.
fn equation_over(atom_count: usize, atoms: &[usize], constant: bool) -> Equation {
    let mut words = vec![0_u64; atom_count.div_ceil(128) * 2];
    for &atom in atoms {
        words[atom / 64] |= 1 << (atom % 64);
    }
    Equation {
        words: words.into(),
        constant,
    }
}

/// The atoms a row names, ascending.
fn atoms_of(row: &Equation) -> Vec<usize> {
    (0..row.words.len() * 64)
        .filter(|&atom| row.words[atom / 64] & (1 << (atom % 64)) != 0)
        .collect()
}

/// Whether `pattern` gives every unit's atom the unit's value.
fn agrees(pattern: usize, units: &[Unit]) -> bool {
    units
        .iter()
        .all(|unit| (pattern & (1 << unit.atom) != 0) == unit.value)
}

/// Every sequence of at most two units over `atom_count` atoms.
fn unit_sequences(atom_count: usize) -> Vec<Vec<(usize, bool)>> {
    let single: Vec<(usize, bool)> = (0..atom_count)
        .flat_map(|atom| [(atom, false), (atom, true)])
        .collect();
    let mut sequences = vec![Vec::new()];
    sequences.extend(single.iter().map(|&unit| vec![unit]));
    for &first in &single {
        sequences.extend(single.iter().map(|&second| vec![first, second]));
    }
    sequences
}

#[test]
fn every_small_system_with_units_preserves_its_solutions() {
    // At most two units and two rows over at most three atoms: the units and
    // rows published have exactly the solutions the units and rows given had.
    for atom_count in 1..=3 {
        let possibilities = 1usize << (atom_count + 1);
        for units in unit_sequences(atom_count) {
            let given: Vec<Unit> = units
                .iter()
                .map(|&(atom, value)| Unit { atom, value })
                .collect();
            for first in 0..possibilities {
                for second in 0..possibilities {
                    let specifications =
                        [(first / 2, first % 2 != 0), (second / 2, second % 2 != 0)];
                    let original = equations(atom_count, &specifications);
                    let expected: Vec<usize> = models(atom_count, &original, &[0, 1])
                        .into_iter()
                        .filter(|&pattern| agrees(pattern, &given))
                        .collect();
                    let (rows, prepared, _, _) =
                        completed_after(atom_count, &units, &specifications);
                    let case = format!("units {units:?}, rows {specifications:?}");
                    if prepared.root_refuted {
                        assert!(expected.is_empty(), "{case}");
                        assert!(prepared.units.is_empty(), "{case}");
                        continue;
                    }
                    assert!(!expected.is_empty(), "{case}");
                    let published: Vec<usize> = models(atom_count, &rows, &prepared.basis)
                        .into_iter()
                        .filter(|&pattern| agrees(pattern, &prepared.units))
                        .collect();
                    assert_eq!(published, expected, "{case}");
                    for (position, unit) in prepared.units.iter().enumerate() {
                        assert!(
                            prepared.units[..position]
                                .iter()
                                .all(|earlier| earlier.atom != unit.atom),
                            "{case}"
                        );
                    }
                    for unit in prepared.direct() {
                        assert!(
                            rows.iter().all(|row| row.words[0] & (1 << unit.atom) == 0),
                            "{case}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn every_small_system_spread_over_mask_words_preserves_its_solutions() {
    // The three atoms of the small systems moved in three mask words of an
    // owner of 192 atoms, so every unit and every row operation reads a word
    // other than the first. Solutions are compared over those three atoms;
    // no other atom may enter a row or a unit.
    let places = [3_usize, 70, 131];
    let atom_count = 192;
    let local = |atom: usize| places.iter().position(|&place| place == atom).unwrap();
    let solutions = |rows: &[Equation], selected: &[usize], units: &[Unit]| -> Vec<usize> {
        (0..8_usize)
            .filter(|&pattern| {
                selected.iter().all(|&row| {
                    let ones = atoms_of(&rows[row])
                        .into_iter()
                        .filter(|&atom| pattern & (1 << local(atom)) != 0)
                        .count();
                    ones % 2 == usize::from(rows[row].constant)
                }) && units
                    .iter()
                    .all(|unit| (pattern & (1 << local(unit.atom)) != 0) == unit.value)
            })
            .collect()
    };
    for units in unit_sequences(3) {
        let moved: Vec<(usize, bool)> = units
            .iter()
            .map(|&(atom, value)| (places[atom], value))
            .collect();
        let given: Vec<Unit> = moved
            .iter()
            .map(|&(atom, value)| Unit { atom, value })
            .collect();
        for first in 0..16_usize {
            for second in 0..16_usize {
                let rows: Vec<Equation> = [first, second]
                    .iter()
                    .map(|&row| {
                        let atoms: Vec<usize> = (0..3)
                            .filter(|bit| (row / 2) & (1 << bit) != 0)
                            .map(|bit| places[bit])
                            .collect();
                        equation_over(atom_count, &atoms, row % 2 != 0)
                    })
                    .collect();
                let expected = solutions(&rows, &[0, 1], &given);
                let (rows, prepared, _, bytes) = completed_after_rows(atom_count, &moved, rows);
                let case = format!("units {moved:?}, rows {first} and {second}");
                assert_eq!(bytes, prepared.retained_bytes(), "{case}");
                if prepared.root_refuted {
                    assert!(expected.is_empty(), "{case}");
                    assert!(prepared.units.is_empty(), "{case}");
                    continue;
                }
                assert_eq!(
                    solutions(&rows, &prepared.basis, &prepared.units),
                    expected,
                    "{case}"
                );
                for unit in prepared.direct() {
                    assert!(
                        rows.iter().all(|row| !atoms_of(row).contains(&unit.atom)),
                        "{case}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_unit_is_added_in_every_word_of_a_row() {
    // Two hundred atoms are four mask words. With atom 130 held and atom 70
    // cut, the row `a70 + a130 + a199 = 1` reads `a199 = 0`.
    let row = equation_over(200, &[70, 130, 199], true);
    let (rows, prepared, _, _) = completed_after_rows(200, &[(130, true), (70, false)], vec![row]);
    assert!(!prepared.root_refuted);
    assert_eq!(atoms_of(&rows[0]), [199]);
    assert!(!rows[0].constant);
    assert_eq!(
        prepared.units,
        [
            Unit {
                atom: 130,
                value: true
            },
            Unit {
                atom: 70,
                value: false
            },
            Unit {
                atom: 199,
                value: false
            }
        ]
    );
}

#[test]
fn units_that_disagree_are_refuted_in_the_chunk_that_reads_the_second() {
    // Two atoms, so two words a row and a mask. The result's vectors and
    // record cost 12 and the pivot directory 2 a cell (16); then one test
    // for an empty unit list, two masks of 3 + 2 * 2 each, and 13 for each
    // of the three units read. Nothing is charged after the third unit's
    // chunk, so an allowance of exactly this much publishes the refutation
    // and one unit less declines.
    let units = [(0, true), (1, false), (0, false)];
    let (_, prepared, spent, _) = completed_after(2, &units, &[(3, true)]);
    assert!(prepared.root_refuted);
    assert_eq!(spent, 16 + 1 + 2 * 7 + 3 * 13);
    for (allowance, refuted) in [(spent, true), (spent - 1, false)] {
        let cancellation = Cancellation::default();
        let mut setup_work = Meter::local(1_000);
        let mut setup = Budget::new(&mut setup_work, 1_000, usize::MAX, &cancellation);
        let mut storage = Storage::default();
        let direct = direct(&units, &mut storage, &mut setup);
        let mut budget_work = Meter::local(1_000);
        let mut budget = Budget::new(&mut budget_work, allowance, usize::MAX, &cancellation);
        let mut rows = equations(2, &[(3, true)]);
        let result = prepare_after_units(direct, &mut rows, 2, &mut storage, &mut budget);
        if refuted {
            assert!(result.unwrap().root_refuted);
        } else {
            assert_eq!(result.unwrap_err(), BudgetError::WorkDeclined);
        }
    }
}

#[test]
fn a_row_its_units_contradict_is_refuted_in_the_chunk_that_reads_it() {
    // One atom. The result costs 12 and the directory 2 (14); the unit list
    // costs 1 + 2 * 7 + 13; adding the unit to the one row costs 2 + 2; the
    // row then has no atom, which costs a scan of 2 + 1, and its constant's
    // test publishes the refutation for 3. Nothing follows.
    let (_, prepared, spent, _) = completed_after(1, &[(0, true)], &[(1, false)]);
    assert!(prepared.root_refuted);
    assert_eq!(spent, 14 + 28 + 4 + 3 + 3);
}

#[test]
fn a_repeated_unit_is_kept_once() {
    let (_, prepared, _, _) = completed_after(2, &[(0, true), (0, true), (1, false)], &[]);
    assert!(!prepared.root_refuted);
    let expected = [
        Unit {
            atom: 0,
            value: true,
        },
        Unit {
            atom: 1,
            value: false,
        },
    ];
    assert_eq!(prepared.units, expected);
    assert_eq!(prepared.direct(), expected);
    assert_eq!(prepared.rank(), 2);
    assert!(prepared.basis.is_empty());
}

#[test]
fn contradicting_units_refute_the_root_and_publish_none() {
    let (rows, prepared, _, bytes) =
        completed_after(2, &[(0, true), (1, false), (0, false)], &[(3, true)]);
    assert!(prepared.root_refuted);
    assert!(prepared.units.is_empty());
    assert!(prepared.basis.is_empty());
    assert_eq!(prepared.rank(), 0, "a refuted root counts no unit");
    assert!(prepared.index.is_none());
    assert_eq!(rows[0].words[0], 3, "no unit was added to a row");
    assert_eq!(bytes, prepared.retained_bytes());
}

#[test]
fn a_unit_is_added_to_each_row_over_its_atom() {
    // With atom 0 held, `a0 + a1 = 1` reads `a1 = 0`: an echelon unit.
    let (rows, prepared, _, _) = completed_after(2, &[(0, true)], &[(3, true)]);
    assert!(!prepared.root_refuted);
    assert_eq!((rows[0].words[0], rows[0].constant), (2, false));
    assert_eq!(
        prepared.units,
        [
            Unit {
                atom: 0,
                value: true
            },
            Unit {
                atom: 1,
                value: false
            }
        ]
    );
    assert_eq!(prepared.direct().len(), 1);
    assert_eq!(prepared.basis, [0]);
    assert_eq!(prepared.rank(), 2);
    assert_eq!(
        prepared.counts.additions, 0,
        "adding units is no row addition"
    );
}

#[test]
fn a_row_its_units_contradict_refutes_the_root() {
    // The row `a0 = 0` after the unit `a0 = 1` reads `0 = 1`.
    let (_, prepared, _, bytes) = completed_after(1, &[(0, true)], &[(1, false)]);
    assert!(prepared.root_refuted);
    assert!(prepared.units.is_empty());
    assert_eq!(prepared.rank(), 0, "the refuting row is no pivot");
    assert!(prepared.index.is_none());
    assert_eq!(bytes, prepared.retained_bytes());
}

#[test]
fn no_unit_leaves_the_rows_as_plain_preparation_does() {
    let specifications = [(3, true), (1, true), (2, false)];
    let (plain_rows, plain, plain_work, plain_bytes) = completed(2, &specifications);
    let (rows, prepared, work, bytes) = completed_after(2, &[], &specifications);
    assert_eq!(prepared.basis, plain.basis);
    assert_eq!(prepared.units, plain.units);
    assert_eq!(prepared.root_refuted, plain.root_refuted);
    for (row, plain) in rows.iter().zip(&plain_rows) {
        assert_eq!(
            (&row.words[..], row.constant),
            (&plain.words[..], plain.constant)
        );
    }
    // One test more; the units' header was initialized by its reservation.
    assert_eq!(work, plain_work - 3 + 1);
    assert_eq!(bytes, plain_bytes);
}

#[test]
fn units_and_rows_fit_the_documented_work_bound() {
    for atom_count in [1usize, 2, 8, 64, 129, 1024] {
        let n = u64::try_from(atom_count).unwrap();
        let width = n.div_ceil(128) * 2;
        let every: Vec<(usize, bool)> = (0..atom_count).map(|atom| (atom, atom % 2 == 0)).collect();
        let low = atom_count.min(8);
        let rows = [((1usize << low) - 1, false), (1, true)];
        for (units, specifications) in [
            (every.clone(), &rows[..0]),
            (every.clone(), &rows[..]),
            (every[..1].to_vec(), &rows[..]),
            ([every.clone(), every.clone()].concat(), &rows[..1]),
        ] {
            let (_, prepared, spent, _) = completed_after(atom_count, &units, specifications);
            let given = u64::try_from(units.len()).unwrap();
            let row_count = u64::try_from(specifications.len()).unwrap();
            let counts = &prepared.counts;
            let mut bound = 1 + 2 * (3 + 2 * width) + 13 * given + row_count * (width + 2);
            // Plain preparation's bound, less the units' header reserved before it.
            bound += 15 - 3
                + 2 * n
                + (row_count + counts.additions) * (width + 4)
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
                "atoms={atom_count}, units={given}, rows={row_count}, spent={spent}, bound={bound}"
            );
        }
    }
}

#[test]
fn units_alone_cost_work_in_proportion_to_their_number() {
    // Every atom decided by a unit, at two sizes sixteen times apart. Were
    // each unit a row of the theory's width, the work would grow 256 times.
    let work = |atom_count: usize| {
        let units: Vec<(usize, bool)> = (0..atom_count).map(|atom| (atom, true)).collect();
        completed_after(atom_count, &units, &[]).2
    };
    let (small, large) = (work(1 << 10), work(1 << 14));
    assert!(large <= 17 * small, "small={small}, large={large}");
}

#[test]
fn a_refusal_while_adding_units_leaves_every_row_whole() {
    let units = [(0, true), (2, false)];
    let given: Vec<Unit> = units
        .iter()
        .map(|&(atom, value)| Unit { atom, value })
        .collect();
    let specifications = [(7, true), (5, true), (6, false)];
    let (_, _, needed, _) = completed_after(3, &units, &specifications);
    let expected: Vec<usize> = models(3, &equations(3, &specifications), &[0, 1, 2])
        .into_iter()
        .filter(|&pattern| agrees(pattern, &given))
        .collect();
    assert!(!expected.is_empty());
    for limit in 0..needed {
        let cancellation = Cancellation::default();
        let mut setup_work = Meter::local(1_000);
        let mut setup = Budget::new(&mut setup_work, 1_000, usize::MAX, &cancellation);
        let mut storage = Storage::default();
        let direct = direct(&units, &mut storage, &mut setup);
        let mut budget_work = Meter::local(1_000_000);
        let mut budget = Budget::new(&mut budget_work, limit, usize::MAX, &cancellation);
        let mut rows = equations(3, &specifications);
        let error =
            prepare_after_units(direct, &mut rows, 3, &mut storage, &mut budget).unwrap_err();
        assert_eq!(error, BudgetError::WorkDeclined, "limit={limit}");
        assert!(budget.work() <= limit);
        // Each row had every unit over its atoms added or none, so with the
        // units given the rows still have the solutions they had.
        let remaining: Vec<usize> = models(3, &rows, &[0, 1, 2])
            .into_iter()
            .filter(|&pattern| agrees(pattern, &given))
            .collect();
        assert_eq!(remaining, expected, "limit={limit}");
    }
}

#[test]
fn retained_bytes_count_the_units_vector_once() {
    for (units, specifications) in [
        (vec![(0, true)], vec![(3, true)]),
        (vec![(0, true), (1, true), (0, true)], vec![]),
        (vec![(1, false)], vec![(1, false), (2, true)]),
    ] {
        let (_, prepared, _, bytes) = completed_after(2, &units, &specifications);
        assert_eq!(bytes, prepared.retained_bytes(), "units {units:?}");
    }
}

#[test]
#[should_panic(expected = "a unit names an admitted atom")]
fn a_unit_outside_the_atoms_is_a_caller_error() {
    let _ = completed_after(2, &[(2, true)], &[]);
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
