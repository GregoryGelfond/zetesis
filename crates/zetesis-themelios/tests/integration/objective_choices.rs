//! Necessary source-choice costs preserve every original model at an admissible
//! lexicographic cost, including ties and conditional group activation.

use std::cmp::Ordering;

use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Limits, Theory, models};
use zetesis_objective::Score;
use zetesis_reference_support::formula;
use zetesis_themelios::objective_bound::{
    ObjectiveBoundErrorKind, ObjectiveBoundLimits, ObjectiveBoundResource, ObjectivePlan,
    ObjectivePlanLimits,
};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, CountPlanStatus, ExpansionLimits, FormulaLimits,
    RequiredChoices, prepare_formula,
};

const DERIVED: &str = include_str!("../fixtures/required-choice-objectives/derived.lp");
const SHARED_KEY: &str = include_str!("../fixtures/required-choice-objectives/shared-key.lp");
const SIGNED: &str = include_str!("../fixtures/required-choice-objectives/signed-priority.lp");
const CYCLE: &str = include_str!("../fixtures/required-choice-objectives/positive-cycle.lp");
const UNPROVED_CYCLE: &str =
    include_str!("../fixtures/required-choice-objectives/unproved-cycle.lp");
const CONDITIONAL: &str =
    include_str!("../fixtures/required-choice-objectives/conditional-element.lp");
const ZERO_LOWER: &str = include_str!("../fixtures/required-choice-objectives/zero-lower.lp");
const EMPTY: &str = include_str!("../fixtures/required-choice-objectives/inconsistent-empty.lp");

