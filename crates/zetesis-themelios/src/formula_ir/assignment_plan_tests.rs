//! Contracts for the compiler-internal plan boundary, including invalid IR
//! that ordinary source safety rejects before scheduling can encounter it.
use super::*;
use crate::ExpansionLimits;
use crate::formula_assignment_plan::{Plan, Step};
use themelios_base::source::{Source, SourceId};
use themelios_program::program::AggregateFunction;

fn atom(compiler: &mut Compiler<'_>, name: &str, terms: &[CoreTerm]) -> AtomPattern {
    let predicate = compiler
        .predicate(name, terms.len(), zetesis_core::Sign::Positive)
        .unwrap();
    compiler.pattern_from_parts(predicate, terms).unwrap()
}

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
fn with_compiler<R>(limits: ExpansionLimits, work: impl FnOnce(&mut Compiler<'_>) -> R) -> R {
    let source = Source::new(SourceId::new(113), String::new()).unwrap();
    let mut budget = Budget::new(limits, 100);
    let formula_limits = FormulaLimits::default();
    let location = Location {
        source: source.id(),
        span: source.span(),
    };
    let mut catalog = SupportCatalog::default();
    let mut counters = Counters::default();
    let admission = catalog
        .component_admission(&formula_limits, &mut counters, location)
        .unwrap();
    let domain = domain::Domain::new(&admission, &formula_limits, &counters, location).unwrap();
    let mut compiler = Compiler {
        options: AdmissionOptions::default(),
        limits: &formula_limits,
        budget: &mut budget,
        source: admission,
        counters: &mut counters,
        domain,
        next_aggregate: 0,
        dependency_projection: false,
        location: Location {
            source: source.id(),
            span: source.span(),
        },
    };
    work(&mut compiler)
}

fn plan(
    body: &[LiteralIr],
    variables: usize,
    limits: ExpansionLimits,
) -> Result<Plan, FormulaFailure> {
    with_compiler(limits, |compiler| {
        compiler
            .assignment_plan(body, variables, variables, &[])
            .map(Option::unwrap)
    })
}

#[test]
fn readiness_orders_producers_before_consumers() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let body = [
            LiteralIr::Bind {
                target: 0,
                value: variable(1),
            },
            aggregate(1),
            LiteralIr::Bind {
                target: 2,
                value: compiler.constant_expression(7).unwrap(),
            },
        ];
        let plan = compiler.assignment_plan(&body, 3, 3, &[]).unwrap().unwrap();
        let actual: Vec<_> = plan
            .steps
            .iter()
            .map(|step| (step.literal, step.produced, step.required.as_slice()))
            .collect();
        assert_eq!(actual, [(2, 2, &[][..]), (1, 1, &[][..]), (0, 0, &[1][..])]);
    });
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
    with_compiler(ExpansionLimits::default(), |compiler| {
        let body = [
            aggregate(0),
            LiteralIr::Bind {
                target: 1,
                value: compiler.constant_expression(9).unwrap(),
            },
            aggregate(2),
        ];
        let plan = compiler.assignment_plan(&body, 3, 3, &[]).unwrap().unwrap();
        assert_eq!(
            plan.steps
                .iter()
                .map(|step| step.literal)
                .collect::<Vec<_>>(),
            [1, 0, 2]
        );
        assert!(!plan.consumers);
    });
}

