use zetesis_algebra::{Cancellation, Stop};

use crate::regions::Work;

use super::super::cardinality_source::Source;
use super::*;

fn bound(atoms: usize, support: &[usize], side: BoundSide, value: u64) -> Bound {
    let mut words = vec![0_u64; atoms.div_ceil(128) * 2];
    for &atom in support {
        words[atom / 64] |= 1 << (atom % 64);
    }
    Bound {
        support: words,
        guard: Vec::new(),
        side,
        value,
        cardinality: u64::try_from(support.len()).unwrap(),
        source: Source {
            kind: Kind::Asserted,
            root: 0,
            node: 0,
        },
    }
}

fn completed(atoms: usize, bounds: &[Bound]) -> (Prepared, u64, u128, u128) {
    let cancellation = Cancellation::default();
    let mut budget = Budget::new(Work::new(1_000_000), 1_000_000, usize::MAX, &cancellation);
    let mut storage = Storage::default();
    let prepared = prepare(bounds, atoms, &mut storage, &mut budget).unwrap();
    assert_eq!(storage.bytes(), prepared.retained_bytes());
    (
        prepared,
        budget.work(),
        storage.bytes(),
        budget.peak_bytes(),
    )
}

fn models(atoms: usize, bounds: &[Bound]) -> Vec<usize> {
    (0..1usize << atoms)
        .filter(|&model| {
            bounds.iter().all(|bound| {
                let active = bound
                    .guard
                    .iter()
                    .all(|literal| (model & (1 << literal.atom) != 0) == literal.positive);
                let count = (0..atoms)
                    .filter(|&atom| {
                        bound.support[atom / 64] & (1 << (atom % 64)) != 0
                            && model & (1 << atom) != 0
                    })
                    .count();
                !active
                    || match bound.side {
                        BoundSide::Lower => u64::try_from(count).unwrap() >= bound.value,
                        BoundSide::Upper => u64::try_from(count).unwrap() <= bound.value,
                    }
            })
        })
        .collect()
}

fn checked(bounds: &[Bound], lower: &[usize], upper: &[usize]) -> bool {
    let cancellation = Cancellation::default();
    let mut budget = Budget::new(Work::new(1_000_000), 1_000_000, usize::MAX, &cancellation);
    let width = bounds.first().map_or(0, |bound| bound.support.len());
    check_pair(
        bounds,
        lower,
        upper,
        (&mut vec![0; width], &mut vec![0; width]),
        &mut budget,
    )
    .unwrap()
}

fn supplied_refutes(bounds: &[Bound], lower: &[usize], upper: &[usize]) -> bool {
    assert!(checked(bounds, lower, upper));
    let mut occurrences = Vec::new();
    for (&row, side) in lower
        .iter()
        .map(|row| (row, BoundSide::Lower))
        .chain(upper.iter().map(|row| (row, BoundSide::Upper)))
    {
        occurrences.push(Occurrence {
            row,
            pair: 0,
            side,
            value: 0,
        });
    }
    let mut families = Families {
        pairs: vec![Pair {
            lower: 0..lower.len(),
            upper: lower.len()..occurrences.len(),
            lower_sum: 0,
            upper_sum: 0,
        }],
        occurrences,
        index: None,
        counts: Counts::default(),
    };
    let cancellation = Cancellation::default();
    let mut budget = Budget::new(Work::new(1_000_000), 1_000_000, usize::MAX, &cancellation);
    reduce_pair(&mut families, bounds, 0, &mut budget).unwrap()
}

