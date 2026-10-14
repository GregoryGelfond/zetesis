//! A prefix keeps original child IDs and shared native rows together.

use zetesis_ferraris::{
    AdmissionError, ExtremumCutDescriptor, FormulaNodes, Node, NodeView, NumericRow, Theory,
    ThresholdDescriptor,
};

use super::super::{CountPlanFailureKind as Fault, Work, bytes};

pub(super) fn copy(
    original: &Theory,
    length: usize,
    work: &mut Work,
) -> Result<FormulaNodes, Fault> {
    if length > original.nodes().len() || length > work.limits.theory.max_nodes {
        return Err(Fault::Theory(AdmissionError::Limit));
    }
    let sizes = sizes(original, length, work)?;
    let mut children = work.vector(original.nodes().len())?;
    for index in 0..original.nodes().len() {
        work.charge(1)?;
        children.push((index < length).then_some(index));
    }
    // Selection's three internal maps have source-arena dimensions, even when
    // the chosen prefix retains only some rows. The policy charges logical
    // requested payload; allocator slack is explicitly outside this allowance.
    work.payload(bytes::<Option<usize>>(original.parts().rows().len())?)?;
    work.payload(bytes::<Option<usize>>(original.parts().thresholds().len())?)?;
    work.payload(bytes::<Option<usize>>(
        original.parts().extremum_cuts().len(),
    )?)?;
    work.payload(u64::try_from(sizes.payload).map_err(|_| Fault::Overflow)?)?;
    work.charge(u64::try_from(sizes.copying).map_err(|_| Fault::Overflow)?)?;
    let parts = original
        .view()
        .try_select(&children)
        .map_err(Fault::Theory)?;
    debug_assert_eq!(parts.payload_bytes(), sizes.payload);
    Ok(FormulaNodes::new(parts))
}

struct Sizes {
    payload: u128,
    copying: u128,
}

fn sizes(original: &Theory, length: usize, work: &mut Work) -> Result<Sizes, Fault> {
    let source = original.parts();
    let mut rows = work.vector(source.rows().len())?;
    let mut descriptors = work.vector(source.thresholds().len())?;
    let mut cuts = work.vector(source.extremum_cuts().len())?;
    for _ in source.rows() {
        work.charge(1)?;
        rows.push(false);
    }
    for _ in source.thresholds() {
        work.charge(1)?;
        descriptors.push(false);
    }
    for _ in source.extremum_cuts() {
        work.charge(1)?;
        cuts.push(false);
    }
    let mut sizes = Sizes {
        payload: length as u128 * size_of::<Node>() as u128,
        // Two source selection passes, output writes/occurrence recount, and
        // the initialization/reset of all internal maps.
        copying: 2 * original.nodes().len() as u128
            + 2 * length as u128
            + 2 * (rows.len() as u128 + descriptors.len() as u128 + cuts.len() as u128),
    };
    let mut occurrences = 0_u128;
    for index in 0..length {
        work.charge(1)?;
        let edges = match original.view().node(index).map_err(Fault::Theory)? {
            NodeView::And(row) | NodeView::Or(row) => {
                if row.len() >= 3 {
                    sizes.payload += row.len() as u128 * size_of::<usize>() as u128;
                }
                sizes.copying += row.len() as u128;
                row.len()
            }
            NodeView::Threshold(threshold) => {
                let descriptor = source.nodes()[index]
                    .threshold_descriptor()
                    .expect("threshold storage");
                if !descriptors[descriptor] {
                    descriptors[descriptor] = true;
                    sizes.payload += size_of::<ThresholdDescriptor>() as u128;
                    sizes.copying += 1;
                    let row = source.thresholds()[descriptor].row;
                    if !rows[row] {
                        rows[row] = true;
                        sizes.payload += size_of::<NumericRow>() as u128
                            + threshold.conditions.len() as u128
                                * (size_of::<usize>() + size_of::<u64>()) as u128;
                        sizes.copying += 1 + 2 * threshold.conditions.len() as u128;
                    }
                }
                threshold.conditions.len()
            }
            NodeView::ExtremumCut(cut) => {
                let descriptor = source.nodes()[index]
                    .extremum_cut_descriptor()
                    .expect("extremum cut storage");
                if !cuts[descriptor] {
                    cuts[descriptor] = true;
                    sizes.payload += size_of::<ExtremumCutDescriptor>() as u128;
                    sizes.copying += 1;
                    let row = source.extremum_cuts()[descriptor].row;
                    if !rows[row] {
                        rows[row] = true;
                        sizes.payload += size_of::<NumericRow>() as u128
                            + cut.row.conditions.len() as u128
                                * (size_of::<usize>() + size_of::<u64>()) as u128;
                        sizes.copying += 1 + 2 * cut.row.conditions.len() as u128;
                    }
                }
                cut.row.conditions.len()
            }
            NodeView::Implies(_, _) => {
                sizes.copying += 2;
                2
            }
            NodeView::Atom(_) | NodeView::False => 0,
        };
        occurrences += edges as u128;
        if occurrences > work.limits.theory.max_operands as u128 {
            return Err(Fault::Theory(AdmissionError::Limit));
        }
    }
    Ok(sizes)
}
