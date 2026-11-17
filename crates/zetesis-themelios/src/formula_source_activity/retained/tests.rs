use super::*;
use crate::formula_support::{Computation, Counters, testing};
use crate::grounding_observer::Work;
use crate::{FormulaLimits, FormulaResource, JoinStrategy};
use zetesis_algebra::{Cancellation, Stop};
use zetesis_core::{Atom, ValueLimits};

const CYCLE: &str = "d(1;2).seed(1).{seed(2)}.p(X):-d(X),q(X),X<=2.q(X):-p(X).q(X):-seed(X).#minimize{X@1:p(X),not not p(X)}.";

type Classifications = Vec<(Atom, u8)>;

#[derive(Debug)]
struct Report {
    outcome: Result<Classifications, FormulaFailure>,
    work: u64,
    substitutions: u64,
    captures: usize,
    hits: usize,
}

fn classifications(
    owner: &SourceEligibility,
    context: &mut Context<'_, '_, '_>,
) -> Classifications {
    assert!(
        owner.round.is_none(),
        "only a whole successful certificate escapes"
    );
    (0..owner.atoms.len())
        .map(|position| {
            let atom = owner
                .atoms
                .atom(
                    position,
                    context.computation,
                    context.limits,
                    context.counters,
                    context.location,
                )
                .unwrap()
                .to_atom(ValueLimits::default())
                .unwrap();
            let activity = match owner.activity[position] {
                super::super::Activity::Absent => 0,
                super::super::Activity::Optional => 1,
                super::super::Activity::Required => 2,
            };
            (atom, activity)
        })
        .collect()
}

fn run(
    source: &str,
    strategy: JoinStrategy,
    enabled: bool,
    allowance: Option<u64>,
    cancelled: bool,
) -> Report {
    let preparation = testing::prepare(source);
    let program = preparation.program;
    assert!(
        program
            .objectives
            .iter()
            .any(|objective| objective.needs_eligibility_query)
    );
    let mut budget = preparation.budget;
    let mut counters = Counters::resume(preparation.accounting, Work::default());
    let mut catalog = crate::formula_support::build(
        preparation.catalog,
        &program,
        None,
        &preparation.limits,
        &mut budget,
        &mut counters,
        preparation.location,
    )
    .unwrap();
    let (completed, mut append) = catalog
        .split(&preparation.limits, &mut counters, preparation.location)
        .unwrap();
    let queries = completed
        .queries(
            strategy,
            &preparation.limits,
            &counters,
            preparation.location,
        )
        .unwrap();
    let mut computation = Computation::new(&mut append, queries.support());
    let presence = crate::formula_objective_dependencies::check_presence(
        &program,
        queries.support(),
        &mut computation,
        &preparation.limits,
        &mut budget,
        &mut counters,
    )
    .unwrap();
    let before = (counters.accounting.work, counters.accounting.substitutions);
    let limits = FormulaLimits {
        max_work: allowance.map_or(preparation.limits.max_work, |units| before.0 + units),
        ..preparation.limits
    };
    let cancellation = Cancellation::default();
    if cancelled {
        cancellation.cancel();
        counters = counters.with_cancellation(Some(&cancellation));
    }
    let mut context = Context {
        computation: &mut computation,
        limits: &limits,
        budget: &mut budget,
        counters: &mut counters,
        location: preparation.location,
    };
    let (outcome, hits, captures) = controlled(enabled, || {
        SourceEligibility::build_from_conditions_with_executor(
            &program,
            &queries,
            presence.retained_entries(),
            program
                .objectives
                .iter()
                .filter(|objective| objective.needs_eligibility_query)
                .map(|objective| objective.condition.literals()),
            &mut context,
            None,
        )
    });
    let work = context.counters.accounting.work - before.0;
    let substitutions = context.counters.accounting.substitutions - before.1;
    // These assertion-only reads do not consume the measured constructor grant.
    context.limits = &preparation.limits;
    let outcome = outcome.map(|owner| classifications(&owner, &mut context));
    Report {
        outcome,
        work,
        substitutions,
        captures,
        hits,
    }
}

#[test]
fn immutable_rows_replay_while_activity_refines_in_later_rounds() {
    let ordinary = run(CYCLE, JoinStrategy::Indexed, false, None, false);
    let retained = run(CYCLE, JoinStrategy::Indexed, true, None, false);
    assert_eq!(ordinary.captures, 0);
    assert_eq!(ordinary.hits, 0);
    assert!(retained.captures >= 3);
    assert!(
        retained.hits > retained.captures,
        "the fixed point actually revisits producers"
    );
    let expected = ordinary.outcome.unwrap();
    let actual = retained.outcome.unwrap();
    assert_eq!(
        actual, expected,
        "identity insertion and classifications retain source order"
    );
    assert_eq!(
        ordinary.substitutions, retained.substitutions,
        "every logical complete-binding consumer visit remains admitted"
    );
    assert!(
        actual
            .iter()
            .any(|(atom, activity)| atom.predicate().name() == "p" && *activity == 2),
        "the p cycle must refine Optional to Required; reusing first-round Activity would fail"
    );
    assert!(
        actual
            .iter()
            .any(|(atom, activity)| atom.predicate().name() == "p" && *activity == 1),
        "the independently optional seed must remain optional"
    );
}