fn exact(input: &AdmittedFormula) -> ObjectivePlan {
    ObjectivePlan::new(
        input.theory(),
        input.atoms(),
        input.objectives(),
        ObjectivePlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap()
}

fn prepare(plan: &mut ObjectivePlan, choices: &RequiredChoices) {
    let statistics = plan
        .prepare_choice_bounds(
            choices,
            ObjectivePlanLimits::default(),
            u128::MAX,
            &Cancellation::default(),
        )
        .unwrap();
    assert!(statistics.work > 0);
}

fn candidate(theory: &Theory, mask: usize) -> Interpretation {
    Interpretation::new(
        theory,
        (0..theory.atom_count()).filter(|atom| mask & (1 << atom) != 0),
    )
    .unwrap()
}

fn score(plan: &ObjectivePlan, mask: usize) -> Score {
    plan.score(
        &candidate(plan.original(), mask),
        zetesis_objective::Limits::default(),
        &Cancellation::default(),
    )
    .unwrap()
    .unwrap()
    .into_score()
}

fn holds(theory: &Theory, mask: usize) -> bool {
    models(
        theory,
        &candidate(theory, mask),
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap()
}

fn preserves_models(source: &str, expected_groups: usize) {
    let input = formula(source);
    assert!(input.theory().atom_count() <= 8);
    let mut plan = exact(&input);
    let choices = input.required_choices().expect("objective capture");
    assert!(choices.capture_failure().is_none());
    prepare(&mut plan, choices);
    assert_eq!(plan.choice_bound_groups(), expected_groups);
    let scores: Vec<_> = (0..1 << input.theory().atom_count())
        .map(|mask| score(&plan, mask))
        .collect();
    let valid: Vec<_> = (0..scores.len())
        .filter(|&mask| holds(input.theory(), mask))
        .collect();
    assert!(!valid.is_empty());
    for &incumbent in &valid {
        let bound = plan
            .bound(
                &scores[incumbent],
                ObjectiveBoundLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert!(bound.choice_failure().is_none());
        assert!(bound.original().same_instance(input.theory()));
        for &mask in &valid {
            assert_eq!(
                holds(bound.theory(), mask),
                scores[mask].compare_costs(&scores[incumbent]) != Ordering::Greater,
                "mask={mask}; incumbent={incumbent}; {source}"
            );
        }
    }
}

#[test]
fn derived_costs_preserve_conditional_optimal_ties() {
    preserves_models(DERIVED, 1);
}

#[test]
fn shared_objective_key_is_never_charged_twice() {
    preserves_models(SHARED_KEY, 1);
}

#[test]
fn signed_priorities_retain_exact_lexicographic_costs() {
    preserves_models(SIGNED, 1);
}

#[test]
fn positive_cycles_require_an_independent_proof() {
    preserves_models(CYCLE, 1);
    preserves_models(UNPROVED_CYCLE, 0);
}

#[test]
fn conditional_members_preserve_required_head_costs() {
    preserves_models(CONDITIONAL, 1);
}

#[test]
fn zero_lower_bound_supplies_no_required_member() {
    preserves_models(ZERO_LOWER, 0);
}

#[test]
fn an_empty_head_never_creates_a_required_member() {
    let input = formula(EMPTY);
    let choices = input.required_choices().unwrap();
    assert!(choices.is_empty());
    let mut plan = exact(&input);
    prepare(&mut plan, choices);
    assert_eq!(plan.choice_bound_groups(), 0);
}

#[test]
fn eager_and_hybrid_capture_the_same_choice_premises() {
    let eager = formula(DERIVED);
    let hybrid = prepare_formula(
        DERIVED.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_hybrid()
    .unwrap();
    let eager_choices = eager.required_choices().unwrap();
    let hybrid_choices = hybrid.core().required_choices().unwrap();
    assert_eq!(eager_choices.len(), hybrid_choices.len());
    assert_eq!(
        eager_choices.capture_statistics(),
        hybrid_choices.capture_statistics()
    );
    assert!(matches!(eager.count_plan(), CountPlanStatus::NotRequested));
    let mut hybrid_plan = ObjectivePlan::new(
        hybrid.core_theory(),
        hybrid.atom_catalog().atoms(),
        hybrid.objectives(),
        ObjectivePlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    prepare(&mut hybrid_plan, hybrid_choices);
    assert_eq!(hybrid_plan.choice_bound_groups(), 1);
    assert!(!hybrid_choices.belongs_to(eager.theory()));
}

#[test]
fn foreign_choice_premises_preserve_the_exact_plan() {
    let input = formula(DERIVED);
    let foreign = formula(DERIVED);
    let mut plan = exact(&input);
    let expected = score(&plan, 0);
    let error = plan
        .prepare_choice_bounds(
            foreign.required_choices().unwrap(),
            ObjectivePlanLimits::default(),
            u128::MAX,
            &Cancellation::default(),
        )
        .unwrap_err();
    assert_eq!(error.kind(), ObjectiveBoundErrorKind::ChoiceOwner);
    assert_eq!(score(&plan, 0), expected);
}

#[test]
fn byte_refusal_preserves_the_exact_plan() {
    let input = formula(DERIVED);
    let mut plan = exact(&input);
    let expected = score(&plan, 0);
    let error = plan
        .prepare_choice_bounds(
            input.required_choices().unwrap(),
            ObjectivePlanLimits::default(),
            0,
            &Cancellation::default(),
        )
        .unwrap_err();
    assert_eq!(
        error.kind(),
        ObjectiveBoundErrorKind::Limit(ObjectiveBoundResource::ChoiceBytes)
    );
    assert!(error.statistics().work > 0);
    assert_eq!(score(&plan, 0), expected);
}

#[test]
fn failed_strengthening_returns_the_exact_bound() {
    let input = formula(DERIVED);
    let mut plan = exact(&input);
    let incumbent = score(&plan, 0);
    let exact_bound = plan
        .bound(
            &incumbent,
            ObjectiveBoundLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    prepare(&mut plan, input.required_choices().unwrap());
    let limits = ObjectiveBoundLimits {
        max_work: exact_bound.statistics().work,
        ..ObjectiveBoundLimits::default()
    };
    let fallback = plan
        .bound(&incumbent, limits, &Cancellation::default())
        .unwrap();
    assert!(fallback.choice_failure().is_some());
    assert_eq!(fallback.theory().nodes(), exact_bound.theory().nodes());
    assert_eq!(
        fallback.theory().operands(),
        exact_bound.theory().operands()
    );
    assert_eq!(fallback.theory().roots(), exact_bound.theory().roots());
}

#[test]
fn required_group_cost_rejects_an_unpaid_activation() {
    let input = formula(DERIVED);
    let mut plan = exact(&input);
    let mut activation = 0;
    let mut incumbent_mask = 0;
    for atom in 0..input.theory().atom_count() {
        let name = zetesis_reference_support::canonical(input.atoms().at(atom).unwrap());
        if name == "enabled" || name == "gate" {
            activation |= 1 << atom;
        }
        if name == "gate" {
            incumbent_mask |= 1 << atom;
        }
    }
    assert!(holds(input.theory(), incumbent_mask));
    assert!(!holds(input.theory(), activation));
    let incumbent = score(&plan, incumbent_mask);
    let exact_bound = plan
        .bound(
            &incumbent,
            ObjectiveBoundLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(holds(exact_bound.theory(), activation));
    prepare(&mut plan, input.required_choices().unwrap());
    let strengthened = plan
        .bound(
            &incumbent,
            ObjectiveBoundLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(strengthened.choice_failure().is_none());
    assert!(!holds(strengthened.theory(), activation));
}

#[test]
fn failed_repreparation_retains_certified_choice_costs() {
    let input = formula(DERIVED);
    let mut plan = exact(&input);
    let choices = input.required_choices().unwrap();
    prepare(&mut plan, choices);
    assert_eq!(plan.choice_bound_groups(), 1);
    let error = plan
        .prepare_choice_bounds(
            choices,
            ObjectivePlanLimits::default(),
            0,
            &Cancellation::default(),
        )
        .unwrap_err();
    assert_eq!(
        error.kind(),
        ObjectiveBoundErrorKind::Limit(ObjectiveBoundResource::ChoiceBytes)
    );
    assert_eq!(plan.choice_bound_groups(), 1);
}

#[test]
fn source_capture_refusal_keeps_exact_admission() {
    let input = prepare_formula(
        DERIVED.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_with_count_plan(
        zetesis_themelios::CountPlanLimits {
            max_groups: 0,
            ..zetesis_themelios::CountPlanLimits::default()
        },
        &Cancellation::default(),
        None,
    )
    .unwrap();
    let choices = input.required_choices().unwrap();
    let capture = choices.capture_failure().expect("optional capture limit");
    assert!(capture.statistics().work > 0);
    let mut plan = exact(&input);
    let expected = score(&plan, 0);
    let error = plan
        .prepare_choice_bounds(
            choices,
            ObjectivePlanLimits::default(),
            u128::MAX,
            &Cancellation::default(),
        )
        .unwrap_err();
    assert_eq!(error.kind(), ObjectiveBoundErrorKind::ChoiceCapture);
    assert_eq!(score(&plan, 0), expected);
}

fn named_mask(input: &AdmittedFormula, names: &[&str]) -> usize {
    (0..input.theory().atom_count()).fold(0, |mask, atom| {
        let name = zetesis_reference_support::canonical(input.atoms().at(atom).unwrap());
        if names.contains(&name.as_str()) {
            mask | (1 << atom)
        } else {
            mask
        }
    })
}

fn narrow_holding(theory: &Theory, held: usize) -> zetesis_cpu::regions::Narrowing {
    use zetesis_cpu::regions::Region;
    use zetesis_ferraris::{Narrower, NarrowingScratch, OriginalSubject, RegionLimits};
    let mut region = Region::all_open(theory.atom_count());
    for atom in 0..theory.atom_count() {
        if held & (1 << atom) != 0 {
            assert!(region.hold(atom));
        }
    }
    let narrower = Narrower::new(theory);
    narrower
        .narrow_known(
            OriginalSubject::new(theory, None),
            &mut region,
            &mut narrower.knowledge(),
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap()
        .0
}

#[test]
fn prepaid_cost_refutes_an_unaffordable_open_group() {
    const SOURCE: &str = include_str!("../fixtures/required-choice-objectives/residual.lp");
    let input = formula(SOURCE);
    let mut plan = exact(&input);
    let incumbent_mask = named_mask(&input, &["a", "d"]);
    assert!(holds(input.theory(), incumbent_mask));
    let incumbent = score(&plan, incumbent_mask);
    assert_eq!(incumbent.costs(), [(0, 7)]);
    let held = named_mask(&input, &["b"]);
    let exact_bound = plan
        .bound(
            &incumbent,
            ObjectiveBoundLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(matches!(
        narrow_holding(exact_bound.theory(), held),
        zetesis_cpu::regions::Narrowing::Fixed { .. }
    ));
    prepare(&mut plan, input.required_choices().unwrap());
    assert_eq!(plan.choice_bound_groups(), 2);
    let strengthened = plan
        .bound(
            &incumbent,
            ObjectiveBoundLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(strengthened.choice_failure().is_none());
    // The selected cost is six. The still-open second choice necessarily
    // contributes at least two, exceeding the incumbent seven.
    assert_eq!(
        narrow_holding(strengthened.theory(), held),
        zetesis_cpu::regions::Narrowing::Refuted
    );
    preserves_models(SOURCE, 2);
}

#[test]
fn conditional_members_never_supply_partition_caps() {
    const SOURCE: &str =
        include_str!("../fixtures/required-choice-objectives/conditional-partition.lp");
    let input = prepare_formula(
        SOURCE.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_with_count_plan(
        zetesis_themelios::CountPlanLimits::default(),
        &Cancellation::default(),
        None,
    )
    .unwrap();
    // a and b may both be true: only a's condition is true. Treating this
    // conditional upper bound as a bound on true heads would wrongly demand c
    // or d from the unconditional two-of-four group.
    assert!(holds(input.theory(), named_mask(&input, &["a", "b", "e"])));
    assert!(matches!(input.count_plan(), CountPlanStatus::NoPlan(_)));
    assert_eq!(input.required_choices().unwrap().len(), 3);
}

#[derive(Default)]
struct GroundingPeak(std::cell::Cell<u64>);
impl zetesis_themelios::GroundingObserver for GroundingPeak {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        _: zetesis_themelios::GroundingPhase,
        _: Option<zetesis_themelios::ProgramSite>,
        _: zetesis_themelios::GroundingOutcome,
        work: zetesis_themelios::GroundingWork,
    ) {
        self.0
            .set(self.0.get().max(work.support_peak_bytes.unwrap_or(0)));
    }
}

#[test]
fn optional_capture_preserves_mandatory_storage_peak() {
    const SOURCE: &str = include_str!("../fixtures/required-choice-objectives/storage-boundary.lp");
    let prepare = || {
        prepare_formula(
            SOURCE.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap()
    };
    let ordinary_peak = GroundingPeak::default();
    let ordinary = prepare()
        .ground_with_observer(Some(&ordinary_peak))
        .unwrap();
    let captured_peak = GroundingPeak::default();
    let captured = prepare()
        .ground_with_count_plan(
            zetesis_themelios::CountPlanLimits::default(),
            &Cancellation::default(),
            Some(&captured_peak),
        )
        .unwrap();
    let CountPlanStatus::NoPlan(statistics) = captured.count_plan() else {
        panic!("one choice has no partition consequence");
    };
    assert_eq!(statistics.groups, 1);
    assert!(statistics.storage_bytes > 0);
    assert_eq!(ordinary.theory().nodes(), captured.theory().nodes());
    assert_eq!(ordinary.theory().roots(), captured.theory().roots());
    assert!(ordinary_peak.0.get() > 0);
    assert_eq!(ordinary_peak.0.get(), captured_peak.0.get());
}

#[test]
fn bound_reuses_prepared_activation_closure() {
    const BASE: &str = include_str!("../fixtures/required-choice-objectives/activation-base.lp");
    const PADDED: &str =
        include_str!("../fixtures/required-choice-objectives/activation-padding.lp");
    let base = formula(BASE);
    let padded = formula(PADDED);
    assert!(padded.theory().view().len() > base.theory().view().len());
    assert_eq!(base.theory().atom_count(), padded.theory().atom_count());
    let mut preparations = Vec::new();
    let mut bounds = Vec::new();
    for input in [&base, &padded] {
        let mut plan = exact(input);
        let mask = named_mask(input, &["a"]);
        assert!(holds(input.theory(), mask));
        let incumbent = score(&plan, mask);
        preparations.push(
            plan.prepare_choice_bounds(
                input.required_choices().unwrap(),
                ObjectivePlanLimits::default(),
                u128::MAX,
                &Cancellation::default(),
            )
            .unwrap(),
        );
        assert_eq!(plan.choice_bound_groups(), 1);
        let bound = plan
            .bound(
                &incumbent,
                ObjectiveBoundLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert!(bound.choice_failure().is_none());
        if let Some(&earlier) = bounds.first() {
            let exact_allowance = ObjectiveBoundLimits {
                max_work: earlier,
                ..ObjectiveBoundLimits::default()
            };
            let repeated = plan
                .bound(&incumbent, exact_allowance, &Cancellation::default())
                .unwrap();
            assert!(repeated.choice_failure().is_none());
            assert_eq!(repeated.statistics().work, earlier);
        }
        bounds.push(bound.statistics().work);
    }
    // Unrelated original constraints are inspected during preparation, but no
    // longer consume every incumbent bound's work allowance.
    assert!(preparations[1].work > preparations[0].work);
    assert_eq!(preparations[0].nodes, preparations[1].nodes);
    assert!(preparations[0].nodes > 0);
    assert_eq!(bounds[0], bounds[1]);
}

#[test]
fn wide_signed_activation_preserves_optimal_ties() {
    const SOURCE: &str = include_str!("../fixtures/required-choice-objectives/wide-activation.lp");
    let input = formula(SOURCE);
    assert!((0..input.theory().view().len()).any(|index| matches!(
        input.theory().view().node(index).unwrap(),
        zetesis_ferraris::NodeView::And(row) if row.len() >= 3
    )));
    // Signed aggregate lowering places actual wide connective rows inside the
    // choice activation. Every original model includes either truth value of w.
    // Copying the implication-to-falsum with the wrong owner coordinates, or
    // treating the guarded choice as unconditional, changes this family.
    preserves_models(SOURCE, 1);
}
