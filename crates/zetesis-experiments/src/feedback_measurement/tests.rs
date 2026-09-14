use super::*;
use zetesis_cpu::{Control, Stop};
use zetesis_sat::{Check, CheckedInterpretation};

fn checked(source: &zetesis_ferraris::Theory, bits: u64) -> CheckedInterpretation {
    zetesis_sat::check_interpretation(
        fixtures::interpretation(source, bits).unwrap(),
        Configuration::default().native(),
        &Control::default(),
    )
}

#[test]
fn every_guard_matches_its_original_conditional_reduct() {
    let control = Control::default();
    let mut interpretations = 0;
    let mut pairs = 0;
    for case in Case::ALL {
        let theory = case.theory().unwrap();
        let (_, checked) = run::qualify(&theory, 1_000_000, &control).unwrap();
        interpretations += 1 << theory.atom_count();
        pairs += checked;
    }
    assert_eq!(interpretations, 116);
    assert_eq!(pairs, 4742);
}

#[test]
fn conditional_guard_preserves_a_stable_choice_extension() {
    let source = Case::Mixed.theory().unwrap();
    let control = Control::default();
    let witness = fixtures::interpretation(&source, 1).unwrap();
    let mut budget = guard::Budget::new(ConstructionLimits::default(), &control);
    let guard = guard::compile(&source, &witness, &mut budget).unwrap();
    let candidate = fixtures::interpretation(&source, 5).unwrap();
    assert!(
        zetesis_ferraris::check(&source, &candidate, reference_limits(1_000_000), &control)
            .unwrap()
            .accepted()
    );
    assert!(guard.allows(&candidate, 1_000_000, &control).unwrap());
}

#[test]
fn checked_countermodel_excludes_its_actual_subject() {
    let source = Case::Mixed.theory().unwrap();
    let control = Control::default();
    let checked = checked(&source, 3);
    assert!(matches!(checked.verdict(), Check::NonMinimal(_)));
    let mut budget = guard::Budget::new(ConstructionLimits::default(), &control);
    let mut store = guard::Store::new(&mut budget).unwrap();
    assert!(store.learn(&checked, &mut budget).unwrap());
    assert!(
        !store.guards[0]
            .allows(checked.candidate(), 1_000_000, &control)
            .unwrap()
    );
}

#[test]
fn learning_requires_a_native_countermodel() {
    let source = Case::Choice.theory().unwrap();
    let control = Control::default();
    let checked = checked(&source, 1);
    assert!(matches!(checked.verdict(), Check::Stable));
    let mut budget = guard::Budget::new(ConstructionLimits::default(), &control);
    let mut store = guard::Store::new(&mut budget).unwrap();
    assert!(matches!(
        store.learn(&checked, &mut budget),
        Err(Error::Witness)
    ));
    assert!(store.guards.is_empty());
}

#[test]
fn equal_foreign_theory_cannot_rebind_a_guard() {
    let source = Case::Mixed.theory().unwrap();
    let other = Case::Mixed.theory().unwrap();
    assert_eq!(source.nodes(), other.nodes());
    let control = Control::default();
    let mut budget = guard::Budget::new(ConstructionLimits::default(), &control);
    let witness = fixtures::interpretation(&source, 1).unwrap();
    let guard = guard::compile(&source, &witness, &mut budget).unwrap();
    let foreign = fixtures::interpretation(&other, 3).unwrap();
    assert!(matches!(
        guard.allows(&foreign, 1_000_000, &control),
        Err(Error::Owner)
    ));
}

#[test]
fn compilation_rejects_a_foreign_witness() {
    let source = Case::Loops.theory().unwrap();
    let other = Case::Loops.theory().unwrap();
    let witness = fixtures::interpretation(&other, 0).unwrap();
    let control = Control::default();
    let mut budget = guard::Budget::new(ConstructionLimits::default(), &control);
    assert!(matches!(
        guard::compile(&source, &witness, &mut budget),
        Err(Error::Owner)
    ));
    assert_eq!(budget.work, 0);
}

#[test]
fn duplicate_witness_reuses_the_published_guard() {
    let source = Case::Loops.theory().unwrap();
    let control = Control::default();
    let first = checked(&source, 1);
    let second = checked(&source, 2);
    let mut budget = guard::Budget::new(ConstructionLimits::default(), &control);
    let mut store = guard::Store::new(&mut budget).unwrap();
    assert!(store.learn(&first, &mut budget).unwrap());
    let nodes = budget.retained_nodes;
    let bytes = budget.retained_bytes;
    let work = budget.work;
    assert!(!store.learn(&second, &mut budget).unwrap());
    assert_eq!(store.guards.len(), 1);
    assert_eq!(budget.retained_nodes, nodes);
    assert_eq!(budget.retained_bytes, bytes);
    assert_eq!(budget.work - work, 6); // One dedup membership visit per universe atom.
}