#[test]
fn aggregate_inputs_exclude_local_witnesses() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let mut instruction = aggregate(0);
        let LiteralIr::Aggregate(aggregate) = &mut instruction else {
            unreachable!()
        };
        aggregate.elements.push(AggregateElementIr {
            family: crate::formula_ir::LocalFamily(0),
            key: AggregateKey::Tuple(vec![CoreTerm::Variable(2)]),
            condition: vec![LiteralIr::Atom(
                DefaultNegation::None,
                atom(
                    compiler,
                    "p",
                    &[CoreTerm::Variable(1), CoreTerm::Variable(2)],
                ),
            )],
            variables: 3,
        });
        let body = [
            instruction,
            LiteralIr::Atom(
                DefaultNegation::None,
                atom(compiler, "d", &[CoreTerm::Variable(1)]),
            ),
        ];
        let plan = compiler
            .assignment_plan(&body, 2, 2, &[])
            .map(Option::unwrap)
            .unwrap();
        assert_eq!(plan.steps[0].required, [1]);
        assert_eq!(plan.steps[0].produced, 0);
    });
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
    with_compiler(ExpansionLimits::default(), |compiler| {
        let body = [
            aggregate(0),
            LiteralIr::Atom(
                DefaultNegation::None,
                atom(compiler, "d", &[CoreTerm::Variable(1)]),
            ),
            LiteralIr::Range {
                target: 1,
                lower: variable(0),
                upper: variable(0),
                binder: false,
            },
        ];
        let plan = compiler
            .assignment_plan(&body, 2, 2, &[])
            .map(Option::unwrap)
            .unwrap();
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].produced, 0);
        assert!(plan.consumers);
    });
}

fn reading_aggregate(target: usize, input: usize) -> LiteralIr {
    let mut literal = aggregate(target);
    let LiteralIr::Aggregate(aggregate) = &mut literal else {
        unreachable!()
    };
    aggregate.elements.push(AggregateElementIr {
        family: crate::formula_ir::LocalFamily(0),
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

fn conditional_read(compiler: &mut Compiler<'_>, slot: usize) -> LiteralIr {
    LiteralIr::Conditional(crate::formula_conditional_ir::ConditionalIr {
        family: crate::formula_ir::LocalFamily(0),
        consequent: crate::formula_conditional_ir::Consequent::Atoms(
            DefaultNegation::None,
            vec![crate::formula_conditional_ir::Alternative {
                operand: crate::formula_conditional_ir::ConsequentOperand::Atom(atom(
                    compiler,
                    "p",
                    &[CoreTerm::Variable(slot)],
                )),
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
    with_compiler(ExpansionLimits::default(), |compiler| {
        let body = [aggregate(0), conditional_read(compiler, 0)];
        let plan = compiler
            .assignment_plan(&body, 1, 1, &[])
            .map(Option::unwrap)
            .unwrap();
        assert!(plan.consumers);
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].literal, 0);
        assert_eq!(plan.steps[0].produced, 0);
        assert!(plan.steps[0].required.is_empty());
    });
}

#[test]
fn conditional_local_reads_do_not_alias_outer_slots() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let body = [aggregate(0), conditional_read(compiler, 1)];
        let plan = compiler
            .assignment_plan(&body, 1, 1, &[])
            .map(Option::unwrap)
            .unwrap();
        assert!(!plan.consumers);
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].produced, 0);
    });
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
    with_compiler(ExpansionLimits::default(), |compiler| {
        for bounds in [[0, 1], [1, 0]] {
            let body = [
                aggregate(0),
                comparison_guard(&bounds),
                LiteralIr::Atom(
                    DefaultNegation::None,
                    atom(compiler, "d", &[CoreTerm::Variable(1)]),
                ),
            ];
            assert!(
                compiler
                    .assignment_plan(&body, 2, 2, &[])
                    .map(Option::unwrap)
                    .unwrap()
                    .consumers
            );
        }
    });
}

fn nonbinding_element(key: AggregateKey, condition: Vec<LiteralIr>) -> LiteralIr {
    LiteralIr::Aggregate(AggregateIr {
        id: 9,
        binding: None,
        negation: DefaultNegation::None,
        function: AggregateFunction::Count,
        guards: Vec::new(),
        elements: vec![AggregateElementIr {
            family: crate::formula_ir::LocalFamily(0),
            key,
            condition,
            variables: 2,
        }],
    })
}

#[test]
fn nonbinding_tuple_keys_mark_objective_consumers() {
    for negation in [
        DefaultNegation::None,
        DefaultNegation::Not,
        DefaultNegation::NotNot,
    ] {
        let mut consumer =
            nonbinding_element(AggregateKey::Tuple(vec![CoreTerm::Variable(0)]), Vec::new());
        let LiteralIr::Aggregate(value) = &mut consumer else {
            unreachable!()
        };
        value.negation = negation;
        let mut body = [aggregate(0), consumer];
        for reversed in [false, true] {
            if reversed {
                body.reverse();
            }
            let plan = plan(&body, 1, ExpansionLimits::default()).unwrap();
            assert!(plan.consumers, "{negation:?}, reversed={reversed}");
            assert_eq!(plan.steps.len(), 1, "a comparison is not a producer");
            assert_eq!(plan.steps[0].produced, 0);
            assert!(plan.steps[0].required.is_empty());
        }
    }
}

#[test]
fn nonbinding_atom_keys_mark_objective_consumers() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let body = [
            aggregate(0),
            nonbinding_element(
                AggregateKey::Atom(atom(compiler, "p", &[CoreTerm::Variable(0)])),
                Vec::new(),
            ),
        ];
        assert!(
            compiler
                .assignment_plan(&body, 1, 1, &[])
                .map(Option::unwrap)
                .unwrap()
                .consumers
        );
    });
}

