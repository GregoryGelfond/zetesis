//! Complete positive witnesses share lazy formula construction with their join.
//!
//! Only a selected complete descendant may publish an atom or group. Later
//! siblings reuse exactly the prefix left intact by join undo. Matched rows
//! supply identities; support membership supplies no truth premise.
//!
//! Balanced groups have at most three children. Consecutive traversal columns
//! split into two or three nearly equal ranges recursively, so native rows keep
//! reusable subbodies rather than flattening every complete body. Metadata is
//! O(body width), grouping depth logarithmic. A non-prefix group of two or
//! three atomic witnesses can additionally retain reached row coordinates and
//! the canonical node already compiled for them. Only successful complete
//! descendants populate these maps; their separate storage and actual lookup,
//! insertion and movement work belong to the existing source workspace.

#[cfg(test)]
pub(crate) mod testing;
#[cfg(test)]
mod tests;

use std::ops::Range;
use zetesis_ferraris::NodeView;

use super::Builder;
use super::cache::CoordinateMap;
use crate::FormulaFailure;
use crate::formula_ir::RuleIr;
use crate::formula_support::{Buffer, Context, Join, RowHead, witnesses};

#[derive(Clone, Copy)]
enum Input {
    Atom(usize),
    Group(usize),
}

#[derive(Clone, Copy)]
enum Children {
    Pair([Input; 2]),
    Triple([Input; 3]),
}
impl Children {
    fn slice(&self) -> &[Input] {
        match self {
            Self::Pair(inputs) => inputs,
            Self::Triple(inputs) => inputs,
        }
    }
}

struct Group {
    // Postorder ends are monotone; undo invalidates one suffix.
    end: usize,
    children: Children,
    node: Option<usize>,
    // A map is confined to this Body and therefore to one immutable ordered
    // Join/support owner. Row positions from another occurrence/rule/snapshot
    // can never query it. Its values borrow the Builder's append-only nodes;
    // no graph rollback or owner replacement occurs during this Body's life.
    compiled: Option<CoordinateMap<[usize; 3], usize>>,
}

impl Group {
    fn reusable(&self) -> bool {
        matches!(self.children.slice().first(), Some(Input::Atom(first)) if *first > 0)
            && self
                .children
                .slice()
                .iter()
                .all(|input| matches!(input, Input::Atom(_)))
    }
}

struct Body {
    atoms: Buffer<Option<usize>>,
    groups: Buffer<Group>,
}

impl Body {
    fn new(
        width: usize,
        builder: &mut Builder<'_, '_, '_>,
        rule: &RuleIr,
    ) -> Result<Self, FormulaFailure> {
        let mut atoms = Buffer::new(
            builder.computation,
            builder.limits,
            &mut builder.counters,
            rule.location,
        )?;
        atoms.resize(
            width,
            None,
            builder.computation,
            builder.limits,
            &mut builder.counters,
            rule.location,
        )?;
        let mut groups = Buffer::new(
            builder.computation,
            builder.limits,
            &mut builder.counters,
            rule.location,
        )?;
        branch(0..width, &mut groups, builder, rule)?;
        if groups.len() == 1 {
            return Ok(Self { atoms, groups });
        }
        #[cfg(test)]
        if !testing::reuse_enabled() {
            return Ok(Self { atoms, groups });
        }
        for group in groups.slice_mut() {
            builder.work(rule.location)?;
            if !group.reusable() {
                continue;
            }
            // The leased Group buffer already owns the full inline map header.
            // No tuple or formula is precomputed from the possible populations.
            group.compiled = Some(
                CoordinateMap::new(
                    builder.computation,
                    builder.limits,
                    &mut builder.counters,
                    rule.location,
                )?
                .into_embedded(rule.location)?,
            );
        }
        Ok(Self { atoms, groups })
    }

    fn emit(
        &mut self,
        row: &witnesses::WitnessRow<'_, '_>,
        builder: &mut Builder<'_, '_, '_>,
        rule: &RuleIr,
    ) -> Result<usize, FormulaFailure> {
        #[cfg(test)]
        if !testing::reuse_enabled() {
            return self.prefix_emit(row, builder, rule);
        }
        if self.groups.len() == 1 {
            return self.prefix_emit(row, builder, rule);
        }
        self.group(self.groups.len() - 1, row, builder, rule)
    }

