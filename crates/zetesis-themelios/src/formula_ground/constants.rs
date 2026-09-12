//! The builder's Boolean identities and failed-initialization accounting.

use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Span};
use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, Limits, models, models_reduct};

use super::*;
use crate::ExpansionLimits;
use crate::formula_objective_dependencies::eligibility::Context;

fn location() -> Location {
    Location {
        source: SourceId::new(17),
        span: Span::empty(ByteOffset::new(3)),
    }
}

fn constants(root: usize) -> Theory {
    let limits = FormulaLimits::default();
    let mut budget = Budget::new(ExpansionLimits::default(), 0);
    let mut builder = Builder::empty(
        &limits,
        &mut budget,
        Counters::default(),
        Purpose::Theory,
        None,
    );
    builder.initialize(location()).unwrap();
    Theory::new(1, builder.nodes, vec![root], limits.theory).unwrap()
}

#[test]
fn canonical_constants_obey_original_truth() {
    for truth in [false, true] {
        let theory = constants(boolean(truth));
        for atoms in [vec![], vec![0]] {
            let interpretation = Interpretation::new(&theory, atoms).unwrap();
            assert_eq!(
                models(
                    &theory,
                    &interpretation,
                    Limits::default(),
                    &Control::default()
                )
                .unwrap(),
                truth
            );
        }
    }
}

#[test]
fn canonical_constants_obey_frozen_truth() {
    for truth in [false, true] {
        let theory = constants(boolean(truth));
        for candidate in [vec![], vec![0]] {
            let candidate = Interpretation::new(&theory, candidate).unwrap();
            for tested in [vec![], vec![0]] {
                let tested = Interpretation::new(&theory, tested).unwrap();
                assert_eq!(
                    models_reduct(
                        &theory,
                        &candidate,
                        &tested,
                        Limits::default(),
                        &Control::default()
                    )
                    .unwrap(),
                    truth
                );
            }
        }
    }
}

#[test]
fn initialization_obeys_node_ceiling() {
    for purpose in [Purpose::Theory, Purpose::Objective, Purpose::Validation] {
        for cap in 0..=2 {
            let mut limits = FormulaLimits::default();
            let resource = match purpose {
                Purpose::Theory | Purpose::Validation => {
                    limits.theory.max_nodes = cap;
                    FormulaResource::Nodes
                }
                Purpose::Objective => {
                    limits.max_objective_formula_nodes = cap;
                    FormulaResource::ObjectiveFormulaNodes
                }
            };
            let mut budget = Budget::new(ExpansionLimits::default(), 0);
            let mut builder =
                Builder::empty(&limits, &mut budget, Counters::default(), purpose, None);
            let result = builder.initialize(location());
            if cap == 2 {
                result.unwrap();
                assert_eq!(builder.nodes[FALSUM], Node::False);
                assert_eq!(builder.nodes[VERUM], Node::Implies(FALSUM, FALSUM));
            } else {
                let Err(FormulaFailure::Limit {
                    resource: actual,
                    limit,
                    observed,
                    location: origin,
                }) = result
                else {
                    panic!("expected a located node ceiling: {result:?}");
                };
                assert_eq!(actual, resource);
                assert_eq!(limit, cap as u128);
                assert_eq!(observed, cap as u128 + 1);
                assert_eq!(origin, location());
                assert_eq!(builder.nodes.len(), cap);
            }
        }
    }
}

#[test]
fn initialization_obeys_work_ceiling() {
    let limits = FormulaLimits {
        max_work: 1,
        ..FormulaLimits::default()
    };
    let mut budget = Budget::new(ExpansionLimits::default(), 0);
    let mut builder = Builder::empty(
        &limits,
        &mut budget,
        Counters::default(),
        Purpose::Theory,
        None,
    );
    assert!(matches!(
        builder.initialize(location()),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            limit: 1,
            observed: 2,
            ..
        })
    ));
    assert_eq!(builder.counters.work, 1);
    assert_eq!(builder.nodes, [Node::False]);
}