#[test]
fn nonbinding_conditions_read_aggregate_descendants() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        for negation in [
            DefaultNegation::None,
            DefaultNegation::Not,
            DefaultNegation::NotNot,
        ] {
            let body = [
                aggregate(0),
                LiteralIr::Bind {
                    target: 1,
                    value: variable(0),
                },
                nonbinding_element(
                    AggregateKey::Tuple(vec![CoreTerm::Constant(
                        compiler.scalar(&Value::Number(1)).unwrap(),
                    )]),
                    vec![LiteralIr::Atom(
                        negation,
                        atom(compiler, "p", &[CoreTerm::Variable(1)]),
                    )],
                ),
            ];
            // Inspect the consumer directly as well: the scalar producer already
            // marks the whole plan, which alone would conceal an omitted condition.
            assert!(
                compiler
                    .assignment_context(&body[2..], &[], &[false, true])
                    .unwrap()
            );
            let plan = compiler
                .assignment_plan(&body, 2, 2, &[])
                .map(Option::unwrap)
                .unwrap();
            assert!(plan.consumers);
            assert_eq!(plan.steps[1].required, [0]);
        }
    });
}

#[test]
fn nonbinding_local_reads_do_not_alias_outer_slots() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let body = [
            aggregate(0),
            nonbinding_element(
                AggregateKey::Tuple(vec![CoreTerm::Variable(1)]),
                vec![LiteralIr::Atom(
                    DefaultNegation::None,
                    atom(compiler, "p", &[CoreTerm::Variable(1)]),
                )],
            ),
        ];
        let plan = compiler
            .assignment_plan(&body, 1, 1, &[])
            .map(Option::unwrap)
            .unwrap();
        assert!(!plan.consumers);
        assert!(plan.steps[0].required.is_empty());
    });
}

#[test]
fn nonbinding_element_reads_obey_the_work_ceiling() {
    let body = [nonbinding_element(
        AggregateKey::Tuple(vec![CoreTerm::Variable(0)]),
        Vec::new(),
    )];
    let limits = ExpansionLimits {
        max_term_work: 4,
        ..Default::default()
    };
    let failure = with_compiler(limits, |compiler| {
        compiler.assignment_context(&body, &[], &[true])
    })
    .unwrap_err();
    let FormulaFailure::Expansion(ExpansionFailure::Limit {
        resource: ExpansionResource::TermWork,
        limit: 4,
        observed: 5,
        location,
    }) = failure
    else {
        panic!("expected the located element-read work refusal: {failure}")
    };
    assert_eq!(location.source, SourceId::new(113));
}

