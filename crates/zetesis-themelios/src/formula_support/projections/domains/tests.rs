use super::*;
use crate::ProgramSite;
use crate::formula_ir::HeadIr;
use crate::formula_support::testing::{Fixture, binding, budget, numbers, with_completed_source};
use crate::formula_support::{Counters, Coverage};
use crate::grounding_observer::Profile;
use crate::test_support::Observer;
use crate::{FormulaLimits, FormulaResource, GroundingPhase, GroundingWork};
use zetesis_core::catalog::{AssignmentError, ReadError};
use zetesis_core::{Value, ValueLimits};

fn with_join(
    source: &str,
    run: impl FnOnce(&mut Join<'_, '_>, &mut Computation<'_, '_>, &mut Counters, ProgramSite),
) {
    with_selected_join(source, false, run);
}

/// The constraint's join under its totality witness and, when `numeric`,
/// with the numeric probes eager emission selects as well.
fn with_selected_join(
    source: &str,
    numeric: bool,
    run: impl FnOnce(&mut Join<'_, '_>, &mut Computation<'_, '_>, &mut Counters, ProgramSite),
) {
    with_completed_source(source, |program, support, computation, counters| {
        let rule = program
            .rules_without_facts()
            .iter()
            .find(|rule| matches!(rule.head, HeadIr::Normal(None)))
            .unwrap();
        let limits = FormulaLimits::default();
        let mut join =
            Join::rule(rule, support, computation, &limits, &mut budget(), counters).unwrap();
        assert!(
            join.select_total_body(rule, computation, &limits, counters)
                .unwrap()
        );
        assert!(join.certified_total);
        if numeric {
            join.select_numeric_probes(rule, &limits, counters).unwrap();
            assert!(join.expression_keys.is_some());
        }
        run(&mut join, computation, counters, rule.location);
    });
}

fn collect(
    join: &mut Join<'_, '_>,
    computation: &mut Computation<'_, '_>,
    counters: &mut Counters,
    location: ProgramSite,
) -> (Vec<Vec<Value>>, GroundingWork) {
    collect_under(
        join,
        computation,
        counters,
        location,
        &FormulaLimits::default(),
    )
}

fn collect_under(
    join: &mut Join<'_, '_>,
    computation: &mut Computation<'_, '_>,
    counters: &mut Counters,
    location: ProgramSite,
    limits: &FormulaLimits,
) -> (Vec<Vec<Value>>, GroundingWork) {
    try_collect_under(join, computation, counters, location, limits).unwrap()
}

fn try_collect_under(
    join: &mut Join<'_, '_>,
    computation: &mut Computation<'_, '_>,
    counters: &mut Counters,
    location: ProgramSite,
    limits: &FormulaLimits,
) -> Result<(Vec<Vec<Value>>, GroundingWork), FormulaFailure> {
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    counters.observed = profile.work();
    let rows = profile.phase(GroundingPhase::RuleInstantiation, Some(location), || {
        let mut rows = Vec::new();
        while let Some(row) =
            join.next_row(computation, limits, &mut budget(), counters, location)?
        {
            if row.passes {
                rows.push(
                    (0..row.values.len())
                        .map(|slot| {
                            row.values
                                .read(slot, computation.read(), location)
                                .unwrap()
                                .to_value(ValueLimits::default())
                                .unwrap()
                        })
                        .collect(),
                );
            }
        }
        Ok::<_, FormulaFailure>(rows)
    })?;
    Ok((rows, observer.0.get()))
}

#[test]
fn computed_groups_keep_each_matching_input() {
    with_join(
        "p(1..6).q(2). :-p(X),q(Y),X/2=Y.",
        |join, computation, counters, location| {
            let (rows, work) = collect(join, computation, counters, location);
            assert_eq!(rows, vec![numbers(&[4, 2]), numbers(&[5, 2])]);
            assert_eq!(work.table_preparations, Some(1));
            assert_eq!(work.table_probes, Some(1));
            assert_eq!(work.join_rows, Some(3));
        },
    );
}

