//! Contracts for the compiler-internal plan boundary, including invalid IR
//! that ordinary source safety rejects before scheduling can encounter it.
use super::*;
use crate::ExpansionLimits;
use crate::formula_assignment_plan::{Plan, Step};
use themelios_base::source::{Source, SourceId};
use themelios_program::program::AggregateFunction;

fn variable(slot: usize) -> Expression {
    Expression {
        nodes: vec![Operation::Variable(slot)],
    }
}
fn aggregate(target: usize) -> LiteralIr {
    LiteralIr::Aggregate(AggregateIr {
        id: target,
        binding: Some(target),
        negation: DefaultNegation::None,
        function: AggregateFunction::Count,
        guards: vec![AggregateGuard {
            relation: Relation::Eq,
            bound: variable(target),
        }],
        elements: Vec::new(),
    })
}
fn plan(
    body: &[LiteralIr],
    variables: usize,
    limits: ExpansionLimits,
) -> Result<Plan, FormulaFailure> {
    let source = Source::new(SourceId::new(113), String::new()).unwrap();
    let mut budget = Budget::new(limits, 100);
    let mut compiler = Compiler {
        options: AdmissionOptions::default(),
        limits: &FormulaLimits::default(),
        budget: &mut budget,
        domain: BTreeSet::new(),
        next_aggregate: 0,
        dependency_projection: false,
        location: Location {
            source: source.id(),
            span: source.span(),
        },
    };
    compiler
        .assignment_plan(body, variables, &[])
        .map(Option::unwrap)
}

#[test]
fn readiness_orders_producers_before_consumers() {
    let body = [
        LiteralIr::Bind {
            target: 0,
            value: variable(1),
        },
        aggregate(1),
        LiteralIr::Bind {
            target: 2,
            value: Expression {
                nodes: vec![Operation::Constant(Value::Number(7))],
            },
        },
    ];
    let plan = plan(&body, 3, ExpansionLimits::default()).unwrap();
    let actual: Vec<_> = plan
        .steps
        .iter()
        .map(|step| (step.literal, step.produced, step.required.as_slice()))
        .collect();
    assert_eq!(actual, [(2, 2, &[][..]), (1, 1, &[][..]), (0, 0, &[1][..])]);
}

#[test]
fn missing_inputs_have_a_typed_plan_refusal() {
    let body = [
        LiteralIr::Bind {
            target: 0,
            value: variable(1),
        },
        aggregate(2),
    ];
    let error = plan(&body, 3, ExpansionLimits::default()).err().unwrap();
    assert!(matches!(
        error,
        FormulaFailure::UnboundValueInput { variable: 1, .. }
    ));
    assert!(error.to_string().contains("independently produced input 1"));
    assert_eq!(
        error.diagnostics()[0].primary().location.source,
        SourceId::new(113)
    );
}

#[test]
fn plan_storage_is_reserved_before_scheduling() {
    let body = [
        LiteralIr::Bind {
            target: 0,
            value: variable(1),
        },
        aggregate(1),
    ];
    let error = plan(
        &body,
        2,
        ExpansionLimits {
            max_scalar_bytes: 0,
            ..Default::default()
        },
    )
    .err()
    .unwrap();
    assert!(
        matches!(error, FormulaFailure::Expansion(ExpansionFailure::Limit {
        resource: ExpansionResource::ScalarBytes, limit: 0, observed, ..
    }) if observed == (2 * std::mem::size_of::<Option<Step>>()) as u128)
    );
}

#[test]
fn cyclic_inputs_have_a_typed_plan_refusal() {
    let body = [
        LiteralIr::Bind {
            target: 0,
            value: variable(1),
        },
        LiteralIr::Bind {
            target: 1,
            value: variable(0),
        },
        aggregate(2),
    ];
    let error = plan(&body, 3, ExpansionLimits::default()).err().unwrap();
    assert!(matches!(
        error,
        FormulaFailure::CyclicValueInput { variable: 1, .. }
    ));
    assert!(error.to_string().contains("cyclic finite value dependency"));
}

