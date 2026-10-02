//! Necessary domains reuse exact table row selection without changing strategy.

use super::*;

fn with_support(strategy: JoinStrategy, run: impl FnOnce(&Support<'_>, &mut Counters)) {
    let catalog = catalog();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let support = Support::completed(&relations, strategy, &limits, &counters, location()).unwrap();
    run(&support, &mut counters);
}

#[test]
fn finite_domains_share_tables_without_changing_indexed_probes() {
    with_support(JoinStrategy::Indexed, |support, counters| {
        let limits = FormulaLimits::default();
        let flat = pattern(false);
        let pattern = PositivePattern::Flat((&flat).into());
        let binding = [Some(Value::Number(1)), None];
        let values = binding.as_slice().into();
        assert!(
            support
                .select(pattern, values, &limits, counters, location())
                .unwrap()
                .is_none()
        );
        assert!(support.tables.get().is_none());
        let rows = support
            .resolve(pattern.atom(), &limits, counters, location())
            .unwrap();
        let permitted = [Value::Number(8), Value::Number(9), Value::Number(15)];
        let permitted = permitted.each_ref().map(TermRef::from);
        let finite = FiniteDomains {
            values: &permitted,
            variables: &[(1, 0..3)],
        };
        let first = support
            .select_domains_at(
                rows,
                pattern,
                values,
                finite,
                GroundingWork::new(&limits, counters, location()),
            )
            .unwrap()
            .unwrap();
        // The finite second-column domain meets the bound first column, and
        // the mask yields original relation row positions in their order.
        assert_eq!(first.selection.rows().collect::<Vec<_>>(), [8, 15]);
        let second = support
            .select_domains_at(
                rows,
                pattern,
                values,
                finite,
                GroundingWork::new(&limits, counters, location()),
            )
            .unwrap()
            .unwrap();
        assert_eq!(second.selection.rows().collect::<Vec<_>>(), [8, 15]);
        assert_eq!(support.tables.get().unwrap().indices.borrow().len(), 1);
        assert!(
            support
                .select(pattern, values, &limits, counters, location())
                .unwrap()
                .is_none()
        );
        assert_eq!(first.selection.rows().collect::<Vec<_>>(), [8, 15]);
    });
}

#[test]
fn repeated_columns_and_empty_domains_keep_set_meaning() {
    with_support(JoinStrategy::Table, |support, counters| {
        let limits = FormulaLimits::default();
        let flat = pattern(true);
        let pattern = PositivePattern::Flat((&flat).into());
        let binding = [None::<TermRef<'_>>];
        let values = binding.as_slice().into();
        let rows = support
            .resolve(pattern.atom(), &limits, counters, location())
            .unwrap();
        let permitted = [
            Value::Number(1),
            Value::Number(2),
            Value::Number(2),
            Value::Number(8),
        ];
        let permitted = permitted.each_ref().map(TermRef::from);
        let selected = support
            .select_domains_at(
                rows,
                pattern,
                values,
                FiniteDomains {
                    values: &permitted,
                    variables: &[(0, 0..4)],
                },
                GroundingWork::new(&limits, counters, location()),
            )
            .unwrap()
            .unwrap();
        // The repeated source slot is one table domain and both columns must
        // agree. Duplicate input values do not duplicate original row IDs.
        assert_eq!(selected.selection.rows().collect::<Vec<_>>(), [1, 2]);
        let empty = support
            .select_domains_at(
                rows,
                pattern,
                values,
                FiniteDomains {
                    values: &permitted,
                    variables: &[(0, 0..0)],
                },
                GroundingWork::new(&limits, counters, location()),
            )
            .unwrap()
            .unwrap();
        assert_eq!(empty.selection.rows().count(), 0);
        assert_eq!(selected.selection.rows().collect::<Vec<_>>(), [1, 2]);
        assert_eq!(support.tables.get().unwrap().indices.borrow().len(), 1);
    });
}

#[test]
fn malformed_restrictions_refuse_before_table_preparation() {
    with_support(JoinStrategy::Indexed, |support, counters| {
        let limits = FormulaLimits::default();
        let flat = pattern(false);
        let pattern = PositivePattern::Flat((&flat).into());
        let rows = support
            .resolve(pattern.atom(), &limits, counters, location())
            .unwrap();
        let permitted = [Value::Number(1)];
        let permitted = permitted.each_ref().map(TermRef::from);
        for (binding, variables) in [
            ([None, None, None], vec![(1, 0..2)]),
            ([None, None, None], vec![(1, 0..1), (1, 0..1)]),
            ([None, Some(Value::Number(1)), None], vec![(1, 0..1)]),
            ([None, None, None], vec![(2, 0..1)]),
        ] {
            let live = support.live_bytes();
            let result = support.select_domains_at(
                rows,
                pattern,
                binding.as_slice().into(),
                FiniteDomains {
                    values: &permitted,
                    variables: &variables,
                },
                GroundingWork::new(&limits, counters, location()),
            );
            assert!(matches!(
                result,
                Err(FormulaFailure::SupportTable {
                    error: table::Failure {
                        cause: Cause::Domains,
                        ..
                    },
                    ..
                })
            ));
            assert!(support.tables.get().is_none());
            assert_eq!(support.live_bytes(), live);
        }
    });
}
