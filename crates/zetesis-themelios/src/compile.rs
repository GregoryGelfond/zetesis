//! Scalar compilation from the faithful owned program value.

use std::collections::BTreeMap;

use themelios_base::span::Location;
use themelios_program::program::{
    Arguments, Atom, BodyElement, DefaultNegation, Head, Literal, LiteralInner,
    Program as SourceProgram, Relation, Rule, Statement,
};
use themelios_program::provenance::{Origin, WithProvenance};
use themelios_program::symbol::{Sign, Symbol};
use themelios_program::term::{Term as SourceTerm, UnaryOp, Variable};
use zetesis_core::{AtomPattern, Filter, Predicate, Template, Term, Value};

use crate::diagnostic::unsupported;
use crate::{AdmissionFailure, ProfileFeature};

type CompiledProgram = (Vec<Template>, Vec<Vec<Location>>);

pub(crate) fn program(
    source: &SourceProgram,
    fallback: Location,
) -> Result<CompiledProgram, AdmissionFailure> {
    let mut templates = Vec::new();
    let mut origins = Vec::new();
    for part in source.parts() {
        if part.key().name.as_str() != "base" || !part.key().formals.is_empty() {
            return Err(unsupported(ProfileFeature::ProgramPart, fallback));
        }
        for carrier in part.statements() {
            let locations: Vec<_> = carrier
                .provenance()
                .origins()
                .filter_map(|origin| match origin {
                    Origin::Parsed(location) => Some(*location),
                    Origin::Constructed | Origin::Transformed(_) => None,
                })
                .collect();
            let location = locations.first().copied().unwrap_or(fallback);
            templates.push(statement(carrier, location)?);
            origins.push(locations);
        }
    }
    Ok((templates, origins))
}

/// Compile the rule one statement must be; `location` locates its refusal.
pub(crate) fn statement(
    carrier: &WithProvenance<Statement>,
    location: Location,
) -> Result<Template, AdmissionFailure> {
    let Statement::Rule(source_rule) = carrier.get() else {
        return Err(unsupported(ProfileFeature::Statement, location));
    };
    rule(source_rule, location)
}

#[derive(Default)]
struct Variables {
    named: BTreeMap<String, usize>,
    next: usize,
}

impl Variables {
    fn slot(&mut self, variable: &Variable) -> usize {
        if let Variable::Named(name) = variable {
            if let Some(index) = self.named.get(name.as_str()) {
                return *index;
            }
            let index = self.fresh();
            self.named.insert(name.as_str().to_owned(), index);
            index
        } else {
            self.fresh()
        }
    }

    fn fresh(&mut self) -> usize {
        let index = self.next;
        self.next += 1;
        index
    }
}

fn rule(source: &Rule, location: Location) -> Result<Template, AdmissionFailure> {
    let mut variables = Variables::default();
    let (head, choice) = head(source.head().get(), &mut variables, location)?;
    let mut positive = Vec::new();
    let mut gate_true = Vec::new();
    let mut gate_false = Vec::new();
    let mut filters = Vec::new();
    for element in source.body().get().elements() {
        let BodyElement::Literal(literal) = element.get() else {
            return Err(unsupported(ProfileFeature::BodyElement, location));
        };
        match &literal.inner {
            LiteralInner::Atom(value) => {
                if literal.negation != DefaultNegation::None
                    && let Arguments::Single(arguments) = &value.get().arguments
                    && arguments.iter().any(|argument| {
                        matches!(argument, SourceTerm::Variable(Variable::Anonymous))
                    })
                {
                    // A negative anonymous position tests the complete witness
                    // projection. It is not an unbound relational gate variable.
                    return Err(unsupported(ProfileFeature::AnonymousProjection, location));
                }
                let pattern = atom(value.get(), &mut variables, location)?;
                match literal.negation {
                    DefaultNegation::None => positive.push(pattern),
                    DefaultNegation::Not => gate_false.push(pattern),
                    DefaultNegation::NotNot => gate_true.push(pattern),
                }
            }
            LiteralInner::Comparison(value) => {
                if literal.negation != DefaultNegation::None {
                    return Err(unsupported(ProfileFeature::NegatedComparison, location));
                }
                let comparison = value.get();
                let mut steps = comparison.steps();
                let (relation, right) = steps.next().expect("a comparison has at least one step");
                if steps.next().is_some() {
                    return Err(unsupported(ProfileFeature::ComparisonChain, location));
                }
                let left = term(comparison.first(), &mut variables, location)?;
                let right = term(right, &mut variables, location)?;
                let filter = match relation {
                    Relation::Eq => Filter::Eq(left, right),
                    Relation::Neq => Filter::Neq(left, right),
                    _ => return Err(unsupported(ProfileFeature::ComparisonRelation, location)),
                };
                filters.push(filter);
            }
            LiteralInner::True | LiteralInner::False => {
                return Err(unsupported(ProfileFeature::BooleanLiteral, location));
            }
        }
    }
    if needs_binding_analysis(&positive, &filters) {
        return Err(unsupported(ProfileFeature::ScalarBinding, location));
    }
    if choice {
        gate_true.push(
            head.as_ref()
                .expect("an admitted choice has a head atom")
                .clone(),
        );
    }
    Ok(Template::new(
        head, positive, gate_true, gate_false, filters,
    ))
}