#[test]
fn guarded_bounds_preserve_the_five_answer_witness() {
    let mut bounds = vec![
        bound(3, &[0, 1], BoundSide::Lower, 1),
        bound(3, &[0, 1], BoundSide::Upper, 1),
    ];
    bounds[1].guard.push(Literal {
        atom: 2,
        positive: true,
    });
    assert_eq!(models(3, &bounds), [1, 2, 3, 5, 6]);
    let (Prepared::Families(families), _, _, _) = completed(3, &bounds) else {
        panic!("guard is not root truth");
    };
    assert_eq!(
        families.counts,
        Counts {
            carriers: 1,
            attempts: 4,
            accepted: 4,
            rejected: 0
        }
    );
    assert_eq!(families.pairs.len(), 4);
    let index = families.index.unwrap();
    for (pair_id, pair) in families.pairs.iter().enumerate() {
        assert_eq!((pair.lower_sum, pair.upper_sum), (1, 2));
        for (range, side, value) in [
            (pair.lower.clone(), BoundSide::Lower, 1),
            (pair.upper.clone(), BoundSide::Upper, 2),
        ] {
            for position in range {
                let occurrence = &families.occurrences[position];
                assert_eq!(occurrence.pair, pair_id);
                assert_eq!(occurrence.side, side);
                assert_eq!(occurrence.value, value);
                let guarded = !bounds[occurrence.row].guard.is_empty();
                let present =
                    index.words[2 * index.row_words + position / 64] & (1 << (position % 64)) != 0;
                assert_eq!(present, guarded);
            }
        }
    }
    assert_eq!(
        bounds[1].guard,
        [Literal {
            atom: 2,
            positive: true
        }]
    );
}

fn pigeons(holes: usize) -> (usize, Vec<Bound>) {
    let atoms = holes * (holes + 1);
    let mut bounds = Vec::new();
    for pigeon in 0..=holes {
        let support: Vec<_> = (0..holes).map(|hole| pigeon * holes + hole).collect();
        bounds.push(bound(atoms, &support, BoundSide::Lower, 1));
        bounds.push(bound(atoms, &support, BoundSide::Upper, 1));
    }
    for hole in 0..holes {
        let support: Vec<_> = (0..=holes).map(|pigeon| pigeon * holes + hole).collect();
        bounds.push(bound(atoms, &support, BoundSide::Upper, 1));
    }
    (atoms, bounds)
}

#[test]
fn mixed_exact_one_partitions_refute_and_absorb_before_the_index() {
    for holes in 1..=3 {
        let (atoms, bounds) = pigeons(holes);
        let (Prepared::RootRefuted { counts }, work, _, _) = completed(atoms, &bounds) else {
            panic!("pigeon partition must refute");
        };
        assert_eq!(counts.carriers, 1);
        assert_eq!(counts.attempts, 2);
        assert_eq!(counts.accepted, 2);
        assert_eq!(counts.rejected, 0);
        let cancellation = Cancellation::default();
        let mut budget = Budget::new(Work::new(work), work, usize::MAX, &cancellation);
        let result = prepare(&bounds, atoms, &mut Storage::default(), &mut budget).unwrap();
        assert!(matches!(result, Prepared::RootRefuted { .. }));
        assert_eq!(budget.work(), work);
    }
}

#[test]
fn the_checker_rejects_false_sides_overlap_and_missing_cover() {
    let upper = [
        bound(2, &[0], BoundSide::Upper, 0),
        bound(2, &[1], BoundSide::Upper, 0),
        bound(2, &[0, 1], BoundSide::Upper, 1),
    ];
    assert!(!checked(&upper, &[0, 1], &[2]));
    let lower = [
        bound(2, &[0], BoundSide::Lower, 1),
        bound(2, &[1], BoundSide::Lower, 1),
        bound(2, &[0, 1], BoundSide::Lower, 0),
    ];
    assert!(!checked(&lower, &[2], &[0, 1]));
    let overlap = [
        bound(3, &[0, 1], BoundSide::Lower, 1),
        bound(3, &[1, 2], BoundSide::Lower, 1),
        bound(3, &[0, 1, 2], BoundSide::Upper, 2),
    ];
    assert!(!checked(&overlap, &[0, 1], &[2]));
    let missing = [
        bound(2, &[0], BoundSide::Lower, 1),
        bound(2, &[1], BoundSide::Upper, 0),
    ];
    assert!(!checked(&missing, &[0], &[1]));
}