#[test]
fn every_construction_work_refusal_keeps_the_store_empty() {
    let source = Case::Mixed.theory().unwrap();
    let checked = checked(&source, 3);
    let control = Control::default();
    let mut full = guard::Budget::new(ConstructionLimits::default(), &control);
    let mut store = guard::Store::new(&mut full).unwrap();
    let setup = full.work;
    store.learn(&checked, &mut full).unwrap();
    let complete = full.work;
    for maximum in setup..complete {
        let limits = ConstructionLimits {
            max_work: maximum,
            ..ConstructionLimits::default()
        };
        let mut budget = guard::Budget::new(limits, &control);
        let mut refused = guard::Store::new(&mut budget).unwrap();
        assert!(matches!(
            refused.learn(&checked, &mut budget),
            Err(Error::Limit(Resource::Work))
        ));
        assert!(refused.guards.is_empty());
        assert_eq!(budget.retained_nodes, 0);
        assert_eq!(budget.work, maximum);
    }
    let mut budget = guard::Budget::new(
        ConstructionLimits {
            max_work: complete,
            ..ConstructionLimits::default()
        },
        &control,
    );
    assert!(
        guard::Store::new(&mut budget)
            .unwrap()
            .learn(&checked, &mut budget)
            .unwrap()
    );
}

#[test]
fn every_node_refusal_publishes_no_guard() {
    let source = Case::Mixed.theory().unwrap();
    let checked = checked(&source, 3);
    let control = Control::default();
    let mut budget = guard::Budget::new(ConstructionLimits::default(), &control);
    let mut store = guard::Store::new(&mut budget).unwrap();
    store.learn(&checked, &mut budget).unwrap();
    let complete = store.guards[0].restriction.nodes().len();
    for max_nodes in 0..complete {
        let mut budget = guard::Budget::new(
            ConstructionLimits {
                max_nodes,
                ..ConstructionLimits::default()
            },
            &control,
        );
        let mut store = guard::Store::new(&mut budget).unwrap();
        assert!(matches!(
            store.learn(&checked, &mut budget),
            Err(Error::Limit(Resource::Nodes))
        ));
        assert!(store.guards.is_empty());
        assert_eq!(budget.retained_nodes, 0);
    }
    let mut budget = guard::Budget::new(
        ConstructionLimits {
            max_nodes: complete,
            ..ConstructionLimits::default()
        },
        &control,
    );
    assert!(
        guard::Store::new(&mut budget)
            .unwrap()
            .learn(&checked, &mut budget)
            .unwrap()
    );
}

#[test]
fn failed_learning_preserves_the_previous_guard() {
    let source = Case::Mixed.theory().unwrap();
    let control = Control::default();
    let first = checked(&source, 3);
    let mut budget = guard::Budget::new(ConstructionLimits::default(), &control);
    let mut store = guard::Store::new(&mut budget).unwrap();
    store.learn(&first, &mut budget).unwrap();
    let bytes = budget.retained_bytes;
    let nodes = budget.retained_nodes;
    budget.limits.max_work = budget.work;
    assert!(matches!(
        store.learn(&first, &mut budget),
        Err(Error::Limit(Resource::Work))
    ));
    assert_eq!(store.guards.len(), 1);
    assert_eq!(budget.retained_bytes, bytes);
    assert_eq!(budget.retained_nodes, nodes);
    assert!(
        !store.guards[0]
            .allows(first.candidate(), 1_000_000, &control)
            .unwrap()
    );
}

#[test]
fn cancelled_learning_preserves_the_original_stop() {
    let source = Case::Loops.theory().unwrap();
    let checked = checked(&source, 1);
    let control = Control::default();
    let mut budget = guard::Budget::new(ConstructionLimits::default(), &control);
    let mut store = guard::Store::new(&mut budget).unwrap();
    control.cancel();
    assert!(matches!(
        store.learn(&checked, &mut budget),
        Err(Error::Control(Stop::Cancelled))
    ));
    assert!(store.guards.is_empty());
}

