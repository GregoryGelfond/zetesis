//! Decide a default-negated flat atom of a rule's body where the completed
//! support does.
//!
//! Emission reads the completed support, and that support decides two kinds
//! of atom in every answer set.
//!
//! A supported atom that is an explicit fact or belongs to a certain
//! predicate holds in all of them (`fixed_facts`). The retained roots force
//! it into every model of the theory and of its reduct, so reading it as true
//! under a negation preserves both evaluations exactly as at a positive
//! occurrence, and the same check on the emitted theory covers it
//! ([`Builder::fixed_atoms_are_derived`]): the atom is recorded as read
//! through its fixing at either sign.
//!
//! An atom outside the completed support holds in none. Every emitted
//! instance takes its head from that support, and so does every fact, so the
//! theory has no producer for the atom: it occurs in no head. Removing such
//! an atom from an interpretation leaves every implication of the frozen
//! reduct satisfied, since no consequent reads it, so an interpretation that
//! holds it has a proper subset satisfying its reduct and is no answer set.
//! Reading the atom as false therefore changes neither evaluation at an
//! interpretation without it, and no answer set has it
//! (`DecidedNegation.absent_false`, `absent_stable`).
//!
//! `not a` is then false for a fixed `a` and true for an absent one, and
//! `not not a` the reverse. A false literal makes its body false, and the
//! instance is not emitted: it has no root and is no producer of its head.
//! A true literal leaves the body. In neither case does the occurrence add
//! the atom to the theory: an absent atom is never interned, and a fixed one
//! is, to read its fixing, but has a root of its own already. A supported
//! atom that is not fixed keeps its literal.
//!
//! What is decided depends on whose body it is ([`Decide`]):
//!
//! - A producer's body decides both kinds, on every route that emits it: the
//!   complete join, its components and its continuations. Support completion
//!   derives a head from exactly the rows whose decided negated atoms hold
//!   (`formula_support::decided_negations_hold`), and this drops those
//!   instances and any other whose negated atom the completed support
//!   decides, so no emitted head lies outside the support.
//! - A constraint's body decides a fixed atom alone. An atom outside the
//!   support keeps its literal there, and so stays named, and a constraint
//!   dropped for a fixed atom still names the atoms of its other literals:
//!   a constraint that is streamed instead of emitted names every atom its
//!   rows read, and both routes publish the same atoms.
//! - A local scope decides nothing: a conditional literal's parts, a
//!   projection, an aggregate's or a choice's element condition and a head
//!   are emitted as written.
//!
//! Only the theory's builder emits a rule's own body. An objective's
//! condition and a discarded validation body are lowered as scopes, and keep
//! every literal.
//!
//! One unit for the decision, with the pattern's key and the support lookup
//! charging their own.

#[cfg(test)]
mod tests;

use themelios_program::program::DefaultNegation;

use super::{Builder, Purpose, VERUM, boolean};
use crate::formula_binding::Binding;
use crate::formula_ir::{HeadIr, RuleIr};
use crate::formula_support::certain;
use crate::formula_support::components::Pattern as AtomPattern;
use crate::{FormulaFailure, ProgramSite};

/// Which default-negated flat atoms the emission of one body decides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Decide {
    /// A local scope: every literal is emitted as written.
    Nothing,
    /// A constraint's body: an atom that is fixed.
    Fixed,
    /// A producer's body: an atom that is fixed or outside the support.
    Supported,
}

impl Decide {
    /// What the own body of `rule` decides: a constraint's or a producer's.
    pub(super) fn of(rule: &RuleIr) -> Self {
        if matches!(rule.head, HeadIr::Normal(None)) {
            Self::Fixed
        } else {
            Self::Supported
        }
    }
}

/// What the emission of one default-negated body atom found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Negated {
    /// The completed support decides the literal: its value.
    Decided(usize),
    /// The atom is supported and not fixed, so its literal stays: the atom's
    /// node, named by the lookup that found it supported.
    Named(usize),
    /// Nothing was read, or an absent atom keeps its literal: the caller
    /// names the atom as written.
    Written,
}

impl Builder<'_, '_, '_> {
    /// What `decide` and the completed support make of the body occurrence
    /// `negation pattern` under `assignment`, as the module header states.
    /// `negation` is `Not` or `NotNot`. One lookup serves the support test
    /// and names a supported atom.
    pub(super) fn decided_negation(
        &mut self,
        decide: Decide,
        negation: DefaultNegation,
        pattern: AtomPattern,
        assignment: &Binding,
        location: ProgramSite,
    ) -> Result<Negated, FormulaFailure> {
        debug_assert_ne!(negation, DefaultNegation::None);
        if decide == Decide::Nothing || certain::negation_left_out() {
            return Ok(Negated::Written);
        }
        // A rule's own body is emitted by the theory's builder alone.
        debug_assert!(matches!(self.purpose, Purpose::Theory));
        self.work(location)?;
        let prepared =
            self.computation
                .static_pattern(pattern, self.limits, self.counters, location)?;
        let supported = self.computation.supported_atom(
            prepared,
            assignment,
            self.limits,
            self.counters,
            location,
        )?;
        let Some(source) = supported else {
            return Ok(if decide == Decide::Supported {
                Negated::Decided(boolean(negation != DefaultNegation::NotNot))
            } else {
                Negated::Written
            });
        };
        let atom = self.atom_identity(&source, location)?;
        if self.fixed_body_atom(atom, location)? != VERUM {
            return Ok(Negated::Named(atom));
        }
        Ok(Negated::Decided(boolean(
            negation == DefaultNegation::NotNot,
        )))
    }
}
