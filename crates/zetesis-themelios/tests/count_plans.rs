//! Optional source-count plans preserve the original reduct subject.

#[path = "support/finite_bindings.rs"]
mod reference;

use reference::{atom_text, exhaustive, external, holds, native, values};
use zetesis_cpu::Control;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, CountPlanLimits, CountPlanStatus, ExpansionLimits,
    FormulaLimits, prepare_formula,
};

const SOURCES: &[&str] = &[
    "2{a;b;c;d}2.{a;b}1.{c;d}1.",
    "{e}.2{a;b;c;d}2:-e.{a;b}1:-e.{c;d}1:-e.",
    "{e}.2{a;b;c;d}2:-e.{a;b}1.{c;d}1.",
    "2#count{1:a;2:b;3:c;4:d}2.#count{k:a;l:b}<=1.#count{x:c;y:d}<=1.",
    "2{a;a;b;c;d}2.{b;a}1.{d;c}1.",
    "1<{a;b;c;d}3.{a;b}<2.{c;d}<2.",
    "{e}.2{a;b;c;d}2:-not e.{a;b}1:-not e.{c;d}1:-not e.",
    "{e}.2{a;b;c;d}2:-not not e.{a;b}1:-not not e.{c;d}1:-not not e.",
    "{e}.e:-a.2{a;b;c;d}2:-e.{a;b}1:-e.{c;d}1:-e.",
    "bound(2).N{a;b;c;d}N:-bound(N).{a;b}1.{c;d}1.",
];

fn admitted(source: &str, planned: bool) -> AdmittedFormula {
    let source = prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    if planned {
        source
            .ground_with_count_plan(CountPlanLimits::default(), &Control::default(), None)
            .unwrap()
    } else {
        source.ground().unwrap()
    }
}

fn plan(source: &AdmittedFormula) -> &zetesis_themelios::CountPlan {
    match source.count_plan() {
        CountPlanStatus::Ready(plan) => plan,
        other => panic!("{other:?}"),
    }
}

#[test]
fn ordinary_grounding_does_not_request_count_planning() {
    assert!(matches!(
        admitted(SOURCES[0], false).count_plan(),
        CountPlanStatus::NotRequested
    ));
}

#[test]
fn optional_planning_preserves_original_formula_bytes() {
    for source in SOURCES {
        let baseline = admitted(source, false);
        let planned = admitted(source, true);
        assert_eq!(
            baseline.theory().nodes(),
            planned.theory().nodes(),
            "{source}"
        );
        assert_eq!(
            baseline.theory().roots(),
            planned.theory().roots(),
            "{source}"
        );
        assert_eq!(baseline.atoms(), planned.atoms(), "{source}");
        assert_eq!(
            baseline.formula_origins(),
            planned.formula_origins(),
            "{source}"
        );
        assert_eq!(baseline.source().text(), planned.source().text());
    }
}

#[test]
fn source_theory_entails_each_candidate_restriction() {
    for source in SOURCES {
        let admitted = admitted(source, true);
        let plan = plan(&admitted);
        assert!(admitted.theory().same_instance(plan.original_theory()));
        assert!(!admitted.theory().same_instance(plan.restriction()));
        assert_eq!(plan.consequence_count(), 2, "{source}");
        for candidate in 0..1 << admitted.atoms().len() {
            if holds(
                admitted.theory(),
                &values(admitted.theory(), candidate, None),
            ) {
                assert!(
                    holds(
                        plan.restriction(),
                        &values(plan.restriction(), candidate, None)
                    ),
                    "{source}: M={candidate}"
                );
            }
        }
    }
}

#[test]
fn preproposal_restrictions_preserve_stable_models() {
    for source in SOURCES {
        let admitted = admitted(source, true);
        let mut models = zetesis_sat::StableModels::new(
            admitted.theory(),
            zetesis_sat::Limits::default(),
            Control::default(),
        )
        .unwrap();
        models
            .restrict_candidates(plan(&admitted).restriction())
            .unwrap();
        let actual: reference::Models = models
            .by_ref()
            .map(|model| {
                model
                    .unwrap()
                    .atoms()
                    .map(|atom| atom_text(&admitted.atoms()[atom]))
                    .collect()
            })
            .collect();
        assert!(models.exhausted());
        assert_eq!(models.statistics().candidate_restrictions, 1);
        assert_eq!(actual, native(&admitted), "{source}");
        assert_eq!(actual, exhaustive(&admitted), "{source}");
    }
}

