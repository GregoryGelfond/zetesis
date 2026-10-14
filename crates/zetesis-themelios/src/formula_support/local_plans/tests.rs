//! Reference, owner/input identity, nested fill and admission checks.
use super::*;
use crate::formula_support::{Join, testing};
use crate::{FormulaLimits, FormulaResource};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::panic::Location;

#[derive(Default)]
struct Counts {
    hits: usize,
    sites: BTreeMap<(&'static str, u32), usize>,
}
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(true) };
    static COUNTS: RefCell<Counts> = RefCell::new(Counts::default());
    static SITE: Cell<Option<&'static Location<'static>>> = const { Cell::new(None) };
}
pub(in crate::formula_support) fn enabled() -> bool {
    ENABLED.get()
}
pub(in crate::formula_support) fn hit() {
    COUNTS.with_borrow_mut(|counts| {
        counts.hits += 1;
        if let Some(site) = SITE.get() {
            *counts.sites.entry((site.file(), site.line())).or_default() += 1;
        }
    });
}
pub(in crate::formula_support) struct Site(Option<&'static Location<'static>>);
impl Drop for Site {
    fn drop(&mut self) {
        SITE.set(self.0);
    }
}
pub(in crate::formula_support) fn request(site: &'static Location<'static>) -> Site {
    Site(SITE.replace(Some(site)))
}
fn scoped<T>(enabled: bool, action: impl FnOnce() -> T) -> (T, Counts) {
    struct Restore(bool, Counts);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENABLED.set(self.0);
            COUNTS.with_borrow_mut(|counts| *counts = std::mem::take(&mut self.1));
        }
    }
    let restore = Restore(ENABLED.replace(enabled), COUNTS.take());
    let result = action();
    let counts = COUNTS.take();
    drop(restore);
    (result, counts)
}

#[test]
fn reference_control_is_scoped_and_thread_local() {
    assert!(enabled());
    let result = std::panic::catch_unwind(|| {
        scoped(false, || {
            assert!(!enabled());
            assert!(std::thread::spawn(enabled).join().unwrap());
            panic!("restore reference control");
        })
    });
    assert!(result.is_err());
    assert!(enabled());
}