/// S0 comparisons only test relational bindings. Equality involving another
/// slot requires the formula profile's readiness and finite-binding analysis.
/// This classifies capability; it does not establish that the binding is safe.
fn needs_binding_analysis(positive: &[AtomPattern], filters: &[Filter]) -> bool {
    filters.iter().any(|filter| {
        let Filter::Eq(left, right) = filter else {
            return false;
        };
        [left, right].into_iter().any(|term| {
            matches!(term, Term::Variable(_))
                && !positive.iter().any(|atom| atom.terms().contains(term))
        })
    })
}

fn head(
    source: &Head,
    variables: &mut Variables,
    location: Location,
) -> Result<(Option<AtomPattern>, bool), AdmissionFailure> {
    match source {
        Head::Falsum => Ok((None, false)),
        Head::Literal(literal) => Ok((Some(head_atom(literal, variables, location)?), false)),
        Head::Choice(choice) => {
            if choice.left_guard().is_some() || choice.right_guard().is_some() {
                return Err(unsupported(ProfileFeature::BoundedChoice, location));
            }
            let mut elements = choice.elements();
            let Some(element) = elements.next() else {
                return Err(unsupported(ProfileFeature::ChoiceCardinality, location));
            };
            if elements.next().is_some() {
                return Err(unsupported(ProfileFeature::ChoiceCardinality, location));
            }
            if !element.get().condition().is_empty() {
                return Err(unsupported(ProfileFeature::ConditionalChoice, location));
            }
            Ok((
                Some(head_atom(element.get().literal(), variables, location)?),
                true,
            ))
        }
        _ => Err(unsupported(ProfileFeature::Head, location)),
    }
}

fn head_atom(
    literal: &Literal,
    variables: &mut Variables,
    location: Location,
) -> Result<AtomPattern, AdmissionFailure> {
    if literal.negation != DefaultNegation::None {
        return Err(unsupported(ProfileFeature::NegatedHead, location));
    }
    let LiteralInner::Atom(source) = &literal.inner else {
        return Err(unsupported(ProfileFeature::BooleanLiteral, location));
    };
    atom(source.get(), variables, location)
}

fn atom(
    source: &Atom,
    variables: &mut Variables,
    location: Location,
) -> Result<AtomPattern, AdmissionFailure> {
    let Arguments::Single(arguments) = &source.arguments else {
        return Err(unsupported(ProfileFeature::PooledArguments, location));
    };
    let predicate = Predicate::with_sign(
        source.name.as_str(),
        arguments.len(),
        crate::coherence::core_sign(source.sign),
    )
    .map_err(|error| AdmissionFailure::Construction { error, location })?;
    let terms = arguments
        .iter()
        .map(|argument| term(argument, variables, location))
        .collect::<Result<Vec<_>, _>>()?;
    AtomPattern::new(predicate, terms)
        .map_err(|error| AdmissionFailure::Construction { error, location })
}

