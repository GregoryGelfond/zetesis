//! Initial-clause metadata must be admitted from the actual retained CNF.

use super::*;

fn builder() -> Builder {
    let literal = Literal::new(0, true);
    // Canonicalization drops the tautology, coalesces a duplicate into a unit,
    // and preserves a real empty clause. Metadata indexes the retained rows.
    let cnf = Cnf::new(
        1,
        vec![
            vec![literal, literal.negated()],
            vec![literal, literal],
            vec![],
            vec![literal],
        ],
        AdmissionLimits::default(),
    )
    .unwrap();
    Builder {
        data: Data {
            theory: Theory::new(
                1,
                zetesis_ferraris::FormulaParts::new(vec![], vec![]).unwrap(),
                vec![],
                zetesis_ferraris::AdmissionLimits::default(),
            )
            .unwrap(),
            cnf,
            guarded: Vec::new(),
            units: Vec::new(),
            has_empty_clause: false,
            statistics: ReductPreparationStatistics::default(),
        },
        nodes: Vec::new(),
        gates: HashMap::new(),
        strict: Vec::new(),
        scratch: Vec::new(),
    }
}

#[test]
fn unit_metadata_uses_canonical_clause_positions() {
    let mut builder = builder();
    let mut statistics = ReductPreparationStatistics::default();
    let cancellation = Cancellation::default();
    let mut budget = Budget {
        quota: LocalQuota,
        cancellation: &cancellation,
        limits: SearchLimits::default(),
        statistics: SearchStatistics::default(),
    };
    builder
        .initial_clauses(u64::MAX, &mut budget, &mut statistics)
        .unwrap();
    assert_eq!(builder.data.units, [0, 2]);
    assert!(builder.data.has_empty_clause);
    // Counting and collecting visit the three retained clauses once each.
    assert_eq!(budget.statistics.work, 6);
    assert_eq!(statistics.peak_bytes, builder.bytes());
}

#[test]
fn refused_unit_storage_keeps_the_allocated_peak() {
    let mut builder = builder();
    let before = builder.bytes();
    let required = before + 2 * size_of::<usize>() as u128;
    let limit = u64::try_from(required).unwrap() - 1;
    let mut statistics = ReductPreparationStatistics::default();
    let cancellation = Cancellation::default();
    let mut budget = Budget {
        quota: LocalQuota,
        cancellation: &cancellation,
        limits: SearchLimits::default(),
        statistics: SearchStatistics::default(),
    };
    let result = builder.initial_clauses(limit, &mut budget, &mut statistics);
    builder.record(&mut statistics);
    assert_eq!(
        result,
        Err(Incomplete::ReductStorage {
            required,
            limit: u128::from(limit)
        })
    );
    assert_eq!(budget.statistics.work, 3);
    assert_eq!(statistics.peak_bytes, before);
    assert_eq!(statistics.retained_bytes, 0);
    assert_eq!(builder.data.units.capacity(), 0);
    assert!(!builder.data.has_empty_clause);
    builder
        .initial_clauses(u64::MAX, &mut budget, &mut statistics)
        .unwrap();
    assert_eq!(builder.data.units, [0, 2]);
}

#[test]
fn native_threshold_requires_its_original_truth_parameter() {
    use zetesis_ferraris::{FormulaNodes, NodeView, Threshold};
    let mut nodes = FormulaNodes::default();
    let mut transaction = nodes.transaction();
    transaction.push(NodeView::Atom(0), 10, 10).unwrap();
    let root = transaction
        .push(
            NodeView::Threshold(Threshold {
                conditions: &[0],
                weights: &[1],
                bound: 1,
            }),
            10,
            10,
        )
        .unwrap();
    transaction.commit();
    let theory = Theory::new(
        2,
        nodes.into_parts(),
        vec![root],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    let cancellation = Cancellation::default();
    let prepared =
        PreparedReduct::prepare(&theory, ReductPreparationLimits::default(), &cancellation)
            .result
            .unwrap();
    assert_eq!(prepared.0.guarded, [root]);
    assert_eq!(prepared.parameter_count(), 3);
    let mut search = crate::search::PreparedWorkspace::default();
    for original in [false, true, false] {
        let mut budget = Budget {
            quota: LocalQuota,
            limits: SearchLimits::default(),
            cancellation: &cancellation,
            statistics: SearchStatistics::default(),
        };
        // Deliberately bypass the public authenticated mask boundary to inspect
        // the physical gate: N={p}, M={p,q}, and the threshold guard supplied
        // independently. Public queries never accept this forged parameter.
        let result = search.query(
            &prepared.0.cnf,
            &prepared.0.units,
            prepared.0.has_empty_clause,
            &[
                Literal::new(0, true),
                Literal::new(1, false),
                Literal::new(2, true),
                Literal::new(3, true),
                Literal::new(4, original),
            ],
            &mut budget,
        );
        assert_eq!(
            matches!(result, crate::Solve::Sat(_)),
            original,
            "{result:?}"
        );
        assert!(!matches!(result, crate::Solve::Inconclusive(_)));
    }
}

#[test]
fn ordinal_cut_requires_its_original_truth_parameter() {
    use zetesis_ferraris::{ExtremumCut, ExtremumCutRule, ExtremumRow, FormulaNodes, NodeView};
    let mut nodes = FormulaNodes::default();
    let mut transaction = nodes.transaction();
    transaction.push(NodeView::Atom(0), 10, 10).unwrap();
    let root = transaction
        .push(
            NodeView::ExtremumCut(ExtremumCut {
                row: ExtremumRow {
                    conditions: &[0],
                    ranks: &[1],
                    rank_count: 3,
                },
                rule: &ExtremumCutRule::Prefix(2),
            }),
            10,
            10,
        )
        .unwrap();
    transaction.commit();
    let theory = Theory::new(
        2,
        nodes.into_parts(),
        vec![root],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    let cancellation = Cancellation::default();
    let prepared =
        PreparedReduct::prepare(&theory, ReductPreparationLimits::default(), &cancellation)
            .result
            .unwrap();
    assert_eq!(prepared.0.guarded, [root]);
    assert_eq!(prepared.parameter_count(), 3);
    let mut search = crate::search::PreparedWorkspace::default();
    for original in [false, true, false] {
        let mut budget = Budget {
            quota: LocalQuota,
            limits: SearchLimits::default(),
            cancellation: &cancellation,
            statistics: SearchStatistics::default(),
        };
        // Deliberately bypass the public authenticated mask boundary to inspect
        // the physical gate: N={p}, M={p,q}, and the ordinal-cut guard supplied
        // independently. Public queries never accept this forged parameter.
        let result = search.query(
            &prepared.0.cnf,
            &prepared.0.units,
            prepared.0.has_empty_clause,
            &[
                Literal::new(0, true),
                Literal::new(1, false),
                Literal::new(2, true),
                Literal::new(3, true),
                Literal::new(4, original),
            ],
            &mut budget,
        );
        assert_eq!(
            matches!(result, crate::Solve::Sat(_)),
            original,
            "{result:?}"
        );
        assert!(!matches!(result, crate::Solve::Inconclusive(_)));
    }
}
