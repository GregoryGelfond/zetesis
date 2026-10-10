use super::super::{
    AdmissionError, ContributionRow, FormulaParts, FormulaView, Node, NodeView, OperandSpan,
    Storage, ThresholdDescriptor,
};

fn absent_map(length: usize) -> Result<Vec<Option<usize>>, AdmissionError> {
    let mut map = Vec::new();
    map.try_reserve_exact(length)
        .map_err(|_| AdmissionError::Allocation)?;
    map.resize(length, None);
    Ok(map)
}

#[derive(Default)]
struct Sizes {
    nodes: usize,
    operands: usize,
    conditions: usize,
    rows: usize,
    thresholds: usize,
}

fn add(left: usize, right: usize) -> Result<usize, AdmissionError> {
    left.checked_add(right).ok_or(AdmissionError::Limit)
}

fn exact<T>(target: &mut Vec<T>, count: usize) -> Result<(), AdmissionError> {
    target
        .try_reserve_exact(count)
        .map_err(|_| AdmissionError::Allocation)
}

struct Selection<'a> {
    source: FormulaView<'a>,
    children: &'a [Option<usize>],
    result: FormulaParts,
    rows: Vec<Option<usize>>,
    thresholds: Vec<Option<usize>>,
}

impl<'a> Selection<'a> {
    fn new(source: FormulaView<'a>, children: &'a [Option<usize>]) -> Result<Self, AdmissionError> {
        if source.len() != children.len() {
            return Err(AdmissionError::Edge);
        }
        let mut selection = Self {
            source,
            children,
            result: FormulaParts::default(),
            rows: absent_map(source.numeric.rows.len())?,
            thresholds: absent_map(source.numeric.thresholds.len())?,
        };
        selection.reserve_output()?;
        Ok(selection)
    }

    fn reserve_output(&mut self) -> Result<(), AdmissionError> {
        let mut sizes = Sizes::default();
        for (index, selected) in self.children.iter().enumerate() {
            if selected.is_none() {
                continue;
            }
            sizes.nodes = add(sizes.nodes, 1)?;
            match self.source.node(index)? {
                NodeView::And(row) | NodeView::Or(row) if row.len() >= 3 => {
                    sizes.operands = add(sizes.operands, row.len())?;
                }
                _ => {}
            }
            if let Storage::Threshold(descriptor) = self.source.nodes[index].0 {
                if self.thresholds[descriptor].is_none() {
                    self.thresholds[descriptor] = Some(0);
                    sizes.thresholds = add(sizes.thresholds, 1)?;
                }
                let threshold = self.source.numeric.thresholds[descriptor];
                if self.rows[threshold.row].is_none() {
                    self.rows[threshold.row] = Some(0);
                    sizes.rows = add(sizes.rows, 1)?;
                    sizes.conditions = add(
                        sizes.conditions,
                        self.source.numeric.rows[threshold.row].length,
                    )?;
                }
            }
        }
        exact(&mut self.result.nodes, sizes.nodes)?;
        exact(&mut self.result.operands, sizes.operands)?;
        exact(&mut self.result.numeric.conditions, sizes.conditions)?;
        exact(&mut self.result.numeric.weights, sizes.conditions)?;
        exact(&mut self.result.numeric.rows, sizes.rows)?;
        exact(&mut self.result.numeric.thresholds, sizes.thresholds)?;
        self.rows.fill(None);
        self.thresholds.fill(None);
        Ok(())
    }

    fn child(&self, id: usize) -> Result<usize, AdmissionError> {
        self.children
            .get(id)
            .copied()
            .flatten()
            .ok_or(AdmissionError::Edge)
    }

    fn group(&mut self, row: &[usize], conjunction: bool) -> Result<Node, AdmissionError> {
        if let [left, right] = row {
            let pair = [self.child(*left)?, self.child(*right)?];
            return Ok(if conjunction {
                Node::and_pair(pair)
            } else {
                Node::or_pair(pair)
            });
        }
        let span = OperandSpan {
            start: self.result.operands.len(),
            length: row.len(),
        };
        for &id in row {
            self.result.operands.push(self.child(id)?);
        }
        Ok(if conjunction {
            Node::and_span(span)
        } else {
            Node::or_span(span)
        })
    }