    /// Recursion follows the existing balanced tree; its depth is logarithmic
    /// in body width. A compiled leaf group owns its atom materialization as a
    /// whole: hits never leave per-atom prefix state that a later miss could
    /// mistakenly interpret as current.
    fn group(
        &mut self,
        index: usize,
        row: &witnesses::WitnessRow<'_, '_>,
        builder: &mut Builder<'_, '_, '_>,
        rule: &RuleIr,
    ) -> Result<usize, FormulaFailure> {
        builder.work(rule.location)?;
        let group = &self.groups.slice()[index];
        if group.end <= row.unchanged {
            return Ok(group
                .node
                .expect("unchanged group has a selected predecessor"));
        }
        let children = group.children;
        let mut key = [0; 3];
        let compiled = group.compiled.is_some();
        if let Some(map) = &group.compiled {
            for (slot, &input) in children.slice().iter().enumerate() {
                builder.work(rule.location)?;
                let Input::Atom(at) = input else {
                    unreachable!("only atomic leaf groups retain row coordinates")
                };
                key[slot] = row.rows[at].position();
            }
            #[cfg(test)]
            testing::record(testing::Point::Lookup);
            if let Some(node) = map.find(
                &key,
                builder.computation,
                builder.limits,
                &mut builder.counters,
                rule.location,
            )? {
                #[cfg(test)]
                testing::record(testing::Point::Reuse);
                // Observe cancellation at the reuse boundary before publication.
                builder.work(rule.location)?;
                self.groups.slice_mut()[index].node = Some(node);
                return Ok(node);
            }
        }
        let mut values = [0; 3];
        for (slot, &input) in children.slice().iter().enumerate() {
            builder.work(rule.location)?;
            values[slot] = match input {
                Input::Atom(at) => {
                    if compiled || at >= row.unchanged {
                        #[cfg(test)]
                        testing::record(testing::Point::Atom);
                        let node = builder.atom_ref(row.rows[at].atom(), rule.location)?;
                        if !compiled {
                            builder.work(rule.location)?;
                            self.atoms.slice_mut()[at] = Some(node);
                        }
                        node
                    } else {
                        self.atoms.slice()[at].expect("unchanged selected positive atom")
                    }
                }
                Input::Group(child) => self.group(child, row, builder, rule)?,
            };
        }
        #[cfg(test)]
        testing::record(testing::Point::Group);
        // The original ordered positive operands, including duplicates, enter
        // the same sole canonical node index. Support is never assumed true.
        let node = builder.node(
            NodeView::And(&values[..children.slice().len()]),
            rule.location,
        )?;
        if let Some(map) = &mut self.groups.slice_mut()[index].compiled {
            map.insert(
                key,
                node,
                None,
                Context::new(
                    &*builder.computation,
                    builder.limits,
                    &mut builder.counters,
                    rule.location,
                ),
            )?;
        }
        self.groups.slice_mut()[index].node = Some(node);
        Ok(node)
    }

    fn prefix_node(&self, input: Input) -> usize {
        match input {
            Input::Atom(index) => self.atoms.slice()[index].expect("selected positive atom"),
            Input::Group(index) => self.groups.slice()[index]
                .node
                .expect("completed child group"),
        }
    }

