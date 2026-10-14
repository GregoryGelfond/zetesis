use super::*;
use crate::formula_support::{PreparedRule, build};

#[test]
fn completed_queries_reuse_the_authenticated_target() {
    let preparation = testing::prepare(DENSE);
    let program = preparation.program;
    let limits = preparation.limits;
    let mut counters = Counters::resume(
        preparation.accounting,
        crate::grounding_observer::Work::default(),
    );
    let catalog = build(
        preparation.catalog,
        &program,
        None,
        &limits,
        &mut testing::budget(),
        &mut counters,
        preparation.location,
    )
    .unwrap();
    let rule = rule(&program);
    let mut completed = catalog
        .snapshot(&limits, &mut counters, rule.location)
        .unwrap();
    let prepared = PreparedRule::new(
        rule,
        &mut completed,
        &limits,
        &mut testing::budget(),
        &mut counters,
    )
    .unwrap();
    assert!(prepared.multiples.is_some());
    let queries = completed
        .queries(
            crate::JoinStrategy::Indexed,
            &limits,
            &counters,
            rule.location,
        )
        .unwrap();
    let mut computation = queries.computation(rule.location).unwrap();
    let retained = counters.accounting.workspace.bytes();
    for _ in 0..2 {
        let observer = Observer::default();
        let profile = Profile::new(Some(&observer));
        counters.observed = profile.work();
        let mut rows = prepared
            .rows(
                &queries,
                None,
                &computation,
                &limits,
                &mut testing::budget(),
                &mut counters,
            )
            .unwrap();
        let count = profile
            .phase(
                GroundingPhase::RuleInstantiation,
                Some(rule.location),
                || {
                    let mut count = 0;
                    while let Some(row) = rows.next_row(
                        &mut computation,
                        &limits,
                        &mut testing::budget(),
                        &mut counters,
                        rule.location,
                    )? {
                        assert!(row.positives.is_none());
                        count += usize::from(row.passes);
                    }
                    Ok::<_, FormulaFailure>(count)
                },
            )
            .unwrap();
        assert_eq!(count, 8);
        assert_eq!(observer.0.get().join_rows, Some(9));
        drop(rows);
        assert_eq!(counters.accounting.workspace.bytes(), retained);
    }
}
