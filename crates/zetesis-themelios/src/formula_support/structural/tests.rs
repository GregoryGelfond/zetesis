use std::cell::Cell;

use super::*;
use crate::formula_ir::HeadIr;
use crate::formula_support::testing;
use crate::grounding_observer::{GroundingPhase, Profile};
use crate::test_support::Observer;
use crate::{FormulaLimits, FormulaResource, GroundingWork as Receipt};
use zetesis_core::Value;

mod reuse;

thread_local! {
    static REUSE: Cell<(bool, usize)> = const { Cell::new((true, 0)) };
    static CONTROL: Cell<(bool, usize)> = const { Cell::new((true, 0)) };
}

pub(super) fn enabled() -> bool {
    CONTROL.get().0
}
pub(super) fn record() {
    CONTROL.set((enabled(), CONTROL.get().1 + 1));
}
fn scoped<T>(enabled: bool, run: impl FnOnce() -> T) -> (T, usize) {
    struct Restore((bool, usize));
    impl Drop for Restore {
        fn drop(&mut self) {
            CONTROL.set(self.0);
        }
    }
    let _restore = Restore(CONTROL.replace((enabled, 0)));
    let result = run();
    (result, CONTROL.get().1)
}

struct Run {
    rows: Vec<Vec<String>>,
    receipt: Receipt,
    work: u64,
}

fn query_rows(
    program: &crate::formula_ir::Prepared,
    support: &super::super::Support<'_>,
    computation: &mut Computation<'_, '_>,
    counters: &mut super::super::Counters,
) -> Run {
    query_rule(
        program.rules.last().unwrap(),
        support,
        computation,
        counters,
    )
}

fn query_rule(
    rule: &crate::formula_ir::RuleIr,
    support: &super::super::Support<'_>,
    computation: &mut Computation<'_, '_>,
    counters: &mut super::super::Counters,
) -> Run {
    let location = rule.location;
    let limits = FormulaLimits::default();
    let HeadIr::Normal(Some(head)) = rule.head else {
        panic!("normal fixture")
    };
    let head = computation
        .static_pattern(head, &limits, counters, location)
        .unwrap();
    let mut budget = testing::budget();
    let mut join = Join::rule(rule, support, computation, &limits, &mut budget, counters).unwrap();
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    let previous = std::mem::replace(&mut counters.observed, profile.work());
    let before = counters.accounting.work;
    let mut rows = profile
        .phase(GroundingPhase::RuleInstantiation, Some(location), || {
            let mut rows = Vec::new();
            while let Some(row) = join
                .next_row(computation, &limits, &mut budget, counters, location)
                .unwrap()
            {
                if row.passes {
                    rows.push(
                        head.terms()
                            .iter()
                            .map(|term| {
                                row.values
                                    .resolve(term, computation.read(), location)
                                    .unwrap()
                                    .to_string()
                            })
                            .collect::<Vec<_>>(),
                    );
                }
            }
            Ok::<_, FormulaFailure>(rows)
        })
        .unwrap();
    join.take_family().finish().unwrap();
    assert_eq!(join.lease.bytes(), join.storage_bytes());
    cleared(&join);
    rows.sort();
    counters.observed = previous;
    Run {
        rows,
        receipt: observer.0.get(),
        work: counters.accounting.work - before,
    }
}

fn cleared(join: &Join<'_, '_>) {
    if let Some(scratch) = &join.structural_query {
        assert_eq!(scratch.stack.len(), 0);
        assert_eq!(scratch.arguments.len(), 0);
        assert_eq!(scratch.children.len(), 0);
    }
}

fn run(source: &str, enabled: bool) -> (Run, usize) {
    scoped(enabled, || {
        let mut output = None;
        testing::with_completed_source(source, |program, support, computation, counters| {
            output = Some(query_rows(program, support, computation, counters));
        });
        output.expect("completed source callback")
    })
}

#[test]
fn exact_arguments_use_existing_structural_postings() {
    let source = "k(1..2).p(other(1..100)).p(f(1)).p(f(2)).r(X):-k(X),p(f(X)).";
    let (selected, attempts) = run(source, true);
    let (reference, skipped) = run(source, false);
    assert!(attempts > 0);
    assert_eq!(skipped, 0);
    assert_eq!(selected.rows, vec![vec!["1"], vec!["2"]]);
    assert_eq!(selected.rows, reference.rows);
    assert_eq!(selected.receipt.join_rows, Some(2 + 2));
    assert_eq!(reference.receipt.join_rows, Some(2 + 2 * 102));
    assert_eq!(selected.receipt.unindexed_probes, Some(0));
    assert!(selected.receipt.indexed_probes.unwrap() > 0);
    assert!(selected.work < reference.work);
}