    fn prefix_emit(
        &mut self,
        row: &witnesses::WitnessRow<'_, '_>,
        builder: &mut Builder<'_, '_, '_>,
        rule: &RuleIr,
    ) -> Result<usize, FormulaFailure> {
        for (index, witness) in row.rows.iter().enumerate().skip(row.unchanged) {
            #[cfg(test)]
            testing::record(testing::Point::Atom);
            let node = builder.atom_ref(witness.atom(), rule.location)?;
            builder.work(rule.location)?;
            self.atoms.slice_mut()[index] = Some(node);
        }
        // Locate the changed suffix without rescanning every group. Every
        // inspected boundary remains a charged operation.
        let mut start = 0;
        let mut end = self.groups.len();
        while start < end {
            builder.work(rule.location)?;
            let middle = start + (end - start) / 2;
            if self.groups.slice()[middle].end <= row.unchanged {
                start = middle + 1;
            } else {
                end = middle;
            }
        }
        for index in start..self.groups.len() {
            let mut values = [0; 3];
            let children = self.groups.slice()[index].children;
            for (slot, &input) in children.slice().iter().enumerate() {
                builder.work(rule.location)?;
                values[slot] = self.prefix_node(input);
            }
            #[cfg(test)]
            testing::record(testing::Point::Group);
            // Children are atomic formulas or nonempty positive groups, never
            // Boolean constants. Complete ordered duplicates remain operands.
            let node = builder.node(
                NodeView::And(&values[..children.slice().len()]),
                rule.location,
            )?;
            self.groups.slice_mut()[index].node = Some(node);
        }
        Ok(self
            .groups
            .slice()
            .last()
            .expect("nonempty body groups")
            .node
            .expect("completed root group"))
    }
}

/// At most three strictly smaller positive ranges per recursion give a
/// logarithmic stack bound for every admitted source body width.
fn branch(
    range: Range<usize>,
    groups: &mut Buffer<Group>,
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
) -> Result<Input, FormulaFailure> {
    builder.work(rule.location)?;
    let width = range.len();
    if width == 1 {
        return Ok(Input::Atom(range.start));
    }
    let count = if width <= 3 {
        width
    } else {
        width.div_ceil(3).clamp(2, 3)
    };
    let mut children = [Input::Atom(range.start); 3];
    let mut start = range.start;
    for (index, child) in children.iter_mut().enumerate().take(count) {
        let end = start + width / count + usize::from(index < width % count);
        *child = branch(start..end, groups, builder, rule)?;
        start = end;
    }
    let children = if count == 2 {
        Children::Pair([children[0], children[1]])
    } else {
        Children::Triple(children)
    };
    let index = groups.len();
    groups.push(
        Group {
            end: range.end,
            children,
            node: None,
            compiled: None,
        },
        builder.computation,
        builder.limits,
        &mut builder.counters,
        rule.location,
    )?;
    Ok(Input::Group(index))
}

/// Decline without advancing the supplied join when this class does not apply.
pub(super) fn rule(
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
    outer: &mut Join<'_, '_>,
) -> Result<bool, FormulaFailure> {
    #[cfg(test)]
    if !testing::enabled() {
        return Ok(false);
    }
    let Some(pattern) = witnesses::eligible(rule, builder.limits, &mut builder.counters)? else {
        return Ok(false);
    };
    #[cfg(test)]
    testing::record(testing::Point::Run);
    outer.retain_witnesses(Context::new(
        builder.computation,
        builder.limits,
        &mut builder.counters,
        rule.location,
    ))?;
    let mut prepared = None;
    while let Some(row) = outer.next_witness_row(
        None,
        builder.budget,
        Context::new(
            builder.computation,
            builder.limits,
            &mut builder.counters,
            rule.location,
        ),
    )? {
        #[cfg(test)]
        testing::record(testing::Point::Witness);
        if prepared.is_none() {
            let head = RowHead::new(
                pattern,
                &row,
                builder.computation,
                builder.limits,
                &mut builder.counters,
                rule.location,
            )?;
            prepared = Some((head, Body::new(row.rows.len(), builder, rule)?));
        }
        let (head, body) = prepared.as_mut().expect("selected producer preparation");
        let body = body.emit(&row, builder, rule)?;
        let atom = head.atom(
            &row,
            builder.computation,
            builder.limits,
            &mut builder.counters,
            rule.location,
        )?;
        let head = builder.atom_identity(&atom, rule.location)?;
        let formula = builder.node(NodeView::Implies(body, head), rule.location)?;
        builder.root(formula, rule)?;
        builder.producer(head, body, rule)?;
        #[cfg(test)]
        testing::record(testing::Point::Publish);
    }
    Ok(true)
}
