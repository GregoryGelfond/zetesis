use super::{AdmissionError, FormulaParts, FormulaTransaction, Node, count_occurrences};

/// One complete ordered contribution row in parallel condition and weight columns.
/// Empty rows are valid. No source tuple identity is inferred from condition IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContributionRow {
    /// First cell in both columns.
    pub start: usize,
    /// Number of independent contributions, including repeated conditions.
    pub length: usize,
}

/// A raw threshold descriptor, separated from compact formula nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThresholdDescriptor {
    /// Index into the enclosing owner's contribution-row arena.
    pub row: usize,
    /// Inclusive lower bound, without narrowing successor bounds.
    pub bound: i128,
}

/// A borrowed nonnegative weighted threshold over already-coalesced tuple positions.
///
/// Each position contributes its weight exactly when its condition is true.
/// Equal condition IDs remain independent positions. Admission checks equal column
/// lengths, all preceding-child IDs (including zero weights), and the i128 total.
/// No satisfaction or source-key coalescing premise follows from the raw value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Threshold<'a> {
    /// Ordered formula child IDs, one per complete contribution.
    pub conditions: &'a [usize],
    /// Nonnegative weights in the same position order.
    pub weights: &'a [u64],
    /// The weighted sum must be at least this bound.
    pub bound: i128,
}

#[derive(Debug)]
pub(super) struct NumericStorage {
    pub(super) conditions: Vec<usize>,
    pub(super) weights: Vec<u64>,
    pub(super) rows: Vec<ContributionRow>,
    pub(super) thresholds: Vec<ThresholdDescriptor>,
    checked: bool,
}

impl Default for NumericStorage {
    fn default() -> Self {
        Self {
            conditions: Vec::new(),
            weights: Vec::new(),
            rows: Vec::new(),
            thresholds: Vec::new(),
            checked: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Checkpoint {
    pub(super) conditions: usize,
    pub(super) weights: usize,
    pub(super) rows: usize,
    pub(super) thresholds: usize,
    checked: bool,
}

impl NumericStorage {
    pub(super) const fn is_checked(&self) -> bool {
        self.checked
    }

    /// Preserve numeric validation only when the enclosing copy also preserves
    /// every node and its absolute child IDs, as the complete-copy boundary does.
    pub(super) fn try_clone(&self) -> Result<Self, AdmissionError> {
        Ok(Self {
            conditions: super::copying::copy(&self.conditions)?,
            weights: super::copying::copy(&self.weights)?,
            rows: super::copying::copy(&self.rows)?,
            thresholds: super::copying::copy(&self.thresholds)?,
            checked: self.checked,
        })
    }

    /// Record a completed whole-owner admission after node, arena and root checks.
    pub(super) fn mark_checked(&mut self) {
        self.checked = true;
    }

    pub(super) fn invalidate_validation(&mut self) {
        self.checked = false;
    }

    pub(super) fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            conditions: self.conditions.len(),
            weights: self.weights.len(),
            rows: self.rows.len(),
            thresholds: self.thresholds.len(),
            checked: self.checked,
        }
    }

    pub(super) fn restore(&mut self, checkpoint: Checkpoint) {
        self.conditions.truncate(checkpoint.conditions);
        self.weights.truncate(checkpoint.weights);
        self.rows.truncate(checkpoint.rows);
        self.thresholds.truncate(checkpoint.thresholds);
        self.checked = checkpoint.checked;
    }

    pub(super) fn is_empty(&self) -> bool {
        self.conditions.is_empty()
            && self.weights.is_empty()
            && self.rows.is_empty()
            && self.thresholds.is_empty()
    }

    /// Establish that growth cannot repair an unresolved raw prefix reference.
    /// The immutable prefix preserves this cache until its checkpoint is restored.
    pub(super) fn validate_for_append(&mut self, nodes: &[Node]) -> Result<(), AdmissionError> {
        if self.checked {
            return Ok(());
        }
        self.validate()?;
        for node in nodes {
            if let super::Storage::Threshold(descriptor) = node.0 {
                self.view(descriptor)?;
            }
        }
        self.checked = true;
        Ok(())
    }

    pub(super) fn declared_width(&self, descriptor: usize) -> usize {
        self.thresholds
            .get(descriptor)
            .and_then(|threshold| self.rows.get(threshold.row))
            .map_or(0, |row| row.length)
    }

    pub(super) fn view(&self, descriptor: usize) -> Result<Threshold<'_>, AdmissionError> {
        let threshold = self
            .thresholds
            .get(descriptor)
            .ok_or(AdmissionError::Span)?;
        let row = self.rows.get(threshold.row).ok_or(AdmissionError::Span)?;
        self.row(*row, threshold.bound)
    }