#[test]
fn nested_arguments_preserve_constants_order_and_aliases() {
    let source = "k(1..2).p(other(1..20)).p(f(g(1),1,7)).p(f(g(2),2,7)). \
                  p(f(g(1),2,7)).p(f(g(1),1,8)).p(f(g(1),7,1)). \
                  r(X):-k(X),p(f(g(X),X,7)).";
    let (selected, attempts) = run(source, true);
    let (reference, _) = run(source, false);
    assert!(attempts > 0);
    assert_eq!(selected.rows, vec![vec!["1"], vec!["2"]]);
    assert_eq!(selected.rows, reference.rows);
    assert_eq!(selected.receipt.join_rows, Some(4));
}

#[test]
fn missing_constructor_identity_is_an_empty_domain() {
    for argument in ["f(X)", "f(g(X))"] {
        let source = format!("k(1..2).p(other(1..20)).r(X):-k(X),p({argument}).");
        let (selected, attempts) = run(&source, true);
        let (reference, _) = run(&source, false);
        assert!(attempts > 0);
        assert!(selected.rows.is_empty());
        assert_eq!(selected.rows, reference.rows);
        assert_eq!(selected.receipt.join_rows, Some(2));
        assert_eq!(reference.receipt.join_rows, Some(2 + 2 * 20));
    }
}

#[test]
fn incomplete_shapes_keep_the_matcher_route() {
    for source in [
        "k(1..2).p(f(1..3)).r(K,X):-k(K),p(f(X)).",
        "k(1..2).p(f(1,0)).p(f(2,0)).p(other(0)).r(K):-k(K),p(f(K,_)).",
    ] {
        let (selected, attempts) = run(source, true);
        let (reference, _) = run(source, false);
        assert_eq!(attempts, 0);
        assert_eq!(selected.rows, reference.rows);
        assert!(!selected.rows.is_empty());
        assert_eq!(selected.receipt.join_rows, reference.receipt.join_rows);
    }
}

#[test]
fn one_determined_argument_can_restrict_a_partial_pattern() {
    let source = "k(1..2).p(f(1),g(7)).p(f(1),g(8)).p(f(2),g(9)). \
                  p(other(1..20),g(0)).r(K,X):-k(K),p(f(K),g(X)).";
    let (selected, attempts) = run(source, true);
    let (reference, _) = run(source, false);
    assert!(attempts > 0);
    assert_eq!(selected.rows, reference.rows);
    assert_eq!(
        selected.rows,
        vec![vec!["1", "7"], vec!["1", "8"], vec!["2", "9"]]
    );
    assert_eq!(selected.receipt.join_rows, Some(5));
}

const BOUND: &str = "k(1).p(f(g(1),g(1))).p(other(1)).r(X):-k(X),p(f(g(X),g(X))).";

#[test]
fn query_work_refusals_leave_incoming_bindings_unchanged() {
    testing::with_completed_source(BOUND, |program, support, computation, counters| {
        let rule = program.rules.last().unwrap();
        let location = rule.location;
        let mut attempt = |allowance: Option<u64>| {
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
            let occurrence = *join
                .plan
                .patterns
                .iter()
                .find(|p| matches!(p.pattern, PositivePattern::Structural(_)))
                .unwrap();
            let PositivePattern::Structural(pattern) = occurrence.pattern else {
                unreachable!()
            };
            let slot = pattern
                .arguments()
                .iter()
                .flat_map(|a| &a.nodes)
                .find_map(|n| {
                    if let PatternNode::Slot(slot) = n {
                        Some(*slot)
                    } else {
                        None
                    }
                })
                .unwrap();
            let key = computation.number(1, &limits, counters, location).unwrap();
            join.values
                .set(slot, &key, &limits, counters, location)
                .unwrap();
            let source = support
                .resolve(pattern.atom(), &limits, counters, location)
                .unwrap();
            let before = counters.accounting.work;
            let limits = FormulaLimits {
                max_work: allowance.map_or(u64::MAX, |left| before + left),
                ..limits
            };
            let result = join.structural_selection(
                occurrence.pattern,
                source,
                Context::new(computation, &limits, counters, location),
            );
            assert_eq!(
                join.values
                    .read(slot, computation.read(), location)
                    .unwrap(),
                Value::Number(1)
            );
            assert_eq!(join.lease.bytes(), join.storage_bytes());
            if let Some(scratch) = &join.structural_query {
                assert_eq!(scratch.stack.len(), 0);
                assert_eq!(scratch.arguments.len(), 0);
                assert_eq!(scratch.children.len(), 0);
            }
            (result, counters.accounting.work - before)
        };
        let (result, exact) = attempt(None);
        assert!(matches!(result.unwrap(), Selection::Posting(Some(rows)) if rows.len() == 1));
        for cut in 0..exact {
            assert!(matches!(
                attempt(Some(cut)).0,
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    ..
                })
            ));
        }
        assert!(attempt(Some(exact)).0.is_ok());
    });
}

