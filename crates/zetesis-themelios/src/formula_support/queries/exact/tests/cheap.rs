//! Zero/singleton postings keep the old query's exact work and failure path.

use super::*;
use crate::GroundingPhase;
use crate::formula_support::queries::IndexedSelection;
use crate::grounding_observer::Profile;
use crate::test_support::Observer;

#[test]
fn empty_and_singleton_postings_keep_the_original_work() {
    let mut fixture = testing::Fixture::from_atoms([atom(&[1, 2]), atom(&[2, 3])], location());
    let pattern = pattern(vec![Term::Variable(0), Term::Variable(1)]);
    fixture.with(location(), |support, _, setup| {
        let limits = FormulaLimits::default();
        let rows = support
            .resolve((&pattern).into(), &limits, setup, location())
            .unwrap();
        let bytes = support.live_bytes();
        // The final entry selects one necessary row, whose second argument
        // fails the whole matcher. The shortcut must not certify a tuple match.
        for (first, second) in [(1, 2), (0, 2), (1, 3)] {
            let values = [Some(Value::Number(first)), Some(Value::Number(second))];
            let mut old = Counters::default();
            let expected = support
                .probe_at(
                    rows,
                    (&pattern).into(),
                    values.as_slice().into(),
                    &limits,
                    &mut old,
                    location(),
                )
                .unwrap();
            assert!(expected.is_some_and(|posting| posting.len() <= 1));
            let mut actual = Counters::default();
            let (selection, points) = scoped(true, || {
                support
                    .indexed_selection_at(
                        rows,
                        PositivePattern::Flat((&pattern).into()),
                        values.as_slice().into(),
                        (None, 0),
                        GroundingWork::new(&limits, &mut actual, location()),
                    )
                    .unwrap()
            });
            let IndexedSelection::Posting(posting) = selection else {
                panic!("cheap posting must retain the original route");
            };
            assert_eq!(posting, expected);
            assert_eq!(actual.accounting.work, old.accounting.work);
            assert_eq!(points, 0);
            assert_eq!(support.live_bytes(), bytes);
        }
    });
}

#[test]
fn a_nullary_source_keeps_its_original_unrestricted_probe() {
    let mut fixture = testing::Fixture::from_atoms([atom(&[])], location());
    let pattern = pattern(vec![]);
    fixture.with(location(), |support, _, setup| {
        let limits = FormulaLimits::default();
        let rows = support
            .resolve((&pattern).into(), &limits, setup, location())
            .unwrap();
        let values = (&[] as &[Option<Value>]).into();
        let mut old = Counters::default();
        let expected = support
            .probe_at(
                rows,
                (&pattern).into(),
                values,
                &limits,
                &mut old,
                location(),
            )
            .unwrap();
        assert!(expected.is_none());
        let mut actual = Counters::default();
        let (selection, points) = scoped(true, || {
            support
                .indexed_selection_at(
                    rows,
                    PositivePattern::Flat((&pattern).into()),
                    values,
                    (None, 0),
                    GroundingWork::new(&limits, &mut actual, location()),
                )
                .unwrap()
        });
        assert!(matches!(selection, IndexedSelection::Posting(None)));
        assert_eq!(points, 0);
        assert_eq!(actual.accounting.work, old.accounting.work);
    });
}