#[test]
fn a_relation_within_the_table_bound_is_selected_through_its_table() {
    // 1000 distinct values over 1000 rows: 1000 masks of 32 words, within
    // the 65536 words a table may take to narrow a probe.
    with_join(
        "p(1..1000).q(2). :-p(X),q(Y),X/2=Y.",
        |join, computation, counters, location| {
            let (rows, work) = collect(join, computation, counters, location);
            assert_eq!(rows, vec![numbers(&[4, 2]), numbers(&[5, 2])]);
            assert_eq!(work.table_preparations, Some(1));
            assert_eq!(work.join_rows, Some(3));
        },
    );
}

#[test]
fn a_relation_past_the_table_bound_is_scanned() {
    // A table keeps a row mask for each value of each column: for the 3000
    // distinct values of `p` that is 3000 masks of 94 words, far more than
    // the relation itself. The selection is declined, the join scans the
    // rows, and the equality still selects the same two.
    with_join(
        "p(1..3000).q(2). :-p(X),q(Y),X/2=Y.",
        |join, computation, counters, location| {
            let (rows, work) = collect(join, computation, counters, location);
            assert_eq!(rows, vec![numbers(&[4, 2]), numbers(&[5, 2])]);
            assert_eq!(work.table_preparations, Some(0));
            assert_eq!(work.table_index_bytes, Some(0));
        },
    );
}

#[test]
fn a_column_without_postings_counts_a_value_for_each_row() {
    // No join binds the second column of `p`, so it has no postings to
    // count and is taken to hold 3000 values: with the two of the first
    // column that is far past the bound, and the relation is scanned.
    with_join(
        "p(1..2,1..1500).q(1). :-p(X,Z),q(Y),X/2=Y.",
        |join, computation, counters, location| {
            let (rows, work) = collect(join, computation, counters, location);
            assert_eq!(rows.len(), 1500);
            assert_eq!(work.table_preparations, Some(0));
        },
    );
}

#[test]
fn a_probe_repeated_for_outer_rows_gets_its_table_once_its_scans_repay_it() {
    // The 3000 distinct values of `p` want 3000 masks of 94 words, 282000
    // words, and a scanned row weighs eight: 35250 rows repay them, which
    // twelve scans of `p` pass. The thirteenth row of `m` prepares the
    // table and the seven after it use it.
    with_join(
        "p(1..3000).m(1..20). :-m(Y),p(X),X/3=Y.",
        |join, computation, counters, location| {
            let (rows, work) = collect(join, computation, counters, location);
            assert_eq!(rows.len(), 60);
            assert_eq!(work.table_inapplicable_probes, Some(12));
            assert_eq!(work.table_preparations, Some(1));
            assert_eq!(work.table_reuses, Some(7));
        },
    );
}

#[test]
fn twelve_scans_do_not_repay_the_table() {
    with_join(
        "p(1..3000).m(1..12). :-m(Y),p(X),X/3=Y.",
        |join, computation, counters, location| {
            let (rows, work) = collect(join, computation, counters, location);
            assert_eq!(rows.len(), 36);
            assert_eq!(work.table_inapplicable_probes, Some(12));
            assert_eq!(work.table_preparations, Some(0));
        },
    );
}

#[test]
fn a_declined_selection_served_by_a_numeric_probe_counts_no_scan() {
    // Eager emission also selects numeric probes. The equality's postings
    // then serve each declined probe, no row is read in the table's place,
    // and twenty outer rows prepare no table where thirteen scans would.
    with_selected_join(
        "p(1..3000).m(1..20). :-m(Y),p(X),X/3=Y.",
        true,
        |join, computation, counters, location| {
            let (rows, work) = collect(join, computation, counters, location);
            assert_eq!(rows.len(), 60);
            assert_eq!(work.table_inapplicable_probes, Some(20));
            assert_eq!(work.table_preparations, Some(0));
        },
    );
}

