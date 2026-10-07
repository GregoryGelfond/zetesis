//! Prepared objective lookup keeps original dense identities and typed keys.
//! Query work is compared separately from the fully charged preparation cost.

use zetesis_core::{
    Atom, AtomPattern, Filter, Model, Predicate, Sign, Term, Value, ValueLimits, ValueNode,
};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Node, Theory, models};
use zetesis_objective::{Condition, ConditionNode, ObjectiveProgram, ObjectiveTemplate, evaluate};
use zetesis_test_support::programs::signed as atom;
use zetesis_themelios::objective_bound::{
    ObjectiveBoundErrorKind, ObjectiveBoundLimits, ObjectiveBoundResource, ObjectivePlan,
    ObjectivePlanLimits,
};

fn theory(count: usize) -> Theory {
    Theory::new(
        count,
        zetesis_ferraris::FormulaParts::new((0..count).map(Node::atom).collect(), vec![]).unwrap(),
        vec![],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap()
}

fn program(templates: Vec<ObjectiveTemplate>) -> ObjectiveProgram {
    ObjectiveProgram::new(templates, zetesis_objective::AdmissionLimits::default()).unwrap()
}

fn closed(query: Atom, weight: i32) -> ObjectiveTemplate {
    ObjectiveTemplate::new(
        Term::Constant(Value::Number(weight)),
        0,
        vec![Term::Constant(Value::Number(weight))],
        vec![],
        vec![],
    )
    .with_condition(Condition::new(vec![ConditionNode::Atom(query)]))
}

fn shuffled_atoms() -> Vec<Atom> {
    vec![
        atom(
            "p",
            Sign::Positive,
            vec![
                Value::from_nodes(
                    vec![
                        ValueNode::Function {
                            name: "f".into(),
                            sign: Sign::Negative,
                            arity: 1,
                        },
                        ValueNode::String("é".into()),
                    ],
                    ValueLimits::default(),
                )
                .unwrap(),
            ],
        ),
        atom("p", Sign::Negative, vec![Value::Symbol("x".into())]),
        atom("p", Sign::Positive, vec![]),
        atom("p", Sign::Positive, vec![Value::String("x".into())]),
        atom("p", Sign::Positive, vec![Value::Symbol("x".into())]),
    ]
}

#[test]
fn shuffled_catalog_bounds_match_independent_mask_costs() {
    let atoms = shuffled_atoms();
    let original = theory(atoms.len());
    let catalog = zetesis_core::AtomCatalog::new(atoms.clone()).unwrap();
    let mut templates: Vec<_> = atoms
        .iter()
        .enumerate()
        .map(|(index, atom)| closed(atom.clone(), 1 << index))
        .collect();
    let lifted = ObjectiveTemplate::new(
        Term::Constant(Value::Number(2)),
        0,
        vec![Term::Variable(0)],
        vec![AtomPattern::new(Predicate::new("p", 1).unwrap(), vec![Term::Variable(0)]).unwrap()],
        vec![],
    );
    templates.extend([lifted.clone(), lifted]);
    // Absence, a sign mismatch and an arity mismatch must never alias a row.
    templates.extend([
        closed(atom("outside", Sign::Positive, vec![]), 128),
        closed(
            atom("p", Sign::Negative, vec![Value::String("x".into())]),
            256,
        ),
        closed(
            atom("p", Sign::Positive, vec![Value::Symbol("x".into()); 2]),
            512,
        ),
    ]);
    let objectives = program(templates);
    let cancellation = Cancellation::default();
    let plan = ObjectivePlan::new(
        &original,
        catalog.atoms(),
        &objectives,
        ObjectivePlanLimits::default(),
        &cancellation,
    )
    .unwrap();
    let expected: Vec<i64> = (0_usize..32)
        .map(|mask| {
            (0..5)
                .filter(|index| mask & (1 << index) != 0)
                .map(|index| 1_i64 << index)
                .sum::<i64>()
                + [0, 3, 4]
                    .into_iter()
                    .filter(|index| mask & (1 << index) != 0)
                    .map(|_| 2_i64)
                    .sum::<i64>()
        })
        .collect();
    for (incumbent_mask, &incumbent_cost) in expected.iter().enumerate() {
        let incumbent_model = Model::new(
            atoms
                .iter()
                .enumerate()
                .filter(|(index, _)| incumbent_mask & (1 << index) != 0)
                .map(|(_, atom)| atom.clone()),
        )
        .unwrap();
        let incumbent = evaluate(
            &objectives,
            &incumbent_model,
            zetesis_objective::Limits::default(),
            &cancellation,
        )
        .unwrap();
        assert_eq!(incumbent.score().costs(), &[(0, incumbent_cost)]);
        let bound = plan
            .bound(
                incumbent.score(),
                ObjectiveBoundLimits::default(),
                &cancellation,
            )
            .unwrap();
        assert!(bound.original().same_instance(&original));
        assert_eq!(bound.theory().atom_count(), atoms.len());
        for (mask, &cost) in expected.iter().enumerate() {
            let candidate = Interpretation::new(
                bound.theory(),
                (0..atoms.len()).filter(|index| mask & (1 << index) != 0),
            )
            .unwrap();
            assert_eq!(
                models(
                    bound.theory(),
                    &candidate,
                    zetesis_ferraris::Limits::default(),
                    &cancellation
                )
                .unwrap(),
                cost <= incumbent_cost,
                "mask={mask}, incumbent={incumbent_mask}"
            );
        }
    }
}

fn prepared_work(noise: i32) -> (u64, u64) {
    let query = atom("zz_target", Sign::Negative, vec![Value::String("é".into())]);
    let mut atoms: Vec<_> = (0..noise)
        .map(|index| atom("a_noise", Sign::Positive, vec![Value::Number(index)]))
        .collect();
    atoms.push(query.clone());
    let original = theory(atoms.len());
    let catalog = zetesis_core::AtomCatalog::new(atoms).unwrap();
    let empty = program(vec![]);
    let queries = program(vec![closed(query, 1)]);
    let work = |objectives: &ObjectiveProgram| {
        ObjectivePlan::new(
            &original,
            catalog.atoms(),
            objectives,
            ObjectivePlanLimits::default(),
            &Cancellation::default(),
        )
        .unwrap()
        .statistics()
        .work
    };
    let preparation = work(&empty);
    (preparation, work(&queries) - preparation)
}

#[test]
fn query_work_excludes_unrelated_rows_without_hiding_preparation() {
    let (small_preparation, small_query) = prepared_work(100);
    let (large_preparation, large_query) = prepared_work(1_000);
    assert!(large_preparation > small_preparation);
    assert!(small_query > 0);
    assert!(large_query < 2 * small_query);
}

#[test]
fn every_plan_work_cutoff_preserves_its_failure_prefix() {
    let atoms = vec![
        atom(
            "p",
            Sign::Negative,
            vec![Value::String("shared prefix α".into())],
        ),
        atom("q", Sign::Positive, vec![]),
    ];
    let original = theory(atoms.len());
    let catalog = zetesis_core::AtomCatalog::new(atoms).unwrap();
    let objectives = program(vec![closed(
        atom(
            "p",
            Sign::Negative,
            vec![Value::String("shared prefix β".into())],
        ),
        1,
    )]);
    let cancellation = Cancellation::default();
    let limits = ObjectivePlanLimits::default();
    let complete = ObjectivePlan::new(
        &original,
        catalog.atoms(),
        &objectives,
        limits,
        &cancellation,
    )
    .unwrap();
    for limit in 0..complete.statistics().work {
        let error = ObjectivePlan::new(
            &original,
            catalog.atoms(),
            &objectives,
            ObjectivePlanLimits {
                max_work: limit,
                ..limits
            },
            &cancellation,
        )
        .unwrap_err();
        assert_eq!(
            error.kind(),
            ObjectiveBoundErrorKind::Limit(ObjectiveBoundResource::Work)
        );
        assert_eq!(error.statistics().work, limit);
    }
    let repeated = ObjectivePlan::new(
        &original,
        catalog.atoms(),
        &objectives,
        ObjectivePlanLimits {
            max_work: complete.statistics().work,
            ..limits
        },
        &cancellation,
    )
    .unwrap();
    assert_eq!(complete.statistics(), repeated.statistics());
}

#[test]
fn duplicate_catalog_atoms_refuse_planning() {
    let atom = atom("p", Sign::Positive, vec![]);
    let duplicate = zetesis_core::AtomCatalog::new(vec![atom.clone(), atom]).unwrap();
    let error = ObjectivePlan::new(
        &theory(2),
        duplicate.atoms(),
        &program(Vec::new()),
        ObjectivePlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), ObjectiveBoundErrorKind::AtomCatalog);
    assert_eq!(error.template_index(), None);
}