#[test]
fn independent_generators_retain_legacy_order() {
    let body = [
        aggregate(0),
        LiteralIr::Bind {
            target: 1,
            value: Expression {
                nodes: vec![Operation::Constant(Value::Number(9))],
            },
        },
        aggregate(2),
    ];
    let plan = plan(&body, 3, ExpansionLimits::default()).unwrap();
    assert_eq!(
        plan.steps
            .iter()
            .map(|step| step.literal)
            .collect::<Vec<_>>(),
        [1, 0, 2]
    );
    assert!(!plan.consumers);
}

#[test]
fn aggregate_inputs_exclude_local_witnesses() {
    let mut instruction = aggregate(0);
    let LiteralIr::Aggregate(aggregate) = &mut instruction else {
        unreachable!()
    };
    aggregate.elements.push(AggregateElementIr {
        key: AggregateKey::Tuple(vec![CoreTerm::Variable(2)]),
        condition: vec![LiteralIr::Atom(
            DefaultNegation::None,
            AtomPattern::new(
                Predicate::new("p", 2).unwrap(),
                vec![CoreTerm::Variable(1), CoreTerm::Variable(2)],
            )
            .unwrap(),
        )],
        variables: 3,
    });
    let body = [
        instruction,
        LiteralIr::Atom(
            DefaultNegation::None,
            AtomPattern::new(Predicate::new("d", 1).unwrap(), vec![CoreTerm::Variable(1)]).unwrap(),
        ),
    ];
    let plan = plan(&body, 2, ExpansionLimits::default()).unwrap();
    assert_eq!(plan.steps[0].required, [1]);
    assert_eq!(plan.steps[0].produced, 0);
}

#[test]
fn dependent_ranges_follow_complete_endpoint_bindings() {
    let body = [
        LiteralIr::Range {
            target: 0,
            lower: variable(1),
            upper: variable(2),
            binder: true,
        },
        LiteralIr::Bind {
            target: 2,
            value: variable(1),
        },
        aggregate(1),
    ];
    let plan = plan(&body, 3, ExpansionLimits::default()).unwrap();
    assert_eq!(
        plan.steps
            .iter()
            .map(|step| step.literal)
            .collect::<Vec<_>>(),
        [2, 1, 0]
    );
    assert_eq!(plan.steps[2].required, [1, 2]);
    assert!(plan.consumers);
}

#[test]
fn range_filters_do_not_produce_already_bound_targets() {
    let body = [
        aggregate(0),
        LiteralIr::Atom(
            DefaultNegation::None,
            AtomPattern::new(Predicate::new("d", 1).unwrap(), vec![CoreTerm::Variable(1)]).unwrap(),
        ),
        LiteralIr::Range {
            target: 1,
            lower: variable(0),
            upper: variable(0),
            binder: false,
        },
    ];
    let plan = plan(&body, 2, ExpansionLimits::default()).unwrap();
    assert_eq!(plan.steps.len(), 1);
    assert_eq!(plan.steps[0].produced, 0);
    assert!(plan.consumers);
}

fn reading_aggregate(target: usize, input: usize) -> LiteralIr {
    let mut literal = aggregate(target);
    let LiteralIr::Aggregate(aggregate) = &mut literal else {
        unreachable!()
    };
    aggregate.elements.push(AggregateElementIr {
        key: AggregateKey::Tuple(vec![CoreTerm::Variable(input)]),
        condition: Vec::new(),
        variables: target.max(input) + 1,
    });
    literal
}

#[test]
fn aggregate_producers_follow_complete_inputs() {
    let body = [
        reading_aggregate(0, 1),
        LiteralIr::Bind {
            target: 1,
            value: variable(2),
        },
        aggregate(2),
    ];
    let plan = plan(&body, 3, ExpansionLimits::default()).unwrap();
    assert_eq!(
        plan.steps
            .iter()
            .map(|step| (step.literal, step.required.as_slice()))
            .collect::<Vec<_>>(),
        [(2, &[][..]), (1, &[2][..]), (0, &[1][..])]
    );
    assert!(plan.consumers);
}