#[test]
fn conditional_activation_cannot_be_dropped() {
    let source = admitted(SOURCES[2], true);
    let names: Vec<_> = source.atoms().iter().map(atom_text).collect();
    let candidate = 1 << names.iter().position(|name| name == "a").unwrap();
    assert!(holds(
        source.theory(),
        &values(source.theory(), candidate, None)
    ));
    assert!(holds(
        plan(&source).restriction(),
        &values(plan(&source).restriction(), candidate, None)
    ));
    assert!(!names.is_empty());
}

#[test]
fn unsupported_premises_produce_no_plan() {
    for source in [
        "a.",
        "e:-a.2{a;b;c;d}2:-e.{a;b}1:-e.{c;d}1:-e.",
        "{a;b;c;d}. {a;b}1.{c;d}1.",
        "2{a;b;c;d}2.{a;b}1.",
        "2{a;b;c;d}2.{a;b}1.{b;c}1.",
        "{e;f}.2{a;b;c;d}2:-e.{a;b}1:-f.{c;d}1:-f.",
        "{e}.2{a:e;b;c;d}2.{a;b}1.{c;d}1.",
        "2{a;b;c;d}2.1{a;b}1.1{c;d}1.",
    ] {
        assert!(
            matches!(
                admitted(source, true).count_plan(),
                CountPlanStatus::NoPlan(_)
            ),
            "{source}"
        );
    }
}

#[test]
#[ignore = "requires an independently installed clingo"]
fn original_sources_match_clingo_full_models() {
    for source in SOURCES {
        let result = external(source, true);
        assert_eq!(result["Models"]["More"], "no");
        let mut expected = reference::Models::new();
        for call in result["Call"].as_array().unwrap() {
            for model in call["Witnesses"].as_array().into_iter().flatten() {
                assert!(
                    expected.insert(
                        model["Value"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_str().unwrap().to_owned())
                            .collect()
                    )
                );
            }
        }
        assert_eq!(
            result["Models"]["Number"].as_u64(),
            Some(u64::try_from(expected.len()).unwrap())
        );
        assert_eq!(native(&admitted(source, true)), expected, "{source}");
    }
}

fn planned_with(source: &str, limits: CountPlanLimits, control: &Control) -> AdmittedFormula {
    prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_with_count_plan(limits, control, None)
    .unwrap()
}

#[test]
fn greedy_discovery_can_miss_a_valid_cover() {
    let missed = admitted("2{a;b;c;d}2.{a;b}1.{a;c}1.{b;d}1.", true);
    assert!(matches!(missed.count_plan(), CountPlanStatus::NoPlan(_)));
    let alternate = admitted("2{a;b;c;d}2.{a;c}1.{b;d}1.", true);
    assert!(matches!(alternate.count_plan(), CountPlanStatus::Ready(_)));
    let remap = |mask: usize| {
        missed
            .atoms()
            .iter()
            .enumerate()
            .fold(0, |result, (index, atom)| {
                result
                    | (usize::from(mask & (1 << index) != 0)
                        << alternate
                            .atoms()
                            .iter()
                            .position(|other| other == atom)
                            .unwrap())
            })
    };
    assert!(!native(&missed).is_empty());
    // The source with the extra overlapping group still entails the useful
    // restriction supplied by the alternate ac/bd cover.
    for candidate in 0..1 << missed.atoms().len() {
        if holds(missed.theory(), &values(missed.theory(), candidate, None)) {
            assert!(holds(
                plan(&alternate).restriction(),
                &values(plan(&alternate).restriction(), remap(candidate), None)
            ));
        }
    }
}

#[test]
fn inapplicable_sources_allocate_no_optional_storage() {
    for source in ["a.", "{a;b}.", "#count{a:a;b:b}!=0.", "{e}.1{a:e;b}1."] {
        let admitted = admitted(source, true);
        let CountPlanStatus::NoPlan(statistics) = admitted.count_plan() else {
            panic!("{source}");
        };
        assert_eq!(statistics.storage_bytes, 0, "{source}");
    }
}

