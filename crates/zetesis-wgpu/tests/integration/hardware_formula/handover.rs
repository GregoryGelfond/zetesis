//! A residual atom region must retain every proper-subset reduct model.
use super::*;
use zetesis_algebra::region::Region;

/// `universe` is the fixture's complete list of interpretations, including
/// the candidate. The frozen truth test is independent of the GPU masks.
pub(super) fn check_coverage(
    candidate: &Interpretation,
    region: &Region,
    universe: &[Interpretation],
    full_root_stable: bool,
) {
    let theory = candidate.theory();
    assert_eq!(region.len(), theory.atom_count());
    for atom in 0..region.len() {
        if !candidate.contains(atom) {
            assert!(region.is_cut(atom));
        }
    }
    let control = Cancellation::default();
    let reduct = FrozenReduct::new(candidate, Limits::default(), &control).unwrap();
    let mut has_witness = false;
    for subset in universe {
        if !subset.atoms().all(|atom| candidate.contains(atom))
            || candidate.atoms().all(|atom| subset.contains(atom))
        {
            continue;
        }
        if reduct
            .is_satisfied_by(subset, Limits::default(), &control)
            .unwrap()
        {
            assert!(
                region
                    .decided()
                    .all(|(atom, held)| subset.contains(atom) == held),
                "the handed-over region excludes a validated countermodel"
            );
            has_witness = true;
        }
    }
    assert_eq!(full_root_stable, !has_witness);
}

/// Under {x}, the proper subset {} models x → x. With one removable atom
/// the strict-subset step cuts x. A producer that interchanges its two
/// mask columns would hand over {x} and lose the only witness. All shape
/// and outside-candidate checks would still pass for this full candidate.
pub(super) fn check_subset_cut(oracle: &mut GpuFormulaOracle) {
    let graph = theory(1, vec![Node::atom(0), Node::implies(0, 0)], vec![1]);
    let all = candidates(&graph);
    let candidate = &all[1];
    assert!(candidate.contains(0));
    for (rounds, reason, decision) in [
        (0, ResidualReason::RoundLimit, None),
        (1, ResidualReason::RoundLimit, Some(false)),
    ] {
        let results = oracle
            .propagate_batch(
                &graph,
                std::slice::from_ref(candidate),
                FormulaLimits {
                    max_rounds: rounds,
                    ..FormulaLimits::default()
                },
            )
            .unwrap();
        let FormulaVerdict::Residual(actual, root) = results[0].verdict() else {
            panic!("the proper-subset witness remains for host completion");
        };
        assert_eq!(*actual, reason);
        assert_eq!(results[0].statistics().rounds, rounds);
        assert_eq!(root.decision(0), decision);
        check_coverage(candidate, root, &all, false);
    }
}
