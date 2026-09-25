//! Separate outer scopes and independently bound universal-local instances.

use crate::formula_support::components::Pattern as AtomPattern;
use themelios_program::program::{
    Body, BodyElement, ConditionalLiteral, DefaultNegation, Literal, LiteralInner,
};
use themelios_program::term::Term;

use crate::formula_guard::Guard;
use crate::formula_ir::{Compiler, LiteralIr, LocalFamily, Projection, Variables};
use crate::{AdmissionFailure, ExpansionResource, FormulaFailure, InputLimit};

pub(crate) struct ConditionalIr {
    pub family: LocalFamily,
    pub consequent: Consequent,
    pub condition: Vec<LiteralIr>,
    pub variables: usize,
}

/// Local alternatives never bind the outer rule or generate positive support.
pub(crate) enum Consequent {
    Atoms(DefaultNegation, Vec<Alternative>),
    Guard(Guard),
    /// Disjoin complete data alternatives before the universal condition row.
    Guards(Vec<GuardAlternative>),
}

pub(crate) struct GuardAlternative {
    pub guard: Guard,
    /// Value generators only; these do not contribute logical source atoms.
    pub bindings: Vec<LiteralIr>,
    pub variables: usize,
}

pub(crate) struct Alternative {
    pub operand: ConsequentOperand,
    /// Data instructions and optional positive witnesses, scoped to this alternative.
    pub bindings: Vec<LiteralIr>,
    pub variables: usize,
}

/// Source alternatives and anonymous witnesses have distinct quantifiers.
/// Projection completes the witness disjunction before default negation;
/// the caller disjoins the resulting signed source alternatives afterwards.
pub(crate) enum ConsequentOperand {
    Atom(AtomPattern),
    Projection(Projection),
}

