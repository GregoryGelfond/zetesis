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
                vec![],
                vec![],
                zetesis_ferraris::AdmissionLimits::default(),
            )
            .unwrap(),
            cnf,
            implications: Vec::new(),
            units: Vec::new(),
            has_empty_clause: false,
            statistics: ReductPreparationStatistics::default(),
        },
        nodes: Vec::new(),
        gates: HashMap::new(),
        strict: Vec::new(),
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