fn compiled(source: &str, enabled: bool) -> (crate::formula::Compiled, usize) {
    scoped(enabled, || {
        crate::formula_ground::ground(testing::prepare(source), None, None).unwrap()
    })
}

fn equivalent_theories(source: &str) -> (crate::formula::Compiled, crate::formula::Compiled) {
    let (actual, attempts) = compiled(source, true);
    let (reference, skipped) = compiled(source, false);
    assert!(attempts > 0);
    assert_eq!(skipped, 0);
    let atoms = |compiled: &crate::formula::Compiled| {
        compiled
            .atoms
            .atoms()
            .iter()
            .map(|a| a.to_atom(zetesis_core::ValueLimits::default()).unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(atoms(&actual), atoms(&reference));
    assert_eq!(actual.origins, reference.origins);
    (actual, reference)
}

fn worlds(
    actual: &zetesis_ferraris::Theory,
    reference: &zetesis_ferraris::Theory,
) -> Vec<(
    zetesis_ferraris::Interpretation,
    zetesis_ferraris::Interpretation,
)> {
    use zetesis_ferraris::Interpretation;
    assert_eq!(actual.atom_count(), reference.atom_count());
    assert!(actual.atom_count() <= 7);
    (0..1 << actual.atom_count())
        .map(|bits| {
            let ids = || (0..actual.atom_count()).filter(|atom| bits & (1 << atom) != 0);
            (
                Interpretation::new(actual, ids()).unwrap(),
                Interpretation::new(reference, ids()).unwrap(),
            )
        })
        .collect()
}

const OPTIONAL: &str = "{k(1);k(2);p(f(1));p(f(2));p(other(1))}.r(X):-k(X),p(f(X)).";

#[test]
fn exact_probes_preserve_original_truth() {
    use zetesis_cpu::Cancellation;
    use zetesis_ferraris::{Limits, models};
    let (actual, reference) = equivalent_theories(OPTIONAL);
    for (left, right) in worlds(&actual.theory, &reference.theory) {
        assert_eq!(
            models(
                &actual.theory,
                &left,
                Limits::default(),
                &Cancellation::default()
            )
            .unwrap(),
            models(
                &reference.theory,
                &right,
                Limits::default(),
                &Cancellation::default()
            )
            .unwrap()
        );
    }
}

#[test]
fn exact_probes_preserve_arbitrary_frozen_truth() {
    use zetesis_cpu::Cancellation;
    use zetesis_ferraris::{Limits, models_reduct};
    let (actual, reference) = equivalent_theories(OPTIONAL);
    let worlds = worlds(&actual.theory, &reference.theory);
    for (candidate, (left, right)) in worlds.iter().enumerate() {
        for (tested, (left_tested, right_tested)) in worlds.iter().enumerate() {
            assert_eq!(
                models_reduct(
                    &actual.theory,
                    left,
                    left_tested,
                    Limits::default(),
                    &Cancellation::default()
                )
                .unwrap(),
                models_reduct(
                    &reference.theory,
                    right,
                    right_tested,
                    Limits::default(),
                    &Cancellation::default()
                )
                .unwrap(),
                "arbitrary M={candidate}, J={tested}"
            );
        }
    }
}

#[test]
fn recursive_deltas_keep_the_complete_formula() {
    // The first round has no p(f(2)) identity. Later rounds admit it, then
    // the second producer must use it in the same structured body occurrence.
    let source = "p(f(1)).edge(1,2).edge(2,3).p(f(Y)):-edge(X,Y),p(f(X)).";
    let (actual, reference) = equivalent_theories(source);
    assert_eq!(actual.theory.nodes(), reference.theory.nodes());
    assert_eq!(actual.theory.operands(), reference.theory.operands());
    assert_eq!(actual.theory.roots(), reference.theory.roots());
    assert_eq!(actual.atoms.atoms().len(), 5);
}

#[test]
fn frozen_queries_use_the_same_structural_postings() {
    let source = "k(1..3).p(other(1..20)).p(f(1)).p(f(2)).r(X):-k(X),p(f(X)).";
    let preparation = testing::prepare(source);
    let mut counters = super::super::Counters::resume(
        preparation.accounting,
        crate::grounding_observer::Work::default(),
    );
    let mut budget = preparation.budget;
    let limits = preparation.limits;
    let location = preparation.location;
    let completed = super::super::build(
        preparation.catalog,
        &preparation.program,
        None,
        &limits,
        &mut budget,
        &mut counters,
        location,
    )
    .unwrap();
    let closed = completed
        .into_streamed(
            &preparation.program.rules,
            GroundingWork::new(&limits, &mut counters, location),
        )
        .unwrap();
    let snapshot = closed.snapshot(&limits, &mut counters, location).unwrap();
    let queries = snapshot
        .queries(crate::JoinStrategy::Indexed, &limits, &counters, location)
        .unwrap();
    let mut computation = queries.computation(location).unwrap();
    let (actual, attempts) = scoped(true, || {
        query_rows(
            &preparation.program,
            queries.support(),
            &mut computation,
            &mut counters,
        )
    });
    let (reference, skipped) = scoped(false, || {
        query_rows(
            &preparation.program,
            queries.support(),
            &mut computation,
            &mut counters,
        )
    });
    assert!(attempts > 0);
    assert_eq!(skipped, 0);
    assert_eq!(actual.rows, vec![vec!["1"], vec!["2"]]);
    assert_eq!(actual.rows, reference.rows);
    assert_eq!(actual.receipt.join_rows, Some(3 + 2));
    assert_eq!(reference.receipt.join_rows, Some(3 + 3 * 22));
    assert_eq!(actual.receipt.unindexed_probes, Some(0));
}

fn query_envelope(
    max_bytes: Option<usize>,
    cancelled: bool,
) -> (Result<usize, FormulaFailure>, Receipt) {
    let mut output = None;
    testing::with_completed_source(BOUND, |program, support, computation, counters| {
        let rule = program.rules.last().unwrap();
        let location = rule.location;
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
        let occurrence = *join
            .plan
            .patterns
            .iter()
            .find(|p| matches!(p.pattern, PositivePattern::Structural(_)))
            .unwrap();
        let PositivePattern::Structural(pattern) = occurrence.pattern else {
            unreachable!()
        };
        let slot = pattern
            .arguments()
            .iter()
            .flat_map(|a| &a.nodes)
            .find_map(|n| {
                if let PatternNode::Slot(slot) = n {
                    Some(*slot)
                } else {
                    None
                }
            })
            .unwrap();
        let key = computation.number(1, &limits, counters, location).unwrap();
        join.values
            .set(slot, &key, &limits, counters, location)
            .unwrap();
        let source = support
            .resolve(pattern.atom(), &limits, counters, location)
            .unwrap();
        let owner_bytes = support.live_bytes() - support.workspace().bytes();
        let limits = FormulaLimits {
            max_support_bytes: max_bytes.unwrap_or(usize::MAX),
            ..limits
        };
        if cancelled {
            let cancellation = zetesis_cpu::Cancellation::default();
            cancellation.cancel();
            counters.cancellation = Some(cancellation);
        }
        let observer = Observer::default();
        let profile = Profile::new(Some(&observer));
        let previous = std::mem::replace(&mut counters.observed, profile.work());
        let result = profile.phase(GroundingPhase::RuleInstantiation, Some(location), || {
            join.structural_selection(
                occurrence.pattern,
                source,
                Context::new(computation, &limits, counters, location),
            )
            .map(|selection| match selection {
                Selection::Posting(Some(rows)) => rows.len(),
                _ => panic!("determined structural posting"),
            })
        });
        counters.observed = previous;
        counters.cancellation = None;
        assert_eq!(
            join.values
                .read(slot, computation.read(), location)
                .unwrap(),
            Value::Number(1)
        );
        assert_eq!(
            owner_bytes,
            support.live_bytes() - support.workspace().bytes()
        );
        assert_eq!(join.lease.bytes(), join.storage_bytes());
        cleared(&join);
        output = Some((result, observer.0.get()));
    });
    output.unwrap()
}

#[test]
fn query_scratch_has_an_inclusive_storage_boundary() {
    let (result, receipt) = query_envelope(None, false);
    assert_eq!(result.unwrap(), 1);
    let exact = usize::try_from(receipt.support_peak_bytes.unwrap()).unwrap();
    assert_eq!(query_envelope(Some(exact), false).0.unwrap(), 1);
    assert!(matches!(
        query_envelope(Some(exact - 1), false).0,
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            ..
        })
    ));
}

#[test]
fn cancelled_queries_cannot_publish_a_posting() {
    assert!(matches!(
        query_envelope(None, true).0,
        Err(FormulaFailure::Interrupted {
            reason: zetesis_cpu::Stop::Cancelled,
            ..
        })
    ));
}

pub(super) fn reuse_enabled() -> bool {
    REUSE.get().0
}
pub(super) fn reuse_record() {
    REUSE.set((reuse_enabled(), REUSE.get().1 + 1));
}
fn reuse_scoped<T>(enabled: bool, run: impl FnOnce() -> T) -> (T, usize) {
    struct Restore((bool, usize));
    impl Drop for Restore {
        fn drop(&mut self) {
            REUSE.set(self.0);
        }
    }
    let _restore = Restore(REUSE.replace((enabled, 0)));
    let result = run();
    (result, REUSE.get().1)
}
