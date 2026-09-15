//! Directive-local compilation, independent of logical grounding and its carrier.

mod scopes;
mod patterns;
mod plan;
mod bindings;
mod inverse;
mod alternatives;
mod anonymous;

use std::collections::{BTreeMap, BTreeSet};

use themelios_base::span::Location;
use themelios_program::program::{Arguments, BodyElement, LiteralInner, Program, Show, Statement};
use themelios_program::symbol::{Name, Sign, Symbol};
use themelios_program::term::{Term, UnaryOp, Variable};

use super::{
    AdmissionLimits, AtomTest, Binder, Condition, DefaultNegation, Directive, Error, ErrorKind,
    Feature, ObservationProgram, Operand, Pattern, Predicate, Query, Relation, Resource,
    Statistics, Template,
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
    slots: usize,
    generated: Vec<(usize, Template)>,
    used: BTreeSet<usize>,
    scope_outer: usize,
    pool_binding: bool,
    capture_pools: bool,
}
impl Compiler<'_> {
    fn error(&self, kind: ErrorKind) -> Error {
        Error {
            source: None,
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
            if !self.pool_binding || slot < self.scope_outer {
                self.used.insert(slot);
            }
            return Ok(slot);
        }
        self.check(
            Resource::Variables,
            self.slots.saturating_add(1),
            self.limits.max_variables as usize,
        )?;
        self.text(name.as_str())?;
        let slot = self.slots;
        self.slots += 1;
        self.variables.insert(name.as_str().to_owned(), slot);
        if !self.pool_binding || slot < self.scope_outer {
            self.used.insert(slot);
        }
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
            Term::UnaryOperation { operator, argument } => {
                Template::Unary(*operator, Box::new(self.template(argument, depth + 1)?))
            }
            Term::BinaryOperation {
                operator,
                left,
                right,
            } => Template::Binary(
                *operator,
                Box::new(self.template(left, depth + 1)?),
                Box::new(self.template(right, depth + 1)?),
            ),
            Term::Absolute(argument) => {
                Template::Absolute(Box::new(self.template(argument, depth + 1)?))
            }
            Term::Pool(arguments) => {
                let previous = self.pool_binding;
                self.pool_binding |= self.capture_pools;
                let result = arguments
                    .iter()
                    .map(|argument| self.template(argument, depth + 1))
                    .collect::<Result<_, _>>();
                self.pool_binding = previous;
                Template::Pool(result?)
            }
            Term::Interval { lower, upper } => Template::Interval(
                Box::new(self.template(lower, depth + 1)?),
                Box::new(self.template(upper, depth + 1)?),
            ),
            Term::Tuple(arguments) => {
                self.arity(arguments.len())?;
                let mut terms = Vec::new();
                for argument in arguments {
                    terms.push(self.template(argument, depth + 1)?);
                }
                Template::Tuple(terms)
            }
            Term::External { .. } => return Err(self.unsupported(Feature::Term)),
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
    fn lift(&mut self, term: Template) -> Result<Template, Error> {
        if !term.multiple() {
            return Ok(term);
        }
        self.node(1)?;
        Ok(Template::Variable(self.generate(term)?))
    }
    fn slot(&mut self) -> Result<usize, Error> {
        self.check(
            Resource::Variables,
            self.slots.saturating_add(1),
            self.limits.max_variables as usize,
        )?;
        let slot = self.slots;
        self.slots += 1;
        Ok(slot)
    }
    fn generate(&mut self, term: Template) -> Result<usize, Error> {
        let slot = self.slot()?;
        self.generated.push((slot, term));
        self.used.insert(slot);
        Ok(slot)
    }
    fn test(&mut self, literal: &themelios_program::program::Literal) -> Result<Condition, Error> {
        Ok(match &literal.inner {
            LiteralInner::Atom(atom) => {
                Condition::Atom(literal.negation, self.atom_tests(atom.get())?)
            }
            LiteralInner::Comparison(comparison) => {
                let first = self.comparison_template(comparison.get().first(), literal.negation)?;
                let mut steps = Vec::new();
                for (relation, right) in comparison.get().steps() {
                    let right = self.comparison_template(right, literal.negation)?;
                    steps.push((relation, right));
                }
                Condition::Compare(literal.negation, first, steps)
            }
            LiteralInner::True | LiteralInner::False => {
                let truth = matches!(literal.inner, LiteralInner::True);
                Condition::Boolean(truth != (literal.negation == DefaultNegation::Not))
            }
        })
    }
    fn comparison_template(
        &mut self,
        term: &Term,
        negation: DefaultNegation,
    ) -> Result<Template, Error> {
        let previous = self.capture_pools;
        self.capture_pools = negation == DefaultNegation::None;
        let result = self.template(term, 1);
        self.capture_pools = previous;
        let term = result?;
        if negation == DefaultNegation::None && self.structural_capture(&term) {
            Ok(term)
        } else {
            self.lift(term)
        }
    }
    fn literal(
        &mut self,
        literal: &themelios_program::program::Literal,
        positive: &mut Vec<Vec<Pattern>>,
        conditions: &mut Vec<Condition>,
    ) -> Result<(), Error> {
        if literal.negation == DefaultNegation::None
            && let LiteralInner::Atom(atom) = &literal.inner
        {
            positive.push(self.patterns(atom.get())?);
        } else {
            conditions.push(self.test(literal)?);
        }
        Ok(())
    }
    fn directive(
        &mut self,
        term: &Term,
        body: Option<&themelios_program::program::Body>,
        origins: Vec<Location>,
    ) -> Result<Directive, Error> {
        self.variables.clear();
        self.safe.clear();
        self.slots = 0;
        self.generated.clear();
        self.used.clear();
        self.scope_outer = 0;
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
                if let BodyElement::Literal(literal) = element.get() {
                    self.literal(literal, &mut positive, &mut conditions)?;
                }
            }
        }
        let mut pending = Vec::new();
        if let Some(body) = body {
            for element in body.elements() {
                match element.get() {
                    BodyElement::Aggregate {
                        negation,
                        aggregate,
                    } => {
                        pending.push((Some((*negation, aggregate, self.guards(aggregate)?)), None));
                    }
                    BodyElement::Conditional(conditional) => {
                        pending.push((None, Some(conditional)));
                    }
                    BodyElement::Literal(_) => {}
                    _ => return Err(self.unsupported(Feature::Body)),
                }
            }
        }
        let mut conditionals = Vec::new();
        for (aggregate, conditional) in pending {
            if let Some((negation, aggregate, guards)) = aggregate {
                conditions.push(Condition::Aggregate(
                    negation,
                    self.aggregate(aggregate)?,
                    guards,
                ));
            }
            if let Some(conditional) = conditional {
                conditionals.push(conditional);
            }
        }
        for condition in &conditions {
            if let Condition::Aggregate(_, aggregate, _) = condition {
                self.used.extend(
                    aggregate
                        .elements
                        .iter()
                        .flat_map(|element| &element.query.inputs),
                );
            }
        }
        let mut query = self.finish(positive, conditions)?;
        for conditional in conditionals {
            query.conditions.push(self.conditional(conditional)?);
        }
        Ok(Directive {
            term,
            query,
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
        slots: 0,
        generated: Vec::new(),
        used: BTreeSet::new(),
        scope_outer: 0,
        pool_binding: false,
        capture_pools: false,
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