#[test]
fn cancelled_planning_preserves_successful_admission() {
    let control = Control::default();
    control.cancel();
    for source in ["a.", SOURCES[0]] {
        let admitted = planned_with(source, CountPlanLimits::default(), &control);
        let CountPlanStatus::Incomplete(error) = admitted.count_plan() else {
            panic!("{source}");
        };
        assert_eq!(
            error.kind(),
            zetesis_themelios::CountPlanFailureKind::Stopped(zetesis_cpu::Stop::Cancelled)
        );
        assert_eq!(native(&admitted), native(&self::admitted(source, false)));
    }
}

#[test]
fn optional_ceilings_are_inclusive() {
    use zetesis_themelios::{CountPlanFailureKind as Fault, CountPlanResource as Resource};
    type Setting = (Resource, usize, fn(&mut CountPlanLimits, usize));
    let baseline = admitted(SOURCES[0], true);
    let statistics = plan(&baseline).statistics();
    println!("count_plan_limits={statistics:?}");
    let settings: &[Setting] = &[
        (Resource::Groups, statistics.groups, |v, n| v.max_groups = n),
        (Resource::Members, statistics.members, |v, n| {
            v.max_members = n;
        }),
        (Resource::Origins, statistics.origins, |v, n| {
            v.max_origins = n;
        }),
        (Resource::Attempts, statistics.attempts, |v, n| {
            v.max_attempts = n;
        }),
        (
            Resource::Work,
            usize::try_from(statistics.work).unwrap(),
            |v, n| v.max_work = u64::try_from(n).unwrap(),
        ),
        (
            Resource::Bytes,
            usize::try_from(statistics.storage_bytes).unwrap(),
            |v, n| v.max_bytes = u64::try_from(n).unwrap(),
        ),
    ];
    for &(resource, exact, set) in settings {
        assert!(exact > 0);
        let mut limits = CountPlanLimits::default();
        set(&mut limits, exact);
        let exact_input = planned_with(SOURCES[0], limits, &Control::default());
        assert!(
            matches!(exact_input.count_plan(), CountPlanStatus::Ready(_)),
            "{resource:?}: {:?}",
            exact_input.count_plan()
        );
        set(&mut limits, exact - 1);
        let refused = planned_with(SOURCES[0], limits, &Control::default());
        let CountPlanStatus::Incomplete(error) = refused.count_plan() else {
            panic!("{resource:?}");
        };
        assert_eq!(error.kind(), Fault::Limit(resource), "{resource:?}");
        assert_eq!(error.location().source, refused.source().id());
        assert_eq!(native(&refused), native(&baseline));
    }
}

#[test]
fn partition_failure_retains_its_storage_prefix() {
    let full = admitted(SOURCES[0], true);
    let mut limits = CountPlanLimits::default(); // Two capacity dimensions are inspected before coverage storage is admitted.
    limits.partition.max_work = 2;
    let input = planned_with(SOURCES[0], limits, &Control::default());
    let CountPlanStatus::Incomplete(error) = input.count_plan() else {
        panic!();
    };
    let zetesis_themelios::CountPlanFailureKind::Partition(nested) = error.kind() else {
        panic!("{error}");
    };
    assert_eq!(
        nested.kind(),
        zetesis_ferraris::partition::ErrorKind::Limit(zetesis_ferraris::partition::Resource::Work)
    );
    assert!(nested.statistics().construction_bytes > 0);
    assert!(error.statistics().storage_bytes >= nested.statistics().construction_bytes);
    assert!(error.statistics().work >= nested.statistics().work);
    assert_eq!(native(&input), native(&full));
}

#[test]
fn emission_failure_publishes_no_partial_plan() {
    let mut limits = CountPlanLimits::default();
    limits.theory.max_roots = 1;
    let input = planned_with(SOURCES[0], limits, &Control::default());
    assert!(
        matches!(input.count_plan(), CountPlanStatus::Incomplete(error)
        if error.kind()==zetesis_themelios::CountPlanFailureKind::Theory(zetesis_ferraris::AdmissionError::Limit))
    );
    assert_eq!(native(&input), native(&admitted(SOURCES[0], false)));
}

