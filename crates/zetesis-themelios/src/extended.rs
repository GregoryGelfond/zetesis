//! Checked scalar definitions and normalization over original syntax and logical values.

use std::collections::{BTreeMap, BTreeSet};

use themelios_base::span::Location;
use themelios_program::program::{
    BodyElement, Const, Head, Literal, LiteralInner, Program as SourceProgram, Rule, Statement,
};
use themelios_program::provenance::{Origin, WithProvenance};
use themelios_program::symbol::{Sign, Symbol};
use themelios_program::term::{EvalError, Term, TermParts};
use themelios_syntax::ast::{self, AstToken};
use themelios_syntax::parse::Parse;
use themelios_syntax::tree::AstNode;

use crate::diagnostic::unsupported;
use crate::expansion::{Budget, check};
use crate::{
    ExpansionFailure, ExpansionLimits, ExpansionResource, ProfileFeature, ProgramSite, StatementId,
    compile,
};

pub(crate) fn check_definitions_in(
    parsed: &Parse<ast::Program>,
    limits: ExpansionLimits,
    names: &mut BTreeMap<String, Location>,
) -> Result<(), ExpansionFailure> {
    for statement in parsed.tree().statements() {
        let ast::Statement::Const(constant) = statement else {
            continue;
        };
        let location = parsed.location(constant.syntax().text_range());
        if constant.annotation().is_some() {
            return Err(ExpansionFailure::ConstantPolicy {
                location: location.into(),
            });
        }
        let Some(name) = constant.name() else {
            return Err(unsupported(ProfileFeature::Statement, location).into());
        };
        if let Some(first) = names.insert(name.text().to_owned(), location) {
            return Err(ExpansionFailure::DuplicateConstant {
                name: name.text().to_owned(),
                first: first.into(),
                duplicate: location.into(),
            });
        }
        check(
            ExpansionResource::Constants,
            names.len() as u128,
            limits.max_constants,
            location.into(),
        )?;
    }
    Ok(())
}

struct Definition<'a> {
    constant: &'a Const,
    location: ProgramSite,
    dependencies: Vec<String>,
}

pub(crate) fn resolve(
    source: &SourceProgram,
    budget: &mut Budget,
    fallback: ProgramSite,
) -> Result<BTreeMap<String, Symbol>, ExpansionFailure> {
    let mut definitions: BTreeMap<String, Definition<'_>> = BTreeMap::new();
    for (index, carrier) in source.statements().enumerate() {
        let location = origin(carrier, fallback.with_statement(StatementId::new(index)));
        budget.poll(location)?;
        let Statement::Const(constant) = carrier.get() else {
            continue;
        };
        if constant.policy.is_some() {
            return Err(ExpansionFailure::ConstantPolicy { location });
        }
        // Canonical declarations are identified by their content. Additional
        // provenance records evidence, not another definition of the name.
        if let Some(first) = definitions
            .get(constant.name.as_str())
            .map(|entry| entry.location)
        {
            return Err(ExpansionFailure::DuplicateConstant {
                name: constant.name.as_str().to_owned(),
                first,
                duplicate: location,
            });
        }
        budget.check_constants(definitions.len().saturating_add(1), location)?;
        definitions.insert(
            constant.name.as_str().to_owned(),
            Definition {
                constant,
                location,
                dependencies: Vec::new(),
            },
        );
    }
    let names: BTreeSet<_> = definitions.keys().cloned().collect();
    for definition in definitions.values_mut() {
        let mut dependencies = BTreeSet::new();
        for term in definition.constant.value.subterms() {
            budget.charge(ExpansionResource::TermWork, 1, definition.location)?;
            if let Term::Symbolic(symbol) = term {
                for nested in symbol.subsymbols() {
                    if !symbol.arguments().is_empty() {
                        budget.charge(ExpansionResource::TermWork, 1, definition.location)?;
                    }
                    if let Some(name) = constant_name(nested).filter(|name| names.contains(*name)) {
                        dependencies.insert(name.to_owned());
                    }
                }
            }
        }
        definition.dependencies = dependencies.into_iter().collect();
    }
    let mut values = BTreeMap::new();
    let mut active = BTreeSet::new();
    for name in definitions.keys() {
        if values.contains_key(name) {
            continue;
        }
        active.insert(name.clone());
        let mut stack = vec![(name.clone(), 0)];
        while let Some((name, next)) = stack.last_mut() {
            let definition = &definitions[name];
            budget.poll(definition.location)?;
            if let Some(dependency) = definition.dependencies.get(*next) {
                *next += 1;
                if values.contains_key(dependency) {
                    continue;
                }
                if active.contains(dependency) {
                    let mut names: Vec<_> = stack.iter().map(|(name, _)| name.clone()).collect();
                    names.push(dependency.clone());
                    return Err(ExpansionFailure::ConstantCycle {
                        names,
                        location: definition.location,
                    });
                }
                active.insert(dependency.clone());
                stack.push((dependency.clone(), 0));
                continue;
            }
            let value = definition.constant.value.clone().try_fold(|parts| {
                normalize_node(Term::from(parts), &values, budget, definition.location)
            })?;
            let TermParts::Symbolic(value) = value.into_parts() else {
                return Err(ExpansionFailure::Evaluation {
                    error: EvalError::Undefined,
                    location: definition.location,
                });
            };
            values.insert(name.clone(), value);
            active.remove(name);
            stack.pop();
        }
    }
    Ok(values)
}

