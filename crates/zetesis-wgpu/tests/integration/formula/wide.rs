//! Native groups are checked against Boolean semantics, not binary expansion.
use super::{group_allowed, model, propagate_with, values};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, FormulaParts, FrozenReduct, Interpretation, Limits, Node, OperandSpan, Theory,
    Verdict, check,
};
use zetesis_wgpu::{FormulaVerdict, GateProjection};

pub(crate) fn theories() -> Vec<Theory> {
    let mut theories = Vec::new();
    for conjunction in [false, true] {
        for children in [
            vec![1, 2, 3],
            vec![1, 1, 3],
            vec![4, 5, 1, 3],
            vec![0, 1, 2, 3, 4, 5],
            (0..129).map(|i| 1 + i % 3).collect(),
        ] {
            for roots in [vec![6], vec![7], vec![6, 8], vec![]] {
                let span = OperandSpan {
                    start: 0,
                    length: children.len(),
                };
                let nodes = vec![
                    Node::falsum(),
                    Node::atom(0),
                    Node::atom(1),
                    Node::atom(0),
                    Node::implies(1, 0),
                    Node::implies(2, 0),
                    if conjunction {
                        Node::and_span(span)
                    } else {
                        Node::or_span(span)
                    },
                    Node::implies(6, 1),
                    Node::implies(6, 0),
                ];
                theories.push(
                    Theory::new(
                        3,
                        FormulaParts::new(nodes, children.clone()).unwrap(),
                        roots,
                        AdmissionLimits::default(),
                    )
                    .unwrap(),
                );
            }
        }
    }
    theories
}

#[test]
fn native_groups_preserve_arbitrary_frozen_truth_and_completions() {
    for theory in theories() {
        for candidate in 0..8 {
            let interpretation =
                Interpretation::new(&theory, (0..3).filter(|a| candidate & (1 << a) != 0)).unwrap();
            let checked = check(
                &theory,
                &interpretation,
                Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
            assert_eq!(
                model(&theory, candidate, None),
                !matches!(checked.verdict(), Verdict::NotModel { .. })
            );
            let reduct =
                FrozenReduct::new(&interpretation, Limits::default(), &Cancellation::default())
                    .unwrap();
            let frozen = values(&theory, candidate, None);
            for inner in 0..8 {
                // Arbitrary J, including J not contained in M, compares truth;
                // only the propagation test constrains proper subsets.
                let interpretation =
                    Interpretation::new(&theory, (0..3).filter(|a| inner & (1 << a) != 0)).unwrap();
                assert_eq!(
                    model(&theory, inner, Some(&frozen)),
                    reduct
                        .is_satisfied_by(
                            &interpretation,
                            Limits::default(),
                            &Cancellation::default()
                        )
                        .unwrap()
                );
            }
            for projection in GateProjection::ALL {
                match propagate_with(&theory, candidate, 32, projection) {
                    FormulaVerdict::NotModel => assert!(!model(&theory, candidate, None)),
                    FormulaVerdict::NoProperSubset => {
                        assert!((0..8).all(|inner| inner == candidate
                            || inner & !candidate != 0
                            || !model(&theory, inner, Some(&frozen))));
                    }
                    FormulaVerdict::Residual(_) => assert!(model(&theory, candidate, None)),
                }
            }
        }
    }
}

#[test]
fn repeated_slots_preserve_every_racing_snapshot_completion() {
    for conjunction in [false, true] {
        for ids in [[0, 0, 0], [0, 1, 0], [0, 0, 1], [0, 1, 1]] {
            for code in 0u16..256 {
                let observed = [code & 3, (code >> 2) & 3, (code >> 4) & 3]
                    .map(|value| u8::try_from(value).unwrap());
                let output = u8::try_from((code >> 6) & 3).unwrap();
                let (allowed, forced) = group_allowed(conjunction, &ids, &observed, output);
                for current in 0..64 {
                    let domains = [current & 3, (current >> 2) & 3, (current >> 4) & 3];
                    if domains[2] & !output != 0
                        || !(0..3).all(|i| domains[ids[i]] & !observed[i] == 0)
                    {
                        continue;
                    }
                    let mut narrowed = domains;
                    narrowed[2] &= allowed;
                    if let Some((slot, bit)) = forced {
                        if slot == usize::MAX {
                            for &slot in &ids {
                                narrowed[slot] &= bit;
                            }
                        } else {
                            narrowed[slot] &= bit;
                        }
                    }
                    for bits in 0..8 {
                        let assignment = [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0];
                        let truth = if conjunction {
                            ids.iter().all(|&i| assignment[i])
                        } else {
                            ids.iter().any(|&i| assignment[i])
                        };
                        if truth != assignment[2]
                            || !(0..3).all(|i| domains[i] & (1 << u8::from(assignment[i])) != 0)
                        {
                            continue;
                        }
                        assert!((0..3).all(|i| narrowed[i] & (1 << u8::from(assignment[i])) != 0));
                    }
                }
            }
        }
    }
}

#[test]
fn unique_witnesses_count_physical_slots_instead_of_occurrences() {
    for conjunction in [false, true] {
        let all_bit = if conjunction { 2 } else { 1 };
        let witness_bit = 3 ^ all_bit;
        assert_eq!(
            group_allowed(conjunction, &[0, 1, 0], &[3, all_bit, 3], witness_bit),
            (3, Some((0, witness_bit)))
        );
        assert_eq!(
            group_allowed(conjunction, &[0, 1, 0], &[3, 3, 3], witness_bit),
            (3, None)
        );
    }
}