#[test]
fn feedback_saves_calls_without_skipping_candidates() {
    let source = Case::Loops.theory().unwrap();
    let control = Control::default();
    let mut progress = Progress::default();
    let mut elapsed = StageTimes::default();
    replay::fixed(
        &source,
        true,
        Configuration::default(),
        &control,
        &mut progress,
        &mut elapsed,
    )
    .unwrap();
    assert!(progress.exhausted);
    assert_eq!(progress.stable, [0]);
    assert_eq!(progress.candidates_completed, 64);
    assert_eq!(progress.membership_calls, 2);
    assert_eq!(progress.filtered, 62);
    assert_eq!(progress.witnesses, [0]);
    assert_eq!(progress.restarts, 0);
    assert!(progress.native.is_none());
}

#[test]
fn refused_learning_retains_completed_membership() {
    let source = Case::Loops.theory().unwrap();
    let configuration = Configuration {
        construction: ConstructionLimits {
            max_guards: 0,
            ..ConstructionLimits::default()
        },
        ..Configuration::default()
    };
    let mut progress = Progress::default();
    let result = replay::fixed(
        &source,
        true,
        configuration,
        &Control::default(),
        &mut progress,
        &mut StageTimes::default(),
    );
    assert!(matches!(result, Err(Error::Limit(Resource::Guards))));
    assert_eq!(progress.candidates_completed, 2);
    assert_eq!(progress.membership_completed, 2);
    assert_eq!(progress.nonminimal, 1);
    assert_eq!(progress.stable, [0]);
    assert_eq!(progress.construction.guards, 0);
    assert!(!progress.exhausted);
}

#[test]
fn actual_restart_preserves_already_delivered_models() {
    let source = Case::Mixed.theory().unwrap();
    let control = Control::default();
    let config = Configuration::default();
    let mut fixed = Progress::default();
    let store = replay::fixed(
        &source,
        true,
        config,
        &control,
        &mut fixed,
        &mut StageTimes::default(),
    )
    .unwrap();
    assert!(!store.guards.is_empty());
    let mut search = Progress::default();
    replay::search(
        &source,
        Some(&store),
        config,
        &control,
        &mut search,
        &mut StageTimes::default(),
    )
    .unwrap();
    run::family(&search, &fixed.stable).unwrap();
    assert_eq!(search.restarts, store.guards.len() as u64);
    assert_eq!(search.installation_attempts, search.restarts);
    assert_eq!(search.native.unwrap().restrictions, search.restarts);
    assert_eq!(search.stable.len(), 8);
}

#[test]
fn all_routes_preserve_the_complete_reference_families() {
    let configuration = Configuration {
        warmups: 0,
        repetitions: 0,
        ..Configuration::default()
    };
    let mut samples = Vec::new();
    let mut complete = false;
    measure(&configuration, |event| {
        match event {
            Event::Sample(sample) => samples.push((*sample).clone()),
            Event::Complete { samples } => {
                assert_eq!(*samples, 32);
                complete = true;
            }
            _ => {}
        }
        Ok(())
    })
    .unwrap();
    assert!(complete);
    assert_eq!(samples.len(), 32);
    for sample in samples {
        assert!(sample.progress.exhausted);
        assert_eq!(sample.phase, "qualification");
    }
}

#[test]
fn empty_guard_has_a_five_node_fifteen_step_boundary() {
    let source = Case::Empty.theory().unwrap();
    let witness = fixtures::interpretation(&source, 0).unwrap();
    let control = Control::default();
    // Four reservations; falsum, verum, proper, excluded, implication;
    // then five node and one root admission visits. No universe/map/root fold.
    let limits = ConstructionLimits {
        max_nodes: 5,
        max_work: 15,
        ..ConstructionLimits::default()
    };
    let mut budget = guard::Budget::new(limits, &control);
    let guard = guard::compile(&source, &witness, &mut budget).unwrap();
    assert_eq!(budget.work, 15);
    assert_eq!(guard.restriction.nodes().len(), 5);
    assert!(guard.allows(&witness, 1_000_000, &control).unwrap());
    let mut budget = guard::Budget::new(
        ConstructionLimits {
            max_work: 14,
            ..limits
        },
        &control,
    );
    assert!(matches!(
        guard::compile(&source, &witness, &mut budget),
        Err(Error::Limit(Resource::Work))
    ));
}

