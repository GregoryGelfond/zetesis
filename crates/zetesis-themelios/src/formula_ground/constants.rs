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
        builder.metadata.origins(0).collect::<Vec<_>>(),
        [location(), origin(2), origin(8)]
    );
    assert_eq!(
        builder.metadata.origins(1).collect::<Vec<_>>(),
        [location(), origin(3)]
    );
    let expected = (0..2)
        .map(|atom| builder.metadata.origins(atom).collect::<Vec<_>>())
        .collect::<Vec<_>>();
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
    // One tail comparison plus one published location; spare arena capacity requires no relocation.
    limits.max_work = 2;
    let mut builder = Builder::empty(
        &limits,
        &mut budget,
        Counters::default(),
        Purpose::Theory,
        None,
    );
    builder
        .metadata
        .atom(location(), &mut builder.counters, builder.limits)
        .unwrap();
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
    refused
        .metadata
        .atom(location(), &mut refused.counters, refused.limits)
        .unwrap();
    assert!(
        matches!(refused.record_head_origins(0, &rule), Err(FormulaFailure::Limit { resource: FormulaResource::Work, observed: 2, limit: 1, location: found }) if found == next)
    );
    assert_eq!(
        refused.metadata.origins(0).collect::<Vec<_>>(),
        [location()]
    );
}

#[test]
fn guard_evidence_admission_precedes_copy_work() {
    for resource in [FormulaResource::Roots, FormulaResource::Origins] {
        let mut limits = FormulaLimits::default();
        match resource {
            FormulaResource::Roots => limits.theory.max_roots = 0,
            FormulaResource::Origins => limits.max_origin_locations = 0,
            _ => unreachable!("the two emitted-evidence bounds"),
        }
        let mut budget = Budget::new(ExpansionLimits::default(), 0);
        let mut builder = Builder::empty(
            &limits,
            &mut budget,
            Counters::default(),
            Purpose::Theory,
            None,
        );
        builder.initialize(location()).unwrap();
        let pattern =
            AtomPattern::new(zetesis_core::Predicate::new("p", 0).unwrap(), vec![]).unwrap();
        builder
            .atom(&pattern, &Binding::default(), location())
            .unwrap();
        let before = builder.counters.work;
        assert!(
            matches!(builder.support_guards(), Err(FormulaFailure::Limit { resource: found, .. }) if found == resource)
        );
        // One atom lookup and three implication nodes, with no origin-copy work.
        assert_eq!(builder.counters.work - before, 4);
        assert!(builder.roots.is_empty());
        assert!(builder.origins.is_empty());
        assert_eq!(builder.origin_count, 0);
    }
}

#[test]
fn interleaved_producers_retain_the_support_fold() {
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
    let mut nodes = Vec::new();
    for name in ["p", "q", "x", "y", "z"] {
        let pattern =
            AtomPattern::new(zetesis_core::Predicate::new(name, 0).unwrap(), vec![]).unwrap();
        nodes.push(
            builder
                .atom(&pattern, &Binding::default(), location())
                .unwrap(),
        );
    }
    let rule = RuleIr {
        head: HeadIr::Normal(None),
        body: vec![],
        body_variables: 0,
        bindings: None,
        variables: 0,
        location: location(),
        origins: vec![location()],
    };
    for (head, body) in [
        (nodes[0], nodes[2]),
        (nodes[1], nodes[4]),
        (nodes[0], nodes[3]),
        (nodes[0], nodes[2]),
    ] {
        builder.producer(head, body, &rule).unwrap();
    }
    let before = builder.nodes.len();
    builder.support_guards().unwrap();
    assert_eq!(builder.nodes[before], Node::Or(nodes[2], nodes[3]));
    assert_eq!(builder.nodes[before + 1], Node::Or(before, nodes[2]));
    assert_eq!(
        builder.nodes[before + 2],
        Node::Implies(nodes[0], before + 1)
    );
    assert_eq!(
        builder.roots.len(),
        5,
        "every atom retains its necessary guard"
    );
}
