use super::*;

fn classifications(source: &str, strategy: JoinStrategy) -> Vec<(zetesis_core::Atom, u8)> {
    let preparation = testing::prepare(source);
    let program = preparation.program;
    assert!(program.rules_without_facts().iter().any(|rule| {
        rule.body.iter().any(|literal| matches!(literal, LiteralIr::Compare(_)))
            || matches!(&rule.head, HeadIr::Choice(group) if group.elements.iter().any(|element| element.condition.iter().any(|literal| matches!(literal, LiteralIr::Compare(_)))))
    }), "the lowered producer really retains an ordinary comparison");
    let mut budget = preparation.budget;
    let observed = Work::worker(true);
    let mut counters = Counters::resume(preparation.accounting, observed.clone());
    let mut catalog = crate::formula_support::build(
        preparation.catalog,
        &program,
        None,
        &preparation.limits,
        &mut budget,
        &mut counters,
        preparation.location,
    )
    .unwrap();
    let (completed, mut append) = catalog
        .split(&preparation.limits, &mut counters, preparation.location)
        .unwrap();
    let queries = completed
        .queries(
            strategy,
            &preparation.limits,
            &counters,
            preparation.location,
        )
        .unwrap();
    let mut computation = Computation::new(&mut append, queries.support());
    let presence = crate::formula_objective_dependencies::check_presence(
        &program,
        queries.support(),
        &mut computation,
        &preparation.limits,
        &mut budget,
        &mut counters,
    )
    .unwrap();
    // Attribute the actual route only to the eligibility traversal below.
    let before_table_probes = observed.snapshot().table_probes.unwrap();
    let mut context = Context {
        computation: &mut computation,
        limits: &preparation.limits,
        budget: &mut budget,
        counters: &mut counters,
        location: preparation.location,
    };
    let owner = SourceEligibility::build_from_conditions(
        &program,
        &queries,
        presence.retained_entries(),
        program
            .objectives
            .iter()
            .map(|objective| objective.condition.literals()),
        &mut context,
    )
    .unwrap();
    assert!(owner.round.is_none());
    if strategy == JoinStrategy::Table {
        assert!(
            observed.snapshot().table_probes.unwrap() > before_table_probes,
            "the completed positive relation must really use the requested table route"
        );
    }
    (0..owner.atoms.len())
        .map(|index| {
            let atom = owner
                .atoms
                .atom(
                    index,
                    context.computation,
                    context.limits,
                    context.counters,
                    context.location,
                )
                .unwrap()
                .to_atom(ValueLimits::default())
                .unwrap();
            let activity = match owner.activity[index] {
                Activity::Absent => 0,
                Activity::Optional => 1,
                Activity::Required => 2,
            };
            (atom, activity)
        })
        .collect()
}

#[test]
fn normal_and_choice_producers_preserve_complete_activity_in_both_join_modes() {
    for (source, expected) in [
        ("d(1;2).p(X):-d(X),X+1<3.#minimize{X@1:p(X)}.", 2),
        (
            "d(1;2).{p(X):d(X),X+1<3}.#minimize{X@1:p(X),not not p(X)}.",
            1,
        ),
    ] {
        let indexed = classifications(source, JoinStrategy::Indexed);
        let table = classifications(source, JoinStrategy::Table);
        assert_eq!(indexed, table);
        let retained: Vec<_> = indexed
            .iter()
            .filter(|(atom, _)| atom.predicate().name() == "p")
            .collect();
        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].1, expected);
        assert_eq!(retained[0].0.values(), &[Value::Number(1)]);
    }
}

#[test]
fn original_membership_and_all_optimal_ties_survive_comparison_reuse() {
    let source = "d(1;2).{a;b}.p(X):-d(X),X+1<3,not a.#minimize{X@1:p(X)}.";
    let program = crate::formula_ground::ground(testing::prepare(source), None, None).unwrap();
    let atoms = crate::test_support::compiled_atoms(&program);
    let position = |name: &str| {
        atoms
            .iter()
            .position(|atom| atom.predicate().name() == name)
            .unwrap()
    };
    let a = position("a");
    let p = position("p");
    assert_eq!(
        atoms
            .iter()
            .filter(|atom| atom.predicate().name() == "p")
            .count(),
        1
    );
    assert_eq!(atoms[p].values(), &[Value::Number(1)]);
    assert!(atoms.len() <= 8);
    let mut complete = 0;
    let mut optimal = 0;
    for mask in 0..(1_usize << atoms.len()) {
        let positions = || (0..atoms.len()).filter(|&atom| mask & (1 << atom) != 0);
        let interpretation =
            zetesis_ferraris::Interpretation::new(&program.theory, positions()).unwrap();
        let verdict = zetesis_ferraris::check(
            &program.theory,
            &interpretation,
            zetesis_ferraris::Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        if !verdict.accepted() {
            continue;
        }
        complete += 1;
        let selected_a = mask & (1 << a) != 0;
        assert_eq!(mask & (1 << p) != 0, !selected_a);
        let model = zetesis_core::Model::from_positions(&program.atoms, positions()).unwrap();
        let score = zetesis_objective::evaluate(
            &program.objectives,
            &model,
            zetesis_objective::Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(score.score().costs(), &[(1, i64::from(!selected_a))]);
        optimal += usize::from(selected_a);
    }
    assert_eq!(complete, 4);
    assert_eq!(optimal, 2, "both choices of b are retained at the optimum");
}