#[test]
fn fresh_source_owners_do_not_share_a_retained_family() {
    let first = run(CYCLE, JoinStrategy::Indexed, true, None, false);
    let second = run(CYCLE, JoinStrategy::Indexed, true, None, false);
    assert!(first.captures > 0 && first.hits > 0);
    assert_eq!((first.captures, first.hits), (second.captures, second.hits));
    assert_eq!(first.outcome.unwrap(), second.outcome.unwrap());
    assert_eq!(
        (first.work, first.substitutions),
        (second.work, second.substitutions)
    );
}

#[test]
fn table_keeps_its_original_producer_traversal() {
    let ordinary = run(CYCLE, JoinStrategy::Table, false, None, false);
    let retained = run(CYCLE, JoinStrategy::Table, true, None, false);
    assert_eq!((retained.captures, retained.hits), (0, 0));
    assert_eq!(retained.outcome.unwrap(), ordinary.outcome.unwrap());
    assert_eq!(retained.substitutions, ordinary.substitutions);
}

#[test]
fn late_refusal_or_cancellation_does_not_publish_a_partial_activity_certificate() {
    let full = run(CYCLE, JoinStrategy::Indexed, true, None, false);
    assert!(full.outcome.is_ok() && full.hits > 0);
    for allowance in [0, full.work / 2, full.work - 1] {
        let refused = run(CYCLE, JoinStrategy::Indexed, true, Some(allowance), false);
        assert!(matches!(
            refused.outcome,
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            })
        ));
        assert!(
            refused.work <= allowance,
            "rejected charges are not fabricated performed work"
        );
    }
    let cancelled = run(CYCLE, JoinStrategy::Indexed, true, Some(0), true);
    assert!(matches!(
        cancelled.outcome,
        Err(FormulaFailure::Interrupted {
            reason: Stop::Cancelled,
            ..
        })
    ));
    let retry = run(CYCLE, JoinStrategy::Indexed, true, None, false);
    assert_eq!(retry.outcome.unwrap(), full.outcome.unwrap());
    assert_eq!(
        (retry.work, retry.substitutions),
        (full.work, full.substitutions)
    );
}

fn assert_program(reference: &crate::formula::Compiled, actual: &crate::formula::Compiled) {
    assert_eq!(
        crate::test_support::compiled_atoms(reference),
        crate::test_support::compiled_atoms(actual)
    );
    assert_eq!(reference.theory.nodes(), actual.theory.nodes());
    assert_eq!(reference.theory.operands(), actual.theory.operands());
    assert_eq!(reference.theory.roots(), actual.theory.roots());
    assert_eq!(reference.origins, actual.origins);
    assert_eq!(reference.warnings, actual.warnings);
    assert!(
        reference
            .objectives
            .templates()
            .iter()
            .eq(actual.objectives.templates().iter())
    );
    assert_eq!(
        reference.objectives.priorities(),
        actual.objectives.priorities()
    );
    assert_eq!(reference.objective_origins, actual.objective_origins);
    assert_eq!(
        reference.objective_declarations,
        actual.objective_declarations
    );
}

