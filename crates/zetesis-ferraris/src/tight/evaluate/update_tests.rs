//! When a plan builds its readers index: which route a reused workspace takes,
//! and what it charges (a one-atom change updates the retained evaluation, a
//! large change or another theory evaluates completely, an update follows rows
//! whose extremum moves, and every check fits the plan's per-check bound); what
//! building the index charges and reports; and how a reused workspace's bytes
//! are admitted.

use super::{TightWorkspace, large_change};
use crate::{
    AdmissionLimits, ExtremumCutRule, ExtremumRow, FormulaNodes, FormulaParts, Interpretation,
    Node, NodeView, Theory, TightCheckLimits, TightError, TightPlan, TightPlanLimits,
    TightResource, TightVerdict,
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

/// `atoms` atomic choices with one extremum row over atoms 0 to 3 at ranks 0 to
/// 3, and constraints against its maximum reaching 2 and its minimum falling
/// below 1.
fn extremum_choices(atoms: usize) -> Theory {
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
    let guards = append
        .push_extremum_cut_family_range(
            ExtremumRow {
                conditions: &[0, 1, 2, 3],
                ranks: &[0, 1, 2, 3],
                rank_count: 4,
            },
            &[ExtremumCutRule::Suffix(2), ExtremumCutRule::Prefix(1)],
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    for guard in guards {
        roots.push(
            append
                .push(NodeView::Implies(guard, falsum), usize::MAX, usize::MAX)
                .unwrap(),
        );
    }
    append.commit();
    Theory::new(atoms, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap()
}

/// Every incidence kind the readers index lists: atomic choices, a fact, a
/// rule whose body is a wide conjunction, and a numeric row over three atoms
/// with two guards under constraints.
fn mixed_theory() -> Theory {
    let atoms = 5;
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..atoms).map(Node::atom).collect(), vec![]).unwrap());
    let mut append = nodes.transaction();
    let falsum = append
        .push(NodeView::False, usize::MAX, usize::MAX)
        .unwrap();
    let mut roots = vec![4];
    for atom in 0..3 {
        let not = append
            .push(NodeView::Implies(atom, falsum), usize::MAX, usize::MAX)
            .unwrap();
        roots.push(
            append
                .push(NodeView::Or(&[atom, not]), usize::MAX, usize::MAX)
                .unwrap(),
        );
    }
    let wide = append
        .push(NodeView::And(&[0, 1, 2]), usize::MAX, usize::MAX)
        .unwrap();
    roots.push(
        append
            .push(NodeView::Implies(wide, 3), usize::MAX, usize::MAX)
            .unwrap(),
    );
    let guards = append
        .push_extremum_cut_family_range(
            ExtremumRow {
                conditions: &[0, 1, 2],
                ranks: &[0, 1, 2],
                rank_count: 3,
            },
            &[ExtremumCutRule::Suffix(2), ExtremumCutRule::Prefix(1)],
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    for guard in guards {
        roots.push(
            append
                .push(NodeView::Implies(guard, falsum), usize::MAX, usize::MAX)
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

/// The second checks of the two routes, a one-atom change that updates and a
/// large change that evaluates completely, with whether each updates.
fn route_pairs(theory: &Theory) -> [(Interpretation, Interpretation, bool); 2] {
    [
        (world(theory, [2, 3]), world(theory, [2, 3, 4]), true),
        (world(theory, []), world(theory, 2..102), false),
    ]
}

/// Assert the route a check of a reused workspace took: an update charges less
/// than a complete evaluation of the same candidate, a complete check at least
/// as much.
fn assert_route(plan: &TightPlan, candidate: &Interpretation, work: u64, updated: bool) {
    let complete = plan
        .check_accounted(
            candidate,
            TightCheckLimits::default(),
            &Cancellation::default(),
        )
        .work;
    assert_eq!(work < complete, updated, "{work} against {complete}");
}

#[test]
fn each_route_fits_the_plan_bound() {
    let theory = choices(200);
    let plan = prepared(&theory);
    let bound = plan.check_work_bound();
    for (first, second, updated) in route_pairs(&theory) {
        let (verdict, work) = second_check(&plan, &first, &second, u64::MAX);
        assert_route(&plan, &second, work, updated);
        assert!(verdict.is_ok());
        assert!(u128::from(work) <= bound, "{work} against {bound}");
    }
}

#[test]
fn each_route_refuses_one_unit_short_of_its_work() {
    let theory = choices(200);
    let plan = prepared(&theory);
    for (first, second, updated) in route_pairs(&theory) {
        let (verdict, work) = second_check(&plan, &first, &second, u64::MAX);
        assert_route(&plan, &second, work, updated);
        assert_eq!(second_check(&plan, &first, &second, work).0, verdict);
        assert_eq!(
            second_check(&plan, &first, &second, work - 1).0,
            Err(TightError::Limit(TightResource::Work))
        );
    }
}

#[test]
fn updates_follow_a_moving_extremum() {
    // The maximum moves from 3 to 2 to 1 and the minimum from 1 to 0 to none,
    // each time by removing the atom that held it.
    let theory = extremum_choices(200);
    let plan = prepared(&theory);
    let cancellation = Cancellation::default();
    let mut workspace = TightWorkspace::default();
    let mut stable = Vec::new();
    for (step, atoms) in [vec![3, 1], vec![2, 1], vec![1], vec![0], vec![]]
        .into_iter()
        .enumerate()
    {
        let candidate = world(&theory, atoms);
        let complete = plan
            .check_accounted(&candidate, TightCheckLimits::default(), &cancellation)
            .result
            .unwrap();
        let check = workspace
            .check(
                &plan,
                &candidate,
                TightCheckLimits::default(),
                &cancellation,
            )
            .result
            .unwrap();
        assert_eq!(check.verdict, complete.verdict);
        // After the first, complete check, each check updates.
        assert!(step == 0 || check.work < complete.work, "step {step}");
        stable.push(check.verdict == TightVerdict::Stable);
    }
    assert_eq!(stable, [false, false, true, false, true]);
}

#[test]
fn an_index_the_work_allowance_cannot_build_spends_nothing() {
    let theory = choices(200);
    let cancellation = Cancellation::default();
    let mut plan = TightPlan::compile(&theory, TightPlanLimits::default(), &cancellation).unwrap();
    let before = plan.statistics();
    let limits = TightPlanLimits {
        max_work: u64::try_from(plan.index_work_bound()).unwrap() - 1,
        ..TightPlanLimits::default()
    };
    let attempt = plan.prepare_updates(limits, &cancellation);
    assert_eq!((attempt.result, attempt.work), (Ok(false), 0));
    assert_eq!(plan.statistics(), before);
}

#[test]
fn without_numeric_rows_the_index_build_charges_its_work_bound() {
    // Each pass visits every node, operand occurrence, producer and root once.
    let theory = choices(200);
    let cancellation = Cancellation::default();
    let mut plan = TightPlan::compile(&theory, TightPlanLimits::default(), &cancellation).unwrap();
    let attempt = plan.prepare_updates(TightPlanLimits::default(), &cancellation);
    assert_eq!(attempt.result, Ok(true));
    assert_eq!(u128::from(attempt.work), plan.index_work_bound());
}

#[test]
fn an_index_builds_within_its_work_bound() {
    let theory = mixed_theory();
    let cancellation = Cancellation::default();
    let mut plan = TightPlan::compile(&theory, TightPlanLimits::default(), &cancellation).unwrap();
    let bound = plan.index_work_bound();
    let limits = TightPlanLimits {
        max_work: u64::try_from(bound).unwrap(),
        ..TightPlanLimits::default()
    };
    let attempt = plan.prepare_updates(limits, &cancellation);
    assert_eq!(attempt.result, Ok(true));
    assert!(u128::from(attempt.work) <= bound);
}

#[test]
fn a_build_the_byte_allowance_cannot_hold_spends_nothing() {
    let theory = extremum_choices(200);
    let cancellation = Cancellation::default();
    let compile =
        || TightPlan::compile(&theory, TightPlanLimits::default(), &cancellation).unwrap();
    let bound = u64::try_from(compile().index_byte_bound()).unwrap();
    for (max_bytes, built) in [(bound, true), (bound - 1, false)] {
        let mut plan = compile();
        let limits = TightPlanLimits {
            max_bytes,
            ..TightPlanLimits::default()
        };
        let attempt = plan.prepare_updates(limits, &cancellation);
        assert_eq!(attempt.result, Ok(built));
        assert_eq!(attempt.work > 0, built);
        assert_eq!(plan.readers.is_some(), built);
    }
}

/// The bytes the index build holds at once, as a plan that built its index for
/// `theory` reports them: the finished index, within the plan's resident bytes,
/// and beside it one count and one cursor per node, atom and numeric row, and
/// one row mark per 64 rows.
fn build_held(plan: &TightPlan, theory: &Theory) -> u64 {
    let (nodes, atoms, rows) = (
        theory.nodes().len() as u64,
        theory.atom_count() as u64,
        theory.parts().rows().len() as u64,
    );
    let word = size_of::<usize>() as u64;
    plan.statistics().resident_bytes
        + 2 * (nodes + atoms + rows) * word
        + rows.div_ceil(64) * size_of::<u64>() as u64
}

#[test]
fn preparing_the_index_reports_its_build_bytes() {
    // Compilation's own figure is set aside, so the reported construction bytes
    // can only come from the build.
    let theory = extremum_choices(200);
    let cancellation = Cancellation::default();
    let mut plan = TightPlan::compile(&theory, TightPlanLimits::default(), &cancellation).unwrap();
    plan.statistics.construction_bytes = 0;
    assert_eq!(
        plan.prepare_updates(TightPlanLimits::default(), &cancellation)
            .result,
        Ok(true)
    );
    assert_eq!(
        plan.statistics().construction_bytes,
        build_held(&plan, &theory)
    );
}

#[test]
fn construction_bytes_keep_the_larger_of_compilation_and_the_build() {
    // Here compilation's figure exceeds the build's, so the larger one is
    // compilation's and the build leaves it in place.
    let theory = extremum_choices(200);
    let cancellation = Cancellation::default();
    let mut plan = TightPlan::compile(&theory, TightPlanLimits::default(), &cancellation).unwrap();
    let compiled = plan.statistics().construction_bytes;
    assert_eq!(
        plan.prepare_updates(TightPlanLimits::default(), &cancellation)
            .result,
        Ok(true)
    );
    let held = build_held(&plan, &theory);
    assert!(compiled > held);
    assert_eq!(plan.statistics().construction_bytes, compiled.max(held));
}

/// A workspace that checked `[2, 3]` and then updated to `[2, 3, 4]`, so it
/// holds a summarized evaluation and the scratch of an update.
fn warmed(plan: &TightPlan, theory: &Theory) -> TightWorkspace {
    let cancellation = Cancellation::default();
    let mut workspace = TightWorkspace::default();
    for atoms in [vec![2, 3], vec![2, 3, 4]] {
        workspace
            .check(
                plan,
                &world(theory, atoms),
                TightCheckLimits::default(),
                &cancellation,
            )
            .result
            .unwrap();
    }
    workspace
}

/// The bytes `workspace` holds under `plan`: the plan's resident bytes and the
/// workspace's retained capacity.
fn held(plan: &TightPlan, workspace: &TightWorkspace) -> u64 {
    u64::try_from(u128::from(plan.statistics().resident_bytes) + workspace.retained_bytes())
        .unwrap()
}

/// Default check limits with a byte allowance of `max_bytes`.
fn within(max_bytes: u64) -> TightCheckLimits {
    TightCheckLimits {
        max_bytes,
        ..TightCheckLimits::default()
    }
}

#[test]
fn an_update_reports_the_bytes_it_holds_after_its_last_reservation() {
    // The first update reserves its worklist, which its report includes.
    let theory = choices(200);
    let plan = prepared(&theory);
    let cancellation = Cancellation::default();
    let mut workspace = TightWorkspace::default();
    let limits = TightCheckLimits::default();
    workspace
        .check(&plan, &world(&theory, [2, 3]), limits, &cancellation)
        .result
        .unwrap();
    let second = world(&theory, [2, 3, 4]);
    let update = workspace
        .check(&plan, &second, limits, &cancellation)
        .result
        .unwrap();
    assert_route(&plan, &second, update.work, true);
    assert_eq!(update.logical_bytes, held(&plan, &workspace));
}

#[test]
fn an_update_beyond_the_byte_allowance_is_refused_before_its_work() {
    // Twin workspaces with the same history: at exactly the bytes held, the
    // third check updates (which pins the route); one byte short, it is
    // refused with no work charged. One changed word fits the changed-word
    // list's first reservation and the few queued nodes the worklist's, so
    // the update grows nothing.
    let theory = choices(200);
    let plan = prepared(&theory);
    let cancellation = Cancellation::default();
    let third = world(&theory, [2, 3, 4, 5]);
    let holdings = held(&plan, &warmed(&plan, &theory));
    let admitted = warmed(&plan, &theory).check(&plan, &third, within(holdings), &cancellation);
    assert!(admitted.result.is_ok());
    assert_route(&plan, &third, admitted.work, true);
    let refused = warmed(&plan, &theory).check(&plan, &third, within(holdings - 1), &cancellation);
    assert_eq!(
        refused.result.map(|check| check.verdict),
        Err(TightError::Limit(TightResource::Bytes))
    );
    assert_eq!(refused.work, 0);
}

#[test]
fn a_storage_refusal_at_the_start_keeps_the_retained_evaluation() {
    // The start's admission precedes the take of the retained evaluation, so
    // its refusal changes nothing, and the next check, given the bytes it
    // needs, updates. No work charged marks the refusal as the start's
    // admission: a refused growth, like the final admission, follows charged
    // work.
    let theory = choices(200);
    let plan = prepared(&theory);
    let cancellation = Cancellation::default();
    let mut workspace = warmed(&plan, &theory);
    let third = world(&theory, [2, 3, 4, 5]);
    let short = within(held(&plan, &workspace) - 1);
    let refused = workspace.check(&plan, &third, short, &cancellation);
    assert_eq!(
        refused.result.map(|check| check.verdict),
        Err(TightError::Limit(TightResource::Bytes))
    );
    assert_eq!(refused.work, 0);
    let after = workspace.check(&plan, &third, TightCheckLimits::default(), &cancellation);
    assert!(after.result.is_ok());
    assert_route(&plan, &third, after.work, true);
}

#[test]
fn a_growth_is_refused_before_it_is_reserved() {
    // The first update grows its empty changed-word list, under an allowance
    // of exactly the bytes held. The one charged operation, the comparison of
    // the first candidate word, places the refusal at that growth: the start's
    // admission charges nothing, and the final one follows more work. The
    // retained capacity shows that nothing was reserved.
    let theory = choices(200);
    let plan = prepared(&theory);
    let cancellation = Cancellation::default();
    let mut workspace = TightWorkspace::default();
    workspace
        .check(
            &plan,
            &world(&theory, [2, 3]),
            TightCheckLimits::default(),
            &cancellation,
        )
        .result
        .unwrap();
    let before = workspace.retained_bytes();
    let limits = within(held(&plan, &workspace));
    let attempt = workspace.check(&plan, &world(&theory, [2, 3, 4]), limits, &cancellation);
    assert_eq!(
        attempt.result.map(|check| check.verdict),
        Err(TightError::Limit(TightResource::Bytes))
    );
    assert_eq!(attempt.work, 1);
    assert_eq!(workspace.retained_bytes(), before);
}