#[test]
fn optional_planning_preserves_original_resource_failures() {
    for maximum in [0, 1, 10, 50, 100] {
        let prepare = || {
            prepare_formula(
                SOURCES[0].into(),
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                FormulaLimits {
                    max_work: maximum,
                    ..FormulaLimits::default()
                },
            )
            .unwrap()
        };
        let ordinary = prepare().ground().unwrap_err();
        let planned = prepare()
            .ground_with_count_plan(CountPlanLimits::default(), &Control::default(), None)
            .unwrap_err();
        assert_eq!(ordinary.to_string(), planned.to_string());
        assert_eq!(ordinary.diagnostics(), planned.diagnostics());
    }
}

#[test]
fn bundle_plans_retain_resolvable_premise_origins() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("entry.lp"),
        "#include \"groups.lp\".2{a;b;c;d}2.",
    )
    .unwrap();
    std::fs::write(directory.path().join("groups.lp"), "{a;b}1.{c;d}1.").unwrap();
    let bundle = zetesis_themelios::SourceBundle::load(
        directory.path().join("entry.lp"),
        zetesis_themelios::BundleLimits::default(),
    )
    .unwrap();
    let input = zetesis_themelios::prepare_bundle_formula(
        bundle,
        zetesis_themelios::BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_with_count_plan(CountPlanLimits::default(), &Control::default(), None)
    .unwrap();
    let CountPlanStatus::Ready(plan) = input.count_plan() else {
        panic!();
    };
    assert!(input.theory().same_instance(plan.original_theory()));
    let mut sources = std::collections::BTreeSet::new();
    for origin in plan.origins() {
        let text = input
            .bundle()
            .get(origin.source)
            .unwrap()
            .source()
            .slice(origin.span)
            .unwrap();
        assert!(!text.is_empty());
        sources.insert(origin.source);
    }
    assert_eq!(sources.len(), 2);
    assert_eq!(plan.consequence_count(), 2);
}

#[test]
fn included_source_refusal_keeps_bundle_ownership() {
    let directory = tempfile::tempdir().unwrap();
    let source = SOURCES[0];
    std::fs::write(directory.path().join("input.lp"), source).unwrap();
    let bundle = zetesis_themelios::SourceBundle::load(
        directory.path().join("input.lp"),
        zetesis_themelios::BundleLimits::default(),
    )
    .unwrap();
    let input = zetesis_themelios::prepare_bundle_formula(
        bundle,
        zetesis_themelios::BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits {
            max_support_rounds: 0,
            ..FormulaLimits::default()
        },
    )
    .unwrap();
    let error = input
        .ground_with_count_plan(CountPlanLimits::default(), &Control::default(), None)
        .unwrap_err();
    assert!(matches!(
        error.error(),
        zetesis_themelios::FormulaFailure::Limit {
            resource: zetesis_themelios::FormulaResource::SupportRounds,
            ..
        }
    ));
    assert_eq!(error.bundle().sources()[0].source().text(), source);
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(128))]
    #[test]
    fn planned_restrictions_follow_direct_partition_arithmetic(left in 2_usize..=3, right in 2_usize..=3, active in proptest::bool::ANY) {
        let atoms = |start: usize, count: usize| (start..start+count).map(|i| format!("p({i})")).collect::<Vec<_>>().join(";");
        let whole=atoms(0,left+right); let first=atoms(0,left); let second=atoms(left,right);
        let body=if active { ":-e" } else { "" };
        let source=format!("{{e}}.2{{{whole}}}2{body}.{{{first}}}1{body}.{{{second}}}1{body}.");
        let admitted=admitted(&source,true);
        let plan=plan(&admitted);
        let positions:Vec<_> = admitted.atoms().iter().map(atom_text).collect();
        for mask in 0..1<<positions.len() {
            let selected = |name:&str| positions.iter().position(|n| n==name).is_some_and(|i| mask & (1<<i)!=0);
            let original = !active || selected("e");
            let first_count=(0..left).filter(|i| selected(&format!("p({i})"))).count();
            let second_count=(left..left+right).filter(|i| selected(&format!("p({i})"))).count();
            let expected= !original || (first_count>=1 && second_count>=1);
            proptest::prop_assert_eq!(holds(plan.restriction(), &values(plan.restriction(),mask,None)),expected);
        }
    }
}