#[test]
fn all_complete_models_scores_and_provenance_match_on_the_cyclic_source() {
    let (reference, _, _) = controlled(false, || {
        crate::formula_ground::ground(testing::prepare(CYCLE), None, None)
    });
    let (actual, hits, captures) = controlled(true, || {
        crate::formula_ground::ground(testing::prepare(CYCLE), None, None)
    });
    let reference = reference.unwrap();
    let actual = actual.unwrap();
    assert!(
        hits > 0 && captures > 0,
        "the complete frontend must reach the retained cycle route"
    );
    assert_program(&reference, &actual);
    let count = reference.theory.atom_count();
    assert!(count <= 8);
    let mut accepted = 0;
    for mask in 0..(1_usize << count) {
        let mut results = Vec::new();
        for program in [&reference, &actual] {
            let positions = || (0..count).filter(|&atom| mask & (1 << atom) != 0);
            let interpretation =
                zetesis_ferraris::Interpretation::new(&program.theory, positions()).unwrap();
            let verdict = zetesis_ferraris::check(
                &program.theory,
                &interpretation,
                zetesis_ferraris::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
            let model = zetesis_core::Model::from_positions(&program.atoms, positions()).unwrap();
            let score = zetesis_objective::evaluate(
                &program.objectives,
                &model,
                zetesis_objective::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
            results.push((verdict.accepted(), score.score().costs().to_vec()));
        }
        assert_eq!(results[0], results[1], "complete interpretation {mask}");
        accepted += usize::from(results[0].0);
    }
    assert!(accepted > 0);
}

#[test]
fn table_and_all_unsupported_cycles_never_construct_dense_retention() {
    let negative =
        "d(1;2).p(X):-d(X),not q(X).q(X):-d(X),not p(X).#minimize{X@1:p(X),not not p(X)}.";
    for (source, strategy) in [
        (CYCLE, JoinStrategy::Table),
        (negative, JoinStrategy::Indexed),
    ] {
        let ordinary = run(source, strategy, false, None, false);
        let (actual, first) = allocation(|| run(source, strategy, true, None, false));
        assert_eq!(
            first, None,
            "no dense owner or cell initialization was attempted"
        );
        assert_eq!((actual.captures, actual.hits), (0, 0));
        assert_eq!(actual.outcome.unwrap(), ordinary.outcome.unwrap());
        assert_eq!(actual.substitutions, ordinary.substitutions);
    }
}

#[test]
fn the_first_real_eligible_producer_allocates_once_at_its_original_index() {
    let source = format!("a_unrelated:-seed(2).{CYCLE}");
    let preparation = testing::prepare(&source);
    let expected_cells = preparation.program.rules_without_facts().len();
    // The raised program uses content order, not textual prefix order.
    // Assert that this unrelated producer really precedes the retained cycle.
    let earlier = &preparation.program.rules_without_facts()[0];
    let crate::formula_ir::HeadIr::Normal(Some(head)) = earlier.head else {
        panic!("the earlier unrelated producer must retain its normal head");
    };
    let [
        crate::formula_ir::LiteralIr::Atom(
            themelios_program::program::DefaultNegation::None,
            input,
        ),
    ] = earlier.body.as_slice()
    else {
        panic!("the earlier unrelated producer must retain its optional seed dependency");
    };
    let mut premise_counters = Counters::default();
    let view = preparation
        .catalog
        .component_view(&preparation.limits, &mut premise_counters, earlier.location)
        .unwrap()
        .unwrap();
    assert_eq!(
        head.get(
            view,
            &preparation.limits,
            &mut premise_counters,
            earlier.location
        )
        .unwrap()
        .predicate()
        .name(),
        "a_unrelated"
    );
    assert_eq!(
        input
            .get(
                view,
                &preparation.limits,
                &mut premise_counters,
                earlier.location
            )
            .unwrap()
            .predicate()
            .name(),
        "seed"
    );
    let ordinary = run(&source, JoinStrategy::Indexed, false, None, false);
    let (actual, first) = allocation(|| run(&source, JoinStrategy::Indexed, true, None, false));
    let (rule, cells) = first.expect("a real post-lowering cyclic producer must qualify");
    assert!(
        rule > 0,
        "unrelated earlier source producers do not trigger retention setup"
    );
    assert_eq!(
        cells, expected_cells,
        "the unchanged dense rule-index authority is retained"
    );
    assert!(actual.captures > 0 && actual.hits > 0);
    assert_eq!(actual.outcome.unwrap(), ordinary.outcome.unwrap());
    assert_eq!(actual.substitutions, ordinary.substitutions);
}

#[test]
fn first_allocation_refusal_and_cancellation_leave_the_owner_absent() {
    testing::with_completed_source(CYCLE, |program, _support, computation, counters| {
        let mut current = None;
        let mut retention = Retention::new(&mut current, program);
        let mut budget = testing::budget();
        let location = program.rules_without_facts()[0].location;
        let before = counters.accounting.work;
        let limits = FormulaLimits {
            max_work: before,
            ..FormulaLimits::default()
        };
        let (result, first) = allocation(|| {
            retention.admit(
                1,
                Context {
                    computation,
                    limits: &limits,
                    budget: &mut budget,
                    counters,
                    location,
                },
            )
        });
        assert!(matches!(result,
            Err(FormulaFailure::Limit { resource: FormulaResource::Work, limit, observed, .. })
            if limit == u128::from(before) && observed == limit + 1));
        assert_eq!(counters.accounting.work, before);
        assert_eq!(first, None);
        assert!(retention.current.is_none());
        let limits = FormulaLimits {
            max_support_bytes: 0,
            ..FormulaLimits::default()
        };
        let (result, first) = allocation(|| {
            retention.admit(
                1,
                Context {
                    computation,
                    limits: &limits,
                    budget: &mut budget,
                    counters,
                    location,
                },
            )
        });
        assert!(matches!(
            result,
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                limit: 0,
                ..
            })
        ));
        assert_eq!(first, Some((1, program.rules_without_facts().len())));
        assert!(
            retention.current.is_none(),
            "failed construction never publishes partial cells"
        );
        let cancellation = Cancellation::default();
        counters.attach_test_control(&cancellation);
        cancellation.cancel();
        let (result, first) = allocation(|| {
            retention.admit(
                1,
                Context {
                    computation,
                    limits: &FormulaLimits::default(),
                    budget: &mut budget,
                    counters,
                    location,
                },
            )
        });
        assert!(matches!(
            result,
            Err(FormulaFailure::Interrupted {
                reason: Stop::Cancelled,
                ..
            })
        ));
        assert_eq!(first, None);
        assert!(retention.current.is_none());
        counters.detach_test_control();
    });
}
