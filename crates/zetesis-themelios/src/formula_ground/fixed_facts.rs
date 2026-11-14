//! Substitute explicit facts and certain atoms only at positive body occurrences.
//!
//! Complete source support has already admitted every fact identity. This pass
//! records the facts before any dependent rule is emitted, independently of
//! source order. It does not publish formula atoms, roots or origins. Ordinary
//! emission still retains every fact root and names every body atom before its
//! occurrence is simplified. A merely possible support row is never a premise;
//! a supported atom of a certain predicate is one, as stated below.
//!
//! If F is the recorded fact set, the retained roots force F into every original
//! model I and every model J of its frozen reduct. Replacing a positive occurrence
//! of p in F by true therefore preserves both evaluations, by induction through
//! the enclosing formula. Candidates omitting a fact fail its retained root in
//! both theories. Heads, negative occurrences and objective queries are unchanged.
//!
//! The supported atoms of a certain predicate (`formula_support::certain`) are
//! fixed in the same way. A named body atom was read from the completed
//! support, and each supported atom of a certain predicate has a derivation
//! from facts through definite rules; the roots of those rules force it as a
//! fact's root forces the fact (`CertainAtoms.derived_forced`,
//! `fixed_stable`).
//!
//! That premise is checked on the emitted theory rather than taken from the
//! analysis ([`Builder::fixed_atoms_are_derived`]). Every instance that derives
//! an atom of a certain predicate has one ordinary head and a body the
//! substitution made true, so each of the atom's producers is an unconditional
//! ordinary root; an explicit fact has at least its own. The check requires
//! exactly that of every atom a body read as true, once no further body will
//! be read. It establishes the premise for rules without aggregates: an
//! unconditional root stands for an instance whose positive body atoms were
//! all read as true and so passed the same check, and support completion
//! derives an atom after the atoms its first derivation reads, so by induction
//! in that order every atom read as true is derived from facts. A choice, a
//! disjunct, a negation or an unread atom in any producer of a wrongly certain
//! atom breaks the equality, and the theory is not published. The check does
//! not see why an exact assignment's family was emitted as true: that rests on
//! the listing of its required elements. Nor does it examine a predicate no
//! body reads.
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

use super::metadata::Fixing;
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

    /// The caller already named and validated this occurrence normally. An
    /// atom is fixed when it is an explicit fact or its predicate is certain;
    /// either lookup runs once for the atom.
    pub(super) fn fixed_body_atom(
        &mut self,
        node: usize,
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        if self.fixed_facts.is_none() && self.certainty.is_none() {
            return Ok(node);
        }
        self.counters.work(self.limits, location)?;
        let Node::Atom(atom) = self.nodes.view().node(node).expect("owned body node") else {
            return Ok(node);
        };
        self.counters.work(self.limits, location)?;
        let fixing = if let Some(fixing) = self.metadata.fixing(atom) {
            fixing
        } else {
            let mut fixing = Fixing::Written;
            if let Some(facts) = &self.fixed_facts {
                let source = self
                    .catalog
                    .source(atom, self.counters, self.limits, location)?;
                if facts
                    .position(&source, self.limits, self.counters, location)?
                    .is_some()
                {
                    fixing = Fixing::Fact;
                }
            }
            if fixing == Fixing::Written
                && let Some(certainty) = self.certainty
            {
                // A named body atom is a supported one: the join that named
                // it read its row from the completed support.
                let predicate = self
                    .catalog
                    .get(atom, self.computation, self.counters, self.limits, location)?
                    .predicate();
                if certainty.certain(predicate, self.limits, self.counters, location)? {
                    fixing = Fixing::Certain;
                }
            }
            self.counters.work(self.limits, location)?;
            self.metadata.record_fixing(atom, fixing);
            fixing
        };
        Ok(if fixing.fixed() { VERUM } else { node })
    }

    /// Every atom a body read as true is derived by unconditional ordinary
    /// roots, as the module header requires: an explicit fact has one, and
    /// every producer of an atom read through its certain predicate is one.
    /// This is checked over the emitted theory, after the last body is read,
    /// rather than assumed from the analysis that called the predicate
    /// certain. The roots are counted where their producers are recorded
    /// ([`Builder::ordinary_producer`]).
    ///
    /// One unit per atom.
    ///
    /// # Errors
    /// [`FormulaFailure::SourceActivity`] at the first atom that fails the
    /// check, a disagreement between completed support and emission: no
    /// theory is published. Otherwise the work refusal.
    pub(super) fn fixed_atoms_are_derived(&mut self) -> Result<(), FormulaFailure> {
        if self.fixed_facts.is_none() && self.certainty.is_none() {
            return Ok(());
        }
        for atom in 0..self.catalog.len() {
            let location = self.metadata.location(atom);
            self.counters.work(self.limits, location)?;
            let roots = self.metadata.unconditional_roots(atom);
            let derived = match self.metadata.fixing(atom) {
                None | Some(Fixing::Written) => true,
                Some(Fixing::Fact) => roots > 0,
                Some(Fixing::Certain) => roots > 0 && roots == self.metadata.producers(atom).len(),
            };
            if !derived {
                return Err(FormulaFailure::SourceActivity { location });
            }
        }
        Ok(())
    }

    /// Producer groups have at most three children. Keep their old ordered
    /// representation when no fact changed, including repeated operands.
    pub(super) fn fixed_body_group(
        &mut self,
        values: &[usize],
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        if self.fixed_facts.is_none() && self.certainty.is_none() {
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
