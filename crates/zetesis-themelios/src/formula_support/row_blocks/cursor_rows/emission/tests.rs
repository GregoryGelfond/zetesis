use super::*;
use crate::formula_support::row_blocks::tests::{JOIN, prepared_source};
use crate::formula_support::{Context, Ir, Support, delta, testing};

#[test]
fn table_cursors_are_declined_before_allocation_or_claim() {
    {
        let strategy = JoinStrategy::Table;
        let (program, mut catalog) = prepared_source(JOIN);
        let rule = program.rules_without_facts().last().unwrap();
        let limits = FormulaLimits::default();
        let mut counters = Counters::default();
        let mut budget = testing::budget();
        let (relations, mut append) = catalog
            .split(&limits, &mut counters, rule.location)
            .unwrap();
        let support =
            Support::completed(&relations, strategy, &limits, &counters, rule.location).unwrap();
        let mut computation = Computation::new(&mut append, &support);
        let mut cursor = Join::variant_rule(
            Ir::Source(rule),
            delta::Variant::Full,
            &support,
            None,
            &mut budget,
            Context::new(&computation, &limits, &mut counters, rule.location),
        )
        .unwrap();
        let positions = cursor.positions.clone();
        let bytes = computation.query_support().live_bytes();
        let work = counters.accounting.work;
        let accepted = CursorRows::prepare_emission(
            rule,
            &mut cursor,
            &mut computation,
            &limits,
            &mut counters,
        )
        .unwrap()
        .is_some();
        assert!(!accepted);
        assert_eq!(cursor.positions, positions);
        assert_eq!(counters.accounting.work, work + 1);
        assert_eq!(computation.query_support().live_bytes(), bytes);
        cursor
            .retain_witnesses(Context::new(
                &computation,
                &limits,
                &mut counters,
                rule.location,
            ))
            .unwrap();
        let mut selected = 0;
        while cursor
            .next_witness_row(
                None,
                &mut budget,
                Context::new(&mut computation, &limits, &mut counters, rule.location),
            )
            .unwrap()
            .is_some()
        {
            selected += 1;
        }
        assert_eq!(
            selected, 4,
            "original complete source cursor remains usable"
        );
    }
}