    fn row(&self, row: ContributionRow, bound: i128) -> Result<Threshold<'_>, AdmissionError> {
        if self.conditions.len() != self.weights.len() {
            return Err(AdmissionError::ThresholdShape);
        }
        let end = row
            .start
            .checked_add(row.length)
            .ok_or(AdmissionError::Span)?;
        Ok(Threshold {
            conditions: self
                .conditions
                .get(row.start..end)
                .ok_or(AdmissionError::Span)?,
            weights: self
                .weights
                .get(row.start..end)
                .ok_or(AdmissionError::Span)?,
            bound,
        })
    }

    pub(super) fn within_limits(&self, max_nodes: usize, max_operands: usize) -> bool {
        self.conditions.len() <= max_operands
            && self.weights.len() <= max_operands
            && self.rows.len() <= max_nodes
            && self.thresholds.len() <= max_nodes
    }

    pub(super) fn validation_work(&self) -> u128 {
        self.rows
            .iter()
            .fold(self.thresholds.len() as u128, |work, row| {
                work.saturating_add(1 + row.length as u128)
            })
    }

    pub(super) fn validate(&self) -> Result<(), AdmissionError> {
        if self.conditions.len() != self.weights.len() {
            return Err(AdmissionError::ThresholdShape);
        }
        for &row in &self.rows {
            checked_total(self.row(row, 0)?.weights)?;
        }
        for threshold in &self.thresholds {
            self.rows.get(threshold.row).ok_or(AdmissionError::Span)?;
        }
        Ok(())
    }

    fn reserve_family(
        &mut self,
        cells: usize,
        rows: usize,
        thresholds: usize,
        max_nodes: usize,
        max_operands: usize,
    ) -> Result<(), AdmissionError> {
        if self.conditions.len() != self.weights.len() {
            return Err(AdmissionError::ThresholdShape);
        }
        let within = |current: usize, added: usize, limit: usize| {
            current
                .checked_add(added)
                .is_some_and(|total| total <= limit)
        };
        if !within(self.conditions.len(), cells, max_operands)
            || !within(self.weights.len(), cells, max_operands)
            || !within(self.rows.len(), rows, max_nodes)
            || !within(self.thresholds.len(), thresholds, max_nodes)
        {
            return Err(AdmissionError::Limit);
        }
        self.conditions
            .try_reserve(cells)
            .map_err(|_| AdmissionError::Allocation)?;
        self.weights
            .try_reserve(cells)
            .map_err(|_| AdmissionError::Allocation)?;
        self.rows
            .try_reserve(rows)
            .map_err(|_| AdmissionError::Allocation)?;
        self.thresholds
            .try_reserve(thresholds)
            .map_err(|_| AdmissionError::Allocation)?;
        Ok(())
    }

    pub(super) fn copy_suffix(&self, checkpoint: Checkpoint) -> Result<Self, AdmissionError> {
        let mut suffix = Self {
            conditions: super::copying::copy(&self.conditions[checkpoint.conditions..])?,
            weights: super::copying::copy(&self.weights[checkpoint.weights..])?,
            rows: super::copying::copy(&self.rows[checkpoint.rows..])?,
            thresholds: super::copying::copy(&self.thresholds[checkpoint.thresholds..])?,
            checked: true,
        };
        for row in &mut suffix.rows {
            row.start = row
                .start
                .checked_sub(checkpoint.conditions)
                .ok_or(AdmissionError::Span)?;
        }
        for threshold in &mut suffix.thresholds {
            threshold.row = threshold
                .row
                .checked_sub(checkpoint.rows)
                .ok_or(AdmissionError::Span)?;
        }
        Ok(suffix)
    }
}