impl Compiler<'_> {
    /// Bound one independent alternative's copied scope and instruction owners.
    pub(super) fn alternative_scope(
        &mut self,
        work: u128,
        carrier_bytes: usize,
        condition: &Variables,
    ) -> Result<Variables, FormulaFailure> {
        self.budget.charge(
            ExpansionResource::TermWork,
            work * 2
                + condition.named.len() as u128
                + condition.safe.len() as u128
                + condition.argument_inputs.len() as u128,
            self.location,
        )?;
        // Each alternative owns its scope and binding plan. Reserve selected
        // map/set payload and three instruction-vector equivalents (source,
        // pending, scheduled) before cloning or allocating them. Tree allocator
        // overhead remains outside logical byte accounting.
        let scope_bytes = condition
            .named
            .keys()
            .map(|name| name.len() as u128 + std::mem::size_of::<(String, usize)>() as u128)
            .sum::<u128>()
            + (condition.safe.len() + condition.argument_inputs.len()) as u128
                * std::mem::size_of::<usize>() as u128;
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            scope_bytes
                + carrier_bytes as u128
                + (work + 1) * 3 * std::mem::size_of::<LiteralIr>() as u128,
            self.location,
        )?;
        Ok(condition.clone())
    }

    pub(super) fn conditional_syntax(
        &mut self,
        conditional: &ConditionalLiteral,
    ) -> Result<(), FormulaFailure> {
        self.conditional_terms(&conditional.literal)?;
        for (index, literal) in conditional.condition.literals().enumerate() {
            if index >= self.options.max_body_elements {
                return Err(AdmissionFailure::Limit {
                    resource: InputLimit::BodyElements,
                    limit: self.options.max_body_elements,
                    observed: index + 1,
                    location: self.location,
                }
                .into());
            }
            self.conditional_terms(literal.get())?;
        }
        // Only occurrences outside conditionals establish global names. A
        // consequent-only positive witness belongs to its own local scope.
        Ok(())
    }

    fn conditional_terms(&mut self, literal: &Literal) -> Result<(), FormulaFailure> {
        self.budget
            .charge(ExpansionResource::TermWork, 1, self.location)?;
        let mut scan = |term: &Term| -> Result<(), FormulaFailure> {
            for _ in term.subterms() {
                self.budget
                    .charge(ExpansionResource::TermWork, 1, self.location)?;
            }
            Ok(())
        };
        match &literal.inner {
            LiteralInner::Atom(atom) => {
                for term in atom.get().argument_terms() {
                    scan(term)?;
                }
            }
            LiteralInner::Comparison(comparison) => {
                scan(comparison.get().first())?;
                for (_, term) in comparison.get().steps() {
                    scan(term)?;
                }
            }
            LiteralInner::True | LiteralInner::False => {}
        }
        Ok(())
    }

    pub(super) fn body_conditionals(
        &mut self,
        source: &Body,
        variables: &Variables,
        body: &mut Vec<LiteralIr>,
    ) -> Result<(), FormulaFailure> {
        for (index, element) in source.elements().enumerate() {
            let BodyElement::Conditional(conditional) = element.get() else {
                continue;
            };
            let family = LocalFamily(index);
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            for condition in self.condition_alternatives(&conditional.condition)? {
                let mut local = variables.clone();
                let mut condition = self.condition(&condition, &mut local)?;
                self.bindings(&mut condition, &mut local)?;
                self.variable_limit(&local)?;
                local.safety(self.location)?;
                // A positive consequent must not repair an unsafe condition.
                let consequent = self.conditional_consequent(&conditional.literal, &mut local)?;
                body.push(LiteralIr::Conditional(ConditionalIr {
                    family,
                    consequent,
                    condition,
                    variables: local.count,
                }));
            }
        }
        Ok(())
    }

    fn conditional_consequent(
        &mut self,
        literal: &Literal,
        variables: &mut Variables,
    ) -> Result<Consequent, FormulaFailure> {
        if let LiteralInner::Atom(atom) = &literal.inner {
            let mut alternatives = Vec::new();
            for atom in self.conditional_atoms(atom.get())? {
                alternatives.push(self.consequent_alternative(
                    &atom,
                    literal.negation,
                    variables,
                )?);
            }
            return Ok(Consequent::Atoms(literal.negation, alternatives));
        }
        if let LiteralInner::Comparison(comparison) = &literal.inner {
            let terms = || {
                std::iter::once(comparison.get().first())
                    .chain(comparison.get().steps().map(|(_, term)| term))
            };
            let count = terms().flat_map(Term::subterms).count() as u128;
            self.budget
                .charge(ExpansionResource::TermWork, count, self.location)?;
            if terms()
                .flat_map(Term::subterms)
                .any(|term| matches!(term, Term::Pool(_) | Term::Interval { .. }))
            {
                let mut alternatives = Vec::new();
                for literal in self.literal_alternatives(literal)? {
                    alternatives.push(self.guard_alternative(&literal, variables, count)?);
                }
                return Ok(Consequent::Guards(alternatives));
            }
        }
        let guard = if let LiteralInner::Comparison(comparison) = &literal.inner {
            self.comparison_guard(comparison.get(), literal.negation, variables)?
        } else {
            self.literal(literal, variables)?
        };
        let LiteralIr::Guard(guard) = guard else {
            unreachable!("non-atom conditional consequent is a ground guard")
        };
        variables.safety(self.location)?;
        Ok(Consequent::Guard(guard))
    }

    fn guard_alternative(
        &mut self,
        literal: &Literal,
        condition: &Variables,
        work: u128,
    ) -> Result<GuardAlternative, FormulaFailure> {
        let LiteralInner::Comparison(comparison) = &literal.inner else {
            unreachable!("comparison alternatives retain their kind")
        };
        let mut local =
            self.alternative_scope(work, std::mem::size_of::<GuardAlternative>(), condition)?;
        let mut bindings = Vec::new();
        let ranged = std::iter::once(comparison.get().first())
            .chain(comparison.get().steps().map(|(_, term)| term))
            .flat_map(Term::subterms)
            .any(|term| matches!(term, Term::Interval { .. }));
        let guard = if ranged {
            self.ranged_guard(
                comparison.get(),
                literal.negation,
                &mut local,
                &mut bindings,
            )?
        } else {
            let LiteralIr::Guard(guard) =
                self.comparison_guard(comparison.get(), literal.negation, &mut local)?
            else {
                unreachable!("comparison guard")
            };
            guard
        };
        self.bindings(&mut bindings, &mut local)?;
        self.variable_limit(&local)?;
        local.safety(self.location)?;
        Ok(GuardAlternative {
            guard,
            bindings,
            variables: local.count,
        })
    }
}