#[test]
fn joins_of_one_snapshot_repay_one_table_together() {
    // Twelve scans of `p` do not repay its table, and one join makes twelve.
    // The snapshot keeps the count, so a second join over it finds the table
    // repaid at its first probe, prepares it, and uses it for the other
    // eleven.
    with_completed_source(
        "p(1..3000).m(1..12). :-m(Y),p(X),X/3=Y.",
        |program, support, computation, counters| {
            let rule = program
                .rules_without_facts()
                .iter()
                .find(|rule| matches!(rule.head, HeadIr::Normal(None)))
                .unwrap();
            let limits = FormulaLimits::default();
            let mut declined = Vec::new();
            let mut prepared = Vec::new();
            let mut reused = Vec::new();
            for _ in 0..2 {
                let mut join =
                    Join::rule(rule, support, computation, &limits, &mut budget(), counters)
                        .unwrap();
                assert!(
                    join.select_total_body(rule, computation, &limits, counters)
                        .unwrap()
                );
                let (rows, work) = collect(&mut join, computation, counters, rule.location);
                assert_eq!(rows.len(), 36);
                declined.push(work.table_inapplicable_probes);
                prepared.push(work.table_preparations);
                reused.push(work.table_reuses);
            }
            assert_eq!(declined, [Some(12), Some(0)]);
            assert_eq!(prepared, [Some(0), Some(1)]);
            assert_eq!(reused, [Some(0), Some(11)]);
        },
    );
}

#[test]
fn rows_read_over_one_relation_repay_no_table_of_another() {
    // Eleven scans of `e` read 33000 rows, short of the 35250 its table
    // costs. `f` has a table of the same cost and a count of its own: three
    // probes read 9000 rows and prepare nothing. One count for both would
    // prepare the table of `f` at its second probe.
    with_completed_source(
        "e(1..3000).f(1..3000).m(1..11).n(1..3). :-m(Y),e(X),X/3=Y. :-n(Y),f(X),X/3=Y.",
        |program, support, computation, counters| {
            let limits = FormulaLimits::default();
            let mut declined = Vec::new();
            let mut prepared = Vec::new();
            for rule in program
                .rules_without_facts()
                .iter()
                .filter(|rule| matches!(rule.head, HeadIr::Normal(None)))
            {
                let mut join =
                    Join::rule(rule, support, computation, &limits, &mut budget(), counters)
                        .unwrap();
                assert!(
                    join.select_total_body(rule, computation, &limits, counters)
                        .unwrap()
                );
                let (_, work) = collect(&mut join, computation, counters, rule.location);
                declined.push(work.table_inapplicable_probes);
                prepared.push(work.table_preparations);
            }
            assert_eq!(declined, [Some(11), Some(3)]);
            assert_eq!(prepared, [Some(0), Some(0)]);
        },
    );
}

#[test]
fn a_declined_probe_counts_the_rows_its_replacement_offers() {
    // `e` holds 4000 rows and 2002 values in its two columns: 2002 masks of
    // 125 words, which 31282 rows repay. Each probe has the first column
    // bound, and the posting that replaces a declined selection offers the
    // 2000 rows of that value: sixteen probes repay the table and the
    // seventeenth of forty prepares it. Counting the relation's 4000 rows
    // for each would prepare it at the ninth.
    with_join(
        "e(1..2,1..2000).m(1..20,1..2). :-m(Z,W),e(W,X),X/3=Z.",
        |join, computation, counters, location| {
            let (rows, work) = collect(join, computation, counters, location);
            assert_eq!(rows.len(), 120);
            assert_eq!(work.table_inapplicable_probes, Some(16));
            assert_eq!(work.table_preparations, Some(1));
            assert_eq!(work.table_reuses, Some(23));
        },
    );
}

