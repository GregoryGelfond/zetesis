//! Directive-local compilation, independent of logical grounding and its carrier.

use std::collections::{BTreeMap, BTreeSet};

use themelios_base::span::Location;
use themelios_program::program::{Arguments, BodyElement, LiteralInner, Program, Show, Statement};
use themelios_program::symbol::{Name, Sign, Symbol};
use themelios_program::term::{Term, UnaryOp, Variable};

use super::{
    AdmissionLimits, Condition, DefaultNegation, Directive, Error, ErrorKind, Feature,
    ObservationProgram, Operand, Pattern, Predicate, Resource, Statistics, Template,
};
use crate::expansion::Budget;
use crate::{AdmissionOptions, FormulaFailure};

struct Compiler<'a> {
    limits: AdmissionLimits,
    constants: &'a BTreeMap<String, Symbol>,
    location: Location,
    nodes: usize,
    bytes: usize,
    origins: usize,
    variables: BTreeMap<String, usize>,
    safe: BTreeSet<usize>,
}
impl Compiler<'_> {
    fn error(&self, kind: ErrorKind) -> Error {
        Error {
            kind,
            location: Some(self.location),
            statistics: Statistics::default(),
        }
    }
    fn check(&self, resource: Resource, observed: usize, limit: usize) -> Result<(), Error> {
        if observed > limit {
            return Err(self.error(ErrorKind::Limit {
                resource,
                observed: observed as u128,
                limit: limit as u128,
            }));
        }
        Ok(())
    }
    fn unsupported(&self, feature: Feature) -> Error {
        self.error(ErrorKind::Unsupported(feature))
    }
    fn node(&mut self, depth: usize) -> Result<(), Error> {
        self.check(Resource::Depth, depth, self.limits.max_depth as usize)?;
        self.check(
            Resource::Nodes,
            self.nodes.saturating_add(1),
            self.limits.max_nodes as usize,
        )?;
        self.nodes += 1;
        Ok(())
    }
    fn text(&mut self, value: &str) -> Result<(), Error> {
        self.check(
            Resource::Bytes,
            self.bytes.saturating_add(value.len()),
            self.limits.max_bytes as usize,
        )?;
        self.bytes += value.len();
        if value.contains('\0') {
            return Err(self.unsupported(Feature::Term));
        }
        Ok(())
    }
    fn arity(&self, count: usize) -> Result<(), Error> {
        self.check(Resource::Arity, count, self.limits.max_arity as usize)
    }
    fn variable(&mut self, variable: &Variable) -> Result<usize, Error> {
        let Variable::Named(name) = variable else {
            return Err(self.unsupported(Feature::AnonymousOutput));
        };
        if let Some(&slot) = self.variables.get(name.as_str()) {
            return Ok(slot);
        }
        self.check(
            Resource::Variables,
            self.variables.len().saturating_add(1),
            self.limits.max_variables as usize,
        )?;
        self.text(name.as_str())?;
        let slot = self.variables.len();
        self.variables.insert(name.as_str().to_owned(), slot);
        Ok(slot)
    }
    // Recursion is capped before descent; source depth is also capped before raising.
    fn symbol(&mut self, value: &Symbol, depth: usize, resolve: bool) -> Result<Symbol, Error> {
        self.node(depth)?;
        if resolve
            && let Symbol::Function {
                name,
                arguments,
                sign: Sign::Positive,
            } = value
            && arguments.is_empty()
            && let Some(replacement) = self.constants.get(name.as_str())
        {
            return self.symbol(replacement, depth, false);
        }
        Ok(match value {
            Symbol::Infimum => Symbol::Infimum,
            Symbol::Supremum => Symbol::Supremum,
            Symbol::Number(value) => Symbol::Number(*value),
            Symbol::String(text) => {
                self.text(text)?;
                Symbol::String(text.clone())
            }
            Symbol::Function {
                name,
                arguments,
                sign,
            } => {
                self.text(name.as_str())?;
                self.arity(arguments.len())?;
                let mut values = Vec::new();
                for argument in arguments {
                    values.push(self.symbol(argument, depth + 1, resolve)?);
                }
                Symbol::Function {
                    name: name.clone(),
                    arguments: values,
                    sign: *sign,
                }
            }
            Symbol::Tuple(arguments) => {
                self.arity(arguments.len())?;
                let mut values = Vec::new();
                for argument in arguments {
                    values.push(self.symbol(argument, depth + 1, resolve)?);
                }
                Symbol::Tuple(values)
            }
        })
    }
    fn template(&mut self, term: &Term, depth: usize) -> Result<Template, Error> {
        self.node(depth)?;
        Ok(match term {
            Term::Symbolic(symbol) => Template::Value(self.symbol(symbol, depth, true)?),
            Term::Variable(variable) => Template::Variable(self.variable(variable)?),
            Term::Function { name, arguments } => {
                self.function(name, arguments, depth, Sign::Positive)?
            }
            Term::UnaryOperation {
                operator: UnaryOp::Negate,
                argument,
            } => match argument.as_ref() {
                Term::Function { name, arguments } => {
                    self.node(depth + 1)?;
                    self.function(name, arguments, depth + 1, Sign::Negative)?
                }
                Term::Symbolic(symbol) => {
                    let mut value = self.symbol(symbol, depth + 1, true)?;
                    let Symbol::Function { sign, .. } = &mut value else {
                        return Err(self.unsupported(Feature::Term));
                    };
                    *sign = match sign {
                        Sign::Positive => Sign::Negative,
                        Sign::Negative => Sign::Positive,
                    };
                    Template::Value(value)
                }
                _ => return Err(self.unsupported(Feature::Term)),
            },
            Term::Tuple(arguments) => {
                self.arity(arguments.len())?;
                let mut terms = Vec::new();
                for argument in arguments {
                    terms.push(self.template(argument, depth + 1)?);
                }
                Template::Tuple(terms)
            }
            _ => return Err(self.unsupported(Feature::Term)),
        })
    }
    fn function(
        &mut self,
        name: &Name,
        arguments: &[Term],
        depth: usize,
        sign: Sign,
    ) -> Result<Template, Error> {
        self.text(name.as_str())?;
        self.arity(arguments.len())?;
        let mut terms = Vec::new();
        for argument in arguments {
            terms.push(self.template(argument, depth + 1)?);
        }
        Ok(Template::Function(sign, name.clone(), terms))
    }
    fn operand(&mut self, term: &Term, anonymous: bool) -> Result<Operand, Error> {
        self.node(1)?;
        Ok(match term {
            Term::Variable(Variable::Anonymous) if anonymous => Operand::Any,
            Term::Variable(variable) => Operand::Variable(self.variable(variable)?),
            Term::Symbolic(symbol) => {
                let symbol = self.symbol(symbol, 1, true)?;
                Operand::Value(
                    crate::structural_value::from_symbol(&symbol)
                        .map_err(|_| self.unsupported(Feature::Comparison))?,
                )
            }
            _ => return Err(self.unsupported(Feature::Term)),
        })
    }
    fn pattern(
        &mut self,
        atom: &themelios_program::program::Atom,
        positive: bool,
    ) -> Result<Pattern, Error> {
        let Arguments::Single(arguments) = &atom.arguments else {
            return Err(self.unsupported(Feature::Atom));
        };
        self.text(atom.name.as_str())?;
        self.arity(arguments.len())?;
        let mut terms = Vec::new();
        for argument in arguments {
            if !positive
                && atom.sign == Sign::Negative
                && matches!(argument, Term::Variable(Variable::Anonymous))
            {
                return Err(self.unsupported(Feature::UnsafeVariable));
            }
            let term = self.operand(argument, true)?;
            if positive && let Operand::Variable(slot) = term {
                self.safe.insert(slot);
            }
            terms.push(term);
        }
        let predicate = Predicate::with_sign(
            atom.name.as_str(),
            terms.len(),
            crate::coherence::core_sign(atom.sign),
        )
        .map_err(|_| self.error(ErrorKind::InvalidSymbol))?;
        Ok(Pattern { predicate, terms })
    }
    fn directive(
        &mut self,
        term: &Term,
        body: Option<&themelios_program::program::Body>,
        origins: Vec<Location>,
    ) -> Result<Directive, Error> {
        self.variables.clear();
        self.safe.clear();
        let term = self.template(term, 1)?;
        let mut positive = Vec::new();
        let mut conditions = Vec::new();
        if let Some(body) = body {
            for (index, element) in body.elements().enumerate() {
                self.check(
                    Resource::BodyElements,
                    index.saturating_add(1),
                    self.limits.max_body_elements as usize,
                )?;
                self.node(1)?;
                let BodyElement::Literal(literal) = element.get() else {
                    return Err(self.unsupported(Feature::Body));
                };
                match &literal.inner {
                    LiteralInner::Atom(atom) => {
                        let is_positive = literal.negation == DefaultNegation::None;
                        let pattern = self.pattern(atom.get(), is_positive)?;
                        if is_positive {
                            positive.push(pattern);
                        } else {
                            conditions.push(Condition::Atom(literal.negation, pattern));
                        }
                    }
                    LiteralInner::Comparison(comparison) => {
                        if literal.negation != DefaultNegation::None {
                            return Err(self.unsupported(Feature::Comparison));
                        }
                        let mut steps = comparison.get().steps();
                        let (relation, right) =
                            steps.next().expect("shared comparison has a guard");
                        if steps.next().is_some() {
                            return Err(self.unsupported(Feature::Comparison));
                        }
                        conditions.push(Condition::Compare(
                            self.operand(comparison.get().first(), false)?,
                            relation,
                            self.operand(right, false)?,
                        ));
                    }
                    _ => return Err(self.unsupported(Feature::Body)),
                }
            }
        }
        if self
            .variables
            .values()
            .any(|slot| !self.safe.contains(slot))
        {
            return Err(self.unsupported(Feature::UnsafeVariable));
        }
        Ok(Directive {
            term,
            positive,
            conditions,
            variables: self.variables.len(),
            origins,
        })
    }
}

