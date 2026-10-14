//! One immutable GPU transport view of an admitted formula. The fixed headers
//! and complete wide operand rows share one storage binding. Unused source arena
//! cells are omitted; overlapping Boolean spans are copied once per logical row.
//! Thresholds and extremum cuts retain their semantic node IDs and share numeric
//! condition/value columns by the admitted Theory's row identity. No Boolean
//! expansion or second numeric semantic owner is introduced.

use crate::{GpuError, GpuErrorKind};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{ExtremumCutRule, NodeView, Theory};

mod numeric;
pub(crate) use numeric::Rows;

pub(crate) struct Shape {
    pub(crate) bytes: u64,
    pub(crate) nodes: u32,
    pub(crate) rows: Rows,
    pub(crate) numeric_work: u32,
    pub(crate) edges: u32,
    pub(crate) wide_words: u32,
    pub(crate) auxiliary: u32,
}

fn capacity(detail: &str) -> GpuError {
    GpuError::new(GpuErrorKind::Capacity, detail)
}

fn address(value: usize) -> Result<u32, GpuError> {
    u32::try_from(value).map_err(|_| capacity("formula transport address exceeds u32"))
}

fn bytes(nodes: u32, wide_words: u32) -> Result<u64, GpuError> {
    let words = nodes
        .checked_mul(4)
        .and_then(|n| n.checked_add(wide_words))
        .ok_or_else(|| capacity("formula header and operand indexing exceeds u32"))?;
    Ok(u64::from(words.max(4)) * 4)
}

impl Shape {
    #[cfg(test)]
    pub(crate) fn new(theory: &Theory, device: &wgpu::Limits) -> Result<Self, GpuError> {
        Self::with_control(theory, device, u64::MAX, &Cancellation::default())
    }

    pub(crate) fn with_control(
        theory: &Theory,
        device: &wgpu::Limits,
        max_staging_bytes: u64,
        cancellation: &Cancellation,
    ) -> Result<Self, GpuError> {
        let mut nodes = 0_u32;
        let mut edges = 0_u32;
        let mut wide_words = 0u32;
        let mut auxiliary = 0u32;
        let mut numeric_work = 0u32;
        let mut numeric_nodes = 0usize;
        cancellation.poll().map_err(GpuError::interrupted)?;
        for index in 0..theory.view().len() {
            cancellation.poll().map_err(GpuError::interrupted)?;
            let node = theory.view().node(index).expect("admitted formula node");
            let (added_nodes, added_edges) = match node {
                _ if numeric::constant(node, cancellation)?.is_some() => (1, 0),
                NodeView::Threshold(threshold) => {
                    numeric_nodes += 1;
                    let length = address(threshold.conditions.len())?;
                    // Reserve condition visits and four-limb arithmetic for
                    // original evaluation and both linear projection passes.
                    // These are conservative work units, not GPU instructions.
                    numeric_work = numeric_work
                        .checked_add(
                            length
                                .checked_mul(48)
                                .and_then(|n| n.checked_add(32))
                                .ok_or_else(|| capacity("numeric row work overflows"))?,
                        )
                        .ok_or_else(|| capacity("numeric formula work overflows"))?;
                    wide_words = wide_words
                        .checked_add(5)
                        .ok_or_else(|| capacity("numeric descriptors overflow"))?;
                    (1, length)
                }
                NodeView::ExtremumCut(cut) => {
                    // Ranked witness reduction and at most one forcing scan.
                    numeric_nodes += 1;
                    let length = address(cut.row.conditions.len())?;
                    numeric_work = numeric_work
                        .checked_add(
                            length
                                .checked_mul(24)
                                .and_then(|n| n.checked_add(16))
                                .ok_or_else(|| capacity("ordinal row work overflows"))?,
                        )
                        .ok_or_else(|| capacity("numeric formula work overflows"))?;
                    wide_words = wide_words
                        .checked_add(5)
                        .ok_or_else(|| capacity("numeric descriptors overflow"))?;
                    (1, length)
                }
                NodeView::And(children) | NodeView::Or(children) => (1, address(children.len())?),
                NodeView::Implies(..) => (1, 2),
                NodeView::Atom(_) | NodeView::False => (1, 0),
            };
            nodes = nodes
                .checked_add(added_nodes)
                .ok_or_else(|| capacity("physical formula node count overflows"))?;
            edges = edges
                .checked_add(added_edges)
                .ok_or_else(|| capacity("physical formula edge count overflows"))?;
            if !matches!(node, NodeView::Atom(_)) {
                auxiliary = auxiliary
                    .checked_add(added_nodes)
                    .ok_or_else(|| capacity("physical formula auxiliary count overflows"))?;
            }
            if let NodeView::And(children) | NodeView::Or(children) = node
                && children.len() > 2
            {
                wide_words = wide_words
                    .checked_add(address(children.len())?)
                    .ok_or_else(|| capacity("formula wide operand count exceeds u32"))?;
            }
        }
        let rows = Rows::new(
            theory,
            wide_words,
            numeric_nodes,
            max_staging_bytes,
            cancellation,
        )?;
        wide_words = rows.tail_words;
        let bytes = bytes(nodes, wide_words)?;
        if bytes > device.max_buffer_size || bytes > device.max_storage_buffer_binding_size {
            return Err(capacity("formula transport exceeds granted device limits"));
        }
        Ok(Self {
            bytes,
            nodes,
            rows,
            numeric_work,
            edges,
            wide_words,
            auxiliary,
        })
    }
}

