use std::ops::Range;

use super::super::{
    AdmissionError, ContributionRow, FormulaTransaction, FormulaView, Node, OperandSpan, Storage,
    ThresholdDescriptor, count_occurrences, validate_children,
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

fn relocate(node: Node, operands: usize, thresholds: usize) -> Node {
    let span = |span: OperandSpan| OperandSpan {
        start: span.start + operands,
        length: span.length,
    };
    Node(match node.0 {
        Storage::AndSpan(row) => Storage::AndSpan(span(row)),
        Storage::OrSpan(row) => Storage::OrSpan(span(row)),
        Storage::Threshold(descriptor) => Storage::Threshold(descriptor + thresholds),
        other => other,
    })
}

impl FormulaTransaction<'_> {
    /// Append complete raw storage, preserving shared rows and absolute child IDs.
    ///
    /// Internal Boolean-span, numeric-row and threshold-descriptor offsets are
    /// relocated into this owner. Each child must precede its destination node;
    /// source node positions need not equal those absolute IDs. Atom admission stays
    /// with Theory. All six source arenas, including unused cells, are copied once.
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
        if numeric.conditions.len() != numeric.weights.len() {
            return Err(AdmissionError::ThresholdShape);
        }
        sum(
            numeric.conditions.len(),
            source.numeric.conditions.len(),
            max_operands,
        )?;
        sum(
            numeric.weights.len(),
            source.numeric.weights.len(),
            max_operands,
        )?;
        sum(numeric.rows.len(), source.numeric.rows.len(), max_nodes)?;
        sum(
            numeric.thresholds.len(),
            source.numeric.thresholds.len(),
            max_nodes,
        )?;
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
        reserve(&mut numeric.conditions, source.numeric.conditions.len())?;
        reserve(&mut numeric.weights, source.numeric.weights.len())?;
        reserve(&mut numeric.rows, source.numeric.rows.len())?;
        reserve(&mut numeric.thresholds, source.numeric.thresholds.len())?;
        let operands = self.parts.operands.len();
        let conditions = numeric.conditions.len();
        let rows = numeric.rows.len();
        let thresholds = numeric.thresholds.len();
        self.parts.nodes.extend(
            source
                .nodes
                .iter()
                .copied()
                .map(|node| relocate(node, operands, thresholds)),
        );
        self.parts.operands.extend_from_slice(source.operands);
        numeric
            .conditions
            .extend_from_slice(&source.numeric.conditions);
        numeric.weights.extend_from_slice(&source.numeric.weights);
        numeric
            .rows
            .extend(source.numeric.rows.iter().map(|row| ContributionRow {
                start: row.start + conditions,
                length: row.length,
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
        self.parts.occurrences = occurrences;
        if *self.validated == first {
            *self.validated = end;
        }
        Ok(first..end)
    }
}