/// The normalization contract a kept formula statement must match. Sentinels
/// `#inf` and `#sup` consume one unit of term work without scalar payload work.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Normalization {
    Formula,
}

/// Whether `normalization` would return `statement` exactly as it is. Its
/// terms are charged and validated as the rewrite would charge and validate
/// them, in the rewrite's order, on a tentative budget that becomes the
/// budget only when every term is a [`Leaf`]; otherwise the budget is left
/// as it was, and the rewrite charges the statement from the start. A
/// failure is the one the rewrite would raise first, since it arises before
/// any term the rewrite would rebuild. Nothing is cloned but the budget.
pub(crate) fn kept_as_is(
    statement: &Statement,
    constants: &BTreeMap<String, Symbol>,
    normalization: Normalization,
    budget: &mut Budget,
    location: ProgramSite,
) -> Result<bool, ExpansionFailure> {
    let Statement::Rule(rule) = statement else {
        return Ok(false);
    };
    #[cfg(test)]
    if testing::rewrites_every_statement() {
        return Ok(false);
    }
    let mut tentative = budget.clone();
    let kept = every_term(rule, &mut |term| match Leaf::of(term, constants) {
        Some(leaf) => leaf
            .account(normalization, &mut tentative, location)
            .map(|()| true),
        None => Ok(false),
    })?;
    if kept {
        *budget = tentative;
        #[cfg(test)]
        testing::note_kept();
    }
    Ok(kept)
}

/// Apply `keep` to the terms of a rule of the shape this profile compiles,
/// in the order the rewrite rebuilds them: the head, then each body element;
/// an atom's arguments in order, a comparison's first term and then its
/// steps. `Ok(false)` at the first term `keep` refuses, or at once for a
/// head or body element of another shape, which the rewrite descends in an
/// order this walk does not reproduce.
fn every_term(
    rule: &Rule,
    keep: &mut impl FnMut(&Term) -> Result<bool, ExpansionFailure>,
) -> Result<bool, ExpansionFailure> {
    // A head or body element of any other shape, including one the pinned
    // themelios tier may add, is left to the rewrite.
    let head = match rule.head().get() {
        Head::Literal(literal) => literal_terms(literal, keep)?,
        Head::Falsum | Head::Verum => true,
        _ => false,
    };
    if !head {
        return Ok(false);
    }
    for element in rule.body().get().elements() {
        let kept = match element.get() {
            BodyElement::Literal(literal) => literal_terms(literal, keep)?,
            _ => false,
        };
        if !kept {
            return Ok(false);
        }
    }
    Ok(true)
}

