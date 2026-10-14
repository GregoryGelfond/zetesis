use std::ops::Range;

use super::super::{
    AdmissionError, ExtremumCutDescriptor, FormulaTransaction, FormulaView, Node, NumericRow,
    OperandSpan, Storage, ThresholdDescriptor, count_occurrences, validate_children,
};

fn sum(left: usize, right: usize, maximum: usize) -> Result<usize, AdmissionError> {
    left.checked_add(right)
        .filter(|&total| total <= maximum)
        .ok_or(AdmissionError::Limit)
}

fn reserve<T>(target: &mut Vec<T>, additional: usize) -> Result<(), AdmissionError> {
    target
        .try_reserve(additional)
        .map_err(|_| AdmissionError::Allocation)
}

fn reserve_numeric(
    target: &mut super::super::numeric::NumericStorage,
    source: &super::super::numeric::NumericStorage,
) -> Result<(), AdmissionError> {
    reserve(&mut target.conditions, source.conditions.len())?;
    reserve(&mut target.values, source.values.len())?;
    reserve(&mut target.rows, source.rows.len())?;
    reserve(&mut target.thresholds, source.thresholds.len())?;
    reserve(&mut target.cuts, source.cuts.len())
}

fn relocate(node: Node, operands: usize, thresholds: usize, cuts: usize) -> Node {
    let span = |span: OperandSpan| OperandSpan {
        start: span.start + operands,
        length: span.length,
    };
    Node(match node.0 {
        Storage::AndSpan(row) => Storage::AndSpan(span(row)),
        Storage::OrSpan(row) => Storage::OrSpan(span(row)),
        Storage::Threshold(descriptor) => Storage::Threshold(descriptor + thresholds),
        Storage::ExtremumCut(descriptor) => Storage::ExtremumCut(descriptor + cuts),
        other => other,
    })
}

impl FormulaTransaction<'_> {
    /// Append complete raw storage, preserving shared rows and absolute child IDs.
    ///
    /// Internal Boolean-span, numeric-row and numeric-descriptor offsets are
    /// relocated into this owner. Each child must precede its destination node;
    /// source node positions need not equal those absolute IDs. Atom admission stays
    /// with Theory. All seven source arenas, including unused cells, are copied once.
    /// Validate and reserve every destination before publishing any cell. On error
    /// lengths/frontier are unchanged, but successfully reserved capacity remains.
    /// Work covers source nodes, logical edges, numeric rows and their entries,
    /// descriptors and copied cells; retained payload is all source cells. This
    /// operation does not allocate a second complete source copy or root vector.
    /// Numeric growth also validates an unchecked destination numeric prefix once,
    /// scanning its nodes, rows and entries, and descriptors before appending.
    /// Committed immutable prefixes reuse this cache; rollback restores it.
    ///
    /// # Errors
    /// Refuses invalid source storage or destination topology, numeric shape or
    /// total overflow, inclusive dimensions or failed reservations.
    pub fn append(
        &mut self,
        source: FormulaView<'_>,
        max_nodes: usize,
        max_operands: usize,
    ) -> Result<Range<usize>, AdmissionError> {
        if !source.numeric.is_empty() {
            self.parts.numeric.validate_for_append(&self.parts.nodes)?;
        }
        let first = self.parts.nodes.len();
        let end = sum(first, source.len(), max_nodes)?;
        sum(
            self.parts.operands.len(),
            source.operands.len(),
            max_operands,
        )?;
        let numeric = &self.parts.numeric;
        if numeric.conditions.len() != numeric.values.len() {
            return Err(AdmissionError::NumericShape);
        }
        sum(
            numeric.conditions.len(),
            source.numeric.conditions.len(),
            max_operands,
        )?;
        sum(
            numeric.values.len(),
            source.numeric.values.len(),
            max_operands,
        )?;
        sum(numeric.rows.len(), source.numeric.rows.len(), max_nodes)?;
        sum(
            numeric.thresholds.len(),
            source.numeric.thresholds.len(),
            max_nodes,
        )?;
        sum(numeric.cuts.len(), source.numeric.cuts.len(), max_nodes)?;
        let occurrences = sum(
            self.parts.occurrences,
            count_occurrences(source.nodes, source.numeric)?,
            max_operands,
        )?;
        for index in 0..source.len() {
            validate_children(first + index, source.node(index)?)?;
        }
        source.numeric.validate()?;
        reserve(&mut self.parts.nodes, source.nodes.len())?;
        reserve(&mut self.parts.operands, source.operands.len())?;
        let numeric = &mut self.parts.numeric;
        reserve_numeric(numeric, source.numeric)?;
        let operands = self.parts.operands.len();
        let conditions = numeric.conditions.len();
        let rows = numeric.rows.len();
        let thresholds = numeric.thresholds.len();
        let cuts = numeric.cuts.len();
        self.parts.nodes.extend(
            source
                .nodes
                .iter()
                .copied()
                .map(|node| relocate(node, operands, thresholds, cuts)),
        );
        self.parts.operands.extend_from_slice(source.operands);
        numeric
            .conditions
            .extend_from_slice(&source.numeric.conditions);
        numeric.values.extend_from_slice(&source.numeric.values);
        numeric
            .rows
            .extend(source.numeric.rows.iter().map(|row| NumericRow {
                start: row.start + conditions,
                length: row.length,
                domain: row.domain,
            }));
        numeric
            .thresholds
            .extend(
                source
                    .numeric
                    .thresholds
                    .iter()
                    .map(|threshold| ThresholdDescriptor {
                        row: threshold.row + rows,
                        bound: threshold.bound,
                    }),
            );
        numeric
            .cuts
            .extend(source.numeric.cuts.iter().map(|cut| ExtremumCutDescriptor {
                row: cut.row + rows,
                rule: cut.rule,
            }));
        self.parts.occurrences = occurrences;
        if *self.validated == first {
            *self.validated = end;
        }
        Ok(first..end)
    }
}