impl FormulaParts {
    /// Work required to validate an unchecked numeric prefix before its next
    /// numeric append: all nodes, contribution rows and entries, and descriptors.
    /// Checked immutable prefixes return zero. This only reports pending work;
    /// it neither validates storage nor changes its cache. Callers charge it
    /// before [`FormulaTransaction::push_threshold_family_range`] or a numeric
    /// [`FormulaTransaction::append`], whose successful validation caches it.
    #[must_use]
    pub fn pending_numeric_validation_work(&self) -> u128 {
        if self.numeric.is_checked() {
            0
        } else {
            self.nodes.len() as u128 + self.numeric.validation_work()
        }
    }
}

impl FormulaParts {
    /// Transfer every raw formula arena together without copying or admission.
    ///
    /// Declared referenced widths are counted in O(nodes). Missing descriptor or
    /// row references contribute zero here and are rejected at admission; valid
    /// references contribute their declared width even when a span is malformed.
    /// Unused raw rows, descriptors and column cells remain owned and bounded.
    ///
    /// # Errors
    /// Refuses overflow in declared logical occurrences. Allocates no storage.
    pub fn from_raw(
        nodes: Vec<Node>,
        operands: Vec<usize>,
        conditions: Vec<usize>,
        weights: Vec<u64>,
        rows: Vec<ContributionRow>,
        thresholds: Vec<ThresholdDescriptor>,
    ) -> Result<Self, AdmissionError> {
        let numeric = NumericStorage {
            conditions,
            weights,
            rows,
            thresholds,
            checked: false,
        };
        let occurrences = count_occurrences(&nodes, &numeric)?;
        Ok(Self {
            nodes,
            operands,
            numeric,
            occurrences,
        })
    }

    /// Raw condition column, including unused cells. Pairs remain in their nodes.
    #[must_use]
    pub fn conditions(&self) -> &[usize] {
        &self.numeric.conditions
    }

    /// Raw nonnegative weight column, parallel to [`Self::conditions`].
    #[must_use]
    pub fn weights(&self) -> &[u64] {
        &self.numeric.weights
    }

    /// Raw contribution spans over both numeric columns.
    #[must_use]
    pub fn rows(&self) -> &[ContributionRow] {
        &self.numeric.rows
    }

    /// Raw row-and-bound descriptors addressed by threshold nodes.
    #[must_use]
    pub fn thresholds(&self) -> &[ThresholdDescriptor] {
        &self.numeric.thresholds
    }
}

/// Checks topology and arithmetic together, visiting every supplied occurrence.
/// A constant bound does not bypass validation of any position.
pub(super) fn validate_threshold(
    index: usize,
    threshold: Threshold<'_>,
) -> Result<(), AdmissionError> {
    if threshold.conditions.len() != threshold.weights.len() {
        return Err(AdmissionError::ThresholdShape);
    }
    let mut total = 0i128;
    for (&condition, &weight) in threshold.conditions.iter().zip(threshold.weights) {
        if condition >= index {
            return Err(AdmissionError::Edge);
        }
        total = total
            .checked_add(i128::from(weight))
            .ok_or(AdmissionError::ThresholdMeasure)?;
    }
    Ok(())
}

fn checked_total(weights: &[u64]) -> Result<i128, AdmissionError> {
    weights.iter().try_fold(0i128, |total, &weight| {
        total
            .checked_add(i128::from(weight))
            .ok_or(AdmissionError::ThresholdMeasure)
    })
}