#[test]
fn the_bounded_policy_preserves_its_documented_partition_miss() {
    let mut bounds: Vec<_> = (0..4)
        .map(|atom| bound(4, &[atom], BoundSide::Lower, 1))
        .collect();
    bounds.extend([
        bound(4, &[0, 1], BoundSide::Upper, 1),
        bound(4, &[2, 3], BoundSide::Upper, 1),
        bound(4, &[0], BoundSide::Upper, 0),
        bound(4, &[0, 1, 2], BoundSide::Upper, 2),
    ]);
    assert!(supplied_refutes(&bounds, &[0, 1, 2, 3], &[4, 5]));
    let (Prepared::Families(families), _, _, _) = completed(4, &bounds) else {
        panic!("policy must not invent another proposal");
    };
    assert_eq!(
        families.counts,
        Counts {
            carriers: 1,
            attempts: 4,
            accepted: 0,
            rejected: 4
        }
    );
    assert!(families.pairs.is_empty());
    assert!(families.occurrences.is_empty());
}

#[test]
fn guard_atoms_do_not_connect_carriers_and_empty_supports_stay_isolated() {
    let mut bounds = vec![
        bound(4, &[0], BoundSide::Lower, 1),
        bound(4, &[0], BoundSide::Upper, 0),
        bound(4, &[1], BoundSide::Lower, 1),
        bound(4, &[1], BoundSide::Upper, 1),
        bound(4, &[], BoundSide::Lower, 0),
    ];
    bounds[0].guard.push(Literal {
        atom: 3,
        positive: true,
    });
    bounds[1].guard.push(Literal {
        atom: 3,
        positive: false,
    });
    bounds[2].guard.push(Literal {
        atom: 3,
        positive: true,
    });
    let (prepared, _, _, _) = completed(4, &bounds);
    assert!(matches!(prepared, Prepared::Families(_)));
    assert_eq!(prepared.counts().carriers, 3);
    assert_eq!(prepared.counts().attempts, 12);
}

#[test]
fn canonical_guard_keys_keep_original_repetitions_and_ascending_ties() {
    let mut bounds = vec![
        bound(3, &[0, 1], BoundSide::Lower, 1),
        bound(3, &[0, 1], BoundSide::Lower, 1),
        bound(3, &[0, 1], BoundSide::Upper, 2),
    ];
    bounds[0].guard = vec![
        Literal {
            atom: 2,
            positive: true
        };
        2
    ];
    bounds[1].guard = vec![Literal {
        atom: 2,
        positive: true,
    }];
    bounds[0].source.root = 1;
    let (Prepared::Families(families), _, _, _) = completed(3, &bounds) else {
        panic!("satisfiable bounds");
    };
    for pair in &families.pairs {
        assert_eq!(pair.lower.len(), 1);
        assert_eq!(families.occurrences[pair.lower.start].row, 1);
    }
    assert_eq!(bounds[0].guard.len(), 2);
    assert_eq!(bounds[1].guard.len(), 1);
}

#[test]
fn empty_inventory_has_no_atom_directory_or_incidence_allocation() {
    let (small, work, bytes, peak) = completed(0, &[]);
    let (large, large_work, large_bytes, large_peak) = completed(usize::MAX, &[]);
    assert_eq!((work, bytes, peak), (large_work, large_bytes, large_peak));
    for prepared in [small, large] {
        let Prepared::Families(families) = prepared else {
            panic!("empty conjunction");
        };
        assert_eq!(families.counts, Counts::default());
        assert!(families.index.is_none());
    }
}

#[test]
fn preparation_refusals_publish_no_partial_family() {
    let bounds = [
        bound(1, &[0], BoundSide::Lower, 0),
        bound(1, &[0], BoundSide::Upper, 1),
    ];
    let cancellation = Cancellation::default();
    let mut budget = Budget::new(Work::new(1_000_000), 1, usize::MAX, &cancellation);
    assert!(matches!(
        prepare(&bounds, 1, &mut Storage::default(), &mut budget),
        Err(BudgetError::WorkDeclined)
    ));
    let mut budget = Budget::new(Work::new(1_000_000), 1_000_000, 0, &cancellation);
    assert!(matches!(
        prepare(&bounds, 1, &mut Storage::default(), &mut budget),
        Err(BudgetError::StorageDeclined { .. })
    ));
    cancellation.cancel();
    let mut budget = Budget::new(Work::new(0), 0, 0, &cancellation);
    assert!(matches!(
        prepare(&bounds, 1, &mut Storage::default(), &mut budget),
        Err(BudgetError::Stopped(Stop::Cancelled))
    ));
}
