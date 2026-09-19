//! Opt-in scalar definitions and bounded fact expansion over original syntax.

use std::collections::{BTreeMap, BTreeSet};

use themelios_base::span::Location;
use themelios_program::program::{
    BodyElement, Const, Head, Literal, LiteralInner, Program as SourceProgram, Rule, Statement,
};
use themelios_program::provenance::{Origin, TransformTag, WithProvenance};
use themelios_program::raise::raise;
use themelios_program::symbol::{Sign, Symbol};
use themelios_program::term::{EvalError, Term, TermParts};
use themelios_program::transform::{Rewrite, rewrite};
use themelios_syntax::ast::{self, AstToken};
use themelios_syntax::parse::Parse;
use themelios_syntax::tree::AstNode;
use zetesis_core::{AdmissionLimits, Program};

use crate::diagnostic::unsupported;
use crate::expansion::{Budget, check};
use crate::{
    AdmissionFailure, AdmissionOptions, Admitted, ExpansionFailure, ExpansionLimits,
    ExpansionResource, ExpansionUsage, ParsedSource, ProfileFeature, SourceFailure, SourceMetadata,
    compile, fact_expansion, metadata, profile,
};

/// Admit a bounded extension of S0: unannotated acyclic scalar `#const`
/// definitions; checked ground scalar arithmetic in ordinary S0 terms; and
/// finite pools/intervals in ordinary fact heads. A bare undefined constant name
/// remains a symbolic value. Definition names do not rename predicates.
///
/// Intervals have scalar integer endpoints; arithmetic over a set, variable
/// arithmetic, and pools/intervals in non-fact rules remain unsupported. A
/// descending interval denotes no facts. Fact products are bounded before
/// allocation. The resulting core program uses the existing S0 semantics.
/// Original source bytes and parsed rule locations are preserved; no source text
/// is regenerated. Signed `#defined` signatures and signature/empty `#show`
/// are retained as metadata; display selection never changes model identity.
/// Includes require bundle admission and other directives remain unsupported.
///
/// # Errors
/// Refuses every parser/raiser diagnostic, unsupported construct, ambiguous or
/// cyclic definition, undefined/overflowing scalar operation, expansion limit,
/// or independent core admission error. No successful partial program is
/// returned. This partial profile refuses undefined arithmetic rather than
/// emulating clingo's warning-and-drop behavior.
pub fn admit_extended(
    text: String,
    options: AdmissionOptions,
    limits: ExpansionLimits,
) -> Result<Admitted, ExpansionFailure> {
    ParsedSource::new(text, options)?
        .admit_extended(limits)
        .map_err(SourceFailure::into_error)
}

/// One admission's compiled program with the evidence its boundary retains.
pub(crate) struct Compilation {
    pub(crate) program: Program,
    pub(crate) template_origins: Vec<Vec<Location>>,
    pub(crate) metadata: SourceMetadata,
    pub(crate) expansion: ExpansionUsage,
}

pub(crate) fn admit_parsed(
    source: ParsedSource,
    limits: ExpansionLimits,
) -> Result<Admitted, SourceFailure<ExpansionFailure>> {
    match compile_parsed(&source, limits) {
        Ok(compiled) => Ok(Admitted {
            program: compiled.program,
            source: source.into_source(),
            template_origins: compiled.template_origins,
            metadata: compiled.metadata,
            expansion: compiled.expansion,
        }),
        Err(error) => Err(SourceFailure::new(source, error)),
    }
}

fn compile_parsed(
    source: &ParsedSource,
    limits: ExpansionLimits,
) -> Result<Compilation, ExpansionFailure> {
    let parsed = source.parsed();
    let options = source.options();
    profile::check_extended(parsed, options)?;
    check_definitions(parsed, limits)?;
    metadata::check_count(parsed, limits, &mut 0)?;
    let raised = raise(parsed);
    if !raised.diagnostics().is_empty() {
        return Err(AdmissionFailure::Raise(raised.diagnostics().to_vec()).into());
    }
    let mut source_metadata = metadata::Builder::default();
    metadata::collect(raised.program(), &mut source_metadata)?;
    let location = Location {
        source: source.source().id(),
        span: source.source().span(),
    };
    compile_owned(
        raised.program(),
        options.core_limits,
        limits,
        location,
        source_metadata.finish(),
    )
}

