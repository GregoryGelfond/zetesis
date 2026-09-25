//! Optional search bounds preserve closed objective-query costs and every tie.

use std::cmp::Ordering;

use zetesis_core::{Atom, AtomPattern, Model, Predicate, Term, Value};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Node, Theory, models};
use zetesis_objective::{Condition, ConditionNode, ObjectiveProgram, ObjectiveTemplate, evaluate};
use zetesis_themelios::objective_bound::{
    ObjectiveBoundLimits, ObjectivePlan, ObjectivePlanLimits,
};

fn atom(name: &str) -> Atom {
    Atom::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap()
}

fn row(weight: i32, priority: i32, condition: Vec<ConditionNode>) -> ObjectiveTemplate {
    ObjectiveTemplate::new(
        Term::Constant(Value::Number(weight)),
        priority,
        vec![],
        vec![],
        vec![],
    )
    .with_condition(Condition::new(condition))
}

fn objective(atoms: &[Atom; 3]) -> ObjectiveProgram {
    ObjectiveProgram::new(
        vec![
            row(
                -3,
                2,
                vec![ConditionNode::Atom(atoms[0].clone()), ConditionNode::Not(0)],
            ),
            row(
                2,
                2,
                vec![
                    ConditionNode::Atom(atoms[1].clone()),
                    ConditionNode::Atom(atoms[2].clone()),
                    ConditionNode::And(0, 1),
                ],
            ),
            row(
                1,
                1,
                vec![
                    ConditionNode::Atom(atoms[0].clone()),
                    ConditionNode::Atom(atoms[2].clone()),
                    ConditionNode::Or(0, 1),
                ],
            ),
            row(100, 7, vec![ConditionNode::Atom(atom("outside"))]),
            ObjectiveTemplate::new(
                Term::Constant(Value::Number(1)),
                1,
                vec![],
                vec![AtomPattern::new(atoms[2].predicate().clone(), vec![]).unwrap()],
                vec![],
            ),
        ],
        zetesis_objective::AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn closed_query_bounds_preserve_all_candidate_ties() {
    let atoms = [atom("a"), atom("b"), atom("c")];
    let original = Theory::new(
        3,
        vec![Node::Atom(0), Node::Atom(1), Node::Atom(2)],
        vec![],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    let objective = objective(&atoms);
    let cancellation = Cancellation::default();
    let catalog = zetesis_core::AtomCatalog::new(atoms.to_vec()).unwrap();
    let plan = ObjectivePlan::new(
        &original,
        catalog.atoms(),
        &objective,
        ObjectivePlanLimits::default(),
        &cancellation,
    )
    .unwrap();
    let scores: Vec<_> = (0..8)
        .map(|mask| {
            let model = Model::new(
                atoms
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| mask & (1 << index) != 0)
                    .map(|(_, atom)| atom.clone()),
            )
            .unwrap();
            evaluate(
                &objective,
                &model,
                zetesis_objective::Limits::default(),
                &cancellation,
            )
            .unwrap()
            .score()
            .clone()
        })
        .collect();
    for incumbent in &scores {
        let bound = plan
            .bound(incumbent, ObjectiveBoundLimits::default(), &cancellation)
            .unwrap();
        assert!(bound.original().same_instance(&original));
        assert_eq!(bound.theory().atom_count(), atoms.len());
        for (mask, score) in scores.iter().enumerate() {
            let candidate = Interpretation::new(
                bound.theory(),
                (0..3).filter(|index| mask & (1 << index) != 0),
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
                score.compare_costs(incumbent) != Ordering::Greater,
                "mask={mask}, incumbent={:?}",
                incumbent.costs()
            );
        }
    }
}
