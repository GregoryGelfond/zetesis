use super::*;

#[test]
fn every_work_cutoff_retains_typed_refusal() {
    let defaults = FormulaLimits::default();
    let exact = fixture(Shape::Shared, &defaults, None).work;
    for cap in 0..=exact {
        let attempt = fixture(
            Shape::Shared,
            &FormulaLimits {
                max_work: cap,
                ..defaults
            },
            None,
        );
        if cap == exact {
            attempt.result.unwrap();
        } else {
            assert!(matches!(
                attempt.result,
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    ..
                })
            ));
        }
    }
}

#[test]
fn storage_boundaries_include_publication_and_scratch() {
    let defaults = FormulaLimits::default();
    let attempt = fixture(Shape::Shared, &defaults, None);
    let (theory, origins) = attempt.result.unwrap();
    // A tighter allowance can select exact capacity instead of geometric
    // spare capacity. The unconstrained peak is therefore an upper bound,
    // not the least allowance under which the same publication succeeds.
    let support = zetesis_test_support::thresholds::first_success(attempt.peak, |cap| {
        let attempt = fixture(
            Shape::Shared,
            &FormulaLimits {
                max_support_bytes: cap,
                ..defaults
            },
            None,
        );
        match attempt.result {
            Ok((actual, actual_origins)) => {
                assert_eq!(actual.roots(), theory.roots());
                assert_eq!(actual.nodes(), theory.nodes());
                assert_eq!(actual.operands(), theory.operands());
                assert_eq!(actual_origins, origins);
                true
            }
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                observed,
                limit,
                ..
            }) => {
                assert_eq!(limit, cap as u128);
                assert!(observed > limit);
                false
            }
            other => panic!("unexpected support-boundary result: {other:?}"),
        }
    });
    for (resource, exact) in [
        (FormulaResource::Nodes, theory.nodes().len()),
        (FormulaResource::Operands, theory.parts().occurrences()),
        (FormulaResource::Roots, theory.roots().len()),
        (FormulaResource::Origins, origins.iter().map(Vec::len).sum()),
        (FormulaResource::SupportBytes, support),
    ] {
        assert!(exact > 0);
        for cap in [exact - 1, exact] {
            let mut limits = defaults;
            match resource {
                FormulaResource::Nodes => limits.theory.max_nodes = cap,
                FormulaResource::Operands => limits.theory.max_operands = cap,
                FormulaResource::Roots => limits.theory.max_roots = cap,
                FormulaResource::Origins => limits.max_origin_locations = cap,
                FormulaResource::SupportBytes => limits.max_support_bytes = cap,
                _ => unreachable!("selected storage limits"),
            }
            let attempt = fixture(Shape::Shared, &limits, None);
            if cap == exact {
                attempt.result.unwrap();
            } else {
                assert!(
                    matches!(attempt.result, Err(FormulaFailure::Limit { resource: found, .. }) if found == resource),
                    "{resource:?}"
                );
            }
        }
    }
}

#[test]
fn cancellation_retains_its_type_at_each_phase() {
    for point in [
        Point::Eligibility,
        Point::Walk,
        Point::Intersection,
        Point::Publish,
    ] {
        let cancellation = Cancellation::default();
        let (attempt, _) = scoped(true, Some((point, cancellation.clone())), || {
            fixture(
                Shape::Shared,
                &FormulaLimits::default(),
                Some(&cancellation),
            )
        });
        assert!(matches!(
            attempt.result,
            Err(FormulaFailure::Interrupted {
                reason: Stop::Cancelled,
                ..
            })
        ));
    }
}
