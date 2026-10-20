use super::*;

fn numeric_run(
    rule: &RuleIr,
    support: &Support<'_>,
    computation: &mut Computation<'_, '_>,
    counters: &mut Counters,
    variant: Variant,
    selected: bool,
) -> Collected {
    let limits = FormulaLimits::default();
    let mut join = Join::variant_rule(
        rule,
        variant,
        support,
        None,
        &mut testing::budget(),
        Context::new(computation, &limits, counters, rule.location),
    )
    .unwrap();
    if selected {
        assert!(
            join.select_round_multiples(rule, computation, &limits, counters)
                .unwrap()
        );
        assert!(!join.certified_total);
    }
    join.select_numeric_probes(rule, &limits, counters).unwrap();
    assert!(join.expression_keys.is_some());
    collect(&mut join, computation, counters, rule.location)
}

#[test]
fn range_intersections_preserve_first_new_partitions() {
    let mut positions = BTreeSet::new();
    let mut owned = false;
    for reverse in [false, true] {
        let mut snapshot = Snapshot::new("out(D,X,A,Y,B) :- n(X,A),d(D),n(Y,B),X\\D=0,D<X,Y>=1.");
        if reverse {
            for rule in snapshot.program.rules_without_facts_mut() {
                rule.body.reverse();
            }
        }
        snapshot.publish(population(3, 0));
        let old = snapshot.with(|rule, support, computation, counters| {
            numeric_run(rule, support, computation, counters, Variant::Full, false)
                .values
                .into_iter()
                .collect::<BTreeSet<_>>()
        });
        snapshot.publish(population(4, 1));
        snapshot.with(|rule, support, computation, counters| {
            let all =
                numeric_run(rule, support, computation, counters, Variant::Full, false).values;
            let expected = all
                .into_iter()
                .filter(|row| !old.contains(row))
                .collect::<BTreeSet<_>>();
            let registration = registered(rule, support);
            let mut variants = registration
                .variants(0, rule, support, false, &FormulaLimits::default(), counters)
                .unwrap();
            let target = occurrence(rule, computation, counters, true);
            let mut actual = Vec::new();
            while let Some(variant) = variants.next(&FormulaLimits::default(), counters).unwrap() {
                let Variant::Delta(pivot) = variant else {
                    panic!("flat delta")
                };
                positions.insert(target.cmp(&pivot));
                let ordinary = numeric_run(rule, support, computation, counters, variant, false);
                let selected = numeric_run(rule, support, computation, counters, variant, true);
                assert_eq!(selected.values, ordinary.values);
                owned |= selected.owned;
                actual.extend(selected.values);
            }
            let distinct = actual.iter().cloned().collect::<BTreeSet<_>>();
            assert_eq!(
                actual.len(),
                distinct.len(),
                "each current non-old tuple occurs once"
            );
            assert_eq!(distinct, expected);
        });
    }
    assert!(owned, "the intersected posting ran");
    assert_eq!(
        positions,
        BTreeSet::from([
            std::cmp::Ordering::Less,
            std::cmp::Ordering::Equal,
            std::cmp::Ordering::Greater
        ])
    );
}
