use super::*;
use crate::{AdmissionLimits, FormulaParts, Node, Theory, TightPlanLimits};

#[test]
fn checks_retain_their_scratch_allocations() {
    // The plan has no readers index, so every check here evaluates completely;
    // the update route's propositions are in `tight/evaluate/update_tests.rs`
    // and `tight/evaluate/incremental_tests.rs`.
    let theory = Theory::new(
        1,
        FormulaParts::new(vec![Node::atom(0)], vec![]).unwrap(),
        vec![0],
        AdmissionLimits::default(),
    )
    .unwrap();
    let cancellation = Cancellation::default();
    let plan = TightPlan::compile(&theory, TightPlanLimits::default(), &cancellation).unwrap();
    let present = Interpretation::new(&theory, [0]).unwrap();
    let absent = Interpretation::new(&theory, []).unwrap();
    let mut workspace = TightWorkspace::default();
    workspace
        .check(&plan, &present, TightCheckLimits::default(), &cancellation)
        .result
        .unwrap();
    let values = workspace.values.as_ptr();
    let supported = workspace.supported.as_ptr();
    let retained = workspace.retained_bytes();
    assert!(!workspace.values.is_empty());
    assert!(!workspace.supported.is_empty());
    for candidate in [&absent, &present, &absent, &present] {
        workspace
            .check(&plan, candidate, TightCheckLimits::default(), &cancellation)
            .result
            .unwrap();
        assert_eq!(workspace.values.as_ptr(), values);
        assert_eq!(workspace.supported.as_ptr(), supported);
        assert_eq!(workspace.retained_bytes(), retained);
    }
}

#[test]
fn decisive_operands_do_not_require_suffix_work() {
    let cancellation = Cancellation::default();
    for (conjunction, values) in [(true, [0, 1, 1]), (false, [1, 0, 0])] {
        let metered = cancellation.metered(1, |meter| {
            evaluate_operands(&[0, 1, 2], &values, conjunction, meter)
        });
        assert_eq!(metered.result, Ok(!conjunction));
        assert_eq!(metered.charged, 1);
    }
}

#[test]
fn nondecisive_operands_require_the_complete_group() {
    let cancellation = Cancellation::default();
    for conjunction in [false, true] {
        let values = [u8::from(conjunction); 3];
        for max in [2, 3] {
            let metered = cancellation.metered(max, |meter| {
                evaluate_operands(&[0, 1, 2], &values, conjunction, meter)
            });
            assert_eq!(metered.charged, max);
            assert_eq!(
                metered.result,
                if max == 3 {
                    Ok(conjunction)
                } else {
                    Err(TightError::Limit(crate::TightResource::Work))
                },
            );
        }
    }
}

fn fact_carrier(atoms: usize, facts: &[usize]) -> Theory {
    Theory::new(
        atoms,
        FormulaParts::new(facts.iter().copied().map(Node::atom).collect(), vec![]).unwrap(),
        (0..facts.len()).collect(),
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn packed_support_finds_the_first_missing_atom() {
    let facts = [0, 31, 31, 32, 63, 64, 95, 96, 129];
    let theory = fact_carrier(130, &facts);
    let cancellation = Cancellation::default();
    let plan = TightPlan::compile(&theory, TightPlanLimits::default(), &cancellation).unwrap();
    let mut workspace = TightWorkspace::default();
    for extras in [
        &[][..],
        &[128, 65, 33, 1][..],
        &[128, 65, 33][..],
        &[128, 65][..],
        &[128][..],
    ] {
        let candidate = Interpretation::new(&theory, facts.iter().chain(extras).copied()).unwrap();
        let check = workspace
            .check(
                &plan,
                &candidate,
                TightCheckLimits::default(),
                &cancellation,
            )
            .result
            .unwrap();
        assert_eq!(
            check.verdict,
            extras.iter().min().map_or(TightVerdict::Stable, |&atom| {
                TightVerdict::Residual {
                    unsupported_atom: atom,
                }
            }),
        );
    }
    assert_eq!(workspace.supported.len(), 5);
    assert_eq!(workspace.supported[4], 2);
}

#[test]
fn empty_candidates_charge_each_packed_word() {
    let cancellation = Cancellation::default();
    for atoms in [0, 1, 31, 32, 33, 63, 64, 65, 129] {
        let theory = fact_carrier(atoms, &[]);
        let plan = TightPlan::compile(&theory, TightPlanLimits::default(), &cancellation).unwrap();
        let candidate = Interpretation::new(&theory, []).unwrap();
        let words = u64::try_from(atoms.div_ceil(32)).unwrap();
        for max_work in 0..=words {
            let attempt = plan.check_accounted(
                &candidate,
                TightCheckLimits {
                    max_work,
                    ..TightCheckLimits::default()
                },
                &cancellation,
            );
            assert_eq!(attempt.work, max_work);
            if max_work == words {
                assert_eq!(attempt.result.unwrap().verdict, TightVerdict::Stable);
            } else {
                assert_eq!(
                    attempt.result,
                    Err(TightError::Limit(crate::TightResource::Work)),
                );
            }
        }
    }
}

#[test]
fn packed_support_does_not_retain_prior_body_truth() {
    // The plan has no readers index, so every check here evaluates completely;
    // the update route's propositions are in `tight/evaluate/update_tests.rs`
    // and `tight/evaluate/incremental_tests.rs`.
    let theory = Theory::new(
        65,
        FormulaParts::new(
            vec![
                Node::falsum(),
                Node::atom(0),
                Node::implies(1, 0),
                Node::or_pair([1, 2]),
                Node::atom(64),
                Node::implies(1, 4),
            ],
            vec![],
        )
        .unwrap(),
        vec![3, 5],
        AdmissionLimits::default(),
    )
    .unwrap();
    let cancellation = Cancellation::default();
    let plan = TightPlan::compile(&theory, TightPlanLimits::default(), &cancellation).unwrap();
    let mut workspace = TightWorkspace::default();
    for (atoms, expected) in [
        (&[0, 64][..], TightVerdict::Stable),
        (
            &[64][..],
            TightVerdict::Residual {
                unsupported_atom: 64,
            },
        ),
        (&[][..], TightVerdict::Stable),
        (&[0][..], TightVerdict::NotModel { root: 5 }),
        (&[0, 64][..], TightVerdict::Stable),
    ] {
        let candidate = Interpretation::new(&theory, atoms.iter().copied()).unwrap();
        let result = workspace
            .check(
                &plan,
                &candidate,
                TightCheckLimits::default(),
                &cancellation,
            )
            .result
            .unwrap();
        assert_eq!(result.verdict, expected);
    }
}
