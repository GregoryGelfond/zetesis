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
                &[],
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