/// Compile the source into the program with its template origins and the
/// expansion charges it accepted, as one compilation with `metadata`.
pub(crate) fn compile_owned(
    source: &SourceProgram,
    core_limits: AdmissionLimits,
    limits: ExpansionLimits,
    location: Location,
    metadata: SourceMetadata,
) -> Result<Compilation, ExpansionFailure> {
    let mut budget = Budget::new(limits, core_limits.max_templates);
    let constants = resolve(source, &mut budget, location)?;
    let (mut templates, mut template_origins) =
        compile_extended(source, &constants, &mut budget, location)?;
    crate::coherence::append(
        &mut templates,
        &mut template_origins,
        core_limits,
        location,
        |resource, count, origin| budget.charge(resource, count, origin),
    )?;
    let program = Program::new(templates, core_limits).map_err(|error| {
        let location = error
            .template_index()
            .and_then(|index| template_origins.get(index))
            .and_then(|origins| origins.first())
            .copied()
            .unwrap_or(location);
        AdmissionFailure::Core { error, location }
    })?;
    Ok(Compilation {
        program,
        template_origins,
        metadata,
        expansion: budget.usage(),
    })
}

fn check_definitions(
    parsed: &Parse<ast::Program>,
    limits: ExpansionLimits,
) -> Result<(), ExpansionFailure> {
    let mut names = BTreeMap::new();
    check_definitions_in(parsed, limits, &mut names)
}

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
            return Err(ExpansionFailure::ConstantPolicy { location });
        }
        let Some(name) = constant.name() else {
            return Err(unsupported(ProfileFeature::Statement, location).into());
        };
        if let Some(first) = names.insert(name.text().to_owned(), location) {
            return Err(ExpansionFailure::DuplicateConstant {
                name: name.text().to_owned(),
                first,
                duplicate: location,
            });
        }
        check(
            ExpansionResource::Constants,
            names.len() as u128,
            limits.max_constants,
            location,
        )?;
    }
    Ok(())
}

struct Definition<'a> {
    constant: &'a Const,
    location: Location,
    dependencies: Vec<String>,
}

