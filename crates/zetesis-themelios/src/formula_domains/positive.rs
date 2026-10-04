//! Positive-flat rules of a normalized source and their original IR occurrence owner.
//!
//! Canonical source rules may coalesce duplicates. The IR occurrence array is
//! therefore checked independently and remains the sole occurrence/provenance
//! owner. This certificate qualifies rules and dependencies for optional domain
//! guards; producer scheduling separately excludes objective observations. It
//! establishes neither objective semantics, answer-set membership nor uniqueness.
//! Objective observations remain in the exact analyzed program but produce no
//! argument values and receive no rule-domain guards.
//!
//! A body comparison belongs to the profile: it binds no variable and offers no
//! row, so the argument domains stay upper bounds over it, and the guards narrow
//! a variable's candidates by the comparisons that read it alone.

use crate::ProgramSite;
use themelios_program::program::{
    Arguments, BodyElement, DefaultNegation, Head, Literal, LiteralInner, Statement,
};
use themelios_program::symbol::{Sign, Symbol};
use themelios_program::term::{Term, Variable};

use crate::formula_ir::{HeadIr, LiteralIr, Prepared, RuleIr};
use crate::formula_support::Counters;
use crate::{AnalysisBasis, FormulaFailure, FormulaLimits};

/// A single atomic comparison/conversion stays under this payload bound.
const MAX_ATOMIC_BYTES: usize = 4096;

pub(crate) struct PositiveSource<'source> {
    prepared: &'source Prepared,
    objectives: bool,
}

impl<'source> PositiveSource<'source> {
    pub(crate) fn check(
        prepared: &'source Prepared,
        components: Option<zetesis_core::TemplateComponentsRef<'_>>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<Self>, FormulaFailure> {
        applicable(prepared, components, limits, counters, location)
    }

    pub(crate) fn prepared(&self) -> &'source Prepared {
        self.prepared
    }

    pub(crate) fn has_objectives(&self) -> bool {
        self.objectives
    }

    pub(crate) fn contains(&self, index: usize, rule: &RuleIr) -> bool {
        self.prepared
            .rules
            .get(index)
            .is_some_and(|original| std::ptr::eq(original, rule))
    }
}

fn applicable<'source>(
    prepared: &'source Prepared,
    components: Option<zetesis_core::TemplateComponentsRef<'_>>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Option<PositiveSource<'source>>, FormulaFailure> {
    counters.work(limits, location)?;
    if prepared.analysis_basis != AnalysisBasis::NormalizedProgram {
        return Ok(None);
    }
    let mut objectives =
        !prepared.objectives.is_empty() || !prepared.objective_declarations.is_empty();
    // Check every source carrier as well as every compiled rule. These lists
    // need not have equal lengths: source deduplication does not erase the IR's
    // original occurrence/provenance population.
    for part in prepared.analyzed.parts() {
        counters.work(limits, location)?;
        if part.key().name.as_str() != "base" || !part.key().formals.is_empty() {
            return Ok(None);
        }
        for statement in part.statements() {
            counters.work(limits, location)?;
            let rule = match statement.get() {
                Statement::Rule(rule) => rule,
                Statement::Optimize(_) | Statement::WeakConstraint(_) => {
                    objectives = true;
                    continue;
                }
                _ => return Ok(None),
            };
            match rule.head().get() {
                Head::Falsum => {}
                Head::Literal(literal) if source_atom(literal, limits, counters, location)? => {}
                _ => return Ok(None),
            }
            for element in rule.body().get().elements() {
                counters.work(limits, location)?;
                let BodyElement::Literal(literal) = element.get() else {
                    return Ok(None);
                };
                if literal.negation == DefaultNegation::None
                    && matches!(literal.inner, LiteralInner::Comparison(_))
                {
                    continue;
                }
                if !source_atom(literal, limits, counters, location)? {
                    return Ok(None);
                }
            }
        }
    }
    for rule in &prepared.rules {
        counters.work(limits, rule.location)?;
        if rule.bindings.is_some() || rule.body_variables != rule.variables {
            return Ok(None);
        }
        match &rule.head {
            HeadIr::Normal(None) => {}
            HeadIr::Normal(Some(head))
                if flat(
                    *head,
                    components,
                    rule.variables,
                    limits,
                    counters,
                    rule.location,
                )? => {}
            _ => return Ok(None),
        }
        for literal in &rule.body {
            counters.work(limits, rule.location)?;
            if matches!(literal, LiteralIr::Compare(..)) {
                continue;
            }
            let LiteralIr::Atom(DefaultNegation::None, atom) = literal else {
                return Ok(None);
            };
            if !flat(
                *atom,
                components,
                rule.variables,
                limits,
                counters,
                rule.location,
            )? {
                return Ok(None);
            }
        }
    }
    Ok(Some(PositiveSource {
        prepared,
        objectives,
    }))
}

fn source_atom(
    literal: &Literal,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    counters.work(limits, location)?;
    if literal.negation != DefaultNegation::None {
        return Ok(false);
    }
    let LiteralInner::Atom(atom) = &literal.inner else {
        return Ok(false);
    };
    let Arguments::Single(terms) = &atom.get().arguments else {
        return Ok(false);
    };
    for term in terms {
        counters.work(limits, location)?;
        match term {
            Term::Variable(Variable::Named(_)) => {}
            Term::Symbolic(symbol) if atomic(symbol) => {}
            _ => return Ok(false),
        }
    }
    Ok(true)
}

fn atomic(symbol: &Symbol) -> bool {
    match symbol {
        Symbol::Number(_) | Symbol::Infimum | Symbol::Supremum => true,
        Symbol::String(text) => text.len() <= MAX_ATOMIC_BYTES,
        Symbol::Function {
            name,
            arguments,
            sign: Sign::Positive,
        } => arguments.is_empty() && name.as_str().len() <= MAX_ATOMIC_BYTES,
        _ => false,
    }
}

fn flat(
    pattern: crate::formula_support::components::Pattern,
    components: Option<zetesis_core::TemplateComponentsRef<'_>>,
    variables: usize,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    let components =
        components.ok_or_else(|| crate::formula_support::components::missing(location))?;
    let pattern = pattern.get(components, limits, counters, location)?;
    let terms = pattern.terms();
    for index in 0..terms.len() {
        counters.work(limits, location)?;
        let term = terms.at(index).expect("checked pattern arity");
        match term {
            zetesis_core::TemplateTerm::Variable(slot) if slot < variables => {}
            zetesis_core::TemplateTerm::Constant(value) => match value.descriptor() {
                zetesis_core::ValueNodeRef::Number(_)
                | zetesis_core::ValueNodeRef::Infimum
                | zetesis_core::ValueNodeRef::Supremum => {}
                zetesis_core::ValueNodeRef::String(text)
                | zetesis_core::ValueNodeRef::Symbol(text)
                    if text.len() <= MAX_ATOMIC_BYTES => {}
                _ => return Ok(false),
            },
            zetesis_core::TemplateTerm::Variable(_) => return Ok(false),
        }
    }
    Ok(true)
}
