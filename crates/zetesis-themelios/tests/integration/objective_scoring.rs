//! Prepared eligibility must give the same score as complete contribution evidence.

use zetesis_core::{Atom, AtomCatalog, Model, Term, Value};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Node, Theory};
use zetesis_objective::{
    Condition, ConditionNode, Limits, ObjectiveProgram, ObjectiveTemplate, evaluate,
};
use zetesis_test_support::programs::nullary as atom;
use zetesis_themelios::objective_bound::{
    ObjectiveBoundErrorKind, ObjectiveBoundResource, ObjectivePlan, ObjectivePlanLimits,
    ObjectiveScoreErrorKind,
};

struct Fixture {
    catalog: AtomCatalog,
    theory: Theory,
    program: ObjectiveProgram,
    plan: ObjectivePlan,
}
impl Fixture {
    fn new() -> Self {
        let atoms = [atom("a"), atom("b"), atom("c")];
        let catalog = AtomCatalog::new(atoms.to_vec()).unwrap();
        let theory = Theory::new(
            3,
            vec![Node::Atom(0), Node::Atom(1), Node::Atom(2)],
            vec![],
            zetesis_ferraris::AdmissionLimits::default(),
        )
        .unwrap();
        let row = |weight, priority, tuple, condition: Vec<ConditionNode>| {
            ObjectiveTemplate::new(
                Term::Constant(Value::Number(weight)),
                priority,
                vec![Term::Constant(Value::Number(tuple))],
                vec![],
                vec![],
            )
            .with_condition(Condition::new(condition))
        };
        let present = |atom: &Atom| vec![ConditionNode::Atom(atom.clone())];
        let program = ObjectiveProgram::new(
            vec![
                row(3, 2, 0, present(&atoms[0]))
                    .with_weight_polarity(zetesis_objective::WeightPolarity::Negated),
                row(-3, 2, 0, present(&atoms[1])), // One key, OR eligibility.
                row(-3, 2, 1, present(&atoms[0])), // Equal weight, distinct tuple.
                row(5, 2, 0, present(&atoms[2])),
                row(
                    1,
                    0,
                    0,
                    vec![ConditionNode::Atom(atoms[0].clone()), ConditionNode::Not(0)],
                ),
                row(100, 7, 0, present(&atom("outside"))),
                row(
                    2,
                    1,
                    0,
                    vec![
                        ConditionNode::Atom(atoms[0].clone()),
                        ConditionNode::Atom(atoms[2].clone()),
                        ConditionNode::Or(0, 1),
                        ConditionNode::Atom(atoms[1].clone()),
                        ConditionNode::And(2, 3),
                    ],
                ),
            ],
            zetesis_objective::AdmissionLimits::default(),
        )
        .unwrap();
        let plan = ObjectivePlan::new(
            &theory,
            catalog.atoms(),
            &program,
            ObjectivePlanLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        Self {
            catalog,
            theory,
            program,
            plan,
        }
    }
    fn candidate(&self, mask: usize) -> Interpretation {
        Interpretation::new(
            &self.theory,
            (0..3).filter(|index| mask & (1 << index) != 0),
        )
        .unwrap()
    }
    fn detailed(&self, mask: usize) -> zetesis_objective::Score {
        let model = Model::from_positions(&self.catalog, self.candidate(mask).atoms()).unwrap();
        evaluate(
            &self.program,
            &model,
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap()
        .into_score()
    }
}

#[test]
fn prepared_scores_match_complete_contribution_scores() {
    let fixture = Fixture::new();
    for mask in 0..8 {
        let score = fixture
            .plan
            .score(
                &fixture.candidate(mask),
                Limits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .unwrap()
            .into_score();
        assert_eq!(score, fixture.detailed(mask), "mask={mask}");
        let a = mask & 1 != 0;
        let b = mask & 2 != 0;
        let c = mask & 4 != 0;
        let expected = -3 * i64::from(a || b) - 3 * i64::from(a) + 5 * i64::from(c);
        assert_eq!(
            score.costs(),
            [
                (7, 0),
                (2, expected),
                (1, 2 * i64::from((a || c) && b)),
                (0, i64::from(!a))
            ]
        );
    }
}

#[test]
fn equal_structure_does_not_authorize_another_owner() {
    let fixture = Fixture::new();
    let foreign = Theory::new(
        fixture.theory.atom_count(),
        fixture.theory.nodes().to_vec(),
        fixture.theory.roots().to_vec(),
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    let candidate = Interpretation::new(&foreign, [0]).unwrap();
    let error = fixture
        .plan
        .score(&candidate, Limits::default(), &Cancellation::default())
        .unwrap_err();
    assert_eq!(error.kind(), ObjectiveScoreErrorKind::WrongTheory);
    assert_eq!(error.work(), 0);
}

#[test]
fn every_prepared_score_cutoff_keeps_its_prefix() {
    let fixture = Fixture::new();
    let cancellation = Cancellation::default();
    let candidate = fixture.candidate(7);
    let complete = fixture
        .plan
        .score(&candidate, Limits::default(), &cancellation)
        .unwrap()
        .unwrap();
    for cutoff in 0..complete.work() {
        let error = fixture
            .plan
            .score(
                &candidate,
                Limits {
                    max_work: cutoff,
                    ..Default::default()
                },
                &cancellation,
            )
            .unwrap_err();
        assert_eq!(error.work(), cutoff);
        match error.kind() {
            ObjectiveScoreErrorKind::Eligibility(error) => assert_eq!(
                error.kind(),
                ObjectiveBoundErrorKind::Limit(ObjectiveBoundResource::Work)
            ),
            ObjectiveScoreErrorKind::Reduction(error) => assert_eq!(
                error.kind(),
                zetesis_objective::ErrorKind::Stopped(zetesis_objective::Stop::WorkLimit)
            ),
            ObjectiveScoreErrorKind::WrongTheory => panic!("original owner was preserved"),
        }
    }
    let exact = fixture
        .plan
        .score(
            &candidate,
            Limits {
                max_work: complete.work(),
                ..Default::default()
            },
            &cancellation,
        )
        .unwrap()
        .unwrap();
    assert_eq!(exact.work(), complete.work());
    assert_eq!(exact.into_score(), complete.into_score());
}

#[test]
fn population_ceilings_decline_without_spending_work() {
    let fixture = Fixture::new();
    for limits in [
        Limits {
            max_bindings: 0,
            ..Default::default()
        },
        Limits {
            max_keys: 0,
            ..Default::default()
        },
        Limits {
            max_key_bytes: 0,
            ..Default::default()
        },
    ] {
        assert!(
            fixture
                .plan
                .score(&fixture.candidate(0), limits, &Cancellation::default())
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(
        fixture
            .plan
            .score(
                &fixture.candidate(7),
                Limits::default(),
                &Cancellation::default()
            )
            .unwrap()
            .unwrap()
            .into_score(),
        fixture.detailed(7)
    );
}

#[test]
fn cancellation_precedes_population_fallback() {
    let fixture = Fixture::new();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let error = fixture
        .plan
        .score(
            &fixture.candidate(0),
            Limits {
                max_bindings: 0,
                ..Default::default()
            },
            &cancellation,
        )
        .unwrap_err();
    assert!(
        matches!(error.kind(), ObjectiveScoreErrorKind::Eligibility(error) if matches!(error.kind(), ObjectiveBoundErrorKind::Control(_)))
    );
    assert_eq!(error.work(), 0);
}
