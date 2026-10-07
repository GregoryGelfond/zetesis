//! Structural captures are one transactional scratch buffer per join.
use super::*;
use crate::ExpansionLimits;

#[test]
fn structural_scratch_survives_join_backtracking() {
    // k is the smaller relation. Each k binding therefore revisits all p rows,
    // including constructor mismatches and unequal repeated nested variables.
    // The source matcher must not retain either failed captures or old bindings.
    let source = "k(1..2). p(other(1..100)). p(f(g(1),g(1))). \
                  p(f(g(1),g(2))). p(f(g(2),g(2))). \
                  r(K,X) :- k(K), p(f(g(X),g(X))).";
    testing::with_completed_source(source, |program, support, computation, counters| {
        let rule = program.rules.last().unwrap();
        let location = rule.location;
        let limits = FormulaLimits::default();
        let HeadIr::Normal(Some(head)) = rule.head else {
            panic!("normal rule")
        };
        let head = computation
            .static_pattern(head, &limits, counters, location)
            .unwrap();
        let slots: Vec<_> = head
            .terms()
            .iter()
            .map(|term| {
                let TemplateTerm::Variable(slot) = term else {
                    panic!("variable head")
                };
                slot
            })
            .collect();
        let expected_bytes = 3 * size_of::<(usize, TermRef<'_>)>();
        for _ in 0..2 {
            let mut preparation = testing::budget();
            let mut join = Join::rule(
                rule,
                support,
                computation,
                &limits,
                &mut preparation,
                counters,
            )
            .unwrap();
            assert!(join.pattern_captures.is_empty());
            assert_eq!(join.pattern_captures.capacity(), 0);
            // Only matching runs under this allowance. Every other source and
            // join preparation charge has already been admitted independently.
            let mut budget = Budget::new(
                ExpansionLimits {
                    max_scalar_bytes: expected_bytes,
                    ..ExpansionLimits::default()
                },
                usize::MAX,
            );
            let mut output = Vec::new();
            while let Some(row) = join
                .next(computation, &limits, &mut budget, counters, location)
                .unwrap()
            {
                output.push(
                    slots
                        .iter()
                        .map(|&slot| {
                            row.read(slot, computation.read(), location)
                                .unwrap()
                                .to_string()
                        })
                        .collect::<Vec<_>>(),
                );
                assert!(
                    join.pattern_captures.is_empty(),
                    "committed captures must be cleared"
                );
                assert_eq!(join.lease.bytes(), join.storage_bytes());
            }
            output.sort();
            assert_eq!(output, [["1", "1"], ["1", "2"], ["2", "1"], ["2", "2"]]);
            assert!(join.pattern_captures.is_empty());
            assert_eq!(
                join.pattern_captures.capacity() * size_of::<(usize, TermRef<'_>)>(),
                expected_bytes
            );
            assert_eq!(budget.usage().scalar_bytes, expected_bytes);
            assert_eq!(join.lease.bytes(), join.storage_bytes());
        }
    });
}

#[test]
fn commit_refusals_clear_structural_scratch() {
    testing::with_completed_source(
        "p(f(g(1),g(1))). r(X) :- p(f(g(X),g(X))).",
        |program, support, computation, counters| {
            let rule = program.rules.last().unwrap();
            let location = rule.location;
            let mut attempt = |allowance: Option<u64>| {
                let limits = FormulaLimits::default();
                let mut budget = testing::budget();
                let mut join =
                    Join::rule(rule, support, computation, &limits, &mut budget, counters).unwrap();
                let pattern = join.plan.patterns[0].pattern;
                assert!(matches!(pattern, PositivePattern::Structural(_)));
                let atom = support
                    .resolve(pattern.atom(), &limits, counters, location)
                    .unwrap()
                    .unwrap()
                    .row(0)
                    .unwrap();
                let start = counters.accounting.work;
                let limits = FormulaLimits {
                    max_work: allowance.map_or(u64::MAX, |remaining| start + remaining),
                    ..limits
                };
                let result = join.match_row(
                    pattern,
                    atom,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                );
                assert!(
                    join.pattern_captures.is_empty(),
                    "capture cleanup includes commit refusals"
                );
                assert_eq!(join.lease.bytes(), join.storage_bytes());
                let partially_committed = !join.changes[0].is_empty();
                (
                    result,
                    counters.accounting.work - start,
                    partially_committed,
                )
            };
            let (result, exact, _) = attempt(None);
            assert!(result.unwrap());
            let mut refused_after_commit = false;
            for limit in 0..exact {
                let (result, _, committed) = attempt(Some(limit));
                assert!(matches!(
                    result.unwrap_err(),
                    FormulaFailure::Limit {
                        resource: FormulaResource::Work,
                        ..
                    }
                ));
                refused_after_commit |= committed;
            }
            assert!(
                refused_after_commit,
                "exercise a refusal after matching and partial commit"
            );
            assert!(attempt(Some(exact)).0.unwrap());
        },
    );
}