#[test]
fn wider_complete_posting_keeps_point_selection_and_one_logical_probe() {
    let atoms = (0..8).flat_map(|first| (0..8).map(move |second| atom(&[first, second])));
    let mut fixture = testing::Fixture::from_atoms(atoms, location());
    let pattern = pattern(vec![Term::Variable(0), Term::Variable(1)]);
    fixture.with(location(), |support, _, setup| {
        let limits = FormulaLimits::default();
        let rows = support
            .resolve((&pattern).into(), &limits, setup, location())
            .unwrap();
        let values = [Some(Value::Number(3)), Some(Value::Number(4))];
        let mut separate = Counters::default();
        let posting = support
            .probe_at(
                rows,
                (&pattern).into(),
                values.as_slice().into(),
                &limits,
                &mut separate,
                location(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(posting.len(), 8);
        let expected = support
            .exact_at(
                rows,
                PositivePattern::Flat((&pattern).into()),
                values.as_slice().into(),
                (None, 0),
                GroundingWork::new(&limits, &mut separate, location()),
            )
            .unwrap()
            .unwrap();
        let observer = Observer::default();
        let profile = Profile::new(Some(&observer));
        let mut actual = Counters::resume(
            crate::formula_support::Accounting::default(),
            profile.work(),
        );
        let (selection, points) = scoped(true, || {
            profile
                .phase(GroundingPhase::RuleInstantiation, None, || {
                    support.indexed_selection_at(
                        rows,
                        PositivePattern::Flat((&pattern).into()),
                        values.as_slice().into(),
                        (None, 0),
                        GroundingWork::new(&limits, &mut actual, location()),
                    )
                })
                .unwrap()
        });
        let IndexedSelection::Exact(range) = selection else {
            panic!("wider complete posting must retain the point route");
        };
        assert_eq!(range, 28..29);
        assert_eq!(range, expected);
        assert_eq!(points, 1);
        assert_eq!(observer.0.get().join_probes, Some(1));
        assert_eq!(observer.0.get().indexed_probes, Some(1));
    });
}

#[test]
fn wider_partial_posting_keeps_its_full_original_rows() {
    let mut fixture = testing::Fixture::from_atoms([atom(&[1, 2]), atom(&[1, 3])], location());
    let pattern = pattern(vec![Term::Variable(0), Term::Variable(1)]);
    fixture.with(location(), |support, _, counters| {
        let limits = FormulaLimits::default();
        let rows = support
            .resolve((&pattern).into(), &limits, counters, location())
            .unwrap();
        let selection = support
            .indexed_selection_at(
                rows,
                PositivePattern::Flat((&pattern).into()),
                [Some(Value::Number(1)), None].as_slice().into(),
                (None, 0),
                GroundingWork::new(&limits, counters, location()),
            )
            .unwrap();
        let IndexedSelection::Posting(Some(posting)) = selection else {
            panic!("partial query must retain the posting");
        };
        assert_eq!(posting, [0, 1]);
    });
}

#[test]
fn cheap_query_stops_match_the_original_receipts() {
    let mut fixture = testing::Fixture::from_atoms([atom(&[1, 2])], location());
    let pattern = pattern(vec![Term::Variable(0), Term::Variable(1)]);
    fixture.with(location(), |support, _, setup| {
        let defaults = FormulaLimits::default();
        let rows = support
            .resolve((&pattern).into(), &defaults, setup, location())
            .unwrap();
        let values = [Some(Value::Number(1)), Some(Value::Number(2))];
        let mut full = Counters::default();
        support
            .probe_at(
                rows,
                (&pattern).into(),
                values.as_slice().into(),
                &defaults,
                &mut full,
                location(),
            )
            .unwrap();
        for maximum in 0..full.accounting.work {
            let limits = FormulaLimits {
                max_work: maximum,
                ..defaults
            };
            let mut old = Counters::default();
            let expected = support
                .probe_at(
                    rows,
                    (&pattern).into(),
                    values.as_slice().into(),
                    &limits,
                    &mut old,
                    location(),
                )
                .unwrap_err();
            let mut actual = Counters::default();
            let failure = support
                .indexed_selection_at(
                    rows,
                    PositivePattern::Flat((&pattern).into()),
                    values.as_slice().into(),
                    (None, 0),
                    GroundingWork::new(&limits, &mut actual, location()),
                )
                .err()
                .expect("the same original probe must refuse");
            assert_eq!(work_refusal(&failure), work_refusal(&expected));
            assert_eq!(actual.accounting.work, old.accounting.work);
        }
    });
}

fn work_refusal(error: &FormulaFailure) -> (FormulaResource, u128, u128, crate::ProgramSite) {
    let FormulaFailure::Limit {
        resource,
        observed,
        limit,
        location,
    } = error
    else {
        panic!("expected the original work limit, got {error:?}");
    };
    (*resource, *observed, *limit, *location)
}

#[test]
fn cheap_query_cancellation_keeps_the_original_receipt() {
    let mut fixture = testing::Fixture::from_atoms([atom(&[1, 2])], location());
    let pattern = pattern(vec![Term::Variable(0), Term::Variable(1)]);
    fixture.with(location(), |support, _, setup| {
        let defaults = FormulaLimits::default();
        let rows = support
            .resolve((&pattern).into(), &defaults, setup, location())
            .unwrap();
        let values = [Some(Value::Number(1)), Some(Value::Number(2))];
        let cancellation = Cancellation::default();
        cancellation.cancel();
        let mut old = Counters::default();
        old.attach_test_control(&cancellation);
        let expected = support
            .probe_at(
                rows,
                (&pattern).into(),
                values.as_slice().into(),
                &defaults,
                &mut old,
                location(),
            )
            .unwrap_err();
        let mut actual = Counters::default();
        actual.attach_test_control(&cancellation);
        let failure = support
            .indexed_selection_at(
                rows,
                PositivePattern::Flat((&pattern).into()),
                values.as_slice().into(),
                (None, 0),
                GroundingWork::new(&defaults, &mut actual, location()),
            )
            .err()
            .expect("cancellation must not publish a query result");
        let FormulaFailure::Interrupted {
            reason,
            location: actual_location,
        } = failure
        else {
            panic!("expected the original interruption");
        };
        let FormulaFailure::Interrupted {
            reason: old_reason,
            location: old_location,
        } = expected
        else {
            panic!("expected the reference interruption");
        };
        assert_eq!((reason, actual_location), (old_reason, old_location));
        assert_eq!(reason, Stop::Cancelled);
        assert_eq!(actual.accounting.work, old.accounting.work);
    });
}

#[test]
fn cheap_postings_preserve_all_ground_subjects_and_final_filters() {
    // These are the preceding point-route fixtures, now intentionally resolved
    // by their zero/singleton postings. Wider variants remain in the parent test.
    for source in [
        "a(1).b(1,1).{p}.h:-a(X),b(X,X),p.",
        "-a(1).b(1,2).{p}.h:- -a(X),b(X,2),not p.",
        "a(f(1)).b(f(1),2).{p}.h:-a(X),b(X,2),not not p.",
        "a(f(1)).a(g(2)).b(1,2).{p}.h:-a(f(X)),b(X,2),p.",
        "a(1).a(2).b(1,1).b(2,2).{p}.h:-a(X),b(X,X),p,X<2.",
        "a(1).b(1,2).b(2,3).{p}.h:-a(X),b(X,3),p.",
    ] {
        let compile = |enabled| {
            scoped(enabled, || {
                crate::formula_ground::ground(testing::prepare(source), None, None).unwrap()
            })
        };
        let (actual, points) = compile(true);
        let (reference, disabled) = compile(false);
        assert_eq!(points, 0, "cheap route must be exercised: {source}");
        assert_eq!(disabled, 0);
        assert_eq!(
            compiled_atoms(&actual),
            compiled_atoms(&reference),
            "{source}"
        );
        assert_eq!(actual.theory.nodes(), reference.theory.nodes(), "{source}");
        assert_eq!(
            actual.theory.operands(),
            reference.theory.operands(),
            "{source}"
        );
        assert_eq!(actual.theory.roots(), reference.theory.roots(), "{source}");
        assert_eq!(actual.origins, reference.origins, "{source}");
        assert_eq!(actual.warnings, reference.warnings, "{source}");
    }
}
