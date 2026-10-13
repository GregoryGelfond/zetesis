//! Which route a reused workspace takes, and what it charges: a one-atom change
//! updates the retained evaluation, a large change or another theory evaluates
//! completely, and every check fits the plan's per-check bound.

use super::{TightWorkspace, large_change};
use crate::{
    AdmissionLimits, FormulaNodes, FormulaParts, Interpretation, Node, NodeView, Theory,
    TightCheckLimits, TightError, TightPlan, TightPlanLimits, TightResource,
};
use zetesis_cpu::Cancellation;

/// `atoms` atomic choices, a constraint over a chain of conjunctions forbidding
/// all of them together, and one forbidding the first two together.
fn choices(atoms: usize) -> Theory {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..atoms).map(Node::atom).collect(), vec![]).unwrap());
    let mut append = nodes.transaction();
    let falsum = append
        .push(NodeView::False, usize::MAX, usize::MAX)
        .unwrap();
    let mut roots = Vec::new();
    for atom in 0..atoms {
        let not = append
            .push(NodeView::Implies(atom, falsum), usize::MAX, usize::MAX)
            .unwrap();
        roots.push(
            append
                .push(NodeView::Or(&[atom, not]), usize::MAX, usize::MAX)
                .unwrap(),
        );
    }
    let mut chain = 0;
    for atom in 1..atoms {
        chain = append
            .push(NodeView::And(&[chain, atom]), usize::MAX, usize::MAX)
            .unwrap();
    }
    let both = append
        .push(NodeView::And(&[0, 1]), usize::MAX, usize::MAX)
        .unwrap();
    for body in [chain, both] {
        roots.push(
            append
                .push(NodeView::Implies(body, falsum), usize::MAX, usize::MAX)
                .unwrap(),
        );
    }
    append.commit();
    Theory::new(atoms, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap()
}

fn prepared(theory: &Theory) -> TightPlan {
    let cancellation = Cancellation::default();
    let mut plan = TightPlan::compile(theory, TightPlanLimits::default(), &cancellation).unwrap();
    assert_eq!(
        plan.prepare_updates(TightPlanLimits::default(), &cancellation)
            .result,
        Ok(true)
    );
    plan
}

fn world(theory: &Theory, atoms: impl IntoIterator<Item = usize>) -> Interpretation {
    Interpretation::new(theory, atoms).unwrap()
}

/// The verdict and charged work of checking `second` with a workspace that
/// checked `first` before, under `max_work`.
fn second_check(
    plan: &TightPlan,
    first: &Interpretation,
    second: &Interpretation,
    max_work: u64,
) -> (Result<crate::TightVerdict, TightError>, u64) {
    let cancellation = Cancellation::default();
    let mut workspace = TightWorkspace::default();
    workspace
        .check(plan, first, TightCheckLimits::default(), &cancellation)
        .result
        .unwrap();
    let attempt = workspace.check(
        plan,
        second,
        TightCheckLimits {
            max_work,
            ..TightCheckLimits::default()
        },
        &cancellation,
    );
    (attempt.result.map(|check| check.verdict), attempt.work)
}

#[test]
fn a_one_atom_change_updates_the_retained_evaluation() {
    let theory = choices(200);
    let plan = prepared(&theory);
    let (first, second) = (world(&theory, [2, 3]), world(&theory, [2, 3, 4]));
    let complete = plan
        .check_accounted(
            &second,
            TightCheckLimits::default(),
            &Cancellation::default(),
        )
        .result
        .unwrap();
    let (verdict, work) = second_check(&plan, &first, &second, u64::MAX);
    assert_eq!(verdict, Ok(complete.verdict));
    // The update reads a few nodes; a complete check reads all of them.
    assert!(work * 4 < complete.work, "{work} against {}", complete.work);
}

#[test]
fn a_large_change_evaluates_completely() {
    let theory = choices(200);
    let plan = prepared(&theory);
    assert!(large_change(theory.atom_count()) < 100);
    let (first, second) = (world(&theory, []), world(&theory, 2..102));
    let complete = plan
        .check_accounted(
            &second,
            TightCheckLimits::default(),
            &Cancellation::default(),
        )
        .result
        .unwrap();
    let (verdict, work) = second_check(&plan, &first, &second, u64::MAX);
    assert_eq!(verdict, Ok(complete.verdict));
    assert!(work >= complete.work, "{work} against {}", complete.work);
}

#[test]
fn an_equal_theory_is_evaluated_completely() {
    // The workspace keeps a handle to the first theory, so the equal second one
    // is a different instance even after the first's other owners are gone.
    let first_theory = choices(200);
    let first_plan = prepared(&first_theory);
    let cancellation = Cancellation::default();
    let mut workspace = TightWorkspace::default();
    workspace
        .check(
            &first_plan,
            &world(&first_theory, [2, 3]),
            TightCheckLimits::default(),
            &cancellation,
        )
        .result
        .unwrap();
    drop(first_plan);
    drop(first_theory);
    let theory = choices(200);
    let plan = prepared(&theory);
    let candidate = world(&theory, [2, 3, 4]);
    let complete = plan
        .check_accounted(&candidate, TightCheckLimits::default(), &cancellation)
        .result
        .unwrap();
    let reused = workspace
        .check(
            &plan,
            &candidate,
            TightCheckLimits::default(),
            &cancellation,
        )
        .result
        .unwrap();
    assert_eq!(reused.verdict, complete.verdict);
    assert!(reused.work >= complete.work);
}

#[test]
fn each_route_fits_the_plan_bound_and_refuses_one_unit_short() {
    let theory = choices(200);
    let plan = prepared(&theory);
    let bound = plan.check_work_bound();
    for (first, second) in [
        (world(&theory, [2, 3]), world(&theory, [2, 3, 4])),
        (world(&theory, []), world(&theory, 2..102)),
    ] {
        let (verdict, work) = second_check(&plan, &first, &second, u64::MAX);
        assert!(verdict.is_ok());
        assert!(u128::from(work) <= bound, "{work} against {bound}");
        assert_eq!(second_check(&plan, &first, &second, work).0, verdict);
        assert_eq!(
            second_check(&plan, &first, &second, work - 1).0,
            Err(TightError::Limit(TightResource::Work))
        );
    }
}
