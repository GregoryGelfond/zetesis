use super::*;
use crate::ExpansionLimits;
use crate::FormulaLimits;
use crate::expansion::Budget;
use crate::formula_ground::Purpose;
use crate::formula_support::testing::Fixture;
use crate::test_support::location;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    ContributionRow, FormulaParts, Interpretation, Limits, Node as Stored, Theory,
    ThresholdDescriptor, models, models_reduct,
};

fn original() -> FormulaParts {
    FormulaParts::from_raw(
        vec![
            Stored::threshold(0),
            Stored::atom(1),
            Stored::threshold(1),
            Stored::implies(3, 4),
            Stored::threshold(0),
            Stored::and_pair([5, 6]),
        ],
        vec![],
        vec![2, 2],
        vec![2, 3],
        vec![ContributionRow {
            start: 0,
            length: 2,
        }],
        vec![
            ThresholdDescriptor { row: 0, bound: 2 },
            ThresholdDescriptor { row: 0, bound: 5 },
        ],
    )
    .unwrap()
}

#[test]
fn nonadjacent_guards_retain_one_canonical_row() {
    Fixture::default().with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let mut budget = Budget::new(ExpansionLimits::default(), 0);
        let mut builder = Builder::empty(
            computation,
            &limits,
            &mut budget,
            counters,
            Purpose::Theory,
            None,
            location(),
        )
        .unwrap();
        builder.initialize(location()).unwrap();
        builder.node(Node::Atom(0), location()).unwrap();
        let mut transaction = builder.nodes.transaction();
        transaction.append(original().view(), 20, 100).unwrap();
        let expected = Theory::new(
            2,
            transaction.parts().try_clone().unwrap(),
            vec![8],
            limits.theory,
        )
        .unwrap();
        let suffix = transaction.detach().unwrap();
        let map = builder.intern_appended(&suffix, location()).unwrap();
        assert_eq!(map.slice(), [3, 5, 4, 6, 3, 7]);
        assert_eq!(builder.nodes.parts().rows().len(), 1);
        assert_eq!(builder.nodes.parts().conditions(), [2, 2]);
        assert_eq!(builder.nodes.parts().numeric_values(), [2, 3]);
        assert_eq!(builder.nodes.parts().thresholds().len(), 2);
        let actual = Theory::new(
            2,
            std::mem::take(&mut builder.nodes).into_parts(),
            vec![map.slice()[5]],
            limits.theory,
        )
        .unwrap();
        for candidate in 0..4 {
            let left = Interpretation::new(
                &expected,
                (0..2).filter(|atom| candidate & (1 << atom) != 0),
            )
            .unwrap();
            let right = Interpretation::new(&actual, left.atoms()).unwrap();
            assert_eq!(
                models(
                    &expected,
                    &left,
                    Limits::default(),
                    &Cancellation::default()
                )
                .unwrap(),
                models(&actual, &right, Limits::default(), &Cancellation::default()).unwrap()
            );
            for tested in 0..4 {
                let subset_left =
                    Interpretation::new(&expected, (0..2).filter(|atom| tested & (1 << atom) != 0))
                        .unwrap();
                let subset_right = Interpretation::new(&actual, subset_left.atoms()).unwrap();
                assert_eq!(
                    models_reduct(
                        &expected,
                        &left,
                        &subset_left,
                        Limits::default(),
                        &Cancellation::default()
                    )
                    .unwrap(),
                    models_reduct(
                        &actual,
                        &right,
                        &subset_right,
                        Limits::default(),
                        &Cancellation::default()
                    )
                    .unwrap()
                );
            }
        }
    });
}

#[test]
fn repeated_suffix_reuses_every_threshold() {
    Fixture::default().with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let mut budget = Budget::new(ExpansionLimits::default(), 0);
        let mut builder = Builder::empty(
            computation,
            &limits,
            &mut budget,
            counters,
            Purpose::Theory,
            None,
            location(),
        )
        .unwrap();
        builder.initialize(location()).unwrap();
        builder.node(Node::Atom(0), location()).unwrap();
        let mut transaction = builder.nodes.transaction();
        transaction.append(original().view(), 20, 100).unwrap();
        let suffix = transaction.detach().unwrap();
        let first = builder.intern_appended(&suffix, location()).unwrap();
        let before = builder.nodes.view().len();
        let again = builder.intern_appended(&suffix, location()).unwrap();
        assert_eq!(again.slice(), first.slice());
        assert_eq!(builder.nodes.view().len(), before);
        assert_eq!(builder.nodes.parts().rows().len(), 1);
        assert_eq!(builder.nodes.parts().conditions().len(), 2);
    });
}

