use std::mem::size_of;

use super::{
    AdmissionError, ExtremumCutDescriptor, FormulaParts, FormulaTransaction, FormulaView, Node,
    NumericRow, Storage, ThresholdDescriptor,
};

pub(super) fn copy<T: Copy>(source: &[T]) -> Result<Vec<T>, AdmissionError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(source.len())
        .map_err(|_| AdmissionError::Allocation)?;
    result.extend_from_slice(source);
    Ok(result)
}

fn bytes<T>(count: usize) -> u128 {
    count as u128 * size_of::<T>() as u128
}

impl FormulaParts {
    /// Copy all seven arenas fallibly, preserving sharing and absolute child IDs.
    /// The copy is raw storage and carries no admission or original owner identity.
    /// Work and temporary payload are linear in all stored cells, including unused
    /// cells. [`Self::copy_work`] reports the complete logical copy/recount charge.
    /// Actual destination capacities may differ from the source capacities.
    ///
    /// # Errors
    /// Refuses failed reservation or overflow in declared logical occurrences.
    pub fn try_clone(&self) -> Result<Self, AdmissionError> {
        self.view().try_to_parts()
    }

    /// Complete logical work for [`Self::try_clone`]: one visit per copied cell
    /// in all seven arenas, then one visit per node to recount child occurrences.
    /// Shared numeric entries are copied once, irrespective of how many nodes
    /// reference their row. Unreferenced arena cells still contribute.
    ///
    /// This allocation-free measurement is a full-attempt precharge; an early
    /// refusal may perform less work. It excludes fixed reservation bookkeeping,
    /// allocator work, hardware instructions and topology/numeric admission.
    /// It neither checks dimensions nor measures bytes or retained capacity.
    #[must_use]
    pub fn copy_work(&self) -> u128 {
        2 * self.nodes.len() as u128
            + self.operands.len() as u128
            + self.numeric.conditions.len() as u128
            + self.numeric.values.len() as u128
            + self.numeric.rows.len() as u128
            + self.numeric.thresholds.len() as u128
            + self.numeric.cuts.len() as u128
    }

    /// Bytes occupied by the lengths of all seven vector payloads.
    /// Excludes vector headers, this owner, allocator overhead and roots.
    #[must_use]
    pub fn payload_bytes(&self) -> u128 {
        bytes::<Node>(self.nodes.len())
            + bytes::<usize>(self.operands.len())
            + bytes::<usize>(self.numeric.conditions.len())
            + bytes::<u64>(self.numeric.values.len())
            + bytes::<NumericRow>(self.numeric.rows.len())
            + bytes::<ThresholdDescriptor>(self.numeric.thresholds.len())
            + bytes::<ExtremumCutDescriptor>(self.numeric.cuts.len())
    }

    /// Bytes retained by the actual capacities of all seven vector allocations.
    /// Excludes vector headers, this owner, allocator overhead and roots.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        bytes::<Node>(self.nodes.capacity())
            + bytes::<usize>(self.operands.capacity())
            + bytes::<usize>(self.numeric.conditions.capacity())
            + bytes::<u64>(self.numeric.values.capacity())
            + bytes::<NumericRow>(self.numeric.rows.capacity())
            + bytes::<ThresholdDescriptor>(self.numeric.thresholds.capacity())
            + bytes::<ExtremumCutDescriptor>(self.numeric.cuts.capacity())
    }
}

impl FormulaView<'_> {
    /// Fallibly copy the complete raw owner, preserving row sharing and child IDs.
    /// This includes unreferenced cells and established numeric shape validation;
    /// no topology frontier or admitted-theory identity is copied. The numeric
    /// cache is safe because every node, row, descriptor and child ID is identical.
    /// Work and payload are linear in all arena lengths.
    ///
    /// # Errors
    /// Refuses failed reservations or overflow in declared logical occurrences.
    pub fn try_to_parts(self) -> Result<FormulaParts, AdmissionError> {
        let nodes = copy(self.nodes)?;
        let operands = copy(self.operands)?;
        let numeric = self.numeric.try_clone()?;
        let occurrences = super::count_occurrences(&nodes, &numeric)?;
        Ok(FormulaParts {
            nodes,
            operands,
            numeric,
            occurrences,
        })
    }

    /// Copy every arena, remapping absolute child IDs through `children`.
    ///
    /// The node positions and atom IDs remain unchanged. All Boolean operand and
    /// condition cells, including unused raw cells, require a map entry. Descriptor
    /// and row indices retain their sharing. The result is raw remapping input:
    /// remapped children need not precede these local node positions. In particular,
    /// detached suffixes may map their external original child IDs into a new owner.
    /// Work is linear in stored nodes and arena cells; one complete copy is retained.
    ///
    /// # Errors
    /// Refuses a missing map entry, failed reservation or occurrence overflow.
    pub fn try_remap(self, children: &[usize]) -> Result<FormulaParts, AdmissionError> {
        let mut result = self.try_to_parts()?;
        result.numeric.invalidate_validation();
        let child = |id: usize| children.get(id).copied().ok_or(AdmissionError::Edge);
        for node in &mut result.nodes {
            node.0 = match node.0 {
                Storage::Implies(left, right) => Storage::Implies(child(left)?, child(right)?),
                Storage::AndPair([left, right]) => Storage::AndPair([child(left)?, child(right)?]),
                Storage::OrPair([left, right]) => Storage::OrPair([child(left)?, child(right)?]),
                other => other,
            };
        }
        for id in result
            .operands
            .iter_mut()
            .chain(&mut result.numeric.conditions)
        {
            *id = child(*id)?;
        }
        Ok(result)
    }
}

impl FormulaTransaction<'_> {
    /// Current appended payload across all seven arenas, excluding retained capacity.
    /// Includes nodes, both numeric columns and row and numeric descriptors.
    #[must_use]
    pub fn appended_payload_bytes(&self) -> u128 {
        bytes::<Node>(self.parts.nodes.len() - self.first)
            + bytes::<usize>(self.parts.operands.len() - self.operand_start)
            + bytes::<usize>(self.parts.numeric.conditions.len() - self.numeric_start.conditions)
            + bytes::<u64>(self.parts.numeric.values.len() - self.numeric_start.values)
            + bytes::<NumericRow>(self.parts.numeric.rows.len() - self.numeric_start.rows)
            + bytes::<ThresholdDescriptor>(
                self.parts.numeric.thresholds.len() - self.numeric_start.thresholds,
            )
            + bytes::<ExtremumCutDescriptor>(
                self.parts.numeric.cuts.len() - self.numeric_start.cuts,
            )
    }

    /// Physical appended cells across all seven arenas, for complete detachment work.
    /// A shared row is counted once, unlike logical threshold child occurrences.
    #[must_use]
    pub fn appended_cells(&self) -> u128 {
        (self.parts.nodes.len() - self.first) as u128
            + (self.parts.operands.len() - self.operand_start) as u128
            + (self.parts.numeric.conditions.len() - self.numeric_start.conditions) as u128
            + (self.parts.numeric.values.len() - self.numeric_start.values) as u128
            + (self.parts.numeric.rows.len() - self.numeric_start.rows) as u128
            + (self.parts.numeric.thresholds.len() - self.numeric_start.thresholds) as u128
            + (self.parts.numeric.cuts.len() - self.numeric_start.cuts) as u128
    }
}

mod appending;
mod selecting;