#[test]
fn the_same_frame_and_variables_share_but_another_variable_count_does_not() {
    testing::with_completed_source(
        "d(1). d(2). d(3). h(X) :- d(X).",
        |prepared, support, computation, counters| {
            let rule = prepared
                .rules_without_facts()
                .iter()
                .find(|rule| !rule.body.is_empty())
                .unwrap();
            let limits = FormulaLimits::default();
            let location = rule.location;
            let empty = Binding::new(computation, &limits, counters, location).unwrap();
            let table = Table::new(support.workspace());
            let mut budget = testing::budget();
            let first = table
                .plan(
                    &rule.body,
                    &empty,
                    rule.variables,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            let again = table
                .plan(
                    &rule.body,
                    &empty,
                    rule.variables,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            assert!(Rc::ptr_eq(&first, &again));
            let other_variables = table
                .plan(
                    &rule.body,
                    &empty,
                    rule.variables + 1,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            assert!(!Rc::ptr_eq(&first, &other_variables));
        },
    );
}

#[test]
fn prefix_length_and_bound_bits_define_shape_but_values_do_not() {
    testing::with_completed_source(
        "d(1). d(2). d(3). h(X) :- d(X).",
        |prepared, support, computation, counters| {
            let rule = prepared
                .rules_without_facts()
                .iter()
                .find(|rule| !rule.body.is_empty())
                .unwrap();
            let limits = FormulaLimits::default();
            let location = rule.location;
            let mut budget = testing::budget();
            let one = testing::binding(
                &[Some(zetesis_core::Value::Number(1))],
                computation,
                counters,
                location,
            );
            let separate = Table::new(support.workspace());
            let bound = separate
                .plan(
                    &rule.body,
                    &one,
                    rule.variables,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            let two = testing::binding(
                &[Some(zetesis_core::Value::Number(2))],
                computation,
                counters,
                location,
            );
            let other_value = separate
                .plan(
                    &rule.body,
                    &two,
                    rule.variables,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            assert!(
                Rc::ptr_eq(&bound, &other_value),
                "bound values are not planner inputs"
            );
            let unbound = testing::binding(&[None], computation, counters, location);
            assert!(
                separate
                    .plan(
                        &rule.body,
                        &unbound,
                        rule.variables,
                        support,
                        &mut budget,
                        Context::new(computation, &limits, counters, location)
                    )
                    .unwrap()
                    .is_none(),
                "equal length with another bound bit cannot hit"
            );
            let empty = Binding::new(computation, &limits, counters, location).unwrap();
            assert!(
                separate
                    .plan(
                        &rule.body,
                        &empty,
                        rule.variables,
                        support,
                        &mut budget,
                        Context::new(computation, &limits, counters, location)
                    )
                    .unwrap()
                    .is_none(),
                "different prefix length cannot hit"
            );
            assert_eq!(
                separate.state.borrow().entries.len(),
                1,
                "another shape never replaces the first entry"
            );
        },
    );
}

#[test]
fn independent_tables_do_not_share_plan_ownership() {
    testing::with_completed_source(
        "d(1). h(X) :- d(X).",
        |prepared, support, computation, counters| {
            let rule = prepared
                .rules_without_facts()
                .iter()
                .find(|rule| !rule.body.is_empty())
                .unwrap();
            let limits = FormulaLimits::default();
            let location = rule.location;
            let empty = Binding::new(computation, &limits, counters, location).unwrap();
            let mut budget = testing::budget();
            let first = Table::new(support.workspace());
            let second = Table::new(support.workspace());
            let left = first
                .plan(
                    &rule.body,
                    &empty,
                    rule.variables,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            let right = second
                .plan(
                    &rule.body,
                    &empty,
                    rule.variables,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            assert!(!Rc::ptr_eq(&left, &right));
        },
    );
}

#[test]
fn live_shared_plan_allows_another_frame_to_fill_and_matches_fresh_order() {
    testing::with_completed_source(
        "d(1). d(2). d(3). h(X) :- d(X). k(X) :- d(X), X<3.",
        |prepared, support, computation, counters| {
            let rules: Vec<_> = prepared
                .rules_without_facts()
                .iter()
                .filter(|rule| !rule.body.is_empty())
                .collect();
            let limits = FormulaLimits::default();
            let location = rules[0].location;
            let empty = Binding::new(computation, &limits, counters, location).unwrap();
            let mut budget = testing::budget();
            let mut shared = Join::local(
                Ir::Source(rules[0].body.as_slice()),
                &empty,
                rules[0].variables,
                support,
                &mut budget,
                Context::new(computation, &limits, counters, location),
            )
            .unwrap();
            assert!(matches!(shared.plan, Plan::Shared(_)));
            let nested = Join::local(
                Ir::Source(rules[1].body.as_slice()),
                &empty,
                rules[1].variables,
                support,
                &mut budget,
                Context::new(computation, &limits, counters, location),
            )
            .unwrap();
            assert!(matches!(nested.plan, Plan::Shared(_)));
            let mut fresh = Join::new(
                &rules[0].body,
                &empty,
                rules[0].variables,
                support,
                &mut budget,
                Context::new(computation, &limits, counters, location),
            )
            .unwrap();
            assert_eq!(
                shared
                    .plan
                    .patterns
                    .iter()
                    .map(|pattern| pattern.source)
                    .collect::<Vec<_>>(),
                fresh
                    .plan
                    .patterns
                    .iter()
                    .map(|pattern| pattern.source)
                    .collect::<Vec<_>>()
            );
            for depth in 0..=shared.plan.patterns.len() {
                assert_eq!(
                    shared.plan.decisions.decided_at(depth).collect::<Vec<_>>(),
                    fresh.plan.decisions.decided_at(depth).collect::<Vec<_>>()
                );
            }
            loop {
                let left = shared
                    .next(computation, &limits, &mut budget, counters, location)
                    .unwrap();
                let left = left.map(|binding| {
                    binding
                        .read(0, computation.read(), location)
                        .unwrap()
                        .to_value(zetesis_core::ValueLimits::default())
                        .unwrap()
                });
                let right = fresh
                    .next(computation, &limits, &mut budget, counters, location)
                    .unwrap();
                let right = right.map(|binding| {
                    binding
                        .read(0, computation.read(), location)
                        .unwrap()
                        .to_value(zetesis_core::ValueLimits::default())
                        .unwrap()
                });
                assert_eq!(left, right, "complete binding order");
                if left.is_none() {
                    break;
                }
            }
        },
    );
}

#[test]
fn outer_and_empty_local_constructors_keep_owned_plans() {
    testing::with_completed_source(
        "d(1). h(X) :- d(X).",
        |prepared, support, computation, counters| {
            let rule = prepared
                .rules_without_facts()
                .iter()
                .find(|rule| !rule.body.is_empty())
                .unwrap();
            let limits = FormulaLimits::default();
            let location = rule.location;
            let empty = Binding::new(computation, &limits, counters, location).unwrap();
            let mut budget = testing::budget();
            let outer =
                Join::rule(rule, support, computation, &limits, &mut budget, counters).unwrap();
            assert!(
                matches!(outer.plan, Plan::Owned(_)),
                "outer constructor never consults the table"
            );
            let empty_local = Join::local(
                Ir::Source(&[] as &[LiteralIr]),
                &empty,
                0,
                support,
                &mut budget,
                Context::new(computation, &limits, counters, location),
            )
            .unwrap();
            assert!(matches!(empty_local.plan, Plan::Owned(_)));
        },
    );
}

#[test]
fn refused_or_unsafe_plan_never_publishes_an_entry() {
    testing::with_completed_source(
        "d(1). d(2). d(3). h(X) :- d(X).",
        |prepared, support, computation, counters| {
            let rule = prepared
                .rules_without_facts()
                .iter()
                .find(|rule| !rule.body.is_empty())
                .unwrap();
            let limits = FormulaLimits::default();
            let location = rule.location;
            let empty = Binding::new(computation, &limits, counters, location).unwrap();
            let table = Table::new(support.workspace());
            let mut budget = testing::budget();
            let restricted = FormulaLimits {
                max_support_bytes: support.live_bytes(),
                ..limits
            };
            let result = table.plan(
                &rule.body,
                &empty,
                rule.variables,
                support,
                &mut budget,
                Context::new(computation, &restricted, counters, location),
            );
            assert!(matches!(
                result,
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::SupportBytes,
                    ..
                })
            ));
            assert!(table.state.borrow().entries.is_empty());
            let unbound = testing::binding(&[None], computation, counters, location);
            let result = table.plan(
                &rule.body,
                &unbound,
                rule.variables,
                support,
                &mut budget,
                Context::new(computation, &limits, counters, location),
            );
            assert!(matches!(result, Err(FormulaFailure::UnsafeVariable { .. })));
            assert!(table.state.borrow().entries.is_empty());
        },
    );
}

#[test]
fn source_support_and_emission_are_identical_with_nested_reuse() {
    for source in [
        "d(1..3). {p(X,Y):d(Y)} :- d(X).",
        "d(1).d(2).d(3). {p(X,Y):d(Y)} :- d(X). q(X,A) :- d(X), A=#count{Y:p(X,Y)}.",
    ] {
        let compile =
            || crate::formula_ground::ground(testing::prepare(source), None, None).unwrap();
        let (shared, hits) = scoped(true, compile);
        let (reference, skipped) = scoped(false, compile);
        assert!(hits.hits > 0, "shared-plan path: {source}");
        assert_eq!(skipped.hits, 0);
        assert_eq!(
            crate::test_support::compiled_atoms(&shared),
            crate::test_support::compiled_atoms(&reference)
        );
        assert_eq!(shared.theory.nodes(), reference.theory.nodes());
        assert_eq!(shared.theory.operands(), reference.theory.operands());
        assert_eq!(shared.theory.roots(), reference.theory.roots());
        if source.contains("#count") {
            assert!(
                hits.sites
                    .keys()
                    .any(|(file, _)| file.ends_with("formula_assignment.rs")),
                "nested binding cursor reaches the shared table"
            );
        }
        let catalog =
            || {
                let mut atoms = Vec::new();
                testing::with_completed_source(source, |_, support, _, _| {
                    for (_, rows) in support.source_atoms() {
                        atoms.extend(rows.iter().map(|atom| {
                            atom.to_atom(zetesis_core::ValueLimits::default()).unwrap()
                        }));
                    }
                });
                atoms
            };
        assert_eq!(
            scoped(true, catalog).0,
            scoped(false, catalog).0,
            "support catalog and order"
        );
    }
}

#[test]
fn retained_payload_is_counted_once_and_entry_insertion_obeys_its_ceiling() {
    testing::with_completed_source(
        "d(1). h(X) :- d(X).",
        |prepared, support, computation, counters| {
            let rule = prepared
                .rules_without_facts()
                .iter()
                .find(|rule| !rule.body.is_empty())
                .unwrap();
            let location = rule.location;
            let limits = FormulaLimits::default();
            let empty = Binding::new(computation, &limits, counters, location).unwrap();
            let before = support.live_bytes();
            let table = Table::new(support.workspace());
            let mut budget = testing::budget();
            let plan = table
                .plan(
                    &rule.body,
                    &empty,
                    rule.variables,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            let payload =
                usize::try_from(plan.retained_bytes()).unwrap() + shared_header(location).unwrap();
            let retained = table.state.borrow().lease.bytes();
            assert_eq!(
                retained,
                payload + table.state.borrow().entries.capacity() * size_of::<Entry<'_>>()
            );
            assert_eq!(support.live_bytes(), before + retained);
            let borrowed = table
                .plan(
                    &rule.body,
                    &empty,
                    rule.variables,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            assert!(Rc::ptr_eq(&plan, &borrowed));
            assert_eq!(
                support.live_bytes(),
                before + retained,
                "a shared handle retains no second plan payload"
            );
            drop(borrowed);
            drop(plan);
            drop(table);
            assert_eq!(
                support.live_bytes(),
                before,
                "the snapshot table releases its complete receipt"
            );
            let table = Table::new(support.workspace());
            let needed = before + payload + size_of::<Entry<'_>>();
            let restricted = FormulaLimits {
                max_support_bytes: needed - 1,
                ..limits
            };
            let result = table.plan(
                &rule.body,
                &empty,
                rule.variables,
                support,
                &mut budget,
                Context::new(computation, &restricted, counters, location),
            );
            assert!(
                matches!(result, Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes, observed, limit, ..
        }) if observed == needed as u128 && limit == (needed - 1) as u128),
                "the exact one-entry request refuses at insertion"
            );
            assert!(table.state.borrow().entries.is_empty());
            assert_eq!(
                support.live_bytes(),
                before,
                "a refused entry releases its unpublished plan"
            );
        },
    );
}

#[test]
fn a_populated_cache_rejects_a_foreign_computation_before_its_hit() {
    testing::with_completed_source(
        "d(1). h(X) :- d(X).",
        |prepared, support, computation, counters| {
            let rule = prepared
                .rules_without_facts()
                .iter()
                .find(|rule| !rule.body.is_empty())
                .unwrap();
            let limits = FormulaLimits::default();
            let location = rule.location;
            let empty = Binding::new(computation, &limits, counters, location).unwrap();
            let mut budget = testing::budget();
            support
                .local_plan(
                    &rule.body,
                    &empty,
                    rule.variables,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            let observer = crate::test_support::Observer::default();
            let profile = crate::grounding_observer::Profile::new(Some(&observer));
            let mut foreign = testing::Fixture::default();
            foreign.with(location, |_, other, other_counters| {
                other_counters.observed = profile.work();
                let (error, hits) = scoped(true, || {
                    profile
                        .phase(crate::GroundingPhase::RuleInstantiation, None, || {
                            support.local_plan(
                                &rule.body,
                                &empty,
                                rule.variables,
                                &mut budget,
                                Context::new(other, &limits, other_counters, location),
                            )
                        })
                        .err()
                        .expect("foreign context rejected")
                });
                assert!(matches!(
                    error,
                    FormulaFailure::TermAssignment {
                        error: zetesis_core::catalog::AssignmentError::Read(
                            zetesis_core::catalog::ReadError::ForeignCatalog
                        ),
                        ..
                    }
                ));
                assert_eq!(hits.hits, 0, "authentication precedes a cache hit");
                assert_eq!(observer.0.get().local_plan_reuses, Some(0));
            });
            let same = support
                .local_plan(
                    &rule.body,
                    &empty,
                    rule.variables,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap();
            assert!(
                same.is_some(),
                "failed authentication preserves the original entry"
            );
        },
    );
}

#[test]
fn fixed_frame_owners_have_exact_transactional_work_boundaries() {
    testing::with_completed_source(
        "d(1). h(X) :- d(X). k(X) :- d(X), X<3.",
        |prepared, support, computation, counters| {
            let rules: Vec<_> = prepared
                .rules_without_facts()
                .iter()
                .filter(|rule| !rule.body.is_empty())
                .collect();
            assert_eq!(rules.len(), 2);
            let limits = FormulaLimits::default();
            let empty = Binding::new(computation, &limits, counters, rules[0].location).unwrap();
            // Both insertion directions use the same two live IR owners.
            // Re-preparing the source could reverse their address order.
            for order in [[0, 1], [1, 0]] {
                let mut attempt = |limit| {
                    let before = support.live_bytes();
                    let table = Table::new(support.workspace());
                    let mut budget = Budget::new(
                        crate::ExpansionLimits {
                            max_term_work: limit,
                            ..Default::default()
                        },
                        usize::MAX,
                    );
                    let mut published = 0;
                    let result = order.into_iter().try_for_each(|index| {
                        let rule = rules[index];
                        table
                            .plan(
                                &rule.body,
                                &empty,
                                rule.variables,
                                support,
                                &mut budget,
                                Context::new(computation, &limits, counters, rule.location),
                            )?
                            .expect("a fresh frame has no competing prefix shape");
                        published += 1;
                        Ok::<_, FormulaFailure>(())
                    });
                    assert_eq!(table.state.borrow().entries.len(), published);
                    let charged = budget.usage().term_work;
                    drop(table);
                    assert_eq!(support.live_bytes(), before, "all table storage released");
                    (result, charged, published)
                };
                let (result, charged, published) =
                    attempt(crate::ExpansionLimits::default().max_term_work);
                result.unwrap();
                assert_eq!(published, 2);
                let exact = charged;
                let (result, _, published) = attempt(exact - 1);
                let error = result.unwrap_err();
                assert!(
                    matches!(error, FormulaFailure::Expansion(
                    crate::ExpansionFailure::Limit {
                        resource: ExpansionResource::TermWork,
                        limit,
                        observed,
                        location,
                    }
                ) if limit == (exact - 1) as u128
                    && observed > limit
                    && location == rules[order[1]].location),
                    "{error}"
                );
                assert_eq!(published, 1, "the refused second entry was not published");
                let (result, charged, published) = attempt(exact);
                result.unwrap();
                assert_eq!(charged, exact);
                assert_eq!(published, 2);
            }
        },
    );
}
