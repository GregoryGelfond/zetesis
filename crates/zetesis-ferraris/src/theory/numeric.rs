use crate::{ExtremumCut, ExtremumCutRule, ExtremumRow};

use super::{AdmissionError, FormulaParts, FormulaTransaction, Node, count_occurrences};

/// A legacy weighted raw-ingress span for [`FormulaParts::from_raw`].
/// That constructor converts it to one authoritative weight-domain [`NumericRow`].
/// Empty rows are valid. No source tuple identity is inferred from condition IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContributionRow {
    /// First cell in both columns.
    pub start: usize,
    /// Number of independent contributions, including repeated conditions.
    pub length: usize,
}

/// The interpretation of a numeric row's value column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NumericDomain {
    /// Independent nonnegative contributions to a checked i128 sum.
    Weights,
    /// Guard-independent ordered ranks, including both endpoint values.
    /// Rank zero is the least endpoint and `rank_count - 1` the greatest.
    Ordinal {
        /// Carrier size, at least two; each stored rank is strictly smaller.
        rank_count: u64,
    },
}

/// One authoritative row over the shared condition and numeric-value columns.
/// The domain determines whether values are weights or ordinal ranks. No value
/// column or contribution span is duplicated in retained storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NumericRow {
    /// First cell in both columns.
    pub start: usize,
    /// Number of ordered occurrences, including repeated condition IDs.
    pub length: usize,
    /// Required interpretation of every value in this row.
    pub domain: NumericDomain,
}

/// A cut of one ordinal row, separated from the compact formula node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExtremumCutDescriptor {
    /// Index into the enclosing owner's numeric row arena.
    pub row: usize,
    /// Prefix or suffix boundary in that row's carrier, including endpoints.
    pub rule: ExtremumCutRule,
}

/// A raw threshold descriptor, separated from compact formula nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThresholdDescriptor {
    /// Index into the enclosing owner's numeric-row arena; the domain must be weights.
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
    pub(super) values: Vec<u64>,
    pub(super) rows: Vec<NumericRow>,
    pub(super) thresholds: Vec<ThresholdDescriptor>,
    pub(super) cuts: Vec<ExtremumCutDescriptor>,
    checked: bool,
}

