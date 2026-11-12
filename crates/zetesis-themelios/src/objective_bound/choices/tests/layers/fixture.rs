use super::*;
use zetesis_objective::ObjectiveProgram;

pub(super) fn atoms() -> FormulaNodes {
    let mut nodes = FormulaNodes::default();
    let mut transaction = nodes.transaction();
    for atom in 0..8 {
        push(&mut transaction, NodeView::Atom(atom));
    }
    transaction.commit();
    nodes
}

pub(super) fn plan(upper: i32, active: bool) -> (ObjectivePlan, crate::RequiredChoices) {
    let mut original = atoms();
    let mut transaction = original.transaction();
    let falsum = push(&mut transaction, NodeView::False);
    let mut sources = Vec::new();
    let mut roots = Vec::new();
    for start in [0, 3] {
        let choice = push(
            &mut transaction,
            NodeView::Or(&[start, start + 1, start + 2]),
        );
        let rule = push(&mut transaction, NodeView::Implies(6, choice));
        roots.push(rule);
        roots.push(push(&mut transaction, NodeView::Implies(start, falsum)));
        sources.push(crate::formula_count_plan::RequiredGroup {
            body: 6,
            unconditional_members: true,
            members: (start..start + 3).collect(),
            lower: 1,
            upper: 3,
            bound_root: choice,
            asserted_root: rule,
            origins: vec![],
            location: crate::ProgramSite::program(),
        });
    }
    roots.push(if active {
        6
    } else {
        push(&mut transaction, NodeView::Implies(6, falsum))
    });
    let no_optional = push(&mut transaction, NodeView::Implies(7, falsum));
    roots.push(push(&mut transaction, NodeView::Or(&[7, no_optional])));
    transaction.commit();
    let original =
        Theory::new(8, original.into_parts(), roots, AdmissionLimits::default()).unwrap();
    let required = crate::RequiredChoices {
        original: original.clone(),
        groups: sources,
        capture: crate::formula_count_plan::CountPlanStatistics::default(),
        failure: None,
    };
    let positive = [1, 5, upper, 1, 5, upper]
        .into_iter()
        .enumerate()
        .map(|(condition, weight)| AggregateElement { weight, condition })
        .collect();
    let levels = BTreeMap::from([
        (1, positive),
        (
            0,
            vec![AggregateElement {
                weight: -3,
                condition: 7,
            }],
        ),
    ]);
    let plan = ObjectivePlan {
        identity: zetesis_objective::PlanIdentity::fresh(),
        original,
        objectives: ObjectiveProgram::none(),
        nodes: atoms().into_parts(),
        levels,
        choices: None,
        statistics: ObjectiveBoundStatistics::default(),
    };
    (plan, required)
}

/// Use the actual existing implication proof and disjoint-key allocation.
/// The fixture's required groups correspond exactly to its original rule roots.
pub(super) fn checked_groups(
    plan: &ObjectivePlan,
    required: &crate::RequiredChoices,
) -> Vec<Group> {
    let cancellation = Cancellation::default();
    let mut work = Work::new(ObjectivePlanLimits::default(), &cancellation);
    let mut memory = Storage::new(u128::MAX, required, None, &work).unwrap();
    let mut bridges = Bridges::new(plan, required, &mut memory, &mut work).unwrap();
    let elements = &plan.levels[&1];
    let mut level = Level {
        priority: 1,
        elements,
        allocated: vec![false; elements.len()],
    };
    required
        .groups
        .iter()
        .map(|source| {
            bridges
                .cost(source, &mut level, &mut memory, &mut work)
                .unwrap()
                .unwrap()
        })
        .collect()
}

pub(super) fn layer_plan(plan: &ObjectivePlan, groups: &[Group]) -> ObjectivePlan {
    let mut nodes = FormulaNodes::new(plan.nodes.try_clone().unwrap());
    let mut terms = Vec::new();
    for group in groups {
        let keys: Vec<_> = group
            .keys
            .iter()
            .map(|&index| plan.levels[&group.priority][index])
            .collect();
        // This fixture's activation is the same semantic atom at coordinate6
        // in each owner. General activation transport is not implemented here.
        assert_eq!(group.activation, 6);
        terms.extend(compile(&mut nodes, 6, &keys));
    }
    ObjectivePlan {
        identity: zetesis_objective::PlanIdentity::fresh(),
        original: plan.original.clone(),
        objectives: ObjectiveProgram::none(),
        nodes: nodes.into_parts(),
        levels: BTreeMap::from([(1, terms), (0, plan.levels[&0].clone())]),
        choices: None,
        statistics: ObjectiveBoundStatistics::default(),
    }
}

pub(super) fn holds(theory: &Theory, mask: usize) -> bool {
    let candidate =
        Interpretation::new(theory, (0..8).filter(|atom| mask & (1 << atom) != 0)).unwrap();
    models(
        theory,
        &candidate,
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap()
}

pub(super) fn score(plan: &ObjectivePlan, mask: usize) -> Score {
    let costs = plan
        .levels
        .iter()
        .rev()
        .map(|(&priority, elements)| {
            let value: i64 = elements
                .iter()
                .filter(|element| mask & (1 << element.condition) != 0)
                .map(|element| i64::from(element.weight))
                .sum();
            (priority, value)
        })
        .collect();
    Score::from_costs(costs).unwrap()
}

pub(super) fn bound(plan: &ObjectivePlan, incumbent: &Score, strict: bool) -> ObjectiveBound {
    let result = if strict {
        plan.strict_bound(
            incumbent,
            ObjectiveBoundLimits::default(),
            &Cancellation::default(),
        )
    } else {
        plan.bound(
            incumbent,
            ObjectiveBoundLimits::default(),
            &Cancellation::default(),
        )
    }
    .unwrap();
    assert!(result.choice_failure().is_none());
    result
}
