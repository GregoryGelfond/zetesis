use super::*;

#[test]
fn equal_sources_prepare_multiples_before_numeric_selection() {
    for body in ["candidate(N),candidate(D)", "candidate(D),candidate(N)"] {
        let source = format!("candidate(2..24). h(N,D) :- {body},D<N,N\\D=0.");
        testing::with_completed_source(&source, |program, support, computation, counters| {
            let rule = rule(program);
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
                join.select_total_body(rule, computation, &limits, counters)
                    .unwrap()
            );
            let prepared = join.multiples.expect("the tied order binds D before N");
            let Divisor::Variable(divisor) = prepared.divisor else {
                panic!("variable divisor")
            };
            assert!(prepared.depth > 0);
            let earlier = &join.plan.patterns[0];
            assert!(
                earlier
                    .atom()
                    .terms()
                    .variables()
                    .any(|slot| slot == divisor)
            );
            // This is the ordinary composed numeric-probe path, not a join
            // with the range selector silently disabled.
            join.select_numeric_probes(rule, &limits, counters).unwrap();
            assert!(join.expression_keys.is_some());
            let actual = collect(&mut join, computation, counters, rule.location);
            let expected = (2..=24)
                .flat_map(|n| (2..n).filter(move |d| n % d == 0))
                .count();
            assert_eq!(actual.values.len(), expected);
        });
    }
}

#[test]
fn nonzero_remainder_equality_grants_no_multiples_certificate() {
    let source = "candidate(2..8). h(N,D) :- candidate(N),candidate(D),D<N,N\\D=1.";
    testing::with_completed_source(source, |program, support, computation, counters| {
        let rule = rule(program);
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
        join.select_total_body(rule, computation, &limits, counters)
            .unwrap();
        assert!(join.multiples.is_none());
        join.select_numeric_probes(rule, &limits, counters).unwrap();
        let actual = collect(&mut join, computation, counters, rule.location);
        assert!(!actual.owned);
        let expected = (2..=8)
            .flat_map(|n| (2..n).filter(move |d| n % d == 1))
            .count();
        assert_eq!(actual.values.len(), expected);
    });
}