pub(crate) fn resolve(
    source: &SourceProgram,
    budget: &mut Budget,
    fallback: Location,
) -> Result<BTreeMap<String, Symbol>, ExpansionFailure> {
    let mut definitions = BTreeMap::new();
    for carrier in source.statements() {
        if let Statement::Const(constant) = carrier.get() {
            definitions.insert(
                constant.name.as_str().to_owned(),
                Definition {
                    constant,
                    location: origin(carrier, fallback),
                    dependencies: Vec::new(),
                },
            );
        }
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

type Compiled = (Vec<zetesis_core::Template>, Vec<Vec<Location>>);

/// Normalize each statement and compile it. A statement whose every term
/// normalization would return as it is, charged and validated by
/// [`kept_as_is`], is compiled from the original carrier: the rewrite would
/// hand back the same statement, stamped with a transformation tag that
/// nothing after this point reads, at the cost of copying and rebuilding it.
/// Every other statement is rebuilt by the rewrite, alone in a program of
/// its own so that no two statements merge.
fn compile_extended(
    source: &SourceProgram,
    constants: &BTreeMap<String, Symbol>,
    budget: &mut Budget,
    fallback: Location,
) -> Result<Compiled, ExpansionFailure> {
    let mut compiled = (Vec::new(), Vec::new());
    for carrier in source.statements() {
        if matches!(
            carrier.get(),
            Statement::Const(_) | Statement::Defined(_) | Statement::Show(_)
        ) {
            continue;
        }
        let location = origin(carrier, fallback);
        let locations = parsed_origins(carrier);
        if kept_as_is(carrier.get(), constants, budget, location)? {
            emit(carrier, locations, budget, location, &mut compiled)?;
            continue;
        }
        let mut normalizer = Normalizer {
            constants,
            budget,
            location,
            failure: None,
        };
        let rewritten = rewrite(SourceProgram::of_nodes([carrier.clone()]), &mut normalizer);
        if let Some(failure) = normalizer.failure {
            return Err(failure);
        }
        let normalized_carrier = rewritten
            .statements()
            .next()
            .expect("a rule rewrite keeps its statement");
        emit(
            normalized_carrier,
            locations,
            budget,
            location,
            &mut compiled,
        )?;
    }
    Ok(compiled)
}

/// Emit one normalized statement's templates, each with the statement's
/// parsed origins: its facts, expanded, or the one template of its rule.
fn emit(
    statement: &WithProvenance<Statement>,
    locations: Vec<Location>,
    budget: &mut Budget,
    location: Location,
    (templates, origins): &mut Compiled,
) -> Result<(), ExpansionFailure> {
    if let Some(facts) = fact_expansion::facts(statement, budget, location)? {
        budget.charge(
            ExpansionResource::Origins,
            (facts.len() as u128).saturating_mul(locations.len() as u128),
            location,
        )?;
        for fact in facts {
            templates.push(fact);
            origins.push(locations.clone());
        }
    } else {
        budget.charge(ExpansionResource::Templates, 1, location)?;
        budget.charge(
            ExpansionResource::Origins,
            locations.len() as u128,
            location,
        )?;
        templates.push(compile::statement(statement, location)?);
        origins.push(locations);
    }
    Ok(())
}

/// Whether normalization would return `statement` exactly as it is. Its
/// terms are charged and validated as the rewrite would charge and validate
/// them, in the rewrite's order, on a tentative budget that becomes the
/// budget only when every term is a [`Leaf`]; otherwise the budget is left
/// as it was, and the rewrite charges the statement from the start. A
/// failure is the one the rewrite would raise first, since it arises before
/// any term the rewrite would rebuild. Nothing is cloned but the budget.
fn kept_as_is(
    statement: &Statement,
    constants: &BTreeMap<String, Symbol>,
    budget: &mut Budget,
    location: Location,
) -> Result<bool, ExpansionFailure> {
    let Statement::Rule(rule) = statement else {
        return Ok(false);
    };
    let mut tentative = budget.clone();
    let kept = every_term(rule, &mut |term| match Leaf::of(term, constants) {
        Some(leaf) => leaf.account(&mut tentative, location).map(|()| true),
        None => Ok(false),
    })?;
    if kept {
        *budget = tentative;
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
/// by [`Leaf::account`]: a variable, or an argument-free scalar naming no
/// definition. Every other term is rebuilt, or, for a pool or interval
/// former, kept around terms that may not be.
#[derive(Clone, Copy)]
enum Leaf<'a> {
    Variable,
    Scalar(&'a Symbol),
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
            _ => None,
        }
    }

    /// Charge and validate as [`normalize_node`] does for this term: its unit
    /// of term work first, then a scalar's validation and payload.
    fn account(self, budget: &mut Budget, location: Location) -> Result<(), ExpansionFailure> {
        budget.charge(ExpansionResource::TermWork, 1, location)?;
        match self {
            Self::Variable => Ok(()),
            Self::Scalar(symbol) => scalar_leaf(symbol, budget, location),
        }
    }
}

/// The value an argument-free scalar names, when it names a definition.
fn defined<'a>(symbol: &Symbol, constants: &'a BTreeMap<String, Symbol>) -> Option<&'a Symbol> {
    constant_name(symbol).and_then(|name| constants.get(name))
}

/// Validate an argument-free scalar normalization keeps and charge its payload.
fn scalar_leaf(
    symbol: &Symbol,
    budget: &mut Budget,
    location: Location,
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

pub(crate) fn origin(carrier: &WithProvenance<Statement>, fallback: Location) -> Location {
    carrier
        .provenance()
        .origins()
        .find_map(|origin| match origin {
            Origin::Parsed(location) => Some(*location),
            Origin::Constructed | Origin::Transformed(_) => None,
        })
        .unwrap_or(fallback)
}

struct Normalizer<'a> {
    constants: &'a BTreeMap<String, Symbol>,
    budget: &'a mut Budget,
    location: Location,
    failure: Option<ExpansionFailure>,
}

impl Rewrite for Normalizer<'_> {
    fn tag(&self) -> TransformTag {
        TransformTag::new("zetesis-scalar-expansion")
    }
    fn rewrite_term(&mut self, term: Term) -> Term {
        if self.failure.is_some() {
            return term;
        }
        match normalize_node(term, self.constants, self.budget, self.location) {
            Ok(term) => term,
            Err(error) => {
                self.failure = Some(error);
                // Only a failed private intermediate uses this placeholder;
                // the entire rewrite is refused before compilation.
                Term::Symbolic(Symbol::Number(0))
            }
        }
    }
}

pub(crate) fn normalize_node(
    term: Term,
    constants: &BTreeMap<String, Symbol>,
    budget: &mut Budget,
    location: Location,
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