#[test]
fn optional_planning_preserves_every_frozen_world() {
    let mut pairs = 0;
    for source in SOURCES {
        let original = admitted(source, false);
        let planned = admitted(source, true);
        assert_eq!(original.atoms(), planned.atoms());
        assert!(
            original.atoms().len() <= 6,
            "bounded arbitrary interpretation pairs"
        );
        for outer in 0..1 << original.atoms().len() {
            let a = values(original.theory(), outer, None);
            let b = values(planned.theory(), outer, None);
            assert_eq!(a, b, "{source}: M={outer}");
            for inner in 0..1 << original.atoms().len() {
                assert_eq!(
                    values(original.theory(), inner, Some(&a)),
                    values(planned.theory(), inner, Some(&b)),
                    "{source}: M={outer}, J={inner}"
                );
                pairs += 1;
            }
        }
    }
    println!("count_plan_frozen_pairs={pairs}");
}

#[test]
fn source_order_preserves_partition_models() {
    let statements = ["2{a;b;c;d}2.", "{a;b}1.", "{c;d}1."];
    let baseline = native(&admitted(SOURCES[0], false));
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let source = order.map(|index| statements[index]).join("");
        let input = admitted(&source, true);
        assert!(matches!(input.count_plan(), CountPlanStatus::Ready(_)));
        assert_eq!(native(&input), baseline);
        assert_eq!(input.source().text(), source);
    }
}

#[test]
fn count_aliases_do_not_certify_atom_partitions() {
    for source in ["1#count{same:a;same:b}1.", "1#count{x:a;y:a}1."] {
        let prepare = || {
            prepare_formula(
                source.into(),
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                FormulaLimits::default(),
            )
            .unwrap()
        };
        let original = prepare().ground().unwrap();
        let planned = prepare()
            .ground_with_count_plan(CountPlanLimits::default(), &Control::default(), None)
            .unwrap();
        assert!(matches!(planned.count_plan(), CountPlanStatus::NoPlan(_)));
        assert_eq!(planned.theory().nodes(), original.theory().nodes());
        assert_eq!(planned.theory().roots(), original.theory().roots());
        assert_eq!(native(&planned), native(&original));
    }
}

#[test]
fn optional_planning_preserves_objective_templates() {
    let source = format!("{}#minimize{{1@7:a;2@3:b}}.", SOURCES[0]);
    let original = admitted(&source, false);
    let planned = admitted(&source, true);
    assert!(matches!(planned.count_plan(), CountPlanStatus::Ready(_)));
    assert_eq!(
        original.objectives().templates(),
        planned.objectives().templates()
    );
    assert_eq!(
        original.objectives().priorities(),
        planned.objectives().priorities()
    );
    assert_eq!(original.objective_origins(), planned.objective_origins());
    assert_eq!(
        original.objective_declarations(),
        planned.objective_declarations()
    );
}

#[test]
fn stopped_bundle_planning_keeps_original_sources() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("entry.lp"),
        "#include \"groups.lp\".2{a;b;c;d}2.",
    )
    .unwrap();
    std::fs::write(directory.path().join("groups.lp"), "{a;b}1.{c;d}1.").unwrap();
    let bundle = zetesis_themelios::SourceBundle::load(
        directory.path().join("entry.lp"),
        zetesis_themelios::BundleLimits::default(),
    )
    .unwrap();
    let input = zetesis_themelios::prepare_bundle_formula(
        bundle,
        zetesis_themelios::BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_with_count_plan(
        CountPlanLimits {
            max_groups: 0,
            ..CountPlanLimits::default()
        },
        &Control::default(),
        None,
    )
    .unwrap();
    let CountPlanStatus::Incomplete(error) = input.count_plan() else {
        panic!();
    };
    assert_eq!(
        error.kind(),
        zetesis_themelios::CountPlanFailureKind::Limit(
            zetesis_themelios::CountPlanResource::Groups
        )
    );
    let origin = error.location();
    let text = input
        .bundle()
        .get(origin.source)
        .unwrap()
        .source()
        .slice(origin.span)
        .unwrap();
    assert!(text.contains('{'));
    assert_eq!(input.bundle().sources().len(), 2);
    assert_eq!(input.theory().atom_count(), 4);
}