pub(crate) fn compile(
    source: &Program,
    options: AdmissionOptions,
    limits: AdmissionLimits,
    budget: &mut Budget,
    fallback: Location,
) -> Result<ObservationProgram, FormulaFailure> {
    if !source.statements().any(|entry| {
        matches!(
            entry.get(),
            Statement::Show(Show::Term(_) | Show::TermBody { .. })
        )
    }) {
        return Ok(ObservationProgram::default());
    }
    let constants = crate::extended::resolve(source, budget, fallback)?;
    let mut compiler = Compiler {
        limits: AdmissionLimits {
            max_depth: limits.max_depth.min(64),
            max_body_elements: limits
                .max_body_elements
                .min(u32::try_from(options.max_body_elements).unwrap_or(u32::MAX)),
            max_variables: limits.max_variables.min(
                u32::try_from(options.core_limits.max_variables_per_template).unwrap_or(u32::MAX),
            ),
            max_arity: limits
                .max_arity
                .min(u32::try_from(options.core_limits.max_predicate_arity).unwrap_or(u32::MAX)),
            ..limits
        },
        constants: &constants,
        location: fallback,
        nodes: 0,
        bytes: 0,
        origins: 0,
        variables: BTreeMap::new(),
        safe: BTreeSet::new(),
    };
    let mut result = ObservationProgram::default();
    for entry in source.statements() {
        let (term, body) = match entry.get() {
            Statement::Show(Show::Term(term)) => (term, None),
            Statement::Show(Show::TermBody { term, body }) => (term, Some(body.get())),
            _ => continue,
        };
        compiler.location = crate::extended::origin(entry, fallback);
        let directive = (|| {
            compiler.check(
                Resource::Directives,
                result.directives.len().saturating_add(1),
                limits.max_directives as usize,
            )?;
            // Count the borrowed evidence before allocating its owned projection.
            let count = entry
                .provenance()
                .origins()
                .filter(|origin| matches!(origin, themelios_program::provenance::Origin::Parsed(_)))
                .count();
            compiler.check(
                Resource::Origins,
                compiler.origins.saturating_add(count),
                limits.max_origins as usize,
            )?;
            compiler.origins += count;
            compiler.directive(term, body, crate::extended::parsed_origins(entry))
        })()
        .map_err(|error| FormulaFailure::Observation { error })?;
        result.directives.push(directive);
    }
    Ok(result)
}