#[test]
fn guard_bytes_are_admitted_before_publication() {
    let source = Case::Loops.theory().unwrap();
    let checked = checked(&source, 1);
    let control = Control::default();
    for resource in [Resource::BuildBytes, Resource::LiveBytes] {
        let mut budget = guard::Budget::new(ConstructionLimits::default(), &control);
        let mut store = guard::Store::new(&mut budget).unwrap();
        let retained = budget.retained_bytes;
        match resource {
            Resource::BuildBytes => budget.limits.max_build_bytes = 0,
            Resource::LiveBytes => budget.limits.max_retained_bytes = retained,
            _ => unreachable!(),
        }
        assert!(
            matches!(store.learn(&checked, &mut budget), Err(Error::Limit(found)) if found == resource)
        );
        assert!(store.guards.is_empty());
        assert_eq!(budget.retained_bytes, retained);
        assert_eq!(budget.retained_nodes, 0);
    }
}

#[test]
fn failed_event_preserves_the_original_writer_cause_pair() {
    let configuration = Configuration {
        warmups: 0,
        repetitions: 0,
        construction: ConstructionLimits {
            max_work: 0,
            ..ConstructionLimits::default()
        },
        ..Configuration::default()
    };
    let mut samples = 0;
    let mut complete = false;
    let result = measure(&configuration, |event| {
        match event {
            Event::Sample(_) => samples += 1,
            Event::Failed { sample, .. } => {
                assert_eq!(sample.route, Route::Feedback);
                assert!(!sample.progress.exhausted);
                return Err(std::io::Error::other("failed-record sink"));
            }
            Event::Complete { .. } => complete = true,
            _ => {}
        }
        Ok(())
    });
    match result.unwrap_err() {
        Error::FailureOutput { original, output } => {
            assert!(matches!(*original, Error::Limit(Resource::Work)));
            assert_eq!(output.to_string(), "failed-record sink");
        }
        error => panic!("unexpected failure: {error}"),
    }
    assert_eq!(samples, 1);
    assert!(!complete);
}

#[test]
fn retained_node_refusal_publishes_no_guard() {
    let source = Case::Loops.theory().unwrap();
    let checked = checked(&source, 1);
    let control = Control::default();
    let mut budget = guard::Budget::new(
        ConstructionLimits {
            max_total_nodes: 0,
            ..ConstructionLimits::default()
        },
        &control,
    );
    let mut store = guard::Store::new(&mut budget).unwrap();
    let retained = budget.retained_bytes;
    assert!(matches!(
        store.learn(&checked, &mut budget),
        Err(Error::Limit(Resource::TotalNodes))
    ));
    assert!(store.guards.is_empty());
    assert_eq!(budget.retained_bytes, retained);
    assert_eq!(budget.retained_nodes, 0);
}

#[test]
fn refused_installation_retains_the_first_answer() {
    let source = Case::Mixed.theory().unwrap();
    let control = Control::default();
    let configuration = Configuration::default();
    let store = replay::fixed(
        &source,
        true,
        configuration,
        &control,
        &mut Progress::default(),
        &mut StageTimes::default(),
    )
    .unwrap();
    assert!(!store.guards.is_empty());
    let mut reference =
        zetesis_sat::StableModels::new(&source, configuration.native(), control.clone()).unwrap();
    let first = fixtures::bits(&reference.next().unwrap().unwrap());
    // An actually observed native work boundary, not a guessed timeout or quota.
    // The identical setup and first next call fit; the first extra restriction
    // operation cannot be admitted. This is not a timing assertion.
    let configuration = Configuration {
        max_native_work: reference.statistics().search.work,
        ..configuration
    };
    let mut progress = Progress::default();
    let result = replay::search(
        &source,
        Some(&store),
        configuration,
        &control,
        &mut progress,
        &mut StageTimes::default(),
    );
    assert!(matches!(
        result,
        Err(Error::Native(zetesis_sat::Incomplete::WorkLimit))
    ));
    assert_eq!(progress.stable, [first]);
    assert_eq!(progress.installation_attempts, 1);
    assert_eq!(progress.restarts, 0);
    assert_eq!(progress.native.unwrap().projection_entries, 1);
    assert!(!progress.exhausted);
}

#[test]
fn refused_capacity_proposal_does_not_raise_a_peak() {
    let source = Case::Loops.theory().unwrap();
    let checked = checked(&source, 1);
    let control = Control::default();
    let mut budget = guard::Budget::new(ConstructionLimits::default(), &control);
    let mut store = guard::Store::new(&mut budget).unwrap();
    let peak_build = budget.peak_build_bytes;
    let peak_live = budget.peak_live_bytes;
    budget.limits.max_retained_bytes = budget.retained_bytes;
    assert!(matches!(store.learn(&checked, &mut budget), Err(Error::Limit(Resource::LiveBytes))));
    assert_eq!(budget.peak_build_bytes, peak_build);
    assert_eq!(budget.peak_live_bytes, peak_live);
    assert!(store.guards.is_empty());
}
