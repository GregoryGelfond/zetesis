//! Actual optional-bound propagation and retained-owner failure boundaries.
use super::*;
use zetesis_algebra::region::{Narrowing, Region};
use zetesis_ferraris::{
    Narrower, NarrowingAttempt, NarrowingScratch, OriginalSubject, RegionLimits,
};

fn narrow(theory: &Theory) -> NarrowingAttempt {
    let mut region = Region::all_open(theory.atom_count());
    assert!(region.hold(6));
    assert!(region.cut(0));
    assert!(region.cut(3));
    let narrower = Narrower::new(theory);
    narrower.narrow_known(
        OriginalSubject::new(theory, None),
        &mut region,
        &mut narrower.knowledge(),
        &mut NarrowingScratch::default(),
        RegionLimits::default(),
        &Cancellation::default(),
    )
}

#[test]
fn actual_tier_bound_refutes_two_remaining_expensive_choices() {
    let (mut plan, required) = fixture::plan(6, true);
    let preparation = plan
        .prepare_choice_bounds(
            &required,
            ObjectivePlanLimits::default(),
            u128::MAX,
            &Cancellation::default(),
        )
        .unwrap();
    let incumbent = Score::from_costs(vec![(1, 9), (0, 0)]).unwrap();
    let layered = fixture::bound(&plan, &incumbent, false);
    // The test-only control removes only the newly prepared layer owner.
    // It retains exact eligibility, the original source and the existing residual bound.
    let groups = std::mem::take(&mut plan.choices.as_mut().unwrap().layered);
    let preceding = fixture::bound(&plan, &incumbent, false);
    plan.choices.as_mut().unwrap().layered = groups;
    let baseline = narrow(preceding.theory());
    let candidate = narrow(layered.theory());
    assert!(matches!(baseline.result, Ok(Narrowing::Fixed { .. })));
    assert_eq!(candidate.result, Ok(Narrowing::Refuted));
    eprintln!(
        "choice_layers preparation_work={} baseline_bound_work={} layered_bound_work={} baseline_narrow_work={} layered_narrow_work={}",
        preparation.work,
        preceding.statistics().work,
        layered.statistics().work,
        baseline.statistics.work,
        candidate.statistics.work
    );
}

#[test]
fn late_layer_preparation_refusal_keeps_the_previous_complete_owner() {
    let (mut plan, required) = fixture::plan(6, true);
    let limits = ObjectivePlanLimits::default();
    let cancellation = Cancellation::default();
    let complete = plan
        .prepare_choice_bounds(&required, limits, u128::MAX, &cancellation)
        .unwrap();
    let previous = plan.choices.as_ref().unwrap();
    let group_pointer = previous.layered.as_ptr();
    let key_pointers: Vec<_> = previous
        .layered
        .iter()
        .map(|group| group.keys.as_ptr())
        .collect();
    assert_eq!(previous.layered.len(), 2);
    assert!(previous.layered.iter().all(|group| group.keys.len() == 3));
    let error = plan
        .prepare_choice_bounds(
            &required,
            ObjectivePlanLimits {
                max_work: complete.work - 1,
                ..limits
            },
            u128::MAX,
            &cancellation,
        )
        .unwrap_err();
    assert_eq!(error.kind(), Kind::Limit(Resource::Work));
    // The final group sort admits its existing n*n tariff atomically. Refusal
    // records all preceding work and never charges a sort that did not execute.
    assert_eq!(error.statistics().work, complete.work - 9);
    let retained = plan.choices.as_ref().unwrap();
    assert_eq!(retained.layered.as_ptr(), group_pointer);
    assert_eq!(
        retained
            .layered
            .iter()
            .map(|group| group.keys.as_ptr())
            .collect::<Vec<_>>(),
        key_pointers
    );
    let incumbent = Score::from_costs(vec![(1, 9), (0, 0)]).unwrap();
    assert_eq!(
        narrow(fixture::bound(&plan, &incumbent, false).theory()).result,
        Ok(Narrowing::Refuted)
    );
}

#[test]
fn retained_layer_keys_and_remainder_obey_the_existing_choice_byte_ceiling() {
    let (mut plan, required) = fixture::plan(6, true);
    plan.levels.get_mut(&1).unwrap().push(AggregateElement {
        weight: 2,
        condition: 7,
    });
    let cancellation = Cancellation::default();
    plan.prepare_choice_bounds(
        &required,
        ObjectivePlanLimits::default(),
        u128::MAX,
        &cancellation,
    )
    .unwrap();
    let costs = plan.choices.as_ref().unwrap();
    assert_eq!(costs.levels.len(), 1);
    assert_eq!(costs.levels[0].layer_remainder.len(), 1);
    assert_eq!(
        costs.levels[0].layer_remainder[0],
        AggregateElement {
            weight: 2,
            condition: 7
        }
    );
    let expected = required.retained_bytes()
        + size_of::<Costs>() as u128
        + Storage::vector_bytes(&costs.groups)
        + costs.activations.retained_bytes()
        + Storage::vector_bytes(&costs.levels)
        + Storage::vector_bytes(&costs.layered)
        + costs
            .layered
            .iter()
            .map(|group| Storage::vector_bytes(&group.keys))
            .sum::<u128>()
        + costs
            .levels
            .iter()
            .map(|level| {
                Storage::vector_bytes(&level.residuals)
                    + Storage::vector_bytes(&level.layer_remainder)
            })
            .sum::<u128>();
    let work = Work::new(ObjectivePlanLimits::default(), &cancellation);
    let admitted = Storage::new(expected, &required, Some(costs), &work).unwrap();
    assert_eq!(admitted.bytes, expected);
    let Err(refused) = Storage::new(expected - 1, &required, Some(costs), &work) else {
        panic!("one byte short must refuse the named retained owner");
    };
    assert_eq!(refused.kind(), Kind::Limit(Resource::ChoiceBytes));
    assert_eq!(refused.statistics().work, 0);
    // This isolates simultaneous previous-owner admission; it is not a claim
    // that retained bytes equal the larger temporary preparation peak.
}