fn term(
    source: &SourceTerm,
    variables: &mut Variables,
    location: Location,
) -> Result<Term, AdmissionFailure> {
    match source {
        SourceTerm::Variable(variable) => Ok(Term::Variable(variables.slot(variable))),
        SourceTerm::Symbolic(value) => scalar(value, location).map(Term::Constant),
        SourceTerm::Function { .. } | SourceTerm::Tuple(_) => {
            // Strict S0 accepts closed data, not new arithmetic or generators.
            if source.subterms().any(|node| {
                !matches!(
                    node,
                    SourceTerm::Symbolic(_) | SourceTerm::Function { .. } | SourceTerm::Tuple(_)
                )
            }) {
                return Err(unsupported(ProfileFeature::Term, location));
            }
            let value = source
                .evaluate()
                .map_err(|_| unsupported(ProfileFeature::Term, location))?;
            scalar(&value, location).map(Term::Constant)
        }
        SourceTerm::UnaryOperation {
            operator: UnaryOp::Negate,
            argument,
        } => {
            let value = argument
                .evaluate()
                .map_err(|_| unsupported(ProfileFeature::Term, location))?;
            match value.into_parts() {
                themelios_program::symbol::SymbolParts::Number(number) => number
                    .checked_neg()
                    .map(|number| Term::Constant(Value::Number(number)))
                    .ok_or_else(|| unsupported(ProfileFeature::NumericOverflow, location)),
                themelios_program::symbol::SymbolParts::Function {
                    name,
                    arguments,
                    sign,
                } => scalar(
                    &Symbol::Function {
                        name,
                        arguments,
                        sign: match sign {
                            Sign::Positive => Sign::Negative,
                            Sign::Negative => Sign::Positive,
                        },
                    },
                    location,
                )
                .map(Term::Constant),
                _ => Err(unsupported(ProfileFeature::Term, location)),
            }
        }
        _ => Err(unsupported(ProfileFeature::Term, location)),
    }
}

pub(crate) fn scalar(source: &Symbol, location: Location) -> Result<Value, AdmissionFailure> {
    Ok(match classify(source, location)? {
        Scalar::Number(number) => Value::Number(number),
        Scalar::String(text) => Value::String(text.into()),
        Scalar::Symbol(name) => Value::Symbol(name.into()),
        Scalar::Structural(symbol) => crate::structural_value::from_symbol(symbol)
            .map_err(|error| value_failure(error, location))?,
        Scalar::Infimum => Value::Infimum,
        Scalar::Supremum => Value::Supremum,
    })
}

/// Validate a source scalar without manufacturing an owned core value. NUL and
/// typed structure checks are shared with conversion. Logical node/depth/text
/// bounds remain active; actual construction capacity is checked only when a
/// value is constructed. No authored arithmetic evaluation is skipped.
pub(crate) fn validate_scalar(source: &Symbol, location: Location) -> Result<(), AdmissionFailure> {
    if let Scalar::Structural(symbol) = classify(source, location)? {
        crate::structural_value::validate_symbol(symbol)
            .map_err(|error| value_failure(error, location))?;
    }
    Ok(())
}

enum Scalar<'a> {
    Number(i32),
    String(&'a str),
    Symbol(&'a str),
    Structural(&'a Symbol),
    Infimum,
    Supremum,
}

fn classify(source: &Symbol, location: Location) -> Result<Scalar<'_>, AdmissionFailure> {
    match source {
        Symbol::Number(number) => Ok(Scalar::Number(*number)),
        Symbol::String(text) => {
            // clingo's string symbol cannot preserve an embedded NUL; refuse
            // this otherwise parseable value instead of silently truncating it.
            if text.contains('\0') {
                return Err(unsupported(ProfileFeature::NulString, location));
            }
            Ok(Scalar::String(text))
        }
        Symbol::Function {
            arguments,
            sign: Sign::Positive,
            name,
        } if arguments.is_empty() => Ok(Scalar::Symbol(name.as_str())),
        Symbol::Function { .. } | Symbol::Tuple(_) => {
            for child in source.subsymbols() {
                if matches!(child, Symbol::String(text) if text.contains('\0')) {
                    return Err(unsupported(ProfileFeature::NulString, location));
                }
            }
            Ok(Scalar::Structural(source))
        }
        Symbol::Infimum => Ok(Scalar::Infimum),
        Symbol::Supremum => Ok(Scalar::Supremum),
    }
}

fn value_failure(error: zetesis_core::ValueError, location: Location) -> AdmissionFailure {
    AdmissionFailure::Construction {
        error: zetesis_core::ConstructionError::Value(error),
        location,
    }
}
