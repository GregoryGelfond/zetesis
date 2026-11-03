use super::*;
use crate::regions::arithmetic::{
    cardinality::{Counts, Occurrence, Pair},
    parity,
};

fn repeated_partitions(pairs: usize) -> CardinalitySystem {
    let positive = Literal {
        atom: 2,
        positive: true,
    };
    let negative = Literal {
        atom: 2,
        positive: false,
    };
    let bounds = vec![
        bound(3, &[0], BoundSide::Lower, 1, &[positive]),
        bound(3, &[1], BoundSide::Lower, 1, &[positive]),
        bound(3, &[0, 1], BoundSide::Upper, 1, &[negative]),
    ];
    let mut families = Families {
        pairs: vec![],
        occurrences: vec![],
        index: None,
        counts: Counts::default(),
    };
    for pair in 0..pairs {
        let start = families.occurrences.len();
        for (row, side, value) in [
            (0, BoundSide::Lower, 0),
            (1, BoundSide::Lower, 0),
            (2, BoundSide::Upper, 2),
        ] {
            families.occurrences.push(Occurrence {
                row,
                pair,
                side,
                value,
            });
        }
        families.pairs.push(Pair {
            lower: start..start + 2,
            upper: start + 2..start + 3,
            lower_sum: 0,
            upper_sum: 2,
        });
    }
    let cancellation = Cancellation::default();
    let mut meter = Meter::local(u64::MAX);
    let mut budget = Budget::new(&mut meter, u64::MAX, usize::MAX, &cancellation);
    let mut storage = Storage::default();
    let mut index = parity::build_index(
        3,
        families
            .occurrences
            .iter()
            .map(|occurrence| &bounds[occurrence.row].support[..]),
        &mut storage,
        &mut budget,
    )
    .unwrap();
    for (position, occurrence) in families.occurrences.iter().enumerate() {
        for literal in &bounds[occurrence.row].guard {
            index.insert(literal.atom, position, &mut budget).unwrap();
        }
    }
    families.index = Some(index);
    CardinalitySystem {
        owner: theory(3, vec![], vec![]),
        bounds,
        prepared: Prepared::Families(families),
    }
}

#[test]
fn padded_incidence_columns_reach_every_occurrence() {
    // 3 and 129 logical occurrences use physical strides 2 and 4. Each
    // partition is the disjoint carrier {0}+{1} = {0,1}; guarded rows do not
    // refute at the root or after holding atom2, so the whole suffix is read.
    for pair_count in [1, 22, 43] {
        let system = repeated_partitions(pair_count);
        let index = families(&system).index.as_ref().unwrap();
        let rows = pair_count * 3;
        assert_eq!(index.rows, rows);
        assert_eq!(index.row_words, rows.div_ceil(128) * 2);
        let mut input = Region::all_open(3);
        let mut known = system.knowledge();
        assert_eq!(
            run(&system, &mut input, &mut known).result,
            Ok(SubjectStep::Unchanged)
        );
        assert_eq!(
            known.sums,
            vec![PairSums { lower: 0, upper: 2 }; pair_count]
        );
        input.hold(2);
        let mut scratch = NarrowingScratch::default();
        let attempt = system.inspect_reserved(
            &mut input,
            &mut known,
            &mut scratch,
            &Cancellation::default(),
            &mut Meter::local(u64::MAX),
        );
        assert_eq!(attempt.result, Ok(SubjectStep::Unchanged));
        assert_eq!(
            known.sums,
            vec![PairSums { lower: 2, upper: 2 }; pair_count]
        );
        assert_eq!(scratch.arithmetic.touched.len(), rows.div_ceil(64));
        for position in 0..rows {
            assert!(scratch.arithmetic.touched.contains(position));
        }
        assert_eq!(
            scratch
                .arithmetic
                .touched
                .iter()
                .map(|word| usize::try_from(word.count_ones()).unwrap())
                .sum::<usize>(),
            rows
        );
        assert_eq!(known.snapshot.old_masks(), input.decision_words());
    }
}

#[test]
fn semantic_refutation_omits_the_later_family_suffix() {
    let system = guarded();
    let mut root = Region::all_open(3);
    let mut parent = system.knowledge();
    assert_eq!(
        run(&system, &mut root, &mut parent).result,
        Ok(SubjectStep::Unchanged)
    );
    let input = region(3, 7, 0);
    let families = families(&system);
    let first = &families.pairs[0];
    let mut prefix = Words::<u64>::new(vec![0; families.occurrences.len().div_ceil(64)]);
    for position in first.lower.clone().chain(first.upper.clone()) {
        prefix.insert(position);
    }
    let mut complete = Words::<u64>::new(vec![0; prefix.len()]);
    for position in 0..families.occurrences.len() {
        complete.insert(position);
    }
    let mut first_knowledge = parent.clone();
    let mut first_work = Meter::local(u64::MAX);
    assert!(
        system
            .inspect_selected(
                families,
                &input,
                &mut first_knowledge,
                &prefix,
                &mut first_work
            )
            .unwrap()
    );
    let mut all_knowledge = parent.clone();
    let mut exact = Meter::local(first_work.charged());
    assert!(
        system
            .inspect_selected(families, &input, &mut all_knowledge, &complete, &mut exact)
            .unwrap()
    );
    assert_eq!(exact.charged(), first_work.charged());
    assert_eq!(all_knowledge.sums, first_knowledge.sums);
    assert_eq!(&all_knowledge.sums[1..], &parent.sums[1..]);
    assert_eq!(
        all_knowledge.snapshot.old_masks(),
        parent.snapshot.old_masks()
    );
}