    fn threshold(&mut self, descriptor: usize) -> Result<Node, AdmissionError> {
        let threshold = *self
            .source
            .numeric
            .thresholds
            .get(descriptor)
            .ok_or(AdmissionError::Span)?;
        if let Some(mapped) = self.thresholds[descriptor] {
            return Ok(Node::threshold(mapped));
        }
        let original = self.source.numeric.view(descriptor)?;
        let row = if let Some(mapped) = self.rows[threshold.row] {
            mapped
        } else {
            let mapped = self.result.numeric.rows.len();
            self.result.numeric.rows.push(ContributionRow {
                start: self.result.numeric.conditions.len(),
                length: original.conditions.len(),
            });
            for &id in original.conditions {
                self.result.numeric.conditions.push(self.child(id)?);
            }
            self.result
                .numeric
                .weights
                .extend_from_slice(original.weights);
            self.rows[threshold.row] = Some(mapped);
            mapped
        };
        let mapped = self.result.numeric.thresholds.len();
        self.result.numeric.thresholds.push(ThresholdDescriptor {
            row,
            bound: threshold.bound,
        });
        self.thresholds[descriptor] = Some(mapped);
        Ok(Node::threshold(mapped))
    }

    fn append(&mut self, index: usize) -> Result<(), AdmissionError> {
        let node = match self.source.nodes[index].0 {
            Storage::Atom(atom) => Node::atom(atom),
            Storage::False => Node::falsum(),
            Storage::Implies(left, right) => Node::implies(self.child(left)?, self.child(right)?),
            Storage::AndPair([left, right]) => {
                Node::and_pair([self.child(left)?, self.child(right)?])
            }
            Storage::OrPair([left, right]) => {
                Node::or_pair([self.child(left)?, self.child(right)?])
            }
            Storage::AndSpan(span) => {
                self.group(super::super::read_span(self.source.operands, span)?, true)?
            }
            Storage::OrSpan(span) => {
                self.group(super::super::read_span(self.source.operands, span)?, false)?
            }
            Storage::Threshold(descriptor) => self.threshold(descriptor)?,
        };
        self.result.nodes.push(node);
        Ok(())
    }
}

impl FormulaView<'_> {
    /// Copy selected nodes in original order, preserving shared numeric rows.
    ///
    /// `children` has one entry per source node. None omits that node; Some selects
    /// it and supplies its absolute ID for remapping every selected child reference.
    /// Output positions remain the selected source order, independently of the
    /// supplied IDs, so the raw result may be appended into another owner's prefix.
    /// Every selected child needs a selected map entry, including zero-weight rows.
    /// Unreferenced arena cells are discarded. A first pass counts selected cells,
    /// then each destination reserves exactly once before the second pass copies.
    /// Row and descriptor maps use O(source
    /// rows + descriptors) temporary cells; work is O(source nodes + source rows +
    /// source descriptors + selected Boolean edges + selected distinct row entries).
    ///
    /// # Errors
    /// Refuses malformed selected storage, absent child map entries, allocation
    /// failure or occurrence overflow. Atom-universe and topology admission remain
    /// the resulting owner's responsibility.
    pub fn try_select(self, children: &[Option<usize>]) -> Result<FormulaParts, AdmissionError> {
        self.try_select_with_peak(children).map(|(parts, _)| parts)
    }

    /// Select and remap like [`Self::try_select`], measuring actual allocation peak.
    ///
    /// The second return value is the byte capacity of the output's six arenas plus
    /// both internal row/descriptor maps while they coexist. It excludes the borrowed
    /// source and child map, vector headers, stack values and allocator overhead.
    /// Exact reservations request compact storage; this receipt measures actual
    /// capacities even if the allocator supplies more than requested. The maps are
    /// released before returning, leaving only the returned formula's retained bytes.
    ///
    /// # Errors
    /// Preserves the selection's malformed-storage, missing-child, allocation and
    /// occurrence-overflow refusals. No partial output or receipt escapes on error.
    pub fn try_select_with_peak(
        self,
        children: &[Option<usize>],
    ) -> Result<(FormulaParts, u128), AdmissionError> {
        let mut selection = Selection::new(self, children)?;
        for (index, child) in children.iter().enumerate() {
            if child.is_some() {
                selection.append(index)?;
            }
        }
        let peak = selection.result.retained_bytes()
            + (selection.rows.capacity() as u128 + selection.thresholds.capacity() as u128)
                * std::mem::size_of::<Option<usize>>() as u128;
        let result = selection.result;
        let parts = FormulaParts::from_raw(
            result.nodes,
            result.operands,
            result.numeric.conditions,
            result.numeric.weights,
            result.numeric.rows,
            result.numeric.thresholds,
        )?;
        Ok((parts, peak))
    }
}