#[test]
fn scoped_initialization_retains_spent_work() {
    for purpose in [Purpose::Objective, Purpose::Validation] {
        let mut limits = FormulaLimits {
            max_objective_formula_nodes: 1,
            ..FormulaLimits::default()
        };
        limits.theory.max_nodes = 1;
        let mut budget = Budget::new(ExpansionLimits::default(), 0);
        let mut counters = Counters::default();
        counters.charge_work(7, &limits, location()).unwrap();
        let mut context = Context {
            limits: &limits,
            budget: &mut budget,
            counters: &mut counters,
            location: location(),
        };
        assert!(matches!(
            scoped_body::validate_with_purpose(
                &[],
                &crate::formula_binding::Binding::default(),
                &Support::default(),
                &mut context,
                purpose
            ),
            Err(FormulaFailure::Limit {
                limit: 1,
                observed: 2,
                ..
            })
        ));
        assert_eq!(
            counters.work, 9,
            "initialization retains both charged lookups"
        );
        counters.work(&limits, location()).unwrap();
        assert_eq!(counters.work, 10, "the restored owner remains cumulative");
    }
}

#[test]
fn producer_origins_preserve_atom_associations() {
    let limits = FormulaLimits::default();
    let mut budget = Budget::new(ExpansionLimits::default(), 0);
    let mut builder = Builder::empty(
        &limits,
        &mut budget,
        Counters::default(),
        Purpose::Theory,
        None,
    );
    builder.initialize(location()).unwrap();
    for name in ["p", "q"] {
        let pattern =
            AtomPattern::new(zetesis_core::Predicate::new(name, 0).unwrap(), vec![]).unwrap();
        builder
            .atom(&pattern, &Binding::default(), location())
            .unwrap();
    }
    let origin = |offset| Location {
        source: SourceId::new(19),
        span: Span::empty(ByteOffset::new(offset)),
    };
    let rule = |origins| RuleIr {
        head: HeadIr::Normal(None),
        body: vec![],
        body_variables: 0,
        bindings: None,
        variables: 0,
        location: location(),
        origins,
    };
    builder
        .record_head_origins(0, &rule(vec![origin(8), origin(2), origin(8)]))
        .unwrap();
    builder
        .record_head_origins(1, &rule(vec![origin(3)]))
        .unwrap();
    assert_eq!(
        builder.producer_origins[0],
        [location(), origin(2), origin(8)]
    );
    assert_eq!(builder.producer_origins[1], [location(), origin(3)]);
    let expected = builder.producer_origins.clone();
    builder.support_guards().unwrap();
    assert_eq!(builder.origins, expected);
}

#[test]
fn owned_root_provenance_transfers_its_storage() {
    let limits = FormulaLimits::default();
    let mut budget = Budget::new(ExpansionLimits::default(), 0);
    let mut builder = Builder::empty(
        &limits,
        &mut budget,
        Counters::default(),
        Purpose::Theory,
        None,
    );
    let origins = vec![location()];
    let data = origins.as_ptr();
    builder
        .root_at(FALSUM, Cow::Owned(origins), location())
        .unwrap();
    assert!(std::ptr::eq(builder.origins[0].as_ptr(), data));
}

#[test]
fn origin_insertion_obeys_the_work_ceiling() {
    let mut limits = FormulaLimits::default();
    let mut budget = Budget::new(ExpansionLimits::default(), 0);
    let next = Location {
        source: SourceId::new(20),
        span: Span::empty(ByteOffset::new(0)),
    };
    let rule = RuleIr {
        head: HeadIr::Normal(None),
        body: vec![],
        body_variables: 0,
        bindings: None,
        variables: 0,
        location: next,
        origins: vec![next],
    };
    // Full length/capacity one means one growth copy plus one appended location.
    limits.max_work = 2;
    let mut builder = Builder::empty(
        &limits,
        &mut budget,
        Counters::default(),
        Purpose::Theory,
        None,
    );
    builder.producer_origins.push(vec![location()]);
    builder.record_head_origins(0, &rule).unwrap();
    assert_eq!(builder.counters.work, 2);
    let short = FormulaLimits {
        max_work: 1,
        ..limits
    };
    let mut short_budget = Budget::new(ExpansionLimits::default(), 0);
    let mut refused = Builder::empty(
        &short,
        &mut short_budget,
        Counters::default(),
        Purpose::Theory,
        None,
    );
    refused.producer_origins.push(vec![location()]);
    assert!(
        matches!(refused.record_head_origins(0, &rule), Err(FormulaFailure::Limit { resource: FormulaResource::Work, observed: 2, limit: 1, location: found }) if found == next)
    );
    assert_eq!(refused.producer_origins[0], [location()]);
}