fn joined_condition() -> (Vec<Atom>, ObjectiveProgram) {
    let atoms = vec![
        atom(
            "p",
            Sign::Positive,
            vec![Value::String("x".into()), Value::Infimum],
        ),
        atom(
            "p",
            Sign::Positive,
            vec![Value::String("x".into()), Value::Supremum],
        ),
        atom("enabled", Sign::Negative, vec![]),
        atom("q", Sign::Positive, vec![Value::Infimum]),
        atom(
            "p",
            Sign::Positive,
            vec![Value::String("y".into()), Value::Infimum],
        ),
    ];
    // The condition is exactly not -enabled. Both templates contribute the same
    // (priority, weight, tuple) key, so either successful join contributes once.
    let condition = Condition::new(vec![
        ConditionNode::Atom(atoms[2].clone()),
        ConditionNode::Not(0),
        ConditionNode::Boolean(false),
        ConditionNode::Or(1, 2),
        ConditionNode::Boolean(true),
        ConditionNode::And(3, 4),
    ]);
    let pattern = |name, terms: Vec<Term>| {
        AtomPattern::new(Predicate::new(name, terms.len()).unwrap(), terms).unwrap()
    };
    let first = ObjectiveTemplate::new(
        Term::Constant(Value::Number(3)),
        0,
        vec![Term::Variable(1)],
        vec![
            pattern("p", vec![Term::Variable(0), Term::Variable(1)]),
            pattern("q", vec![Term::Variable(1)]),
        ],
        vec![Filter::Eq(
            Term::Variable(0),
            Term::Constant(Value::String("x".into())),
        )],
    )
    .with_condition(condition.clone());
    let second = ObjectiveTemplate::new(
        Term::Constant(Value::Number(3)),
        0,
        vec![Term::Constant(Value::Infimum)],
        vec![
            pattern(
                "p",
                vec![
                    Term::Constant(Value::String("y".into())),
                    Term::Constant(Value::Infimum),
                ],
            ),
            pattern("q", vec![Term::Constant(Value::Infimum)]),
        ],
        vec![],
    )
    .with_condition(condition);
    (atoms, program(vec![first, second]))
}

