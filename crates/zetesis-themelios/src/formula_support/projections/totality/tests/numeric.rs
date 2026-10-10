use super::*;

fn check(source: &str, certified: bool, passing: usize) {
    testing::with_completed_source(source, |program, support, computation, counters| {
        let rule = constraints(program).next().unwrap();
        let limits = FormulaLimits::default();
        let mut join = Join::rule(
            rule,
            support,
            computation,
            &limits,
            &mut testing::budget(),
            counters,
        )
        .unwrap();
        assert_eq!(
            join.select_total_constraint(rule, computation, &limits, counters)
                .unwrap(),
            certified
        );
        assert_eq!(join.certified_total, certified);
        let mut observed = 0;
        while let Some(row) = join
            .next_row(
                computation,
                &limits,
                &mut testing::budget(),
                counters,
                rule.location,
            )
            .unwrap()
        {
            observed += usize::from(row.passes);
        }
        assert_eq!(observed, passing);
    });
}

#[test]
fn sums_over_multiple_columns_are_total() {
    check(
        include_str!(
            "../../../../../tests/fixtures/streamed-consequences/arithmetic/sum-selection.lp"
        ),
        true,
        2,
    );
}

#[test]
fn negated_sums_preserve_numeric_bounds() {
    check(
        include_str!(
            "../../../../../tests/fixtures/streamed-consequences/arithmetic/negated-sum.lp"
        ),
        true,
        1,
    );
}

#[test]
fn intermediate_overflow_declines_the_certificate() {
    // The actual correlated rows are defined, but their rectangular hull
    // contains an overflowing intermediate even though the final range fits.
    check(
        include_str!(
            "../../../../../tests/fixtures/streamed-consequences/arithmetic/correlated-overflow.lp"
        ),
        false,
        1,
    );
}

#[test]
fn mixed_columns_do_not_establish_numeric_totality() {
    check(
        include_str!(
            "../../../../../tests/fixtures/streamed-consequences/arithmetic/mixed-domain.lp"
        ),
        false,
        1,
    );
}

#[test]
fn empty_columns_leave_complete_traversal_available() {
    check(
        include_str!(
            "../../../../../tests/fixtures/streamed-consequences/arithmetic/empty-numeric-column.lp"
        ),
        false,
        0,
    );
}

#[test]
fn reached_intermediate_overflow_remains_fatal() {
    testing::with_completed_source(
        include_str!(
            "../../../../../tests/fixtures/streamed-consequences/arithmetic/fatal-intermediate.lp"
        ),
        |program, support, computation, counters| {
            let rule = constraints(program).next().unwrap();
            let limits = FormulaLimits::default();
            let mut join = Join::rule(
                rule,
                support,
                computation,
                &limits,
                &mut testing::budget(),
                counters,
            )
            .unwrap();
            assert!(
                !join
                    .select_total_constraint(rule, computation, &limits, counters)
                    .unwrap()
            );
            assert!(matches!(
                join.next_row(
                    computation,
                    &limits,
                    &mut testing::budget(),
                    counters,
                    rule.location
                ),
                Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                    error: EvalError::Overflow,
                    ..
                }))
            ));
        },
    );
}

#[test]
fn a_total_domain_preserves_incomplete_prefix_errors() {
    let source = include_str!(
        "../../../../../tests/fixtures/streamed-consequences/arithmetic/deferred-prefix.lp"
    );
    testing::with_completed_source(source, |program, support, computation, counters| {
        let rule = constraints(program).next().unwrap();
        let limits = FormulaLimits::default();
        let mut budget = testing::budget();
        let mut join =
            Join::rule(rule, support, computation, &limits, &mut budget, counters).unwrap();
        assert!(
            join.select_total_constraint(rule, computation, &limits, counters)
                .unwrap()
        );
        assert_eq!(
            join.plan
                .patterns
                .iter()
                .map(|occurrence| occurrence.atom().predicate().name())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        let comparison = rule
            .body
            .iter()
            .position(|literal| matches!(literal, LiteralIr::Compare(..)))
            .unwrap();
        assert_eq!(
            join.plan.decisions.decided_at(0).collect::<Vec<_>>(),
            [comparison]
        );

        // Reach the overflowing prefix before b has a chance to reject it.
        // The smaller b.X column proves totality for complete bindings, not
        // this speculative a row. Its error must therefore remain deferred.
        let pattern = join.plan.patterns[0];
        let row = loop {
            let position = join
                .advance_probe(
                    pattern,
                    Context::new(computation, &limits, counters, rule.location),
                )
                .unwrap()
                .expect("the offered a rows include the overflowing prefix");
            let row = join.resolutions[0].rows().unwrap().row(position).unwrap();
            if row.value(0).unwrap().descriptor() == zetesis_core::ValueNodeRef::Number(i32::MAX) {
                break row;
            }
        };
        assert!(
            join.match_row(
                pattern.pattern,
                row,
                &mut budget,
                Context::new(computation, &limits, counters, rule.location),
            )
            .unwrap()
        );
        assert!(
            join.filter_prefix(computation, &limits, counters, rule.location)
                .unwrap()
        );
        assert!(matches!(join.failure, Some((0, EvalError::Overflow))));
    });

    // Ordinary traversal must discard that incomplete prefix and return every
    // complete binding, without turning its deferred error into a refusal.
    check(source, true, 10);
}
