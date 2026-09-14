//! Optional domains over exactly the normalized positive source and its flat IR.

use themelios_base::span::Location;
use themelios_program::program::{
    Arguments, BodyElement, DefaultNegation, Head, Literal, LiteralInner, Statement,
};
use themelios_program::symbol::{Sign, Symbol};
use themelios_program::term::{Term, Variable};
use zetesis_domain::{Analysis, Status};

use crate::formula_ir::{HeadIr, LiteralIr, Prepared, RuleIr};
use crate::formula_support::Counters;
use crate::grounding_observer::{Event, Profile};
use crate::{AnalysisBasis, DomainLimits, DomainObservation, FormulaFailure, FormulaLimits};

/// The atomic bridge bounds a single comparison/conversion's payload. Richer
/// values remain valid source inputs but do not use this optional consumer.
const MAX_ATOMIC_BYTES: usize = 4096;

/// Only this module constructs the certificate joining an eligible normalized
/// owner, its original rule occurrence array and a completed analysis.
pub(crate) struct Domains<'source> {
    analysis: Analysis<'source>,
    prepared: &'source Prepared,
}

impl Domains<'_> {
    pub(crate) fn for_rule(
        &self,
        index: usize,
        rule: &RuleIr,
    ) -> Result<&Analysis<'_>, FormulaFailure> {
        if self.analysis.belongs_to(&self.prepared.analyzed)
            && self
                .prepared
                .rules
                .get(index)
                .is_some_and(|original| std::ptr::eq(original, rule))
        {
            Ok(&self.analysis)
        } else {
            Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location: rule.location,
            })
        }
    }
}

pub(crate) fn analyze<'source>(
    prepared: &'source Prepared,
    options: Option<DomainLimits>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    profile: &Profile<'_>,
    location: Location,
) -> Result<Option<Domains<'source>>, FormulaFailure> {
    let Some(mut options) = options else {
        profile.domain_analysis(DomainObservation::Disabled);
        return Ok(None);
    };
    let before = counters.work;
    let eligible = applicable(prepared, limits, counters, location);
    counters.record(Event::DomainPrepareWork(counters.work - before));
    if !eligible? {
        profile.domain_analysis(DomainObservation::Inapplicable);
        return Ok(None);
    }
    // This API has no caller Control. Logical populations and remaining work
    // bound the uninterruptible call; no cancellation/deadline is invented.
    counters.charge_work(0, limits, location)?;
    options.max_work = options.max_work.min(limits.max_work - counters.work);
    let analysis = zetesis_domain::analyze(&prepared.analyzed, options);
    let work = analysis.statistics().work;
    counters.charge_work(u128::from(work), limits, location)?;
    counters.record(Event::DomainPrepareWork(work));
    profile.domain_analysis(DomainObservation::Analyzed(&analysis));
    Ok((analysis.status() == Status::FixedPoint).then_some(Domains { analysis, prepared }))
}

fn applicable(
    prepared: &Prepared,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<bool, FormulaFailure> {
    counters.work(limits, location)?;
    if prepared.analysis_basis != AnalysisBasis::NormalizedProgram
        || !prepared.objectives.is_empty()
    {
        return Ok(false);
    }
    // Check every source carrier as well as every compiled rule. These lists
    // need not have equal lengths: source deduplication does not erase the IR's
    // original occurrence/provenance population.
    for part in prepared.analyzed.parts() {
        counters.work(limits, location)?;
        if part.key().name.as_str() != "base" || !part.key().formals.is_empty() {
            return Ok(false);
        }
        for statement in part.statements() {
            counters.work(limits, location)?;
            let Statement::Rule(rule) = statement.get() else {
                return Ok(false);
            };
            match rule.head().get() {
                Head::Falsum => {}
                Head::Literal(literal) if source_atom(literal, limits, counters, location)? => {}
                _ => return Ok(false),
            }
            for element in rule.body().get().elements() {
                counters.work(limits, location)?;
                let BodyElement::Literal(literal) = element.get() else {
                    return Ok(false);
                };
                if !source_atom(literal, limits, counters, location)? {
                    return Ok(false);
                }
            }
        }
    }
    for rule in &prepared.rules {
        counters.work(limits, rule.location)?;
        if rule.bindings.is_some() || rule.body_variables != rule.variables {
            return Ok(false);
        }
        match &rule.head {
            HeadIr::Normal(None) => {}
            HeadIr::Normal(Some(head))
                if flat(head, rule.variables, limits, counters, rule.location)? => {}
            _ => return Ok(false),
        }
        for literal in &rule.body {
            counters.work(limits, rule.location)?;
            let LiteralIr::Atom(DefaultNegation::None, atom) = literal else {
                return Ok(false);
            };
            if !flat(atom, rule.variables, limits, counters, rule.location)? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn source_atom(
    literal: &Literal,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: Location,
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
    pattern: &zetesis_core::AtomPattern,
    variables: usize,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<bool, FormulaFailure> {
    for term in pattern.terms() {
        counters.work(limits, location)?;
        match term {
            zetesis_core::Term::Variable(slot) if *slot < variables => {}
            zetesis_core::Term::Constant(value) => match value {
                zetesis_core::Value::Number(_)
                | zetesis_core::Value::Infimum
                | zetesis_core::Value::Supremum => {}
                zetesis_core::Value::String(text) | zetesis_core::Value::Symbol(text)
                    if text.len() <= MAX_ATOMIC_BYTES => {}
                _ => return Ok(false),
            },
            _ => return Ok(false),
        }
    }
    Ok(true)
}