impl Default for NumericStorage {
    fn default() -> Self {
        Self {
            conditions: Vec::new(),
            values: Vec::new(),
            rows: Vec::new(),
            thresholds: Vec::new(),
            cuts: Vec::new(),
            checked: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Checkpoint {
    pub(super) conditions: usize,
    pub(super) values: usize,
    pub(super) rows: usize,
    pub(super) thresholds: usize,
    pub(super) cuts: usize,
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
            values: super::copying::copy(&self.values)?,
            rows: super::copying::copy(&self.rows)?,
            thresholds: super::copying::copy(&self.thresholds)?,
            cuts: super::copying::copy(&self.cuts)?,
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
            values: self.values.len(),
            rows: self.rows.len(),
            thresholds: self.thresholds.len(),
            cuts: self.cuts.len(),
            checked: self.checked,
        }
    }

    pub(super) fn restore(&mut self, checkpoint: Checkpoint) {
        self.conditions.truncate(checkpoint.conditions);
        self.values.truncate(checkpoint.values);
        self.rows.truncate(checkpoint.rows);
        self.thresholds.truncate(checkpoint.thresholds);
        self.cuts.truncate(checkpoint.cuts);
        self.checked = checkpoint.checked;
    }

    pub(super) fn is_empty(&self) -> bool {
        self.conditions.is_empty()
            && self.values.is_empty()
            && self.rows.is_empty()
            && self.thresholds.is_empty()
            && self.cuts.is_empty()
    }

    /// Establish that growth cannot repair an unresolved raw prefix reference.
    /// The immutable prefix preserves this cache until its checkpoint is restored.
    pub(super) fn validate_for_append(&mut self, nodes: &[Node]) -> Result<(), AdmissionError> {
        if self.checked {
            return Ok(());
        }
        self.validate()?;
        for node in nodes {
            match node.0 {
                super::Storage::Threshold(descriptor) => {
                    self.view(descriptor)?;
                }
                super::Storage::ExtremumCut(descriptor) => {
                    self.cut_view(descriptor)?;
                }
                _ => {}
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

    fn row(&self, row: NumericRow, bound: i128) -> Result<Threshold<'_>, AdmissionError> {
        if row.domain != NumericDomain::Weights {
            return Err(AdmissionError::NumericDomain);
        }
        let (conditions, weights) = self.columns(row)?;
        Ok(Threshold {
            conditions,
            weights,
            bound,
        })
    }

    pub(super) fn columns(&self, row: NumericRow) -> Result<(&[usize], &[u64]), AdmissionError> {
        if self.conditions.len() != self.values.len() {
            return Err(AdmissionError::NumericShape);
        }
        let end = row
            .start
            .checked_add(row.length)
            .ok_or(AdmissionError::Span)?;
        Ok((
            self.conditions
                .get(row.start..end)
                .ok_or(AdmissionError::Span)?,
            self.values
                .get(row.start..end)
                .ok_or(AdmissionError::Span)?,
        ))
    }

    pub(super) fn extremum_row(&self, index: usize) -> Result<ExtremumRow<'_>, AdmissionError> {
        let row = *self.rows.get(index).ok_or(AdmissionError::Span)?;
        let NumericDomain::Ordinal { rank_count } = row.domain else {
            return Err(AdmissionError::NumericDomain);
        };
        if rank_count < 2 {
            return Err(AdmissionError::OrdinalRank);
        }
        let (conditions, ranks) = self.columns(row)?;
        Ok(ExtremumRow {
            conditions,
            ranks,
            rank_count,
        })
    }

    pub(super) fn cut_view(&self, descriptor: usize) -> Result<ExtremumCut<'_>, AdmissionError> {
        let cut = self.cuts.get(descriptor).ok_or(AdmissionError::Span)?;
        let row = self.extremum_row(cut.row)?;
        validate_cut_rule(cut.rule, row.rank_count)?;
        Ok(ExtremumCut {
            row,
            rule: &cut.rule,
        })
    }

    pub(super) fn declared_cut_width(&self, descriptor: usize) -> usize {
        self.cuts
            .get(descriptor)
            .and_then(|cut| self.rows.get(cut.row))
            .map_or(0, |row| row.length)
    }

    pub(super) fn within_limits(&self, max_nodes: usize, max_operands: usize) -> bool {
        self.conditions.len() <= max_operands
            && self.values.len() <= max_operands
            && self.rows.len() <= max_nodes
            && self.thresholds.len() <= max_nodes
            && self.cuts.len() <= max_nodes
    }

    pub(super) fn validation_work(&self) -> u128 {
        self.rows.iter().fold(
            self.thresholds.len() as u128 + self.cuts.len() as u128,
            |work, row| work.saturating_add(1 + row.length as u128),
        )
    }

    pub(super) fn validate(&self) -> Result<(), AdmissionError> {
        if self.conditions.len() != self.values.len() {
            return Err(AdmissionError::NumericShape);
        }
        for &row in &self.rows {
            let (_, values) = self.columns(row)?;
            match row.domain {
                NumericDomain::Weights => {
                    checked_total(values)?;
                }
                NumericDomain::Ordinal { rank_count } => validate_ranks(values, rank_count)?,
            }
        }
        for threshold in &self.thresholds {
            let row = self.rows.get(threshold.row).ok_or(AdmissionError::Span)?;
            if row.domain != NumericDomain::Weights {
                return Err(AdmissionError::NumericDomain);
            }
        }
        for descriptor in 0..self.cuts.len() {
            self.cut_view(descriptor)?;
        }
        Ok(())
    }

    fn reserve_family(
        &mut self,
        cells: usize,
        rows: usize,
        thresholds: usize,
        cuts: usize,
        max_nodes: usize,
        max_operands: usize,
    ) -> Result<(), AdmissionError> {
        if self.conditions.len() != self.values.len() {
            return Err(AdmissionError::NumericShape);
        }
        let within = |current: usize, added: usize, limit: usize| {
            current
                .checked_add(added)
                .is_some_and(|total| total <= limit)
        };
        if !within(self.conditions.len(), cells, max_operands)
            || !within(self.values.len(), cells, max_operands)
            || !within(self.rows.len(), rows, max_nodes)
            || !within(self.thresholds.len(), thresholds, max_nodes)
            || !within(self.cuts.len(), cuts, max_nodes)
        {
            return Err(AdmissionError::Limit);
        }
        self.conditions
            .try_reserve(cells)
            .map_err(|_| AdmissionError::Allocation)?;
        self.values
            .try_reserve(cells)
            .map_err(|_| AdmissionError::Allocation)?;
        self.rows
            .try_reserve(rows)
            .map_err(|_| AdmissionError::Allocation)?;
        self.thresholds
            .try_reserve(thresholds)
            .map_err(|_| AdmissionError::Allocation)?;
        self.cuts
            .try_reserve(cuts)
            .map_err(|_| AdmissionError::Allocation)?;
        Ok(())
    }

    pub(super) fn copy_suffix(&self, checkpoint: Checkpoint) -> Result<Self, AdmissionError> {
        let mut suffix = Self {
            conditions: super::copying::copy(&self.conditions[checkpoint.conditions..])?,
            values: super::copying::copy(&self.values[checkpoint.values..])?,
            rows: super::copying::copy(&self.rows[checkpoint.rows..])?,
            thresholds: super::copying::copy(&self.thresholds[checkpoint.thresholds..])?,
            cuts: super::copying::copy(&self.cuts[checkpoint.cuts..])?,
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
        for cut in &mut suffix.cuts {
            cut.row = cut
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
    /// before either numeric family append or a numeric
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
    /// Transfer legacy weighted raw storage, converting only its row descriptors.
    ///
    /// The supplied contribution spans become weight-domain numeric rows. All
    /// other vectors transfer unchanged. Conversion fallibly reserves one row
    /// vector in O(rows) work; the supplied spans are released before return.
    /// No admission occurs. Use [`Self::from_numeric_raw`] for complete storage
    /// transfer without conversion or additional allocation.
    ///
    /// # Errors
    /// Refuses row reservation or overflow in declared logical occurrences.
    pub fn from_raw(
        nodes: Vec<Node>,
        operands: Vec<usize>,
        conditions: Vec<usize>,
        weights: Vec<u64>,
        rows: Vec<ContributionRow>,
        thresholds: Vec<ThresholdDescriptor>,
    ) -> Result<Self, AdmissionError> {
        let mut numeric_rows = Vec::new();
        numeric_rows
            .try_reserve_exact(rows.len())
            .map_err(|_| AdmissionError::Allocation)?;
        numeric_rows.extend(rows.into_iter().map(|row| NumericRow {
            start: row.start,
            length: row.length,
            domain: NumericDomain::Weights,
        }));
        Self::from_numeric_raw(
            nodes,
            operands,
            conditions,
            weights,
            numeric_rows,
            thresholds,
            Vec::new(),
        )
    }

    /// Transfer all seven raw arenas unchanged, without copying or admission.
    ///
    /// Declared referenced widths are counted in O(nodes). Missing descriptors
    /// or rows contribute zero here and are rejected at admission. All unused
    /// cells and descriptors remain owned, validated and dimension bounded.
    ///
    /// # Errors
    /// Refuses overflow in declared logical occurrences. Allocates no storage.
    pub fn from_numeric_raw(
        nodes: Vec<Node>,
        operands: Vec<usize>,
        conditions: Vec<usize>,
        values: Vec<u64>,
        rows: Vec<NumericRow>,
        thresholds: Vec<ThresholdDescriptor>,
        cuts: Vec<ExtremumCutDescriptor>,
    ) -> Result<Self, AdmissionError> {
        let numeric = NumericStorage {
            conditions,
            values,
            rows,
            thresholds,
            cuts,
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

    /// Raw numeric values, parallel to [`Self::conditions`]. Each row domain
    /// determines whether its cells are weights or ordinal ranks; unused cells
    /// have no interpretation until a validated row references them.
    #[must_use]
    pub fn numeric_values(&self) -> &[u64] {
        &self.numeric.values
    }

    /// Raw typed spans over both numeric columns, including unused rows.
    #[must_use]
    pub fn rows(&self) -> &[NumericRow] {
        &self.numeric.rows
    }

    /// Raw row-and-bound descriptors addressed by threshold nodes.
    #[must_use]
    pub fn thresholds(&self) -> &[ThresholdDescriptor] {
        &self.numeric.thresholds
    }

    /// Raw row-and-rule descriptors addressed by ordinal-cut nodes.
    #[must_use]
    pub fn extremum_cuts(&self) -> &[ExtremumCutDescriptor] {
        &self.numeric.cuts
    }

    /// Borrow one ordinal row from its authoritative columns in constant time.
    ///
    /// # Errors
    /// Refuses missing rows, wrong domains, invalid carrier size or column spans.
    /// Full rank range and child topology validation remain admission's task.
    pub fn extremum_row(&self, row: usize) -> Result<ExtremumRow<'_>, AdmissionError> {
        self.numeric.extremum_row(row)
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
    /// failed allocation, but no length or topology frontier changes on refusal.
    /// Successful prefix shape validation may remain until transaction rollback.
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
            0,
            max_nodes,
            max_operands,
        )?;
        if bounds.is_empty() {
            return Ok(first..first);
        }
        let row = numeric.rows.len();
        numeric.rows.push(NumericRow {
            start: numeric.conditions.len(),
            length: conditions.len(),
            domain: NumericDomain::Weights,
        });
        numeric.conditions.extend_from_slice(conditions);
        numeric.values.extend_from_slice(weights);
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

fn validate_ranks(ranks: &[u64], rank_count: u64) -> Result<(), AdmissionError> {
    if rank_count < 2 || ranks.iter().any(|&rank| rank >= rank_count) {
        Err(AdmissionError::OrdinalRank)
    } else {
        Ok(())
    }
}

fn validate_cut_rule(rule: ExtremumCutRule, rank_count: u64) -> Result<(), AdmissionError> {
    let (ExtremumCutRule::Prefix(boundary) | ExtremumCutRule::Suffix(boundary)) = rule;
    if boundary > rank_count {
        Err(AdmissionError::OrdinalCut)
    } else {
        Ok(())
    }
}

pub(super) fn validate_extremum_cut(
    index: usize,
    cut: ExtremumCut<'_>,
) -> Result<(), AdmissionError> {
    validate_extremum_row(index, cut.row)?;
    validate_cut_rule(*cut.rule, cut.row.rank_count)
}

fn validate_extremum_row(index: usize, row: ExtremumRow<'_>) -> Result<(), AdmissionError> {
    if row.conditions.len() != row.ranks.len() {
        return Err(AdmissionError::NumericShape);
    }
    if row.rank_count < 2 {
        return Err(AdmissionError::OrdinalRank);
    }
    for (&condition, &rank) in row.conditions.iter().zip(row.ranks) {
        if condition >= index {
            return Err(AdmissionError::Edge);
        }
        if rank >= row.rank_count {
            return Err(AdmissionError::OrdinalRank);
        }
    }
    Ok(())
}

impl FormulaTransaction<'_> {
    /// Append ordinal cuts sharing one fresh, guard-independent ranked row.
    ///
    /// The carrier includes rank zero as the least endpoint and `rank_count - 1`
    /// as the greatest. Empty minimum and maximum use those greatest and least
    /// ranks respectively. Every supplied rank and preceding child is checked,
    /// including for constant cuts or an empty cut list. Every requested cut gets
    /// one node; neither conditions nor rules are coalesced. The returned range is
    /// contiguous. No descriptor references a row in an earlier transaction.
    ///
    /// Work and new payload are O(conditions + cuts), plus the one-time pending
    /// raw-prefix validation reported by [`FormulaParts::pending_numeric_validation_work`].
    /// Logical occurrences are conditions * cuts. Columns and occurrences each
    /// obey `max_operands`; nodes, rows and each descriptor arena obey `max_nodes`.
    /// All reservations precede publication. On refusal lengths and the topology
    /// frontier are unchanged; reserved capacities and successful prefix shape
    /// validation may remain until this transaction's rollback.
    ///
    /// # Errors
    /// Refuses inconsistent columns, invalid ranks/carrier/cuts/children, checked
    /// count overflow, inclusive dimension limits or failed reservations.
    pub fn push_extremum_cut_family_range(
        &mut self,
        row: ExtremumRow<'_>,
        cuts: &[ExtremumCutRule],
        max_nodes: usize,
        max_operands: usize,
    ) -> Result<std::ops::Range<usize>, AdmissionError> {
        let first = self.parts.nodes.len();
        validate_extremum_row(first, row)?;
        for &rule in cuts {
            validate_cut_rule(rule, row.rank_count)?;
        }
        if !cuts.is_empty() {
            self.parts.numeric.validate_for_append(&self.parts.nodes)?;
        }
        let end = first.checked_add(cuts.len()).ok_or(AdmissionError::Limit)?;
        let added_edges = row
            .conditions
            .len()
            .checked_mul(cuts.len())
            .ok_or(AdmissionError::Limit)?;
        let occurrences = self
            .parts
            .occurrences
            .checked_add(added_edges)
            .ok_or(AdmissionError::Limit)?;
        if end > max_nodes || self.parts.operands.len() > max_operands || occurrences > max_operands
        {
            return Err(AdmissionError::Limit);
        }
        self.parts
            .nodes
            .try_reserve(cuts.len())
            .map_err(|_| AdmissionError::Allocation)?;
        let numeric = &mut self.parts.numeric;
        numeric.reserve_family(
            if cuts.is_empty() {
                0
            } else {
                row.conditions.len()
            },
            usize::from(!cuts.is_empty()),
            0,
            cuts.len(),
            max_nodes,
            max_operands,
        )?;
        if cuts.is_empty() {
            return Ok(first..first);
        }
        let row_index = numeric.rows.len();
        numeric.rows.push(NumericRow {
            start: numeric.conditions.len(),
            length: row.conditions.len(),
            domain: NumericDomain::Ordinal {
                rank_count: row.rank_count,
            },
        });
        numeric.conditions.extend_from_slice(row.conditions);
        numeric.values.extend_from_slice(row.ranks);
        for &rule in cuts {
            let descriptor = numeric.cuts.len();
            numeric.cuts.push(ExtremumCutDescriptor {
                row: row_index,
                rule,
            });
            self.parts.nodes.push(Node::extremum_cut(descriptor));
        }
        self.parts.occurrences = occurrences;
        if *self.validated == first {
            *self.validated = end;
        }
        Ok(first..end)
    }
}

/// Dispatch numeric views through shared-family admission, leaving Boolean views
/// to the ordinary connective builder. Numeric constants retain one fresh node.
pub(super) fn append_view(
    transaction: &mut FormulaTransaction<'_>,
    node: super::NodeView<'_>,
    max_nodes: usize,
    max_operands: usize,
) -> Option<Result<usize, AdmissionError>> {
    let range = match node {
        super::NodeView::Threshold(threshold) => transaction.push_threshold_family_range(
            threshold.conditions,
            threshold.weights,
            &[threshold.bound],
            max_nodes,
            max_operands,
        ),
        super::NodeView::ExtremumCut(cut) => transaction.push_extremum_cut_family_range(
            cut.row,
            &[*cut.rule],
            max_nodes,
            max_operands,
        ),
        _ => return None,
    };
    Some(range.map(|roots| roots.start))
}
