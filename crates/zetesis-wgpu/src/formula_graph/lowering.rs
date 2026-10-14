//! Physical Boolean headers and the transient semantic-to-physical map.

use super::{address, capacity};
use crate::GpuError;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{NodeView, Theory, extremum_lowering, threshold_lowering};

pub(crate) fn translated(map: &[u32], index: usize) -> Result<u32, GpuError> {
    if map.is_empty() {
        address(index)
    } else {
        map.get(index)
            .copied()
            .ok_or_else(|| capacity("missing semantic formula address"))
    }
}

/// The caller byte-admits and reserves all output, mapping and DP capacity.
/// An empty map reservation selects the unchanged all-Boolean transport path.
pub(crate) fn pack(
    theory: &Theory,
    words: &mut Vec<u32>,
    map: &mut Vec<u32>,
    scratch: &mut Vec<u32>,
    cancellation: &Cancellation,
) -> Result<(), GpuError> {
    if map.capacity() == 0 {
        let mut tail = 0;
        for index in 0..theory.view().len() {
            cancellation.poll().map_err(GpuError::interrupted)?;
            words.extend(super::header(
                theory.view().node(index).expect("admitted node"),
                &mut tail,
            )?);
        }
        return super::append_operands(theory, words, cancellation);
    }
    let mut tail = 0_u32;
    for index in 0..theory.view().len() {
        cancellation.poll().map_err(GpuError::interrupted)?;
        let node = theory.view().node(index).expect("admitted formula node");
        let output = if let Some(output) = lower_numeric(node, words, map, scratch, cancellation)? {
            output
        } else {
            let output = address(words.len() / 4)?;
            let header = match node {
                NodeView::False => [0, 0, 0, 0],
                NodeView::Atom(atom) => [1, address(atom)?, 0, 0],
                NodeView::Implies(left, right) => {
                    [4, translated(map, left)?, translated(map, right)?, 0]
                }
                NodeView::And(children) | NodeView::Or(children) => {
                    let conjunction = matches!(node, NodeView::And(_));
                    if children.len() == 2 {
                        [
                            if conjunction { 2 } else { 3 },
                            translated(map, children[0])?,
                            translated(map, children[1])?,
                            0,
                        ]
                    } else {
                        let length = address(children.len())?;
                        let start = tail;
                        tail = tail
                            .checked_add(length)
                            .ok_or_else(|| capacity("formula operand tail overflows"))?;
                        [if conjunction { 5 } else { 6 }, start, length, 0]
                    }
                }
                NodeView::Threshold(_) | NodeView::ExtremumCut(_) => {
                    unreachable!("numeric node handled above")
                }
            };
            push(words, header)?;
            output
        };
        if map.len() == map.capacity() {
            return Err(capacity("formula map exceeds admitted capacity"));
        }
        map.push(output);
    }
    for index in 0..theory.view().len() {
        cancellation.poll().map_err(GpuError::interrupted)?;
        if let NodeView::And(children) | NodeView::Or(children) =
            theory.view().node(index).expect("admitted node")
            && children.len() > 2
        {
            for &child in children {
                cancellation.poll().map_err(GpuError::interrupted)?;
                if words.len() == words.capacity() {
                    return Err(capacity("formula operands exceed admitted capacity"));
                }
                words.push(translated(map, child)?);
            }
        }
    }
    Ok(())
}

/// Both native domains publish through the same physical sink and address map.
fn lower_numeric(
    node: NodeView<'_>,
    words: &mut Vec<u32>,
    map: &[u32],
    scratch: &mut Vec<u32>,
    cancellation: &Cancellation,
) -> Result<Option<u32>, GpuError> {
    let mut sink = Sink {
        words,
        map,
        cancellation,
        falsum: None,
        truth: None,
    };
    let output = match node {
        NodeView::Threshold(threshold) => {
            let shape = threshold_lowering::Shape::new(threshold, || {
                cancellation.poll().map_err(GpuError::interrupted)
            })?;
            if shape.scratch > scratch.capacity() {
                return Err(capacity("threshold scratch exceeds admitted capacity"));
            }
            while scratch.len() < shape.scratch {
                cancellation.poll().map_err(GpuError::interrupted)?;
                scratch.push(0);
            }
            threshold_lowering::lower(threshold, scratch, &mut sink)?
        }
        NodeView::ExtremumCut(cut) => extremum_lowering::lower(cut, &mut sink)?,
        _ => return Ok(None),
    };
    Ok(Some(output))
}

fn push(words: &mut Vec<u32>, header: [u32; 4]) -> Result<(), GpuError> {
    if words.capacity().saturating_sub(words.len()) < 4 {
        return Err(capacity("physical formula exceeds admitted capacity"));
    }
    words.extend(header);
    Ok(())
}

struct Sink<'a> {
    words: &'a mut Vec<u32>,
    map: &'a [u32],
    cancellation: &'a Cancellation,
    falsum: Option<u32>,
    truth: Option<u32>,
}

impl threshold_lowering::Sink for Sink<'_> {
    type Value = u32;
    type Error = GpuError;

    fn tick(&mut self) -> Result<(), GpuError> {
        self.cancellation.poll().map_err(GpuError::interrupted)
    }

    fn constant(&mut self, value: bool) -> Result<u32, GpuError> {
        let falsum = if let Some(index) = self.falsum {
            index
        } else {
            let index = address(self.words.len() / 4)?;
            push(self.words, [0, 0, 0, 0])?;
            self.falsum = Some(index);
            index
        };
        if !value {
            return Ok(falsum);
        }
        if let Some(index) = self.truth {
            return Ok(index);
        }
        let truth = address(self.words.len() / 4)?;
        push(self.words, [4, falsum, falsum, 0])?;
        self.truth = Some(truth);
        Ok(truth)
    }

    fn condition(&mut self, index: usize) -> Result<u32, GpuError> {
        translated(self.map, index)
    }

    fn gate(&mut self, disjunction: bool, left: u32, right: u32) -> Result<u32, GpuError> {
        self.tick()?;
        let index = address(self.words.len() / 4)?;
        push(
            self.words,
            [if disjunction { 3 } else { 2 }, left, right, 0],
        )?;
        Ok(index)
    }
}