#[test]
fn direct_aggregate_dependencies_are_objective_consumers() {
    let body = [reading_aggregate(0, 1), aggregate(1)];
    let plan = plan(&body, 2, ExpansionLimits::default()).unwrap();
    assert_eq!(
        plan.steps
            .iter()
            .map(|step| step.literal)
            .collect::<Vec<_>>(),
        [1, 0]
    );
    assert!(plan.consumers);
}

#[test]
fn cyclic_aggregate_inputs_have_a_typed_refusal() {
    let body = [reading_aggregate(0, 1), reading_aggregate(1, 0)];
    let error = plan(&body, 2, ExpansionLimits::default()).err().unwrap();
    assert!(matches!(error, FormulaFailure::CyclicValueInput { .. }));
}

fn conditional_read(slot: usize) -> LiteralIr {
    LiteralIr::Conditional(crate::formula_conditional_ir::ConditionalIr {
        consequent: crate::formula_conditional_ir::Consequent::Atoms(
            DefaultNegation::None,
            vec![crate::formula_conditional_ir::Alternative {
                atom: AtomPattern::new(
                    Predicate::new("p", 1).unwrap(),
                    vec![CoreTerm::Variable(slot)],
                )
                .unwrap(),
                bindings: Vec::new(),
                variables: slot + 1,
            }],
        ),
        condition: Vec::new(),
        variables: slot + 1,
    })
}

#[test]
fn conditional_outer_reads_mark_objective_consumers() {
    let body = [aggregate(0), conditional_read(0)];
    let plan = plan(&body, 1, ExpansionLimits::default()).unwrap();
    assert!(plan.consumers);
    assert_eq!(plan.steps.len(), 1);
    assert_eq!(plan.steps[0].literal, 0);
    assert_eq!(plan.steps[0].produced, 0);
    assert!(plan.steps[0].required.is_empty());
}

#[test]
fn conditional_local_reads_do_not_alias_outer_slots() {
    let body = [aggregate(0), conditional_read(1)];
    let plan = plan(&body, 1, ExpansionLimits::default()).unwrap();
    assert!(!plan.consumers);
    assert_eq!(plan.steps.len(), 1);
    assert_eq!(plan.steps[0].produced, 0);
}

fn comparison_guard(bounds: &[usize]) -> LiteralIr {
    LiteralIr::Aggregate(AggregateIr {
        id: 9,
        binding: None,
        negation: DefaultNegation::NotNot,
        function: AggregateFunction::Count,
        guards: bounds
            .iter()
            .map(|&slot| AggregateGuard {
                relation: Relation::Neq,
                bound: variable(slot),
            })
            .collect(),
        elements: Vec::new(),
    })
}

#[test]
fn nonbinding_guards_do_not_add_plan_steps() {
    let body = [
        comparison_guard(&[0, 1]),
        LiteralIr::Bind {
            target: 0,
            value: variable(1),
        },
        aggregate(1),
    ];
    let plan = plan(&body, 2, ExpansionLimits::default()).unwrap();
    assert_eq!(
        plan.steps
            .iter()
            .map(|step| (step.literal, step.produced, step.required.as_slice()))
            .collect::<Vec<_>>(),
        [(2, 1, &[][..]), (1, 0, &[1][..])]
    );
}

#[test]
fn every_nonbinding_guard_can_mark_an_objective_consumer() {
    for bounds in [[0, 1], [1, 0]] {
        let body = [
            aggregate(0),
            comparison_guard(&bounds),
            LiteralIr::Atom(
                DefaultNegation::None,
                AtomPattern::new(Predicate::new("d", 1).unwrap(), vec![CoreTerm::Variable(1)])
                    .unwrap(),
            ),
        ];
        assert!(
            plan(&body, 2, ExpansionLimits::default())
                .unwrap()
                .consumers
        );
    }
}
