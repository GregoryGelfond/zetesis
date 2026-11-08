use super::*;

pub(super) fn numeric(source: &str) -> (Collected, Collected) {
    let mut result = None;
    testing::with_completed_source(source, |program, support, computation, counters| {
        let rule = rule(program);
        let limits = FormulaLimits::default();
        let mut run = |enabled: bool| {
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
            assert!(join.multiples.is_some());
            join.select_numeric_probes(rule, &limits, counters).unwrap();
            assert!(join.expression_keys.is_some());
            if !enabled {
                join.multiples = None;
            }
            collect(&mut join, computation, counters, rule.location)
        };
        let ordinary = run(false);
        let selected = run(true);
        assert_eq!(
            selected.values, ordinary.values,
            "same complete tuple order"
        );
        result = Some((ordinary, selected));
    });
    result.unwrap()
}

#[test]
fn scalar_ranges_compose_with_certified_multiples() {
    for body in ["number(N),divisor(D)", "divisor(D),number(N)"] {
        for relation in ["<", "<=", ">", ">="] {
            let source =
                format!("number(2..30). divisor(2..7). h(N,D) :- {body},D {relation} N,N\\D=0.");
            let (ordinary, selected) = numeric(&source);
            assert!(
                selected.owned || selected.values.is_empty(),
                "{relation}: selected nonempty postings are owned"
            );
            assert_eq!(selected.work.range_probes, Some(0));
            assert!(ordinary.work.range_probes.unwrap() > 0);
            assert!(selected.work.join_rows.unwrap() < ordinary.work.join_rows.unwrap());
            let expected = (2..=30)
                .flat_map(|n| {
                    (2..=7).filter(move |&d| {
                        n % d == 0
                            && match relation {
                                "<" => d < n,
                                "<=" => d <= n,
                                ">" => d > n,
                                ">=" => d >= n,
                                _ => unreachable!(),
                            }
                    })
                })
                .count();
            assert_eq!(selected.values.len(), expected);
        }
    }
}

#[test]
fn sparse_multiples_keep_the_cheaper_existing_range() {
    let (ordinary, selected) =
        numeric("number(2;2147483647). divisor(3). h(N,D) :- number(N),divisor(D),D<N,N\\D=0.");
    assert!(!selected.owned);
    assert_eq!(selected.work.join_rows, ordinary.work.join_rows);
    assert_eq!(selected.work.range_probes, ordinary.work.range_probes);
}

#[test]
fn intersected_postings_preserve_full_tuple_multiplicity() {
    let (ordinary, selected) = numeric(
        "number(2..20,a). number(2..20,b). divisor(3). h(N,D,T) :- number(N,T),divisor(D),D<N,N\\D=0.",
    );
    assert!(selected.owned);
    assert_eq!(selected.values.len(), 10);
    assert!(selected.work.join_rows.unwrap() < ordinary.work.join_rows.unwrap());
}

#[test]
fn strict_range_endpoints_cannot_overflow() {
    for low in 1..=6 {
        for high in low..=10 {
            for wanted in [i32::MIN, 0, 1, 5, 10, i32::MAX] {
                for relation in [Relation::Lt, Relation::Le, Relation::Gt, Relation::Ge] {
                    let expected: Vec<_> = (low..=high)
                        .filter(|&value| match relation {
                            Relation::Lt => value < wanted,
                            Relation::Le => value <= wanted,
                            Relation::Gt => value > wanted,
                            Relation::Ge => value >= wanted,
                            _ => unreachable!(),
                        })
                        .collect();
                    let actual = intersect_range((low, high), (relation, wanted));
                    assert_eq!(
                        actual,
                        expected.first().copied().zip(expected.last().copied())
                    );
                }
            }
        }
    }
    assert_eq!(
        intersect_range((1, i32::MAX), (Relation::Gt, i32::MAX)),
        None
    );
    assert_eq!(
        intersect_range((1, i32::MAX), (Relation::Le, i32::MAX)),
        Some((1, i32::MAX))
    );
}

mod accounting;
