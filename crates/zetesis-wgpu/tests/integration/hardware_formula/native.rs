//! Native semantic owners exercise physical numeric packing on the device.

use super::*;
use zetesis_ferraris::{ExtremumCutRule, ExtremumRow, FormulaNodes, FormulaParts};

pub(super) fn qualify(oracle: &mut GpuFormulaOracle) {
    qualify_full_width(oracle);
    qualify_carry_and_borrow(oracle);
    qualify_settled_rows(oracle);
    qualify_threshold_guard_families(oracle);
    for bound in -1..=8 {
        let mut nodes = FormulaNodes::new(
            FormulaParts::new(
                vec![
                    Node::atom(0),
                    Node::atom(1),
                    Node::falsum(),
                    Node::implies(0, 2),
                    Node::implies(3, 1),
                ],
                vec![],
            )
            .unwrap(),
        );
        let mut transaction = nodes.transaction();
        let guards = transaction
            .push_threshold_family(
                &[0, 0, 1, 3, 4],
                &[1, 2, 0, 2, 1],
                &[bound, bound + 1],
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        let different = transaction
            .push(
                NodeView::Implies(guards[0], guards[1]),
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        let root = transaction
            .push(
                NodeView::Or(&[different, guards[0], 1]),
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        transaction.commit();
        let theory = Theory::new(
            2,
            nodes.into_parts(),
            vec![root],
            zetesis_ferraris::AdmissionLimits::default(),
        )
        .unwrap();
        compare_native(oracle, &theory);
    }
    for rule in [
        ExtremumCutRule::Prefix(0),
        ExtremumCutRule::Prefix(2),
        ExtremumCutRule::Prefix(5),
        ExtremumCutRule::Suffix(0),
        ExtremumCutRule::Suffix(2),
        ExtremumCutRule::Suffix(5),
    ] {
        qualify_ordinal(oracle, rule, false);
    }
    for bound in 0..=5 {
        for rule in [
            ExtremumCutRule::Prefix(bound),
            ExtremumCutRule::Suffix(bound),
        ] {
            qualify_ordinal(oracle, rule, true);
        }
    }
    qualify_wide_ordinals(oracle);
}

// Upper sums cross the u32/u64 boundaries. Removing either contribution during
// backward forcing borrows across them; both atoms remain true in every model.
fn qualify_carry_and_borrow(oracle: &mut GpuFormulaOracle) {
    for weight in [u64::from(u32::MAX), u64::MAX] {
        qualify_carry_and_borrow_at(oracle, weight);
    }
}

fn qualify_carry_and_borrow_at(oracle: &mut GpuFormulaOracle, weight: u64) {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new(vec![Node::atom(0), Node::atom(1)], vec![]).unwrap());
    let mut transaction = nodes.transaction();
    let root = transaction
        .push(
            NodeView::Threshold(zetesis_ferraris::Threshold {
                conditions: &[0, 1],
                weights: &[weight, 1],
                bound: i128::from(weight) + 1,
            }),
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    transaction.commit();
    let theory = Theory::new(
        2,
        nodes.into_parts(),
        vec![root],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    compare_native(oracle, &theory);
    let both = Interpretation::new(&theory, [0, 1]).unwrap();
    let result = oracle
        .propagate_batch(&theory, &[both], FormulaLimits::default())
        .unwrap();
    assert_eq!(result[0].verdict(), FormulaVerdict::NoProperSubset);
}

// The same result must survive both entailed rows and rows still requiring a
// backward transfer. One round makes an accidental skip of all forcing visible.
fn qualify_settled_rows(oracle: &mut GpuFormulaOracle) {
    for cut in [
        None,
        Some(ExtremumCutRule::Prefix(1)),
        Some(ExtremumCutRule::Suffix(2)),
    ] {
        for settled in [false, true] {
            let mut nodes = FormulaNodes::new(
                FormulaParts::new(vec![Node::atom(0), Node::atom(1), Node::atom(2)], vec![])
                    .unwrap(),
            );
            let mut transaction = nodes.transaction();
            let row = if let Some(rule) = cut {
                let ranks = match rule {
                    ExtremumCutRule::Prefix(_) => [0, 0, 1, 2],
                    ExtremumCutRule::Suffix(_) => [2, 2, 1, 0],
                };
                transaction
                    .push_extremum_cut_family_range(
                        ExtremumRow {
                            conditions: &[0, 0, 1, 2],
                            ranks: &ranks,
                            rank_count: 3,
                        },
                        &[rule],
                        usize::MAX,
                        usize::MAX,
                    )
                    .unwrap()
                    .start
            } else {
                transaction
                    .push_threshold_family(
                        &[0, 0, 1, 2],
                        &[1, 1, 1, 1],
                        &[4],
                        usize::MAX,
                        usize::MAX,
                    )
                    .unwrap()[0]
            };
            transaction.commit();
            let mut roots = vec![row];
            if settled {
                roots.extend([0, 1, 2]);
            } else if cut.is_some() {
                // Only atom 0 qualifies, through two occurrences of one slot.
                roots.extend([1, 2]);
            }
            let theory = Theory::new(
                3,
                nodes.into_parts(),
                roots,
                zetesis_ferraris::AdmissionLimits::default(),
            )
            .unwrap();
            compare_native(oracle, &theory);
            let candidate = Interpretation::new(&theory, [0, 1, 2]).unwrap();
            let results = oracle
                .propagate_batch(
                    &theory,
                    &[candidate],
                    FormulaLimits {
                        max_rounds: 1,
                        ..FormulaLimits::default()
                    },
                )
                .unwrap();
            assert!(matches!(
                results[0].verdict(),
                FormulaVerdict::NoProperSubset
            ));
        }
    }
}

fn compare_native(oracle: &mut GpuFormulaOracle, theory: &Theory) {
    let candidates = candidates(theory);
    for inputs in [candidates.clone(), candidates.into_iter().rev().collect()] {
        let actual = oracle
            .propagate_batch(theory, &inputs, FormulaLimits::default())
            .unwrap();
        for (candidate, result) in inputs.iter().zip(actual) {
            let expected = check(
                theory,
                candidate,
                Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
            assert_eq!(
                matches!(result.verdict(), FormulaVerdict::NotModel),
                matches!(expected.verdict(), Verdict::NotModel { .. })
            );
            if matches!(result.verdict(), FormulaVerdict::NoProperSubset) {
                assert!(expected.accepted());
            }
            // Device domains never cross this API. A surviving proper-subset
            // reduct model must leave exact CPU completion as a residual query.
            if !matches!(expected.verdict(), Verdict::NotModel { .. }) {
                let frozen =
                    FrozenReduct::new(candidate, Limits::default(), &Cancellation::default())
                        .unwrap();
                for tested in &inputs {
                    if tested.atoms().all(|atom| candidate.contains(atom))
                        && tested.atoms().count() < candidate.atoms().count()
                        && frozen
                            .is_satisfied_by(tested, Limits::default(), &Cancellation::default())
                            .unwrap()
                    {
                        assert!(matches!(result.verdict(), FormulaVerdict::Residual(_)));
                    }
                }
            }
        }
    }
}

fn qualify_ordinal(oracle: &mut GpuFormulaOracle, rule: ExtremumCutRule, empty: bool) {
    let mut nodes = FormulaNodes::new(
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::falsum(),
                Node::implies(0, 2),
                Node::implies(3, 1),
            ],
            vec![],
        )
        .unwrap(),
    );
    let (conditions, ranks): (&[usize], &[u64]) = if empty {
        (&[], &[])
    } else {
        (&[0, 0, 1, 3, 4], &[0, 4, 2, 3, 1])
    };
    let mut transaction = nodes.transaction();
    let root = transaction
        .push_extremum_cut_family_range(
            ExtremumRow {
                conditions,
                ranks,
                rank_count: 5,
            },
            &[rule],
            usize::MAX,
            usize::MAX,
        )
        .unwrap()
        .start;
    transaction.commit();
    let theory = Theory::new(
        2,
        nodes.into_parts(),
        vec![root],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    compare_native(oracle, &theory);
}

fn qualify_full_width(oracle: &mut GpuFormulaOracle) {
    for bound in [
        i128::MIN,
        0,
        1,
        1i128 << 32,
        i128::from(u64::MAX),
        i128::from(u64::MAX) + 1,
        i128::from(u64::MAX) * 2,
        i128::MAX,
    ] {
        let mut nodes = FormulaNodes::new(
            FormulaParts::new(vec![Node::atom(0), Node::atom(0), Node::atom(1)], vec![]).unwrap(),
        );
        let mut tx = nodes.transaction();
        let root = tx
            .push(
                NodeView::Threshold(zetesis_ferraris::Threshold {
                    conditions: &[0, 1, 2, 2],
                    weights: &[u64::MAX, u64::MAX, 1, 0],
                    bound,
                }),
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        tx.commit();
        let theory = Theory::new(
            2,
            nodes.into_parts(),
            vec![root],
            zetesis_ferraris::AdmissionLimits::default(),
        )
        .unwrap();
        compare_native(oracle, &theory);
    }
}

fn qualify_wide_ordinals(oracle: &mut GpuFormulaOracle) {
    let cases = [0, 1, 1u64 << 32, (1u64 << 32) + 1, u64::MAX - 1, u64::MAX]
        .map(|bound| (bound, u64::MAX, [0, 1u64 << 32, u64::MAX - 1]))
        .into_iter()
        .chain([
            (1, 1u64 << 32, [0, u64::from(u32::MAX), 1]),
            (1, (1u64 << 32) + 1, [0, 1u64 << 32, 1]),
        ]);
    for (bound, rank_count, ranks) in cases {
        for rule in [
            ExtremumCutRule::Prefix(bound),
            ExtremumCutRule::Suffix(bound),
        ] {
            let mut nodes = FormulaNodes::new(
                FormulaParts::new(
                    vec![Node::atom(0), Node::atom(0), Node::atom(1), Node::falsum()],
                    vec![],
                )
                .unwrap(),
            );
            let mut tx = nodes.transaction();
            let cut = tx
                .push_extremum_cut_family_range(
                    ExtremumRow {
                        conditions: &[0, 1, 2],
                        ranks: &ranks,
                        rank_count,
                    },
                    &[rule],
                    usize::MAX,
                    usize::MAX,
                )
                .unwrap()
                .start;
            // Both a direct guard and its negation exercise the M-false frozen
            // boundary under a true original root, with aliased semantic atoms.
            let negated = tx
                .push(NodeView::Implies(cut, 3), usize::MAX, usize::MAX)
                .unwrap();
            let root = tx
                .push(NodeView::Or(&[cut, negated]), usize::MAX, usize::MAX)
                .unwrap();
            tx.commit();
            let theory = Theory::new(
                2,
                nodes.into_parts(),
                vec![root],
                zetesis_ferraris::AdmissionLimits::default(),
            )
            .unwrap();
            compare_native(oracle, &theory);
        }
    }
}

// One shared carrier must produce both kinds of guard without overwriting its
// followers. Some candidates make only one cut true; each original-false cut
// retains its own reduct gate even though its row also serves a true guard.
pub(super) fn qualify_shared_ordinals(oracle: &mut GpuFormulaOracle) {
    let mut nodes = FormulaNodes::new(
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(0),
                Node::atom(1),
                Node::falsum(),
                Node::implies(0, 3),
            ],
            vec![],
        )
        .unwrap(),
    );
    let mut tx = nodes.transaction();
    let family = tx
        .push_extremum_cut_family_range(
            ExtremumRow {
                conditions: &[0, 1, 2, 4],
                ranks: &[0, 1u64 << 32, u64::MAX - 1, 7],
                rank_count: u64::MAX,
            },
            &[
                ExtremumCutRule::Prefix(0),
                ExtremumCutRule::Suffix(u64::MAX),
                ExtremumCutRule::Prefix(1),
                ExtremumCutRule::Suffix(u64::MAX - 1),
                ExtremumCutRule::Prefix(u64::MAX),
                ExtremumCutRule::Suffix(0),
            ],
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    let mut roots = Vec::new();
    for guard in [family.start + 2, family.start + 3] {
        let negated = tx
            .push(NodeView::Implies(guard, 3), usize::MAX, usize::MAX)
            .unwrap();
        roots.push(
            tx.push(NodeView::Or(&[guard, negated]), usize::MAX, usize::MAX)
                .unwrap(),
        );
    }
    tx.commit();
    // An unused third atom supplies proper-subset witnesses for candidates
    // containing it; the shared-cut test must preserve residual completion too.
    let theory = Theory::new(
        3,
        nodes.into_parts(),
        roots,
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    compare_native(oracle, &theory);
}

// The host characterization establishes the corresponding CPU frozen closure.
// This assertion detects the missing cross-guard deduction: the old local
// threshold transfers reached a fixed-point residual on the same candidate.
fn qualify_threshold_guard_families(oracle: &mut GpuFormulaOracle) {
    for weight in [1, u64::MAX] {
        for order in [&[1, 2][..], &[2, 1], &[4, 1, 2]] {
            let theory = crate::formula::frozen_guards::theory(weight, order);
            compare_native(oracle, &theory);
            let candidate = Interpretation::new(&theory, [0, 1, 2]).unwrap();
            let result = oracle
                .propagate_batch(&theory, &[candidate], FormulaLimits::default())
                .unwrap();
            assert_eq!(result[0].verdict(), FormulaVerdict::NoProperSubset);
            assert_eq!(
                oracle.last_batch_stats().unwrap().propagation,
                oracle.compiled_profile().kernel_options().propagation
            );
        }
    }
}
