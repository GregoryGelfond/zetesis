use super::*;

#[test]
fn variable_divisors_certify_nonzero_numeric_domains() {
    for source in [
        include_str!("../../../../../../tests/fixtures/column-totality/positive-quotient.lp"),
        include_str!("../../../../../../tests/fixtures/column-totality/negative-quotient.lp"),
    ] {
        check(source, true, 2);
    }
}

#[test]
fn variable_remainders_certify_signed_numeric_domains() {
    check(
        include_str!("../../../../../../tests/fixtures/column-totality/positive-remainder.lp"),
        true,
        1,
    );
    check(
        include_str!("../../../../../../tests/fixtures/column-totality/negative-remainder.lp"),
        true,
        2,
    );
}

#[test]
fn zero_in_the_hull_declines_without_rejecting_source() {
    check(
        include_str!("../../../../../../tests/fixtures/column-totality/zero-hull-quotient.lp"),
        false,
        1,
    );
    check(
        include_str!("../../../../../../tests/fixtures/column-totality/zero-hull-remainder.lp"),
        false,
        2,
    );
}

#[test]
fn exceptional_variable_pairs_decline_the_certificate() {
    // Each actual tuple is defined; only the rectangular hull contains MIN/-1.
    check(
        include_str!(
            "../../../../../../tests/fixtures/column-totality/exceptional-hull-quotient.lp"
        ),
        false,
        1,
    );
    check(
        include_str!(
            "../../../../../../tests/fixtures/column-totality/exceptional-hull-remainder.lp"
        ),
        false,
        2,
    );
}

#[test]
fn normal_bodies_select_only_certified_complete_rows() {
    let source =
        include_str!("../../../../../../tests/fixtures/column-totality/normal-selection.lp");
    testing::with_completed_source(source, |program, support, computation, counters| {
        let rule = program
            .rules_without_facts()
            .iter()
            .find(|rule| {
                matches!(rule.head, HeadIr::Normal(Some(_)))
                    && rule
                        .body
                        .iter()
                        .any(|literal| matches!(literal, LiteralIr::Compare(..)))
            })
            .unwrap();
        let limits = FormulaLimits::default();
        let mut budget = testing::budget();
        let mut join =
            Join::rule(rule, support, computation, &limits, &mut budget, counters).unwrap();
        assert!(
            join.select_total_body(rule, computation, &limits, counters)
                .unwrap()
        );
        assert!(join.certified_total);
        assert!(join.coverage == Coverage::Selected);
        let mut selected = 0;
        while let Some(row) = join
            .next_row(computation, &limits, &mut budget, counters, rule.location)
            .unwrap()
        {
            assert!(
                row.passes,
                "certified selection must omit the defined false row"
            );
            selected += 1;
        }
        assert_eq!(selected, 2);
    });
}

#[test]
fn body_totality_keeps_generated_head_obligations_separate() {
    let source =
        include_str!("../../../../../../tests/fixtures/column-totality/false-generated-head.lp");
    testing::with_completed_source(source, |program, support, computation, counters| {
        let rule = program
            .rules_without_facts()
            .iter()
            .find(|rule| rule.variables > rule.body_variables)
            .unwrap();
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
        assert!(join.certified_total);
        assert!(
            !join
                .projections
                .total_family(
                    rule,
                    support,
                    &mut Evaluation::default(),
                    Context::new(computation, &limits, counters, rule.location),
                )
                .unwrap()
        );
    });
}
