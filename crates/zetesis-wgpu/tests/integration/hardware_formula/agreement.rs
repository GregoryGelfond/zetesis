//! Verdict classes agree across workgroup sizes away from limits.
//!
//! A size moves sibling gates of a level between "one invocation, in order"
//! and "different invocations, at once", so propagation may take a different
//! number of sweeps. Where every transfer is order-preserving, every size still
//! stops at the same greatest common fixed point. The contract tests establish
//! that for binary gates; for native groups, thresholds and extremum cuts it is
//! an obligation these fixtures check on the device. Each fixture shares
//! operands between siblings of levels wider than the smaller sizes, and the
//! largest has more dirty-domain words than the largest size covers in one
//! stride.

use super::*;
use zetesis_ferraris::{ExtremumCutRule, ExtremumRow, FormulaNodes, FormulaParts};

/// Every compiled size, fixed, and the fitted sizing.
const SIZINGS: [GpuWorkgroupSizing; 5] = [
    GpuWorkgroupSizing::Fixed(GpuWorkgroupSize::Threads32),
    GpuWorkgroupSizing::Fixed(GpuWorkgroupSize::Threads64),
    GpuWorkgroupSizing::Fixed(GpuWorkgroupSize::Threads128),
    GpuWorkgroupSizing::Fixed(GpuWorkgroupSize::Threads256),
    GpuWorkgroupSizing::Fitted,
];

/// Window counts with the size the fitted sizing selects for their levels:
/// `windows + 2` atoms, `windows` covering constraints and `windows / 3` roots.
/// The constraint levels are wider than 32, 64, 128 and 256 nodes in turn; the
/// largest graph has 8,402 nodes, 263 dirty-domain words, which take a second
/// stride even at 256.
const COVERINGS: [(usize, u32); 4] = [(48, 64), (102, 128), (300, 256), (3600, 256)];

/// Covering constraints over consecutive windows of three atoms, cycling a
/// native OR group, a numeric at-least-one threshold and an interior extremum
/// cut, each over its window, so adjacent siblings share two operands. The cut
/// ranks its conditions 1 to 3 in a carrier of 4 and requires a selected maximum
/// of at least 1, which holds exactly when a condition holds; an endpoint
/// boundary would be a constant. Native AND groups over three consecutive
/// constraints are the roots.
fn covering(windows: usize) -> Theory {
    let atoms = windows + 2;
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..atoms).map(Node::atom).collect(), vec![]).unwrap());
    let mut transaction = nodes.transaction();
    let mut constraints = Vec::with_capacity(windows);
    for window in 0..windows {
        let operands = [window, window + 1, window + 2];
        constraints.push(match window % 3 {
            0 => transaction
                .push(NodeView::Or(&operands), usize::MAX, usize::MAX)
                .unwrap(),
            1 => transaction
                .push_threshold_family(&operands, &[1, 1, 1], &[1], usize::MAX, usize::MAX)
                .unwrap()[0],
            _ => {
                transaction
                    .push_extremum_cut_family_range(
                        ExtremumRow {
                            conditions: &operands,
                            ranks: &[1, 2, 3],
                            rank_count: 4,
                        },
                        &[ExtremumCutRule::Suffix(1)],
                        usize::MAX,
                        usize::MAX,
                    )
                    .unwrap()
                    .start
            }
        });
    }
    let roots = constraints
        .chunks_exact(3)
        .map(|three| {
            transaction
                .push(NodeView::And(three), usize::MAX, usize::MAX)
                .unwrap()
        })
        .collect();
    transaction.commit();
    Theory::new(
        atoms,
        nodes.into_parts(),
        roots,
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap()
}

/// Each residue class of the atoms modulo 3 is a minimal cover, refuted by
/// propagation; adding an atom to one leaves a proper subset, a quiescent
/// residual; removing one leaves a window uncovered, which fails a root.
fn candidates(theory: &Theory) -> Vec<Interpretation> {
    let atoms = theory.atom_count();
    let residue = |class: usize| (0..atoms).filter(move |atom| atom % 3 == class);
    let set = |mut members: Vec<usize>| {
        members.sort_unstable();
        members.dedup();
        Interpretation::new(theory, members).unwrap()
    };
    vec![
        set(residue(0).collect()),
        set(residue(1).collect()),
        set(residue(2).collect()),
        set(residue(0).chain([1]).collect()),
        set(residue(0).chain([atoms - 1]).collect()),
        set(residue(1).chain(residue(2)).collect()),
        set((0..atoms).collect()),
        set(residue(0).filter(|atom| *atom != 3).collect()),
    ]
}

pub(super) fn qualify(context: &zetesis_wgpu::GpuContext, projection: GateProjection) {
    for (windows, fitted) in COVERINGS {
        let theory = covering(windows);
        let inputs = candidates(&theory);
        for propagation in [
            GpuPropagation::Dense,
            GpuPropagation::ChangedDependencies,
            GpuPropagation::IncrementalGroups,
        ] {
            let mut agreed: Option<Vec<FormulaVerdict>> = None;
            for workgroup_sizing in SIZINGS {
                let options = GpuKernelOptions {
                    workgroup_sizing,
                    propagation,
                    ..GpuKernelOptions::default()
                };
                let profile =
                    GpuFormulaProfile::from_context_with_kernel(context, projection, options)
                        .unwrap();
                let mut oracle = GpuFormulaOracle::from_profile(&profile).unwrap();
                let verdicts: Vec<_> = oracle
                    .propagate_batch(&theory, &inputs, FormulaLimits::default())
                    .unwrap()
                    .iter()
                    .map(zetesis_wgpu::FormulaCheck::verdict)
                    .collect();
                let context = format!("{windows} windows, {options:?}");
                let stats = oracle.last_batch_stats().unwrap();
                // The requested route ran, at the size this sizing selects.
                assert_eq!(
                    stats.propagation,
                    requested_groups(&oracle, &theory),
                    "{context}"
                );
                assert_eq!(
                    stats.workgroup_size,
                    workgroup_sizing
                        .fixed()
                        .map_or(fitted, GpuWorkgroupSize::invocations),
                    "{context}"
                );
                // Agreement is claimed away from limits only.
                assert!(
                    !verdicts.iter().any(|verdict| matches!(
                        verdict,
                        FormulaVerdict::Residual(
                            ResidualReason::RoundLimit | ResidualReason::WorkLimit
                        )
                    )),
                    "{context}: {verdicts:?}"
                );
                if let Some(expected) = &agreed {
                    assert_eq!(&verdicts, expected, "{context}");
                } else {
                    // Every class occurs, so agreement covers refutation,
                    // quiescence and original truth alike.
                    for class in [
                        FormulaVerdict::NoProperSubset,
                        FormulaVerdict::Residual(ResidualReason::FixedPoint),
                        FormulaVerdict::NotModel,
                    ] {
                        assert!(verdicts.contains(&class), "{context}: {verdicts:?}");
                    }
                    agreed = Some(verdicts);
                }
            }
        }
    }
}