fn literal_terms(
    literal: &Literal,
    keep: &mut impl FnMut(&Term) -> Result<bool, ExpansionFailure>,
) -> Result<bool, ExpansionFailure> {
    match &literal.inner {
        LiteralInner::Atom(atom) => {
            for term in atom.get().argument_terms() {
                if !keep(term)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        LiteralInner::Comparison(comparison) => {
            let comparison = comparison.get();
            if !keep(comparison.first())? {
                return Ok(false);
            }
            for (_, term) in comparison.steps() {
                if !keep(term)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        LiteralInner::True | LiteralInner::False => Ok(true),
    }
}

/// A term [`normalize_node`] returns exactly as it is, charged and validated
/// by [`Leaf::account`]: a variable, an argument-free scalar naming no
/// definition, or a ground symbol with arguments none of whose parts names a
/// definition. Raise yields such a symbol in its canonical form, and the
/// normalizer's symbolic branch folds it back to itself. Every other term is
/// rebuilt, or, for a pool or interval former, kept around terms that may
/// not be.
#[derive(Clone, Copy)]
enum Leaf<'a> {
    Variable,
    Scalar(&'a Symbol),
    Compound(&'a Symbol),
}

impl<'a> Leaf<'a> {
    fn of(term: &'a Term, constants: &BTreeMap<String, Symbol>) -> Option<Self> {
        match term {
            Term::Variable(_) => Some(Self::Variable),
            Term::Symbolic(symbol)
                if symbol.arguments().is_empty() && defined(symbol, constants).is_none() =>
            {
                Some(Self::Scalar(symbol))
            }
            Term::Symbolic(symbol)
                if !symbol.arguments().is_empty()
                    && symbol
                        .subsymbols()
                        .all(|part| defined(part, constants).is_none()) =>
            {
                Some(Self::Compound(symbol))
            }
            _ => None,
        }
    }

    /// Charge and validate as `normalization` does for this term: its unit of
    /// term work first; then a scalar's validation and payload, except a
    /// sentinel the formula route keeps; or a compound symbol's payload, one
    /// unit for each of its parts as the fold visits them, and its validation.
    fn account(
        self,
        normalization: Normalization,
        budget: &mut Budget,
        location: ProgramSite,
    ) -> Result<(), ExpansionFailure> {
        budget.charge(ExpansionResource::TermWork, 1, location)?;
        match self {
            Self::Variable => Ok(()),
            Self::Scalar(Symbol::Infimum | Symbol::Supremum)
                if normalization == Normalization::Formula =>
            {
                Ok(())
            }
            Self::Scalar(symbol) => scalar_leaf(symbol, budget, location),
            Self::Compound(symbol) => {
                budget.charge(
                    ExpansionResource::ScalarBytes,
                    symbol_bytes(symbol),
                    location,
                )?;
                for _ in symbol.subsymbols() {
                    budget.charge(ExpansionResource::TermWork, 1, location)?;
                }
                compile::validate_scalar(symbol, location)?;
                Ok(())
            }
        }
    }
}

/// A switch for tests that compare a kept statement with its rewrite: while it
/// is set on a thread, [`kept_as_is`] keeps nothing there. The statements kept
/// on a thread are counted, so a test can assert which path it took.
#[cfg(test)]
pub(crate) mod testing {
    use std::cell::Cell;

    thread_local! {
        static REWRITE_EVERY_STATEMENT: Cell<bool> = const { Cell::new(false) };
        static KEPT: Cell<usize> = const { Cell::new(0) };
    }

    pub(crate) fn rewrites_every_statement() -> bool {
        REWRITE_EVERY_STATEMENT.with(Cell::get)
    }

    pub(super) fn note_kept() {
        KEPT.with(|kept| kept.set(kept.get() + 1));
    }

    /// The statements kept on this thread so far.
    pub(crate) fn kept_statements() -> usize {
        KEPT.with(Cell::get)
    }

    /// Run `body` with every statement rewritten on this thread.
    pub(crate) fn with_every_statement_rewritten<T>(body: impl FnOnce() -> T) -> T {
        let previous = REWRITE_EVERY_STATEMENT.with(|rewrite| rewrite.replace(true));
        let result = body();
        REWRITE_EVERY_STATEMENT.with(|rewrite| rewrite.set(previous));
        result
    }
}

#[cfg(test)]
mod kept_tests;

/// The value an argument-free scalar names, when it names a definition.
fn defined<'a>(symbol: &Symbol, constants: &'a BTreeMap<String, Symbol>) -> Option<&'a Symbol> {
    constant_name(symbol).and_then(|name| constants.get(name))
}

/// Validate an argument-free scalar normalization keeps and charge its payload.
fn scalar_leaf(
    symbol: &Symbol,
    budget: &mut Budget,
    location: ProgramSite,
) -> Result<(), ExpansionFailure> {
    compile::validate_scalar(symbol, location)?;
    budget.charge(
        ExpansionResource::ScalarBytes,
        symbol_bytes(symbol),
        location,
    )?;
    Ok(())
}

pub(crate) fn parsed_origins(carrier: &WithProvenance<Statement>) -> Vec<Location> {
    carrier
        .provenance()
        .origins()
        .filter_map(|origin| match origin {
            Origin::Parsed(location) => Some(*location),
            Origin::Constructed | Origin::Transformed(_) => None,
        })
        .collect()
}

/// Statement evidence retains logical identity independently of parsed spans.
/// Constructed input contributes its caller-assigned site rather than a fabricated location.
pub(crate) fn program_sites(
    carrier: &WithProvenance<Statement>,
    fallback: ProgramSite,
) -> Vec<ProgramSite> {
    let mut sites: Vec<_> = carrier
        .provenance()
        .origins()
        .filter_map(|origin| match origin {
            Origin::Parsed(location) => Some(fallback.with_location(*location)),
            Origin::Constructed | Origin::Transformed(_) => None,
        })
        .collect();
    if sites.is_empty() {
        sites.push(fallback);
    }
    sites
}

/// Attach actual parsed evidence without changing a caller-assigned statement ID.
pub(crate) fn origin(
    carrier: &WithProvenance<Statement>,
    fallback: impl Into<ProgramSite>,
) -> ProgramSite {
    let fallback = fallback.into();
    carrier
        .provenance()
        .origins()
        .find_map(|origin| match origin {
            Origin::Parsed(location) => Some(fallback.with_location(*location)),
            Origin::Constructed | Origin::Transformed(_) => None,
        })
        .unwrap_or(fallback)
}

pub(crate) fn normalize_node(
    term: Term,
    constants: &BTreeMap<String, Symbol>,
    budget: &mut Budget,
    location: ProgramSite,
) -> Result<Term, ExpansionFailure> {
    budget.charge(ExpansionResource::TermWork, 1, location)?;
    match &term {
        Term::Variable(_) | Term::Pool(_) | Term::Interval { .. } => Ok(term),
        Term::Symbolic(symbol) if !symbol.arguments().is_empty() => {
            budget.charge(
                ExpansionResource::ScalarBytes,
                symbol_bytes(symbol),
                location,
            )?;
            let resolved =
                symbol
                    .clone()
                    .try_fold(|parts| -> Result<Symbol, ExpansionFailure> {
                        budget.charge(ExpansionResource::TermWork, 1, location)?;
                        let value = Symbol::from(parts);
                        if let Some(replacement) = defined(&value, constants) {
                            budget.charge(
                                ExpansionResource::ScalarBytes,
                                symbol_bytes(replacement),
                                location,
                            )?;
                            Ok(replacement.clone())
                        } else {
                            Ok(value)
                        }
                    })?;
            compile::validate_scalar(&resolved, location)?;
            Ok(Term::Symbolic(resolved))
        }
        Term::Symbolic(symbol) => {
            if let Some(value) = defined(symbol, constants) {
                budget.charge(
                    ExpansionResource::ScalarBytes,
                    symbol_bytes(value),
                    location,
                )?;
                Ok(Term::Symbolic(value.clone()))
            } else {
                scalar_leaf(symbol, budget, location)?;
                Ok(term)
            }
        }
        Term::UnaryOperation {
            operator: themelios_program::term::UnaryOp::Negate,
            argument,
        } if matches!(argument.as_ref(), Term::Symbolic(Symbol::Function { .. })) => {
            let Term::Symbolic(Symbol::Function {
                name,
                arguments,
                sign,
            }) = argument.as_ref()
            else {
                unreachable!("checked function operand")
            };
            budget.charge(
                ExpansionResource::ScalarBytes,
                symbol_bytes(match argument.as_ref() {
                    Term::Symbolic(symbol) => symbol,
                    _ => unreachable!("checked symbol"),
                }),
                location,
            )?;
            Ok(Term::Symbolic(Symbol::Function {
                name: name.clone(),
                arguments: arguments.clone(),
                sign: match sign {
                    Sign::Positive => Sign::Negative,
                    Sign::Negative => Sign::Positive,
                },
            }))
        }
        Term::Function { .. } | Term::Tuple(_) if term.is_ground() => {
            for child in term.subterms() {
                budget.charge(ExpansionResource::TermWork, 1, location)?;
                if matches!(child, Term::Pool(_) | Term::Interval { .. }) {
                    return Err(unsupported(ProfileFeature::Term, location).into());
                }
            }
            let symbol = term
                .evaluate()
                .map_err(|error| ExpansionFailure::Evaluation { error, location })?;
            compile::validate_scalar(&symbol, location)?;
            Ok(Term::Symbolic(symbol))
        }
        Term::UnaryOperation { .. } | Term::BinaryOperation { .. } | Term::Absolute(_) => {
            let symbol = term
                .evaluate()
                .map_err(|error| ExpansionFailure::Evaluation { error, location })?;
            compile::validate_scalar(&symbol, location)?;
            Ok(Term::Symbolic(symbol))
        }
        _ => Err(unsupported(ProfileFeature::Term, location).into()),
    }
}

fn constant_name(symbol: &Symbol) -> Option<&str> {
    match symbol {
        Symbol::Function {
            name,
            arguments,
            sign: Sign::Positive,
        } if arguments.is_empty() => Some(name.as_str()),
        _ => None,
    }
}

fn symbol_bytes(symbol: &Symbol) -> u128 {
    crate::structural_value::symbol_bytes(symbol)
}
