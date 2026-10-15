//! One dependency-level wire schedule for original-truth evaluation.
//!
//! Depths temporarily occupy header word three. Root order and original node
//! IDs remain unchanged; nodes within each level retain their original order.
//! Consumers finalize their own output/padding word only after packing the
//! schedule. No candidate truth or semantic classification is retained here.

use super::{address, capacity};
use crate::GpuError;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::Theory;

fn poll(cancellation: &Cancellation) -> Result<(), GpuError> {
    cancellation.poll().map_err(GpuError::interrupted)
}

/// The existing structural choice: a pure chain exposes no independent work.
pub(crate) const fn parallel_levels(nodes: u32, depth_count: u32) -> u32 {
    if nodes == depth_count { 0 } else { depth_count }
}

/// Roots, then L+1 offsets, then N original IDs; serial uses roots alone.
pub(crate) fn root_words(nodes: u32, roots: u32, levels: u32) -> Result<u32, GpuError> {
    if levels == 0 {
        return Ok(roots.max(1));
    }
    if levels > nodes {
        return Err(capacity("formula levels exceed node count"));
    }
    roots
        .checked_add(levels)
        .and_then(|n| n.checked_add(1))
        .and_then(|n| n.checked_add(nodes))
        .filter(|n| *n <= u32::MAX - 64)
        .ok_or_else(|| capacity("formula schedule addresses overflow"))
}

pub(crate) fn label_depths(
    node_count: u32,
    wide_words: u32,
    nodes: &mut [u32],
    cancellation: &Cancellation,
) -> Result<u32, GpuError> {
    let expected = (node_count as usize * 4 + wide_words as usize).max(4);
    if nodes.len() != expected {
        return Err(capacity("physical formula packing coverage differs"));
    }
    let mut levels = 0;
    for index in 0..node_count as usize {
        poll(cancellation)?;
        let start = index * 4;
        let header = [nodes[start], nodes[start + 1], nodes[start + 2]];
        let child_depth = |child: u32| -> Result<u32, GpuError> {
            if child as usize >= index {
                return Err(capacity("formula depth child is not a prior node"));
            }
            Ok(nodes[child as usize * 4 + 3])
        };
        let depth = match header[0] {
            0 | 1 | 10 => 0,
            2..=4 => child_depth(header[1])?
                .max(child_depth(header[2])?)
                .checked_add(1)
                .ok_or_else(|| capacity("formula dependency depth overflows"))?,
            5..=9 => {
                let mut depth = 0_u32;
                for offset in 0..header[2] as usize {
                    poll(cancellation)?;
                    let tail = node_count as usize * 4;
                    let row = if header[0] >= 7 {
                        nodes[tail + header[1] as usize] + 3
                    } else {
                        header[1]
                    };
                    let child = nodes[tail + row as usize + offset];
                    depth = depth.max(child_depth(child)?);
                }
                depth
                    .checked_add(1)
                    .ok_or_else(|| capacity("formula dependency depth overflows"))?
            }
            _ => return Err(capacity("invalid physical formula operation")),
        };
        levels = levels.max(
            depth
                .checked_add(1)
                .ok_or_else(|| capacity("formula levels overflow"))?,
        );
        nodes[start + 3] = depth;
    }
    Ok(levels)
}

pub(crate) fn pack(
    theory: &Theory,
    node_count: u32,
    levels: u32,
    nodes: &[u32],
    roots: &mut Vec<u32>,
    cancellation: &Cancellation,
) -> Result<(), GpuError> {
    poll(cancellation)?;
    roots.resize(
        usize::try_from(u64::from(root_words(
            node_count,
            address(theory.roots().len())?,
            levels,
        )?))
        .map_err(|_| capacity("formula roots exceed host"))?,
        0,
    );
    for (index, root) in theory.roots().iter().enumerate() {
        poll(cancellation)?;
        roots[index] = address(*root)?;
    }
    if levels == 0 {
        return Ok(());
    }
    let prefix = theory.roots().len();
    let levels = levels as usize;
    let order = prefix + levels + 1;
    // The offset segment changes from counts to prefix ends, then to starts.
    for node in nodes.chunks_exact(4).take(node_count as usize) {
        poll(cancellation)?;
        let count = roots
            .get_mut(prefix + node[3] as usize + 1)
            .filter(|_| (node[3] as usize) < levels)
            .ok_or_else(|| capacity("formula depth exceeds schedule"))?;
        *count = count
            .checked_add(1)
            .ok_or_else(|| capacity("formula level count overflows"))?;
    }
    for level in 1..=levels {
        poll(cancellation)?;
        roots[prefix + level] = roots[prefix + level]
            .checked_add(roots[prefix + level - 1])
            .ok_or_else(|| capacity("formula level prefix overflows"))?;
    }
    if roots[prefix + levels] != node_count {
        return Err(capacity("formula level coverage differs"));
    }
    for index in (0..node_count as usize).rev() {
        poll(cancellation)?;
        let end = &mut roots[prefix + nodes[index * 4 + 3] as usize + 1];
        let position = insert_position(end, node_count)?;
        roots[order + position] = address(index)?;
    }
    roots[prefix..order].rotate_left(1);
    roots[order - 1] = node_count;
    Ok(())
}

pub(crate) fn insert_position(end: &mut u32, nodes: u32) -> Result<usize, GpuError> {
    let position = end
        .checked_sub(1)
        .filter(|position| *position < nodes)
        .ok_or_else(|| capacity("formula level insertion exceeds its node prefix"))?;
    *end = position;
    Ok(position as usize)
}
