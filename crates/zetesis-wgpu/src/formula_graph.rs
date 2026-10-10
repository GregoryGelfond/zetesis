//! One immutable GPU transport view of an admitted formula. The fixed headers
//! and complete wide operand rows share one storage binding. Unused source arena
//! cells are omitted; overlapping source spans are copied once per logical row.

use crate::{GpuError, GpuErrorKind};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{NodeView, Theory, threshold_lowering};

mod lowering;
pub(crate) use lowering::{pack, translated};

pub(crate) struct Shape {
    pub(crate) bytes: u64,
    pub(crate) nodes: u32,
    pub(crate) scratch: usize,
    pub(crate) map: usize,
    pub(crate) temporary_bytes: u64,
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
        Self::with_control(theory, device, &Cancellation::default())
    }

    pub(crate) fn with_control(
        theory: &Theory,
        device: &wgpu::Limits,
        cancellation: &Cancellation,
    ) -> Result<Self, GpuError> {
        let mut nodes = 0_u32;
        let mut edges = 0_u32;
        let mut scratch = 0;
        let mut native = false;
        let mut wide_words = 0u32;
        let mut auxiliary = 0u32;
        cancellation.poll().map_err(GpuError::interrupted)?;
        for index in 0..theory.view().len() {
            cancellation.poll().map_err(GpuError::interrupted)?;
            let node = theory.view().node(index).expect("admitted formula node");
            let (added_nodes, added_edges) = match node {
                NodeView::Threshold(threshold) => {
                    native = true;
                    let shape = threshold_lowering::Shape::new(threshold, || {
                        cancellation.poll().map_err(GpuError::interrupted)
                    })?;
                    scratch = scratch.max(shape.scratch);
                    let constants = if shape.constant() == Some(false) {
                        1
                    } else {
                        2
                    };
                    let count = shape
                        .gates
                        .checked_add(constants)
                        .ok_or_else(|| capacity("threshold physical nodes overflow"))?;
                    let edges = shape
                        .gates
                        .checked_mul(2)
                        .and_then(|n| n.checked_add(if constants == 2 { 2 } else { 0 }))
                        .ok_or_else(|| capacity("threshold physical edges overflow"))?;
                    (address(count)?, address(edges)?)
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
        let bytes = bytes(nodes, wide_words)?;
        if bytes > device.max_buffer_size || bytes > device.max_storage_buffer_binding_size {
            return Err(capacity("formula transport exceeds granted device limits"));
        }
        let map = if native { theory.view().len() } else { 0 };
        let temporary_bytes = u64::try_from(
            map.checked_add(scratch)
                .ok_or_else(|| capacity("formula lowering scratch overflows"))?,
        )
        .ok()
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| capacity("formula lowering bytes overflow"))?;
        Ok(Self {
            bytes,
            nodes,
            scratch,
            map,
            temporary_bytes,
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
pub(crate) fn header(node: NodeView<'_>, tail: &mut u32) -> Result<[u32; 4], GpuError> {
    Ok(match node {
        NodeView::False => [0, 0, 0, 0],
        NodeView::Atom(atom) => [1, address(atom)?, 0, 0],
        NodeView::Implies(left, right) => [4, address(left)?, address(right)?, 0],
        NodeView::Threshold(_) => {
            return Err(capacity("native threshold requires physical lowering"));
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

/// Append complete wide rows after all 4*N header words, in the same node order.
/// The caller reserves `Shape::bytes` and retains or drops this whole transport;
/// no individual tail row is an independently published formula owner.
pub(crate) fn append_operands(
    theory: &Theory,
    words: &mut Vec<u32>,
    cancellation: &Cancellation,
) -> Result<(), GpuError> {
    for index in 0..theory.view().len() {
        cancellation.poll().map_err(GpuError::interrupted)?;
        if let NodeView::And(children) | NodeView::Or(children) =
            theory.view().node(index).expect("admitted formula node")
            && children.len() > 2
        {
            for &child in children {
                cancellation.poll().map_err(GpuError::interrupted)?;
                words.push(address(child)?);
            }
        }
    }
    if words.is_empty() {
        words.extend([0; 4]);
    }
    Ok(())
}

#[cfg(test)]
mod tests;

impl From<threshold_lowering::Error> for GpuError {
    fn from(error: threshold_lowering::Error) -> Self {
        capacity(match error {
            threshold_lowering::Error::Invalid => "invalid physical threshold row or scratch",
            threshold_lowering::Error::Capacity => {
                "threshold physical expansion exceeds host capacity"
            }
        })
    }
}
