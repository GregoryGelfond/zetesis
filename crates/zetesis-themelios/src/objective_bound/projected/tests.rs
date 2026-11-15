use super::*;
use crate::{AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits};
use zetesis_algebra::Stop;
use zetesis_core::Model;
use zetesis_ferraris::{Interpretation, models};
use zetesis_objective::{Score, evaluate};

fn text(name: &str) -> String {
    include_str!("../../../tests/fixtures/objective-projection.jsonl")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|case| case["name"] == name)
        .unwrap()["source"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn admit(name: &str) -> AdmittedFormula {
    crate::admit_formula(
        text(name),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}
fn projected(input: &AdmittedFormula, atoms: usize) -> ObjectivePlan {
    ObjectivePlan::new_projected(
        input.objective_source(),
        ObjectivePlanLimits {
            max_atoms: atoms,
            ..ObjectivePlanLimits::default()
        },
        u128::MAX,
        &Cancellation::default(),
    )
    .into_parts()
    .0
    .unwrap()
}
fn detailed(input: &AdmittedFormula, positions: &[usize]) -> Score {
    let model = Model::from_positions(input.atom_catalog(), positions.iter().copied()).unwrap();
    evaluate(
        input.objectives(),
        &model,
        zetesis_objective::Limits::default(),
        &Cancellation::default(),
    )
    .unwrap()
    .score()
    .clone()
}
fn prepared(plan: &ObjectivePlan, positions: &[usize]) -> Score {
    let candidate = Interpretation::new(plan.original(), positions.iter().copied()).unwrap();
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
fn sparse_objective_atoms_keep_original_coordinates() {
    let input = admit("sparse");
    let selected: Vec<_> = (0..input.atom_catalog().atoms().len())
        .filter(|&position| {
            input
                .atom_catalog()
                .atoms()
                .at(position)
                .unwrap()
                .predicate()
                .name()
                == "wanted"
        })
        .collect();
    assert_eq!(selected.len(), 2);
    assert!(selected.iter().any(|&position| position >= selected.len()));
    let plan = projected(&input, selected.len());
    assert!(plan.original().same_instance(input.theory()));
    for (node, &original) in selected.iter().enumerate() {
        assert_eq!(
            plan.nodes.view().node(node).unwrap(),
            NodeView::Atom(original)
        );
    }
    for mask in 0..4 {
        let positions: Vec<_> = selected
            .iter()
            .enumerate()
            .filter_map(|(bit, &position)| (mask & (1 << bit) != 0).then_some(position))
            .collect();
        assert_eq!(prepared(&plan, &positions), detailed(&input, &positions));
    }
    assert_eq!(
        ObjectivePlan::new(
            input.theory(),
            input.atom_catalog().atoms(),
            input.objectives(),
            ObjectivePlanLimits {
                max_atoms: selected.len(),
                ..ObjectivePlanLimits::default()
            },
            &Cancellation::default()
        )
        .unwrap_err()
        .kind(),
        Kind::Limit(Resource::Atoms)
    );
}

#[test]
fn projected_bounds_preserve_every_cost_and_tie() {
    for name in [
        "signed_arity",
        "negative_ties",
        "coalesced",
        "closed_negative",
        "closed_disjunction",
        "missing_closed",
        "constant",
        "empty",
        "no_objective",
    ] {
        let input = admit(name);
        let plan = projected(&input, input.atom_catalog().atoms().len());
        assert!(input.atom_catalog().atoms().len() <= 8);
        let mut all = Vec::new();
        for mask in 0..1 << input.atom_catalog().atoms().len() {
            let positions: Vec<_> = (0..input.atom_catalog().atoms().len())
                .filter(|bit| mask & (1 << bit) != 0)
                .collect();
            let score = detailed(&input, &positions);
            assert_eq!(prepared(&plan, &positions), score, "{name}; mask {mask}");
            all.push((positions, score));
        }
        for (_, incumbent) in &all {
            let bound = plan
                .bound(
                    incumbent,
                    super::super::ObjectiveBoundLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
            assert_eq!(bound.theory().atom_count(), input.theory().atom_count());
            for (positions, score) in &all {
                let candidate =
                    Interpretation::new(bound.theory(), positions.iter().copied()).unwrap();
                assert_eq!(
                    models(
                        bound.theory(),
                        &candidate,
                        zetesis_ferraris::Limits::default(),
                        &Cancellation::default()
                    )
                    .unwrap(),
                    score.compare_costs(incumbent) != Ordering::Greater,
                    "{name}; {positions:?}"
                );
            }
        }
    }
}

#[test]
fn selected_atom_limit_refuses_before_prefix_creation() {
    let input = admit("sparse");
    let error = ObjectivePlan::new_projected(
        input.objective_source(),
        ObjectivePlanLimits {
            max_atoms: 1,
            ..ObjectivePlanLimits::default()
        },
        u128::MAX,
        &Cancellation::default(),
    )
    .error()
    .unwrap();
    assert_eq!(
        error.kind(),
        &ObjectiveProjectionErrorKind::Existing(Kind::Limit(Resource::Atoms))
    );
    assert_eq!(error.statistics().nodes, 0);
    assert!(error.statistics().work > 1);
}

#[test]
fn cancellation_precedes_projection_allocation() {
    let input = admit("sparse");
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let error = ObjectivePlan::new_projected(
        input.objective_source(),
        ObjectivePlanLimits::default(),
        0,
        &cancellation,
    )
    .error()
    .unwrap();
    assert_eq!(
        error.kind(),
        &ObjectiveProjectionErrorKind::Existing(Kind::Control(Stop::Cancelled))
    );
    assert_eq!(error.statistics(), ObjectiveBoundStatistics::default());
    assert_eq!(error.projection_peak_bytes(), 0);
}

#[test]
fn each_work_cutoff_retains_its_performed_prefix() {
    let input = admit("closed_disjunction");
    let complete = projected(&input, input.atom_catalog().atoms().len())
        .statistics()
        .work;
    let mut previous = 0;
    for max_work in 0..complete {
        let error = ObjectivePlan::new_projected(
            input.objective_source(),
            ObjectivePlanLimits {
                max_work,
                ..ObjectivePlanLimits::default()
            },
            u128::MAX,
            &Cancellation::default(),
        )
        .error()
        .unwrap();
        assert_eq!(
            error.kind(),
            &ObjectiveProjectionErrorKind::Existing(Kind::Limit(Resource::Work))
        );
        assert!(error.statistics().work <= max_work);
        assert!(error.statistics().work >= previous);
        previous = error.statistics().work;
    }
    ObjectivePlan::new_projected(
        input.objective_source(),
        ObjectivePlanLimits {
            max_work: complete,
            ..ObjectivePlanLimits::default()
        },
        u128::MAX,
        &Cancellation::default(),
    )
    .into_parts()
    .0
    .unwrap();
}

#[test]
fn projection_byte_refusal_distinguishes_requested_storage() {
    let input = admit("sparse");
    let cancellation = Cancellation::default();
    let mut work = Work::new(ObjectivePlanLimits::default(), &cancellation);
    work.tick().unwrap();
    let mut storage = Storage::new(u128::MAX);
    storage.observe(size_of::<Storage>() as u128).unwrap();
    prepare(input.objective_source(), &mut storage, &mut work).unwrap();
    let peak = storage.peak;
    assert!(peak > size_of::<Storage>() as u128);
    ObjectivePlan::new_projected(
        input.objective_source(),
        ObjectivePlanLimits::default(),
        peak,
        &cancellation,
    )
    .into_parts()
    .0
    .unwrap();
    let error = ObjectivePlan::new_projected(
        input.objective_source(),
        ObjectivePlanLimits::default(),
        peak - 1,
        &cancellation,
    )
    .error()
    .unwrap();
    assert!(
        matches!(error.kind(), ObjectiveProjectionErrorKind::Bytes { required, limit } if *required >= peak && *limit == peak - 1)
    );
    assert!(error.projection_peak_bytes() <= peak);
    assert!(error.statistics().work > 0);
}

#[test]
fn choice_refusal_retains_the_exact_projected_plan() {
    let input = admit("choice");
    let mut plan = projected(&input, 2);
    let required = input.required_choices().expect("complete source choices");
    let limits = ObjectivePlanLimits {
        max_atoms: 2,
        ..ObjectivePlanLimits::default()
    };
    let error = plan
        .prepare_choice_bounds(required, limits, u128::MAX, &Cancellation::default())
        .unwrap_err();
    assert_eq!(error.kind(), Kind::Limit(Resource::Atoms));
    assert_eq!(plan.choice_bound_groups(), 0);
    for position in 0..input.atom_catalog().atoms().len() {
        assert_eq!(prepared(&plan, &[position]), detailed(&input, &[position]));
    }
}

#[test]
fn streamed_owner_mints_the_same_coordinate_authority() {
    let hybrid = crate::prepare_formula(
        text("closed_negative"),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_hybrid()
    .unwrap();
    let core = hybrid.core();
    let plan = ObjectivePlan::new_projected(
        core.objective_source(),
        ObjectivePlanLimits::default(),
        u128::MAX,
        &Cancellation::default(),
    )
    .into_parts()
    .0
    .unwrap();
    assert!(plan.original().same_instance(core.core_theory()));
    let ordinary = ObjectivePlan::new(
        core.core_theory(),
        core.atom_catalog().atoms(),
        core.objectives(),
        ObjectivePlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    for mask in 0..1 << core.atom_catalog().atoms().len() {
        let positions: Vec<_> = (0..core.atom_catalog().atoms().len())
            .filter(|bit| mask & (1 << bit) != 0)
            .collect();
        assert_eq!(prepared(&plan, &positions), prepared(&ordinary, &positions));
    }
}

#[test]
fn index_control_refusal_survives_a_smaller_byte_allowance() {
    let input = admit("no_objective");
    // Both empty selection vectors fit; after dropping the read-set header,
    // the index's fixed owner does not fit. Its control check precedes admission.
    let bytes = size_of::<Storage>() as u128
        + size_of::<Vec<PredicateRef<'_>>>() as u128
        + size_of::<Vec<usize>>() as u128;
    let byte_error = ObjectivePlan::new_projected(
        input.objective_source(),
        ObjectivePlanLimits::default(),
        bytes,
        &Cancellation::default(),
    )
    .error()
    .unwrap();
    assert!(matches!(
        byte_error.kind(),
        ObjectiveProjectionErrorKind::Bytes { .. }
    ));
    assert!(byte_error.projection_peak_bytes() > bytes);
    let limit = byte_error.statistics().work - 1;
    let stopped = ObjectivePlan::new_projected(
        input.objective_source(),
        ObjectivePlanLimits {
            max_work: limit,
            ..ObjectivePlanLimits::default()
        },
        bytes,
        &Cancellation::default(),
    )
    .error()
    .unwrap();
    assert_eq!(
        stopped.kind(),
        &ObjectiveProjectionErrorKind::Existing(Kind::Limit(Resource::Work))
    );
    assert_eq!(stopped.statistics().work, limit);
    assert_eq!(
        stopped.projection_peak_bytes(),
        byte_error.projection_peak_bytes()
    );
}

#[test]
fn complete_closed_graph_contributes_every_read_signature() {
    use zetesis_core::{AtomPattern, Term, Value};
    use zetesis_objective::{Condition, ObjectiveTemplate};
    use zetesis_test_support::programs::nullary;
    let atoms = [
        nullary("positive"),
        nullary("negated"),
        nullary("left"),
        nullary("right"),
    ];
    let condition = Condition::new(vec![
        ConditionNode::Atom(atoms[1].clone()),
        ConditionNode::Not(0),
        ConditionNode::Atom(atoms[2].clone()),
        ConditionNode::And(1, 2),
        ConditionNode::Atom(atoms[3].clone()),
        ConditionNode::Or(3, 4),
        ConditionNode::Boolean(false),
    ]);
    // Even graph nodes unreachable from the last constant are visited by the
    // common compiler. Read-set preparation must include their atom meanings.
    let objective = ObjectiveProgram::new(
        vec![
            ObjectiveTemplate::new(
                Term::Constant(Value::Number(1)),
                0,
                vec![],
                vec![AtomPattern::new(atoms[0].predicate().clone(), vec![]).unwrap()],
                vec![],
            )
            .with_condition(condition),
        ],
        zetesis_objective::AdmissionLimits::default(),
    )
    .unwrap();
    let cancellation = Cancellation::default();
    let mut work = Work::new(ObjectivePlanLimits::default(), &cancellation);
    let mut storage = Storage::new(u128::MAX);
    storage.observe(size_of::<Storage>() as u128).unwrap();
    let read = read_set(&objective, &mut storage, &mut work).unwrap();
    assert_eq!(
        read.iter()
            .map(|predicate| predicate.name())
            .collect::<Vec<_>>(),
        ["left", "negated", "positive", "right"]
    );
}

#[test]
fn bundle_owner_preserves_shared_source_coordinates() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/objective-projection-bundle.lp");
    let source =
        crate::SourceBundle::load_many([&path, &path], crate::BundleLimits::default()).unwrap();
    let input = crate::admit_bundle_formula(
        source,
        crate::BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let plan = ObjectivePlan::new_projected(
        input.objective_source(),
        ObjectivePlanLimits::default(),
        u128::MAX,
        &Cancellation::default(),
    )
    .into_parts()
    .0
    .unwrap();
    let legacy = ObjectivePlan::new(
        input.theory(),
        input.atom_catalog().atoms(),
        input.objectives(),
        ObjectivePlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert!(plan.original().same_instance(input.theory()));
    for mask in 0..1 << input.atom_catalog().atoms().len() {
        let positions: Vec<_> = (0..input.atom_catalog().atoms().len())
            .filter(|bit| mask & (1 << bit) != 0)
            .collect();
        assert_eq!(prepared(&plan, &positions), prepared(&legacy, &positions));
    }
}

#[test]
fn successful_attempt_receipt_matches_completed_plan() {
    let input = admit("sparse");
    let attempt = ObjectivePlan::new_projected(
        input.objective_source(),
        ObjectivePlanLimits::default(),
        u128::MAX,
        &Cancellation::default(),
    );
    assert!(attempt.error().is_none());
    let receipt = attempt.receipt();
    let (result, consumed_receipt) = attempt.into_parts();
    let plan = result.unwrap();
    assert_eq!(receipt, consumed_receipt);
    assert_eq!(receipt.statistics(), plan.statistics());
    assert_eq!(receipt.template_index(), None);
    assert!(receipt.projection_peak_bytes() > 0);
}

#[test]
fn refused_attempt_parts_retain_the_full_diagnostic() {
    let input = admit("sparse");
    let attempt = ObjectivePlan::new_projected(
        input.objective_source(),
        ObjectivePlanLimits::default(),
        0,
        &Cancellation::default(),
    );
    let diagnostic = attempt.error().unwrap();
    let (result, receipt) = attempt.into_parts();
    assert_eq!(result.unwrap_err(), *diagnostic.kind());
    assert_eq!(receipt.statistics(), diagnostic.statistics());
    assert_eq!(receipt.template_index(), diagnostic.template_index());
    assert_eq!(
        receipt.projection_peak_bytes(),
        diagnostic.projection_peak_bytes()
    );
    assert_eq!(receipt.statistics().work, 1);
    assert!(matches!(
        diagnostic.kind(),
        ObjectiveProjectionErrorKind::Bytes { limit: 0, .. }
    ));
    assert_eq!(
        std::error::Error::source(&diagnostic).unwrap().to_string(),
        diagnostic.kind().to_string()
    );
}
