//! Substitute explicit facts only at positive body occurrences.
//!
//! Complete source support has already admitted every fact identity. This pass
//! records the facts before any dependent rule is emitted, independently of
//! source order. It does not publish formula atoms, roots or origins. Ordinary
//! emission still retains every fact root and names every body atom before its
//! occurrence is simplified. Merely possible support rows are never premises.
//!
//! If F is the recorded fact set, the retained roots force F into every original
//! model I and every model J of its frozen reduct. Replacing a positive occurrence
//! of p in F by true therefore preserves both evaluations, by induction through
//! the enclosing formula. Candidates omitting a fact fail its retained root in
//! both theories. Heads, negative occurrences and objective queries are unchanged.
//!
//! The source-scoped selection owns no atom payload. Collection appends R fact
//! occurrences, sorts their numeric source IDs with metered O(R log R) work,
//! and deduplicates before publishing F distinct facts. Its temporary and retained
//! capacity is O(R); no input-history-sized mask or second payload owner is built.
//! Each emitted atom's fact membership is looked up at most once and cached in
//! its existing construction metadata. Group nodes never repeat atom lookups.
//! The initialization pass repeats fact scalar/identity reads and uses one
//! block-arity scratch. Its work, storage or allocation refusal can precede an
//! ordinary emission resource refusal; semantic fact admission already completed
//! in the original support pass. Failed collection publishes no theory. Source
//! order emission continues to validate all siblings of a simplified body.

use super::{Builder, Node, VERUM};
use crate::formula_binding::Binding;
use crate::formula_facts::{Item, Items};
use crate::formula_ir::HeadIr;
use crate::formula_support::{SourceAtom, SourceBatch, SourceSelection, facts::FactHead};
use crate::{FormulaFailure, ProgramSite};

#[cfg(test)]
mod tests;

impl Builder<'_, '_, '_> {
    pub(super) fn collect_fixed_facts(&mut self, items: &Items) -> Result<(), FormulaFailure> {
        #[cfg(test)]
        if !tests::enabled() {
            return Ok(());
        }
        let mut facts = None;
        let mut fact_location = None;
        for item in items.iter() {
            let location = match item {
                Item::Rule(_, rule) => rule.location,
                Item::Facts(block) => block.location(),
            };
            self.work(location)?;
            match item {
                Item::Facts(block) => {
                    let mut head = FactHead::new(
                        block,
                        self.computation,
                        self.limits,
                        self.counters,
                        location,
                    )?;
                    for row in block.rows() {
                        let location = row.location();
                        self.work(location)?;
                        let atom = head.atom(row, self.computation, self.limits, self.counters)?;
                        self.remember_fact(&mut facts, &atom, location)?;
                        fact_location.get_or_insert(location);
                    }
                }
                Item::Rule(_, rule)
                    if rule.body.is_empty() && rule.variables == 0 && rule.bindings.is_none() =>
                {
                    // The ordinary representation of the same ground fact;
                    // disjunctions and choices never establish a fact.
                    if let HeadIr::Normal(Some(head)) = &rule.head {
                        let binding =
                            Binding::new(self.computation, self.limits, self.counters, location)?;
                        let head = self.computation.static_pattern(
                            *head,
                            self.limits,
                            self.counters,
                            location,
                        )?;
                        let atom = self.computation.atom(
                            head,
                            &binding,
                            self.limits,
                            self.counters,
                            location,
                        )?;
                        self.remember_fact(&mut facts, &atom, location)?;
                        fact_location.get_or_insert(location);
                    }
                }
                Item::Rule(..) => {}
            }
        }
        let bound = self.atom_bound();
        self.fixed_facts = facts
            .map(|facts| {
                facts.finish(
                    bound,
                    self.computation,
                    self.limits,
                    self.counters,
                    fact_location.expect("nonempty fact batch has a source site"),
                )
            })
            .transpose()?;
        Ok(())
    }

    fn remember_fact(
        &mut self,
        facts: &mut Option<SourceBatch>,
        atom: &SourceAtom,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        if facts.is_none() {
            *facts = Some(SourceSelection::batch(
                self.computation,
                self.limits,
                self.counters,
                location,
            )?);
        }
        facts.as_mut().expect("initialized fact batch").push(
            atom,
            self.computation,
            self.limits,
            self.counters,
            location,
        )?;
        Ok(())
    }

    /// The caller already named and validated this occurrence normally.
    pub(super) fn fixed_body_atom(
        &mut self,
        node: usize,
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        let Some(facts) = &self.fixed_facts else {
            return Ok(node);
        };
        self.counters.work(self.limits, location)?;
        let Node::Atom(atom) = self.nodes.view().node(node).expect("owned body node") else {
            return Ok(node);
        };
        self.counters.work(self.limits, location)?;
        let fixed = if let Some(fixed) = self.metadata.fixed_fact(atom) {
            fixed
        } else {
            let source = self
                .catalog
                .source(atom, self.counters, self.limits, location)?;
            let fixed = facts
                .position(&source, self.limits, self.counters, location)?
                .is_some();
            self.counters.work(self.limits, location)?;
            self.metadata.record_fixed_fact(atom, fixed);
            fixed
        };
        Ok(if fixed { VERUM } else { node })
    }

    /// Producer groups have at most three children. Keep their old ordered
    /// representation when no fact changed, including repeated operands.
    pub(super) fn fixed_body_group(
        &mut self,
        values: &[usize],
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        if self.fixed_facts.is_none() {
            return self.node(Node::And(values), location);
        }
        let mut mapped = [0; 3];
        let mut retained = 0;
        for &value in values {
            self.work(location)?;
            if value != VERUM {
                mapped[retained] = value;
                retained += 1;
            }
        }
        match retained {
            0 => Ok(VERUM),
            1 => Ok(mapped[0]),
            _ => self.node(Node::And(&mapped[..retained]), location),
        }
    }
}
