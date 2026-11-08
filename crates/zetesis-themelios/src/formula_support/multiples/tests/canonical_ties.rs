//! Canonical spelling must not hide an otherwise eligible remainder probe.

use super::*;

#[test]
fn equal_sources_expose_multiples_despite_canonical_order() {
    let mut expected: Vec<_> = (2_i32..=24)
        .flat_map(|n| (2..n).filter(move |d| n % d == 0).map(move |d| (n, d)))
        .collect();
    expected.sort_unstable();
    let mut all_prepared = true;
    let mut all_intersected = true;
    for body in ["a(N),z(D)", "z(D),a(N)"] {
        // Equal populations and equally useful bindings leave a(N) first in
        // canonical order. A prepared divisor must not depend on those names.
        let source = format!("a(2..24). z(2..24). h(N,D) :- {body},D<N,N\\D=0.");
        testing::with_completed_source(&source, |program, support, computation, counters| {
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
                let slot = |name| {
                    join.plan
                        .patterns
                        .iter()
                        .find(|occurrence| occurrence.atom().predicate().name() == name)
                        .unwrap()
                        .atom()
                        .terms()
                        .variables()
                        .next()
                        .unwrap()
                };
                let slots = [slot("a"), slot("z")];
                assert!(
                    join.select_total_body(rule, computation, &limits, counters)
                        .unwrap()
                );
                let prepared = join.multiples.is_some();
                join.select_numeric_probes(rule, &limits, counters).unwrap();
                assert!(join.expression_keys.is_some());
                if !enabled {
                    join.multiples = None;
                }
                (
                    collect(&mut join, computation, counters, rule.location),
                    prepared,
                    slots,
                )
            };
            let (ordinary, _, slots) = run(false);
            let (selected, prepared, selected_slots) = run(true);
            assert_eq!(slots, selected_slots);
            assert_eq!(
                selected.values, ordinary.values,
                "complete fixed-plan tuples"
            );
            for collected in [&ordinary, &selected] {
                assert_eq!(
                    numeric_pairs(collected, slots),
                    expected,
                    "complete mathematical family: {body}"
                );
            }
            eprintln!(
                "{body}: prepared={prepared}; answers={}; ordinary rows={:?}, ranges={:?}; selected rows={:?}, ranges={:?}, owned={}",
                selected.values.len(),
                ordinary.work.join_rows,
                ordinary.work.range_probes,
                selected.work.join_rows,
                selected.work.range_probes,
                selected.owned,
            );
            all_prepared &= prepared;
            all_intersected &= selected.owned
                && ordinary.work.range_probes.unwrap() > 0
                && selected.work.range_probes == Some(0)
                && selected.work.join_rows.unwrap() < ordinary.work.join_rows.unwrap();
        });
    }
    // Check both complete families before asserting the optimized route, so
    // a route regression still reports its complete tuples and work counts.
    assert!(
        all_prepared,
        "both authored orders prepare certified multiples"
    );
    assert!(
        all_intersected,
        "both use fewer rows through the range intersection"
    );
}

fn numeric_pairs(collected: &Collected, slots: [usize; 2]) -> Vec<(i32, i32)> {
    let mut pairs: Vec<_> = collected
        .values
        .iter()
        .map(|row| {
            assert_eq!(row.len(), 2);
            let (Value::Number(n), Value::Number(d)) = (&row[slots[0]], &row[slots[1]]) else {
                panic!("numeric fixture")
            };
            (*n, *d)
        })
        .collect();
    pairs.sort_unstable();
    pairs
}