fn joined_cost(mask: usize) -> i64 {
    let contains = |position: usize| mask & (1 << position) != 0;
    if !contains(2) && contains(3) && (contains(0) || contains(4)) {
        3
    } else {
        0
    }
}

#[test]
fn joined_conditions_preserve_exact_bound_costs() {
    let (atoms, objectives) = joined_condition();
    let original = theory(atoms.len());
    let catalog = zetesis_core::AtomCatalog::new(atoms.clone()).unwrap();
    let cancellation = Cancellation::default();
    let plan = ObjectivePlan::new(
        &original,
        catalog.atoms(),
        &objectives,
        ObjectivePlanLimits::default(),
        &cancellation,
    )
    .unwrap();
    let scores: Vec<_> = (0..32)
        .map(|mask| {
            let model = Model::new(
                atoms
                    .iter()
                    .enumerate()
                    .filter(|(position, _)| mask & (1 << position) != 0)
                    .map(|(_, atom)| atom.clone()),
            )
            .unwrap();
            let evaluated = evaluate(
                &objectives,
                &model,
                zetesis_objective::Limits::default(),
                &cancellation,
            )
            .unwrap();
            assert_eq!(evaluated.score().costs(), &[(0, joined_cost(mask))]);
            evaluated.score().clone()
        })
        .collect();
    // These incumbents represent both possible cost classes. Every arbitrary
    // interpretation is checked, independently of any answer-set certification.
    for incumbent_mask in [0, 9] {
        let bound = plan
            .bound(
                &scores[incumbent_mask],
                ObjectiveBoundLimits::default(),
                &cancellation,
            )
            .unwrap();
        assert!(bound.original().same_instance(&original));
        assert_eq!(bound.theory().atom_count(), atoms.len());
        for mask in 0..32 {
            let interpretation = Interpretation::new(
                bound.theory(),
                (0..atoms.len()).filter(|position| mask & (1 << position) != 0),
            )
            .unwrap();
            assert_eq!(
                models(
                    bound.theory(),
                    &interpretation,
                    zetesis_ferraris::Limits::default(),
                    &cancellation
                )
                .unwrap(),
                joined_cost(mask) <= joined_cost(incumbent_mask),
                "mask={mask}, incumbent={incumbent_mask}",
            );
        }
    }
}

#[test]
fn joined_planning_refuses_every_incomplete_work_prefix() {
    let (atoms, objectives) = joined_condition();
    let original = theory(atoms.len());
    let catalog = zetesis_core::AtomCatalog::new(atoms).unwrap();
    let run = |max_work| {
        ObjectivePlan::new(
            &original,
            catalog.atoms(),
            &objectives,
            ObjectivePlanLimits {
                max_work,
                ..ObjectivePlanLimits::default()
            },
            &Cancellation::default(),
        )
    };
    let complete = run(ObjectivePlanLimits::default().max_work).unwrap();
    let required = complete.statistics().work;
    assert!(required > 0 && required < 10_000, "bounded prefix control");
    let mut failed_in_template = false;
    for limit in 0..required {
        let error = run(limit).unwrap_err();
        assert_eq!(
            error.kind(),
            ObjectiveBoundErrorKind::Limit(ObjectiveBoundResource::Work)
        );
        assert_eq!(error.statistics().work, limit);
        failed_in_template |= error.template_index().is_some();
    }
    assert!(
        failed_in_template,
        "the prefixes must enter actual join compilation"
    );
    assert_eq!(run(required).unwrap().statistics(), complete.statistics());
}

#[test]
fn incomplete_catalogs_cannot_supply_original_atom_ids() {
    let atoms = [atom("p", Sign::Positive, vec![])];
    let catalog = zetesis_core::AtomCatalog::new(atoms.to_vec()).unwrap();
    for atom_count in [0, 2] {
        let error = ObjectivePlan::new(
            &theory(atom_count),
            catalog.atoms(),
            &program(vec![]),
            ObjectivePlanLimits::default(),
            &Cancellation::default(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), ObjectiveBoundErrorKind::AtomCatalog);
        assert_eq!(error.template_index(), None);
    }
}
