use super::*;

fn holds(value: i32, relation: Relation, bound: i32) -> bool {
    match relation {
        Relation::Lt => value < bound,
        Relation::Le => value <= bound,
        Relation::Gt => value > bound,
        Relation::Ge => value >= bound,
        _ => unreachable!("ordering fixture"),
    }
}

#[test]
fn range_unions_keep_source_order_endpoints_and_delta_cuts() {
    // Insertion/source order differs from key order, with duplicate keys and
    // integer endpoints that cannot safely be incremented or decremented.
    let (mut fixture, pattern) = fixture(&[
        [3, 0],
        [i32::MIN, 1],
        [1, 2],
        [3, 3],
        [i32::MAX, 4],
        [-2, 5],
    ]);
    fixture.with(location(), |support, computation, counters| {
        let limits = FormulaLimits::default();
        let pattern = computation
            .static_pattern(pattern, &limits, counters, location())
            .unwrap();
        let source = support
            .resolve(pattern, &limits, counters, location())
            .unwrap()
            .unwrap();
        let key = Expression::new(vec![Operation::Variable(0)]);
        for relation in [Relation::Lt, Relation::Le, Relation::Gt, Relation::Ge] {
            for bound in [i32::MIN, -2, 1, 3, i32::MAX] {
                for cut in [0..source.row_count(), 1..4, 4..4] {
                    let selected = support
                        .range_key(
                            source,
                            &key,
                            pattern,
                            (relation, bound),
                            cut.clone(),
                            Context::new(computation, &limits, counters, location()),
                        )
                        .unwrap()
                        .unwrap();
                    let expected = cut
                        .filter(|&position| {
                            let value = number(
                                source.row(position).unwrap().value(0).unwrap().descriptor(),
                            )
                            .unwrap();
                            holds(value, relation, bound)
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(selected.slice(), expected, "{relation:?} {bound}");
                    assert!(selected.slice().windows(2).all(|pair| pair[0] < pair[1]));
                }
            }
        }
        let equal = support
            .expression_key(
                source,
                &key,
                pattern,
                3,
                0..source.row_count(),
                Context::new(computation, &limits, counters, location()),
            )
            .unwrap()
            .unwrap();
        assert_eq!(positions(&equal), [0, 3]);
        assert_eq!(
            support
                .expression_keys
                .cache
                .borrow()
                .as_ref()
                .unwrap()
                .entries
                .len(),
            1,
            "range and equality share the same checked column descriptor"
        );
    });
}

#[test]
fn range_scratch_is_leased_and_released_after_cache_publication() {
    let (mut fixture, pattern) = fixture(&[[3, 0], [1, 1], [2, 2]]);
    fixture.with(location(), |support, computation, counters| {
        let limits = FormulaLimits::default();
        let pattern = computation
            .static_pattern(pattern, &limits, counters, location())
            .unwrap();
        let source = support
            .resolve(pattern, &limits, counters, location())
            .unwrap()
            .unwrap();
        let key = Expression::new(vec![Operation::Variable(0)]);
        assert!(
            support
                .expression_key(
                    source,
                    &key,
                    pattern,
                    1,
                    0..3,
                    Context::new(computation, &limits, counters, location()),
                )
                .unwrap()
                .is_some()
        );
        let before = support.live_bytes();
        let selected = support
            .range_key(
                source,
                &key,
                pattern,
                (Relation::Le, 2),
                0..3,
                Context::new(computation, &limits, counters, location()),
            )
            .unwrap()
            .unwrap();
        assert_eq!(selected.slice(), [1, 2]);
        assert_eq!(
            support.live_bytes() - before,
            selected.capacity() * size_of::<usize>(),
            "the preallocated Probe cell owns the returned buffer header"
        );
        drop(selected);
        assert_eq!(support.live_bytes(), before);
        let restricted = FormulaLimits {
            max_support_bytes: before,
            ..limits
        };
        assert!(matches!(
            support.range_key(
                source,
                &key,
                pattern,
                (Relation::Le, 2),
                0..3,
                Context::new(computation, &restricted, counters, location())
            ),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                ..
            })
        ));
        assert_eq!(
            support.live_bytes(),
            before,
            "failed union retention leaks no scratch"
        );
    });
}

#[test]
fn a_range_over_mixed_column_values_declines_the_whole_index() {
    let mut fixture = Fixture::from_atoms(
        [Value::Number(1), Value::Symbol("s".into())]
            .map(|value| Atom::new(Predicate::new("p", 1).unwrap(), vec![value]).unwrap()),
        location(),
    );
    let pattern = fixture.pattern(
        &AtomPattern::new(Predicate::new("p", 1).unwrap(), vec![Term::Variable(0)]).unwrap(),
        location(),
    );
    fixture.with(location(), |support, computation, counters| {
        let limits = FormulaLimits::default();
        let pattern = computation
            .static_pattern(pattern, &limits, counters, location())
            .unwrap();
        let source = support
            .resolve(pattern, &limits, counters, location())
            .unwrap()
            .unwrap();
        let key = Expression::new(vec![Operation::Variable(0)]);
        assert!(
            support
                .range_key(
                    source,
                    &key,
                    pattern,
                    (Relation::Gt, 0),
                    0..2,
                    Context::new(computation, &limits, counters, location())
                )
                .unwrap()
                .is_none()
        );
        let cache = support.expression_keys.cache.borrow();
        let entries = &cache.as_ref().unwrap().entries;
        assert_eq!(entries.len(), 1);
        assert!(entries[0].integer.is_none());
    });
}