#[test]
fn parsed_nonbinding_reads_preserve_outer_scope() {
    for (text, consumed) in [
        ("p(N):-N=#count{},0<=#count{N:q}.", true),
        ("p(N):-N=#count{},0<=#count{1:q(N)}.", true),
        ("p(N):-0<=#count{1: -q(N)},N=#count{}.", true),
        ("p(N):-N=#count{},0<={q(N)}.", true),
        ("p(N):-N=#count{},0<=#count{X:q(X)}.", false),
    ] {
        let source = Source::new(SourceId::new(113), text.into()).unwrap();
        let parsed =
            themelios_syntax::parse::parse(&source, themelios_syntax::dialect::Dialect::Clingo);
        assert!(parsed.diagnostics().is_empty());
        let raised = themelios_program::raise::raise(&parsed);
        assert!(raised.diagnostics().is_empty());
        let Statement::Rule(rule) = raised.program().statements().next().unwrap().get() else {
            unreachable!()
        };
        let compiled = with_compiler(ExpansionLimits::default(), |compiler| {
            compiler.rule(rule, Vec::new(), None).unwrap()
        });
        let plan = compiled.bindings.unwrap();
        assert_eq!(plan.consumers, consumed, "{text}");
        assert_eq!(plan.steps.len(), 1, "the comparison adds no producer");
        assert!(plan.steps[0].required.is_empty());
    }
}

#[test]
fn multiple_aggregates_require_conservative_eligibility() {
    let source = Source::new(
        SourceId::new(113),
        "q(0).p(N):-N=#count{},0<=#count{1:q(N)}.#minimize{1@N:p(N)}.".into(),
    )
    .unwrap();
    let parsed =
        themelios_syntax::parse::parse(&source, themelios_syntax::dialect::Dialect::Clingo);
    assert!(parsed.diagnostics().is_empty());
    let raised = themelios_program::raise::raise(&parsed);
    assert!(raised.diagnostics().is_empty());
    let mut budget = Budget::new(ExpansionLimits::default(), 100);
    let mut catalog = SupportCatalog::default();
    let mut counters = Counters::default();
    let location = Location {
        source: source.id(),
        span: source.span(),
    };
    let mut metadata = crate::metadata::Builder::default();
    crate::metadata::collect_profile(raised.program(), &mut metadata, true).unwrap();
    let metadata = metadata.finish(location).unwrap();
    let mut prepared = PreparationContext {
        options: AdmissionOptions::default(),
        budget: &mut budget,
        catalog: &mut catalog,
        work: GroundingWork::new(&FormulaLimits::default(), &mut counters, location),
    }
    .prepare(
        raised.program(),
        metadata.project_selection().clone(),
        &crate::formula_choice_source::Catalog::default(),
    )
    .unwrap();
    assert_eq!(prepared.objectives.len(), 1);
    assert!(prepared.objectives[0].needs_eligibility_query);
    let plans: Vec<_> = prepared
        .rules
        .iter_mut()
        .filter_map(|rule| rule.bindings.as_mut())
        .collect();
    assert_eq!(plans.len(), 1);
    assert!(plans[0].consumers);
    // Deliberately reproduce the old incomplete summary. The independent
    // single-aggregate applicability restriction must still reject precision.
    for plan in plans {
        plan.consumers = false;
    }
    prepared.objectives[0].needs_eligibility_query = false;
    let limits = FormulaLimits::default();
    let location = Location {
        source: source.id(),
        span: source.span(),
    };
    let admission = catalog
        .component_admission(&limits, &mut counters, location)
        .unwrap();
    let components = admission
        .components(&limits, &mut counters, location)
        .unwrap();
    assert!(
        crate::formula_objective_dependencies::check(
            &prepared.rules,
            &mut prepared.objectives,
            &prepared.analysis,
            components,
            &limits,
            &mut counters,
        )
        .unwrap()
        .is_empty()
    );
    assert!(prepared.objectives[0].needs_eligibility_query);
    assert!(prepared.objectives[0].priority_sources.is_empty());
}