#[test]
fn no_byte_ceiling_turns_a_repaid_table_into_a_refusal() {
    // Six scans of `p` repay its table, whose 1500 masks of 47 words take
    // 282000 bytes. The table's preparation grows them by doubling to 2048
    // masks and is charged 577536 bytes at the last growth, the buffer it
    // replaces still live. A ceiling that leaves room for the masks and not
    // for that growth must leave the join to its scans, not refuse it.
    let run = |max_support_bytes: usize| {
        let mut outcome = None;
        with_join(
            "p(1..1500).m(1..8). :-m(Y),p(X),X/3=Y.",
            |join, computation, counters, location| {
                let limits = FormulaLimits {
                    max_support_bytes,
                    ..FormulaLimits::default()
                };
                outcome = Some(
                    try_collect_under(join, computation, counters, location, &limits)
                        .map(|(rows, work)| (rows.len(), work.table_preparations)),
                );
            },
        );
        outcome.unwrap()
    };
    // The least ceiling, to a kibibyte, under which the join completes.
    let (mut refused, mut admitted) = (0_usize, 64 << 20);
    assert!(run(admitted).is_ok());
    while refused + 1024 < admitted {
        let middle = refused + (admitted - refused) / 2;
        if run(middle).is_ok() {
            admitted = middle;
        } else {
            refused = middle;
        }
    }
    for room in [300_000, 430_000, 560_000] {
        assert!(
            matches!(run(admitted + room), Ok((24, Some(0)))),
            "room {room}"
        );
    }
    // Around what the preparation is weighed at, the table comes to be
    // prepared, and is never refused on the way.
    let mut prepared = false;
    for room in (640_000..=1_280_000).step_by(64_000) {
        match run(admitted + room) {
            Ok((24, Some(0))) => assert!(!prepared, "room {room}"),
            Ok((24, Some(1))) => prepared = true,
            other => panic!("room {room}: {other:?}"),
        }
    }
    assert!(prepared);
}

#[test]
fn a_table_that_cannot_be_admitted_is_never_prepared() {
    // The table wants 3000 support entries and at most 2999 are left:
    // every probe scans, where an attempt would refuse the join.
    with_join(
        "p(1..3000).m(1..20). :-m(Y),p(X),X/3=Y.",
        |join, computation, counters, location| {
            let limits = FormulaLimits {
                max_support_index_entries: 2_999,
                ..FormulaLimits::default()
            };
            let (rows, work) = collect_under(join, computation, counters, location, &limits);
            assert_eq!(rows.len(), 60);
            assert_eq!(work.table_inapplicable_probes, Some(20));
            assert_eq!(work.table_preparations, Some(0));
        },
    );
}

#[test]
fn additional_equalities_remain_residual_checks() {
    with_join(
        "p(1..6).q(2). :-p(X),q(Y),X/2=Y,X\\2=1.",
        |join, computation, counters, location| {
            let (rows, work) = collect(join, computation, counters, location);
            assert_eq!(rows, vec![numbers(&[5, 2])]);
            assert!(work.table_probes.unwrap() > 0);
        },
    );
}

#[test]
fn covering_columns_preserve_complete_bindings() {
    with_join(
        "a(2,1..10).a(4,1..10).b(2;4;5).q(2). :-q(K),b(X),a(X,Y),X/2=K.",
        |join, computation, counters, location| {
            let (rows, work) = collect(join, computation, counters, location);
            // Normalized body order introduces a(X,Y) before q(K): the
            // retained assignment columns are X, Y, K, not authored K, X, Y.
            assert!(matches!(
                target(join, "a").terms().at(0),
                Some(TemplateTerm::Variable(0))
            ));
            assert!(matches!(
                target(join, "a").terms().at(1),
                Some(TemplateTerm::Variable(1))
            ));
            assert!(matches!(
                target(join, "q").terms().at(0),
                Some(TemplateTerm::Variable(2))
            ));
            let expected: Vec<_> = (1..=10).map(|y| numbers(&[4, y, 2])).collect();
            assert_eq!(rows, expected);
            assert!(work.table_probes.unwrap() > 0);
        },
    );
}

#[test]
fn computed_groups_preserve_repeated_columns() {
    with_join(
        "p(4,4).p(4,5).p(5,5).p(6,6).q(2). :-p(X,X),q(Y),X/2=Y,X<5.",
        |join, computation, counters, location| {
            // Exercise computed selection with Y known and X still free,
            // independently of the planner's comparison-readiness preference.
            assert_eq!(join.depth, 0);
            assert!(join.probes.iter().all(Option::is_none));
            join.owned_plan()
                .patterns
                .sort_by_key(|occurrence| occurrence.atom().predicate().name() != "q");
            join.decide(
                computation,
                &FormulaLimits::default(),
                &mut budget(),
                counters,
                location,
            )
            .unwrap();
            assert_eq!(
                join.plan
                    .patterns
                    .iter()
                    .map(|occurrence| occurrence.atom().predicate().name())
                    .collect::<Vec<_>>(),
                ["q", "p"],
            );
            let (rows, work) = collect(join, computation, counters, location);
            assert_eq!(rows, vec![numbers(&[4, 2])]);
            assert_eq!(work.table_probes, Some(1));
        },
    );
}