/// Encode one admitted logical row. Its groups have at least two operands:
/// pairs are inline, wider rows use offsets relative to the appended tail.
/// This does not revalidate topology. The caller traverses the same Theory for
/// headers and operands and checks the final tail count against Shape.
pub(crate) fn header(
    node: NodeView<'_>,
    tail: &mut u32,
    cancellation: &Cancellation,
) -> Result<[u32; 4], GpuError> {
    if let Some(value) = numeric::constant(node, cancellation)? {
        return Ok([if value { 10 } else { 0 }, 0, 0, 0]);
    }
    Ok(match node {
        NodeView::False => [0, 0, 0, 0],
        NodeView::Atom(atom) => [1, address(atom)?, 0, 0],
        NodeView::Implies(left, right) => [4, address(left)?, address(right)?, 0],
        NodeView::Threshold(threshold) => {
            let start = *tail;
            *tail = tail
                .checked_add(5)
                .ok_or_else(|| capacity("numeric descriptor offset overflows"))?;
            [7, start, address(threshold.conditions.len())?, 0]
        }
        NodeView::ExtremumCut(cut) => {
            let start = *tail;
            *tail = tail
                .checked_add(5)
                .ok_or_else(|| capacity("numeric descriptor offset overflows"))?;
            [
                match cut.rule {
                    ExtremumCutRule::Prefix(_) => 8,
                    ExtremumCutRule::Suffix(_) => 9,
                },
                start,
                address(cut.row.conditions.len())?,
                0,
            ]
        }
        NodeView::And(children) | NodeView::Or(children) => {
            let conjunction = matches!(node, NodeView::And(_));
            if children.len() == 2 {
                [
                    if conjunction { 2 } else { 3 },
                    address(children[0])?,
                    address(children[1])?,
                    0,
                ]
            } else {
                let length = address(children.len())?;
                let start = *tail;
                *tail = tail
                    .checked_add(length)
                    .ok_or_else(|| capacity("formula operand offset exceeds u32"))?;
                [if conjunction { 5 } else { 6 }, start, length, 0]
            }
        }
    })
}

/// Pack one header per semantic node and shared columnar numeric rows.
/// Node IDs therefore retain their original meaning, including roots/producers.
/// The caller reserves the admitted word capacity; packing never grows it.
pub(crate) fn pack(
    theory: &Theory,
    rows: &Rows,
    words: &mut Vec<u32>,
    cancellation: &Cancellation,
) -> Result<(), GpuError> {
    let mut tail = 0;
    for index in 0..theory.view().len() {
        cancellation.poll().map_err(GpuError::interrupted)?;
        append(
            words,
            &header(
                theory.view().node(index).expect("admitted node"),
                &mut tail,
                cancellation,
            )?,
        )?;
    }
    numeric::append(theory, rows, words, cancellation)?;
    if words.is_empty() {
        append(words, &[0; 4])?;
    }
    Ok(())
}

fn append(words: &mut Vec<u32>, values: &[u32]) -> Result<(), GpuError> {
    if words.capacity().saturating_sub(words.len()) < values.len() {
        return Err(capacity("formula transport exceeds admitted capacity"));
    }
    words.extend_from_slice(values);
    Ok(())
}

#[cfg(test)]
mod tests;
