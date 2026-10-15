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
                    let output_only = narrowed;
                    if let Some((slot, bit)) = forced {
                        if slot == usize::MAX {
                            for &slot in &ids {
                                narrowed[slot] &= bit;
                            }
                        } else {
                            narrowed[slot] &= bit;
                        }
                    }
                    if allowed != 3 {
                        // The full reference transfer is independent of the
                        // shader shortcut. Even stale aliased observations
                        // cannot make its skipped child intersections useful.
                        assert_eq!(narrowed, output_only);
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

#[test]
fn delayed_group_notifications_preserve_compatible_completions() {
    // Every subset of notifications is a possible prefix of a racing delivery
    // order. Enumerate those prefixes independently of the shader's traversal.
    for conjunction in [false, true] {
        let all = if conjunction { 2u8 } else { 1 };
        let witness = 3 ^ all;
        for ids in [[0usize, 0, 0], [0, 1, 0], [0, 0, 1], [0, 1, 1]] {
            for removed in 0u8..16 {
                let domains = [3 & !removed, 3 & !(removed >> 2)];
                for delivered in 0u8..64 {
                    let notices = [delivered & 3, (delivered >> 2) & 3, (delivered >> 4) & 3];
                    if ids
                        .iter()
                        .zip(notices)
                        .any(|(&slot, notice)| notice & domains[slot] != 0)
                    {
                        continue;
                    }
                    let mut counts = [ids.len(); 2];
                    for notice in notices {
                        counts[0] -= usize::from(notice & all != 0);
                        counts[1] -= usize::from(notice & witness != 0);
                    }
                    let exact = [all, witness]
                        .map(|bit| ids.iter().filter(|&&slot| domains[slot] & bit != 0).count());
                    assert!(counts.iter().zip(exact).all(|(&old, now)| old >= now));
                    if ids
                        .iter()
                        .zip(notices)
                        .all(|(&slot, notice)| notice == (3 ^ domains[slot]))
                    {
                        assert_eq!(counts, exact);
                    }
                    let counted = if counts[0] == ids.len() { all } else { 0 }
                        | if counts[1] != 0 { witness } else { 0 };
                    for output in 0u8..4 {
                        let (allowed, forced) = if counted == 3 && output == witness {
                            group_allowed(conjunction, &ids, &ids.map(|slot| domains[slot]), output)
                        } else {
                            (
                                counted,
                                (counted == 3 && output == all).then_some((usize::MAX, all)),
                            )
                        };
                        let mut narrowed = [domains[0], domains[1], output & allowed];
                        if let Some((slot, bit)) = forced {
                            if slot == usize::MAX {
                                for &child in &ids {
                                    narrowed[child] &= bit;
                                }
                            } else {
                                narrowed[slot] &= bit;
                            }
                        }
                        for bits in 0u8..8 {
                            let assignment = [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0];
                            let truth = if conjunction {
                                ids.iter().all(|&slot| assignment[slot])
                            } else {
                                ids.iter().any(|&slot| assignment[slot])
                            };
                            let before = [domains[0], domains[1], output];
                            if truth == assignment[2]
                                && before
                                    .iter()
                                    .zip(assignment)
                                    .all(|(&domain, value)| domain & (1 << u8::from(value)) != 0)
                            {
                                assert!(
                                    narrowed
                                        .iter()
                                        .zip(assignment)
                                        .all(
                                            |(&domain, value)| domain & (1 << u8::from(value)) != 0
                                        )
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