#[test]
fn typed_known_values_are_not_coerced() {
    with_join(
        "p(1..6).q(2;\"2\";two). :-p(X),q(Y),X/2=Y.",
        |join, computation, counters, location| {
            let (rows, work) = collect(join, computation, counters, location);
            assert_eq!(rows, vec![numbers(&[4, 2]), numbers(&[5, 2])]);
            assert_eq!(work.table_probes, Some(1));
        },
    );
}

#[test]
fn all_allowed_groups_keep_indexed_probes() {
    with_join(
        "p(4;5).q(2). :-p(X),q(Y),X/2=Y.",
        |join, computation, counters, location| {
            let values = binding(
                &[None, Some(Value::Number(2))],
                computation,
                counters,
                location,
            );
            let limits = FormulaLimits::default();
            let witness = computation.lease();
            let admitted = limits.max_support_bytes
                - computation.allowance(&witness, &limits, location).unwrap();
            let bounded = FormulaLimits {
                max_support_bytes: admitted,
                ..limits
            };
            // No spare storage is available for even a temporary domain header.
            assert!(
                join.projections
                    .domains(
                        join.literals.get(),
                        target(join, "p"),
                        &values,
                        Context::new(&*computation, &bounded, counters, location),
                    )
                    .unwrap()
                    .is_none()
            );
            let (rows, work) = collect(join, computation, counters, location);
            assert_eq!(rows, vec![numbers(&[4, 2]), numbers(&[5, 2])]);
            assert_eq!(work.table_preparations, Some(0));
            assert_eq!(work.table_probes, Some(0));
        },
    );
}

#[test]
fn matching_prefix_is_not_compared_twice() {
    with_join(
        "p(0..63).q(0;1). :-p(X),q(Y),X/32=Y.",
        |join, computation, counters, location| {
            let limits = FormulaLimits::default();
            assert_eq!(join.projections.values.len(), 1);
            let projection = &join.projections.values[0];
            assert_eq!(projection.covered, Some(64));
            let zero = computation.number(0, &limits, counters, location).unwrap();
            let zero = computation.read().term(&zero).unwrap();
            // Establish the actual covered order, not merely the source order:
            // one query has a long matching prefix, the other none at all.
            for position in 0..64 {
                assert_eq!(
                    projection
                        .matches(
                            position,
                            zero,
                            &mut Context::new(&*computation, &limits, counters, location),
                        )
                        .unwrap(),
                    position < 32,
                );
            }
            let mut work = Vec::new();
            for wanted in 0..2 {
                let values = binding(
                    &[None, Some(Value::Number(wanted))],
                    computation,
                    counters,
                    location,
                );
                let live = counters.workspace_bytes();
                let before = counters.accounting.work;
                let domains = join
                    .projections
                    .domains(
                        join.literals.get(),
                        target(join, "p"),
                        &values,
                        Context::new(&*computation, &limits, counters, location),
                    )
                    .unwrap()
                    .unwrap();
                work.push(counters.accounting.work - before);
                let actual: Vec<_> = domains
                    .borrowed()
                    .values
                    .iter()
                    .map(|value| value.to_value(ValueLimits::default()).unwrap())
                    .collect();
                let expected: Vec<_> = (wanted * 32..(wanted + 1) * 32)
                    .map(Value::Number)
                    .collect();
                assert_eq!(actual, expected);
                drop(domains);
                assert_eq!(counters.workspace_bytes(), live);
            }
            // Both queries compare every covered output once and emit 32 inputs.
            // Repeating the checked prefix makes the first receipt larger.
            assert_eq!(work[0], work[1]);
        },
    );
}

#[test]
fn source_evidence_retains_complete_row_coverage() {
    with_join(
        "p(1..6).q(2). :-p(X),q(Y),X/2=Y.",
        |join, computation, counters, location| {
            join.evidence();
            assert!(join.coverage == Coverage::Complete);
            let (_, work) = collect(join, computation, counters, location);
            assert_eq!(work.join_rows, Some(7));
            assert_eq!(work.table_preparations, Some(0));
            assert_eq!(work.table_probes, Some(0));
        },
    );
}