fn mixed_original() -> FormulaParts {
    use zetesis_ferraris::{ExtremumCutDescriptor, ExtremumCutRule, NumericDomain, NumericRow};
    FormulaParts::from_numeric_raw(
        vec![
            Stored::extremum_cut(0),
            Stored::threshold(0),
            Stored::atom(1),
            Stored::extremum_cut(1),
            Stored::implies(3, 6),
            Stored::extremum_cut(0),
            Stored::and_pair([7, 8]),
        ],
        vec![],
        vec![2, 2, 2, 2],
        vec![2, 3, 1, 3],
        vec![
            NumericRow {
                start: 0,
                length: 2,
                domain: NumericDomain::Weights,
            },
            NumericRow {
                start: 2,
                length: 2,
                domain: NumericDomain::Ordinal { rank_count: 4 },
            },
        ],
        vec![ThresholdDescriptor { row: 0, bound: 3 }],
        vec![
            ExtremumCutDescriptor {
                row: 1,
                rule: ExtremumCutRule::Prefix(2),
            },
            ExtremumCutDescriptor {
                row: 1,
                rule: ExtremumCutRule::Suffix(3),
            },
        ],
    )
    .unwrap()
}

fn assert_same_worlds(expected: &Theory, actual: &Theory) {
    let cancellation = Cancellation::default();
    for original in 0..4 {
        let left = Interpretation::new(expected, (0..2).filter(|atom| original & (1 << atom) != 0))
            .unwrap();
        let right = Interpretation::new(actual, left.atoms()).unwrap();
        assert_eq!(
            models(expected, &left, Limits::default(), &cancellation).unwrap(),
            models(actual, &right, Limits::default(), &cancellation).unwrap()
        );
        for tested in 0..4 {
            let a = Interpretation::new(expected, (0..2).filter(|atom| tested & (1 << atom) != 0))
                .unwrap();
            let b = Interpretation::new(actual, a.atoms()).unwrap();
            assert_eq!(
                models_reduct(expected, &left, &a, Limits::default(), &cancellation).unwrap(),
                models_reduct(actual, &right, &b, Limits::default(), &cancellation).unwrap()
            );
        }
    }
}

#[test]
fn mixed_nonadjacent_families_preserve_shared_rows_and_frozen_truth() {
    Fixture::default().with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let mut budget = Budget::new(ExpansionLimits::default(), 0);
        let mut builder = Builder::empty(
            computation,
            &limits,
            &mut budget,
            counters,
            Purpose::Theory,
            None,
            location(),
        )
        .unwrap();
        builder.initialize(location()).unwrap();
        builder.node(Node::Atom(0), location()).unwrap();
        let mut transaction = builder.nodes.transaction();
        transaction
            .append(mixed_original().view(), 30, 100)
            .unwrap();
        let expected = Theory::new(
            2,
            transaction.parts().try_clone().unwrap(),
            vec![9],
            limits.theory,
        )
        .unwrap();
        let suffix = transaction.detach().unwrap();
        let map = builder.intern_appended(&suffix, location()).unwrap();
        assert_eq!(map.slice(), [3, 5, 6, 4, 7, 3, 8]);
        assert_eq!(builder.nodes.parts().rows().len(), 2);
        assert_eq!(builder.nodes.parts().numeric_values(), [1, 3, 2, 3]);
        assert_eq!(builder.nodes.parts().conditions(), [2, 2, 2, 2]);
        assert_eq!(builder.nodes.parts().thresholds().len(), 1);
        assert_eq!(builder.nodes.parts().extremum_cuts().len(), 2);
        let before = builder.nodes.parts().try_clone().unwrap();
        let repeated = builder.intern_appended(&suffix, location()).unwrap();
        assert_eq!(repeated.slice(), map.slice());
        assert_eq!(builder.nodes.parts().nodes(), before.nodes());
        assert_eq!(builder.nodes.parts().rows(), before.rows());
        assert_eq!(
            builder.nodes.parts().numeric_values(),
            before.numeric_values()
        );
        assert_eq!(
            builder.nodes.parts().extremum_cuts(),
            before.extremum_cuts()
        );
        let actual = Theory::new(
            2,
            std::mem::take(&mut builder.nodes).into_parts(),
            vec![map.slice()[6]],
            limits.theory,
        )
        .unwrap();
        assert_same_worlds(&expected, &actual);
    });
}
