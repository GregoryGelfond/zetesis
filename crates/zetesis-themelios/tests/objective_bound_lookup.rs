//! Prepared objective lookup keeps original dense identities and typed keys.
//! Query work is compared separately from the fully charged preparation cost.

use zetesis_core::{
    Atom, AtomPattern, Model, Predicate, Sign, Term, Value, ValueLimits, ValueNode,
};
use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, Node, Theory, models};
use zetesis_objective::{Condition, ConditionNode, ObjectiveProgram, ObjectiveTemplate, evaluate};
use zetesis_themelios::objective_bound::{
    ObjectiveBoundErrorKind, ObjectiveBoundLimits, ObjectiveBoundResource, ObjectivePlan,
    ObjectivePlanLimits,
};

fn atom(name: &str, sign: Sign, values: Vec<Value>) -> Atom {
    Atom::new(
        Predicate::with_sign(name, values.len(), sign).unwrap(),
        values,
    )
    .unwrap()
}

fn theory(count: usize) -> Theory {
    Theory::new(
        count,
        (0..count).map(Node::Atom).collect(),
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
    let control = Control::default();
    let plan = ObjectivePlan::new(
        &original,
        &atoms,
        &objectives,
        ObjectivePlanLimits::default(),
        &control,
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
        );
        let incumbent = evaluate(
            &objectives,
            &incumbent_model,
            zetesis_objective::Limits::default(),
            &control,
        )
        .unwrap();
        assert_eq!(incumbent.score().costs(), &[(0, incumbent_cost)]);
        let bound = plan
            .bound(incumbent.score(), ObjectiveBoundLimits::default(), &control)
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
                    &control
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
    let empty = program(vec![]);
    let queries = program(vec![closed(query, 1)]);
    let work = |objectives: &ObjectiveProgram| {
        ObjectivePlan::new(
            &original,
            &atoms,
            objectives,
            ObjectivePlanLimits::default(),
            &Control::default(),
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
fn catalog_validation_and_query_refusals_preserve_failure_evidence() {
    let atoms = vec![
        atom(
            "p",
            Sign::Negative,
            vec![Value::String("shared prefix α".into())],
        ),
        atom("q", Sign::Positive, vec![]),
    ];
    let original = theory(atoms.len());
    let objectives = program(vec![closed(
        atom(
            "p",
            Sign::Negative,
            vec![Value::String("shared prefix β".into())],
        ),
        1,
    )]);
    let control = Control::default();
    let limits = ObjectivePlanLimits::default();
    let complete = ObjectivePlan::new(&original, &atoms, &objectives, limits, &control).unwrap();
    for limit in 0..complete.statistics().work {
        let error = ObjectivePlan::new(
            &original,
            &atoms,
            &objectives,
            ObjectivePlanLimits {
                max_work: limit,
                ..limits
            },
            &control,
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
        &atoms,
        &objectives,
        ObjectivePlanLimits {
            max_work: complete.statistics().work,
            ..limits
        },
        &control,
    )
    .unwrap();
    assert_eq!(complete.statistics(), repeated.statistics());
    let duplicate = [atoms[0].clone(), atoms[0].clone()];
    let error =
        ObjectivePlan::new(&original, &duplicate, &objectives, limits, &control).unwrap_err();
    assert_eq!(error.kind(), ObjectiveBoundErrorKind::AtomCatalog);
    assert_eq!(error.template_index(), None);
}