impl FormulaTransaction<'_> {
    /// Atomically append one family of bounds sharing one fresh contribution row.
    ///
    /// Validates every condition and weight once, before interpreting constant
    /// bounds or an empty bound list. No tuple deduplication occurs. Sharing stays
    /// inside this call, so nested detach owns all its rows and preserves child IDs.
    /// Work and additional payload are O(conditions + bounds); the occurrence
    /// counter remains conditions * bounds, covering consumers that scan each node.
    /// An unchecked raw numeric prefix first incurs one additional scan of its N
    /// nodes, contribution rows and declared entries, and descriptors. This proves
    /// suffix growth cannot repair an unresolved prefix reference and invalidate
    /// occurrence accounting. The cache is retained on commit and restored on
    /// rollback; freshly built prefixes require no repeated scan.
    /// Each column and logical occurrences obey `max_operands` independently; rows,
    /// descriptors and nodes each obey `max_nodes`. Reserved capacity may grow on a
    /// failed allocation, but no length or validation frontier changes on refusal.
    ///
    /// # Errors
    /// Refuses unequal columns, invalid children, arithmetic overflow, inclusive
    /// dimension limits or failed reservations. Empty bounds still validate input.
    pub fn push_threshold_family(
        &mut self,
        conditions: &[usize],
        weights: &[u64],
        bounds: &[i128],
        max_nodes: usize,
        max_operands: usize,
    ) -> Result<Vec<usize>, AdmissionError> {
        let mut roots = Vec::new();
        let range = self.append_threshold_family(
            conditions,
            weights,
            bounds,
            max_nodes,
            max_operands,
            || {
                roots
                    .try_reserve_exact(bounds.len())
                    .map_err(|_| AdmissionError::Allocation)
            },
        )?;
        roots.extend(range);
        Ok(roots)
    }

    /// Append a shared threshold family and return its contiguous node range.
    ///
    /// This has the validation, bounds, rollback and O(conditions + bounds) work
    /// contract of [`Self::push_threshold_family`], without allocating a separate
    /// vector of root IDs. The range is empty when `bounds` is empty.
    ///
    /// # Errors
    /// Preserves the family operation's shape, topology, arithmetic, dimension
    /// and reservation refusals; no partial family is published on failure.
    pub fn push_threshold_family_range(
        &mut self,
        conditions: &[usize],
        weights: &[u64],
        bounds: &[i128],
        max_nodes: usize,
        max_operands: usize,
    ) -> Result<std::ops::Range<usize>, AdmissionError> {
        self.append_threshold_family(conditions, weights, bounds, max_nodes, max_operands, || {
            Ok(())
        })
    }

    fn append_threshold_family(
        &mut self,
        conditions: &[usize],
        weights: &[u64],
        bounds: &[i128],
        max_nodes: usize,
        max_operands: usize,
        reserve_roots: impl FnOnce() -> Result<(), AdmissionError>,
    ) -> Result<std::ops::Range<usize>, AdmissionError> {
        let first = self.parts.nodes.len();
        validate_threshold(
            first,
            Threshold {
                conditions,
                weights,
                bound: 0,
            },
        )?;
        if !bounds.is_empty() {
            self.parts.numeric.validate_for_append(&self.parts.nodes)?;
        }
        let count = first
            .checked_add(bounds.len())
            .ok_or(AdmissionError::Limit)?;
        let added_edges = conditions
            .len()
            .checked_mul(bounds.len())
            .ok_or(AdmissionError::Limit)?;
        let occurrences = self
            .parts
            .occurrences
            .checked_add(added_edges)
            .ok_or(AdmissionError::Limit)?;
        let added_rows = usize::from(!bounds.is_empty());
        let added_cells = if bounds.is_empty() {
            0
        } else {
            conditions.len()
        };
        if count > max_nodes
            || self.parts.operands.len() > max_operands
            || occurrences > max_operands
        {
            return Err(AdmissionError::Limit);
        }
        // Reserve every destination before publishing a single cell.
        reserve_roots()?;
        self.parts
            .nodes
            .try_reserve(bounds.len())
            .map_err(|_| AdmissionError::Allocation)?;
        let numeric = &mut self.parts.numeric;
        numeric.reserve_family(
            added_cells,
            added_rows,
            bounds.len(),
            max_nodes,
            max_operands,
        )?;
        if bounds.is_empty() {
            return Ok(first..first);
        }
        let row = numeric.rows.len();
        numeric.rows.push(ContributionRow {
            start: numeric.conditions.len(),
            length: conditions.len(),
        });
        numeric.conditions.extend_from_slice(conditions);
        numeric.weights.extend_from_slice(weights);
        for &bound in bounds {
            let descriptor = numeric.thresholds.len();
            numeric.thresholds.push(ThresholdDescriptor { row, bound });
            self.parts.nodes.push(Node::threshold(descriptor));
        }
        self.parts.occurrences = occurrences;
        if *self.validated == first {
            *self.validated = count;
        }
        Ok(first..count)
    }
}
