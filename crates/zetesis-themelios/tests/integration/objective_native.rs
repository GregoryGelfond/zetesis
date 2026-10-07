//! Native eligibility rows retain complete-key scoring and candidate-only bounds.

use zetesis_core::{AtomCatalog, AtomPattern, Term, Value};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{AdmissionLimits, FormulaParts, Interpretation, Node, NodeView, Theory};
use zetesis_objective::{ObjectiveProgram, ObjectiveTemplate, Score};
use zetesis_test_support::programs::nullary;
use zetesis_themelios::objective_bound::{
    ObjectiveBoundErrorKind, ObjectiveBoundLimits, ObjectiveBoundResource, ObjectivePlan,
    ObjectivePlanLimits,
};

struct Fixture {
    atoms: AtomCatalog,
    theory: Theory,
    objective: ObjectiveProgram,
}

impl Fixture {
    fn new() -> Self {
        let atoms = [nullary("a"), nullary("b"), nullary("c")];
        let positive = atoms
            .iter()
            .map(|atom| AtomPattern::new(atom.predicate().clone(), vec![]).unwrap())
            .collect();
        let objective = ObjectiveProgram::new(
            vec![ObjectiveTemplate::new(
                Term::Constant(Value::Number(5)),
                0,
                vec![],
                positive,
                vec![],
            )],
            zetesis_objective::AdmissionLimits::default(),
        )
        .unwrap();
        let theory = Theory::new(
            atoms.len(),
            FormulaParts::new((0..atoms.len()).map(Node::atom).collect(), vec![]).unwrap(),
            vec![],
            AdmissionLimits::default(),
        )
        .unwrap();
        Self {
            atoms: AtomCatalog::new(atoms.to_vec()).unwrap(),
            theory,
            objective,
        }
    }

    fn plan(
        &self,
        limits: ObjectivePlanLimits,
    ) -> Result<ObjectivePlan, zetesis_themelios::objective_bound::ObjectiveBoundError> {
        ObjectivePlan::new(
            &self.theory,
            self.atoms.atoms(),
            &self.objective,
            limits,
            &Cancellation::default(),
        )
    }
}

fn score(plan: &ObjectivePlan, mask: usize) -> Score {
    let candidate = Interpretation::new(
        plan.original(),
        (0..3).filter(|index| mask & (1 << index) != 0),
    )
    .unwrap();
    plan.score(
        &candidate,
        zetesis_objective::Limits::default(),
        &Cancellation::default(),
    )
    .unwrap()
    .unwrap()
    .into_score()
}

#[test]
fn native_eligibility_counts_the_complete_key_once() {
    let fixture = Fixture::new();
    let plan = fixture.plan(ObjectivePlanLimits::default()).unwrap();
    for mask in 0..8 {
        assert_eq!(
            score(&plan, mask).costs(),
            [(0, if mask == 7 { 5 } else { 0 })]
        );
    }
}

#[test]
fn copied_native_eligibility_preserves_candidate_bounds() {
    let fixture = Fixture::new();
    let plan = fixture.plan(ObjectivePlanLimits::default()).unwrap();
    let bound = plan
        .bound(
            &score(&plan, 0),
            ObjectiveBoundLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!((0..bound.theory().view().len()).any(|index| {
        matches!(bound.theory().view().node(index).unwrap(), NodeView::And(row) if row == [0, 1, 2])
    }));
    for mask in 0..8 {
        let candidate = Interpretation::new(
            bound.theory(),
            (0..3).filter(|index| mask & (1 << index) != 0),
        )
        .unwrap();
        assert_eq!(
            zetesis_ferraris::models(
                bound.theory(),
                &candidate,
                zetesis_ferraris::Limits::default(),
                &Cancellation::default()
            )
            .unwrap(),
            mask != 7
        );
    }
}

#[test]
fn native_eligibility_obeys_the_operand_ceiling() {
    let fixture = Fixture::new();
    // Truth contributes two occurrences; the three-atom condition contributes three.
    let limits = ObjectivePlanLimits {
        max_operands: 5,
        ..ObjectivePlanLimits::default()
    };
    fixture.plan(limits).unwrap();
    let error = fixture
        .plan(ObjectivePlanLimits {
            max_operands: 4,
            ..limits
        })
        .unwrap_err();
    assert_eq!(
        error.kind(),
        ObjectiveBoundErrorKind::Limit(ObjectiveBoundResource::Operands)
    );
}

#[test]
fn native_bound_obeys_the_operand_ceiling() {
    let fixture = Fixture::new();
    let plan = fixture.plan(ObjectivePlanLimits::default()).unwrap();
    let incumbent = score(&plan, 0);
    let limits = ObjectiveBoundLimits::default();
    let reference = plan
        .bound(&incumbent, limits, &Cancellation::default())
        .unwrap();
    let mut exact = limits;
    exact.aggregate.max_operands = reference.theory().parts().occurrences();
    plan.bound(&incumbent, exact, &Cancellation::default())
        .unwrap();
    exact.aggregate.max_operands -= 1;
    let error = plan
        .bound(&incumbent, exact, &Cancellation::default())
        .unwrap_err();
    assert_eq!(
        error.kind(),
        ObjectiveBoundErrorKind::Limit(ObjectiveBoundResource::Operands)
    );
}