fn target<'a>(join: &Join<'a, '_>, name: &str) -> PatternRef<'a> {
    join.plan
        .patterns
        .iter()
        .find(|pattern| pattern.atom().predicate().name() == name)
        .unwrap()
        .atom()
}

#[test]
fn uncovered_known_inputs_decline_selection() {
    with_join(
        "a(0;1).b(1,1..10).c(1..4). :-a(K),b(K,Y),c(X),1/K=X/2.",
        |join, computation, counters, location| {
            let values = binding(
                &[Some(Value::Number(0)), None, None],
                computation,
                counters,
                location,
            );
            let domains = join
                .projections
                .domains(
                    join.literals.get(),
                    target(join, "c"),
                    &values,
                    Context::new(&*computation, &FormulaLimits::default(), counters, location),
                )
                .unwrap();
            assert!(domains.is_none());
            assert!(join.failure.is_none());
        },
    );
}

#[test]
fn foreign_known_values_cannot_select_local_rows() {
    let mut foreign = Fixture::default();
    with_join(
        "p(1..6).q(2). :-p(X),q(Y),X/2=Y.",
        |join, computation, counters, location| {
            let values = foreign.with(location, |_, foreign_computation, foreign_counters| {
                binding(
                    &[None, Some(Value::Number(2))],
                    foreign_computation,
                    foreign_counters,
                    location,
                )
            });
            assert!(matches!(
                join.projections.domains(
                    join.literals.get(),
                    target(join, "p"),
                    &values,
                    Context::new(&*computation, &FormulaLimits::default(), counters, location)
                ),
                Err(FormulaFailure::TermAssignment {
                    error: AssignmentError::Read(ReadError::ForeignCatalog),
                    ..
                })
            ));
        },
    );
}

#[test]
fn domain_work_cutoffs_are_inclusive() {
    with_join(
        "p(0..6).q(0). :-p(X),q(Y),X/6=Y.",
        |join, computation, counters, location| {
            let values = binding(
                &[None, Some(Value::Number(0))],
                computation,
                counters,
                location,
            );
            let pattern = target(join, "p");
            let limits = FormulaLimits::default();
            let before = counters.accounting.work;
            let domains = join
                .projections
                .domains(
                    join.literals.get(),
                    pattern,
                    &values,
                    Context::new(&*computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            assert_eq!(domains.borrowed().values.len(), 6);
            drop(domains);
            let required = counters.accounting.work - before;
            assert!(required > 0);
            let live = counters.workspace_bytes();
            for cutoff in 0..=required {
                let bounded = FormulaLimits {
                    max_work: counters.accounting.work + cutoff,
                    ..limits
                };
                let result = join.projections.domains(
                    join.literals.get(),
                    pattern,
                    &values,
                    Context::new(&*computation, &bounded, counters, location),
                );
                if cutoff == required {
                    assert_eq!(result.unwrap().unwrap().borrowed().values.len(), 6);
                } else {
                    assert!(matches!(
                        result,
                        Err(FormulaFailure::Limit {
                            resource: FormulaResource::Work,
                            location: actual,
                            ..
                        }) if actual == location
                    ));
                }
                assert!(counters.accounting.work <= bounded.max_work);
                assert_eq!(counters.workspace_bytes(), live);
            }
        },
    );
}

#[test]
fn cancelled_domains_cannot_publish_a_restriction() {
    with_join(
        "p(1..6).q(2). :-p(X),q(Y),X/2=Y.",
        |join, computation, counters, location| {
            let values = binding(
                &[None, Some(Value::Number(2))],
                computation,
                counters,
                location,
            );
            let cancellation = zetesis_algebra::Cancellation::default();
            cancellation.cancel();
            counters.attach_test_control(&cancellation);
            assert!(matches!(
                join.projections.domains(
                    join.literals.get(),
                    target(join, "p"),
                    &values,
                    Context::new(&*computation, &FormulaLimits::default(), counters, location)
                ),
                Err(FormulaFailure::Interrupted {
                    reason: zetesis_algebra::Stop::Cancelled,
                    ..
                })
            ));
        },
    );
}
