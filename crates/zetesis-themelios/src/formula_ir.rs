//! Bounded normalization and explicit global/element-local variable scopes.

#[cfg(test)]
#[path = "formula_assignment_plan_tests.rs"]
mod assignment_plan_tests;

use std::collections::{BTreeMap, BTreeSet};

use themelios_base::span::Location;
use themelios_program::program::{
    Arguments, BodyElement, Choice, DefaultNegation, Direction, HasGuards, Head, Literal,
    LiteralInner, Optimize, OptimizeElement, Program as SourceProgram, Relation, Rule, Statement,
};
use themelios_program::provenance::{Origin, TransformTag};
use themelios_program::symbol::Symbol;
use themelios_program::term::{BinaryOp, Term, TermParts, UnaryOp, Variable};
use themelios_program::transform::{Rewrite, rewrite};
use themelios_syntax::ast;
use themelios_syntax::parse::Parse;
use themelios_syntax::tree::{AstNode, SyntaxKind};
use zetesis_core::{AtomPattern, Filter, Predicate, Term as CoreTerm, Value};
use zetesis_objective::{ObjectiveProgram, ObjectiveTemplate, WeightPolarity};

use crate::diagnostic::unsupported;
use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::{
    AdmissionFailure, AdmissionOptions, ExpansionFailure, ExpansionResource, FormulaFailure,
    FormulaLimits, FormulaResource, ProfileFeature, compile, extended, fact_expansion,
};

pub(crate) struct Prepared {
    pub analysis: themelios_analysis::Analysis,
    pub analysis_basis: crate::AnalysisBasis,
    pub analyzed: SourceProgram,
    pub rules: Vec<RuleIr>,
    pub objectives: Vec<ObjectiveIr>,
    pub objective_declarations: Vec<Location>,
    /// Extrema tuple carriers requiring a numeric-objective presence certificate
    /// after support completion. Unqualified mixed carriers remain refused.
    pub objective_extrema: BTreeSet<usize>,
}
pub(crate) struct ObjectiveIr {
    pub template: ObjectiveTemplate,
    pub condition: Vec<LiteralIr>,
    pub variables: usize,
    pub origins: Vec<Location>,
    pub location: Location,
}
pub(crate) struct RuleIr {
    pub head: HeadIr,
    pub body: Vec<LiteralIr>,
    pub bindings: Option<crate::formula_assignment_plan::Plan>,
    pub variables: usize,
    pub origins: Vec<Location>,
    pub location: Location,
}
pub(crate) enum HeadIr {
    Normal(Option<AtomPattern>),
    Disjunction(Vec<HeadLiteral>),
    Choice(ChoiceIr),
}
/// One activated group owns both its permission elements and numeric measure.
pub(crate) struct ChoiceIr {
    pub measure: HeadMeasure,
    pub guards: Vec<AggregateGuard>,
    pub elements: Vec<Element>,
}
/// A semantic head occurrence; default negation never supplies positive support.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct HeadLiteral<A = AtomPattern> {
    pub negation: DefaultNegation,
    pub operand: HeadOperand<A>,
}
/// The same semantic distinction precedes and follows atom instantiation.
/// Boolean operands carry truth without introducing a semantic atom.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum HeadOperand<A = AtomPattern> {
    Atom(A),
    Boolean(bool),
}
impl<A> HeadOperand<A> {
    pub(crate) fn atom(&self) -> Option<&A> {
        match self {
            Self::Atom(atom) => Some(atom),
            Self::Boolean(_) => None,
        }
    }
}
impl<A> HeadLiteral<A> {
    pub(crate) fn atom(&self) -> Option<&A> {
        self.operand.atom()
    }
    pub(crate) fn positive_atom(&self) -> Option<&A> {
        if self.negation == DefaultNegation::None {
            self.atom()
        } else {
            None
        }
    }
}
/// Numeric function used only for the bound constraint; every element retains
/// its signed activity and separate unsigned atom permission, including neutral contributions.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum HeadMeasure {
    Count,
    Sum,
    SumPlus,
    Min,
    Max,
}
pub(crate) struct Element {
    pub key: HeadElementKey,
    pub head: HeadLiteral,
    pub condition: Vec<LiteralIr>,
    pub variables: usize,
}
/// Complete tuples have set identity. Ordinary atoms use their sign and grounded atom;
/// ordinary Boolean elements instead retain every original source occurrence.
pub(crate) enum HeadElementKey {
    Atom,
    BooleanOccurrences(Vec<Location>),
    Tuple(Vec<CoreTerm>),
}
impl HeadElementKey {
    pub(crate) fn tuple(&self) -> Option<&[CoreTerm]> {
        match self {
            Self::Tuple(terms) => Some(terms),
            Self::Atom | Self::BooleanOccurrences(_) => None,
        }
    }
}
pub(crate) enum LiteralIr {
    Atom(DefaultNegation, AtomPattern),
    PatternAtom(crate::formula_pattern::PatternAtom),
    ProjectedAtom(DefaultNegation, Projection),
    Compare(Expression, Relation, Expression),
    /// Equality between a captured positive subterm and its source expression.
    /// This is always a consumer; neither side supplies a binding instruction.
    ArgumentCheck {
        captured: Expression,
        value: Expression,
    },
    TupleCompare(Vec<Expression>, Relation, Vec<Expression>),
    Guard(crate::formula_guard::Guard),
    Conditional(crate::formula_conditional_ir::ConditionalIr),
    Aggregate(AggregateIr),
    Bind {
        target: usize,
        value: Expression,
    },
    Range {
        target: usize,
        lower: Expression,
        upper: Expression,
        binder: bool,
    },
}
pub(crate) struct Projection {
    pub predicate: Predicate,
    pub terms: Vec<Option<CoreTerm>>,
}
pub(crate) struct AggregateGuard {
    pub relation: Relation,
    pub bound: Expression,
}
pub(crate) struct AggregateIr {
    pub id: usize,
    pub binding: Option<usize>,
    pub negation: DefaultNegation,
    pub function: themelios_program::program::AggregateFunction,
    pub guards: Vec<AggregateGuard>,
    pub elements: Vec<AggregateElementIr>,
}
pub(crate) struct AggregateElementIr {
    pub key: AggregateKey,
    pub condition: Vec<LiteralIr>,
    pub variables: usize,
}
pub(crate) enum AggregateKey {
    Tuple(Vec<CoreTerm>),
    Atom(AtomPattern),
}
pub(crate) struct Expression {
    pub nodes: Vec<Operation>,
}
impl Expression {
    /// Complete declared input occurrences; unrelated environment slots are never read.
    pub(crate) fn inputs(&self) -> impl Iterator<Item = usize> + '_ {
        self.nodes.iter().filter_map(|node| match node {
            Operation::Variable(slot) => Some(*slot),
            _ => None,
        })
    }
}
pub(crate) enum Operation {
    Constant(Value),
    Variable(usize),
    Unary(UnaryOp, usize),
    Binary(BinaryOp, usize, usize),
    Absolute(usize),
    Constructor(Box<crate::formula_value::Constructor>),
}

pub(crate) fn prepare(
    source: &SourceProgram,
    choices: &crate::formula_choice_source::Catalog,
    options: AdmissionOptions,
    limits: &FormulaLimits,
    budget: &mut Budget,
    fallback: Location,
) -> Result<Prepared, FormulaFailure> {
    let constants = extended::resolve(source, budget, fallback)?;
    let mut rules = Vec::new();
    let mut analyzed = Vec::new();
    let mut pool_projection_nodes = 0;
    let mut objectives = Vec::new();
    let mut objective_declarations = Vec::new();
    let mut compiler = Compiler {
        options,
        limits,
        budget,
        domain: BTreeSet::new(),
        next_aggregate: 0,
        dependency_projection: false,
        location: fallback,
    };
    for carrier in choices.statements(source, fallback) {
        let carrier = carrier?;
        if matches!(
            carrier.get(),
            Statement::Const(_) | Statement::Defined(_) | Statement::Show(_)
        ) {
            continue;
        }
        compiler.location = extended::origin(carrier, fallback);
        let mut normalizer = Normalizer {
            constants: &constants,
            budget: compiler.budget,
            location: compiler.location,
            failure: None,
        };
        let rewritten = rewrite(SourceProgram::of([carrier.clone()]), &mut normalizer);
        if let Some(error) = normalizer.failure {
            return Err(error.into());
        }
        let statement = rewritten.statements().next().expect("rewrite keeps a rule");
        let origins = extended::parsed_origins(carrier);
        let weak_objective =
            crate::formula_weak::normalize(statement, compiler.budget, compiler.location)?;
        let statement = weak_objective.as_ref().unwrap_or(statement);
        if let Statement::Optimize(optimize) = statement.get() {
            analyzed.push(statement.clone());
            compiler.objectives(
                optimize,
                &origins,
                &extended::parsed_origins(statement),
                &mut objectives,
                &mut objective_declarations,
            )?;
            continue;
        }
        if let Some(facts) = fact_expansion::facts(statement, compiler.budget, compiler.location)? {
            compiler.budget.charge(
                ExpansionResource::Origins,
                (facts.len() as u128).saturating_mul(origins.len() as u128),
                compiler.location,
            )?;
            for fact in facts {
                let head = fact.head().expect("expanded facts have a head").clone();
                analyzed.push(crate::formula_analysis::fact(
                    &head,
                    statement,
                    compiler.location,
                )?);
                rules.push(compiler.fact_rule(head, &origins)?);
            }
        } else {
            compiler.source_rules(
                statement,
                &origins,
                &mut pool_projection_nodes,
                &mut rules,
                &mut analyzed,
            )?;
        }
    }
    let analyzed = SourceProgram::of(analyzed);
    let analysis = crate::formula_analysis::analyze(&analyzed, limits, compiler.budget, fallback)?;
    let objective_extrema = crate::formula_objective_dependencies::check(
        &rules,
        &objectives,
        &analysis,
        &analyzed,
        fallback,
    )?;
    validate_objectives(&objectives, limits, fallback)?;
    let analysis_basis = if compiler.dependency_projection {
        crate::AnalysisBasis::DependencyProjection
    } else {
        crate::AnalysisBasis::NormalizedProgram
    };
    Ok(Prepared {
        analysis,
        analysis_basis,
        analyzed,
        rules,
        objectives,
        objective_declarations,
        objective_extrema,
    })
}

fn validate_objectives(
    objectives: &[ObjectiveIr],
    limits: &FormulaLimits,
    fallback: Location,
) -> Result<(), FormulaFailure> {
    ObjectiveProgram::new(
        objectives
            .iter()
            .map(|objective| objective.template.clone())
            .collect(),
        limits.objective,
    )
    .map_err(|error| FormulaFailure::Objective {
        location: error
            .template_index()
            .and_then(|index| objectives.get(index))
            .map_or(fallback, |objective| objective.location),
        error,
    })?;
    Ok(())
}

struct Normalizer<'a> {
    constants: &'a BTreeMap<String, Symbol>,
    budget: &'a mut Budget,
    location: Location,
    failure: Option<ExpansionFailure>,
}
impl Rewrite for Normalizer<'_> {
    fn tag(&self) -> TransformTag {
        TransformTag::new("zetesis-finite-formula")
    }
    fn rewrite_term(&mut self, term: Term) -> Term {
        if self.failure.is_some() {
            return term;
        }
        let result = (|| {
            // Aggregate-local admission accepts real sentinel tuple values and
            // bounds. Other source contexts retain their own scalar checks.
            if matches!(term, Term::Symbolic(Symbol::Infimum | Symbol::Supremum)) {
                self.budget
                    .charge(ExpansionResource::TermWork, 1, self.location)?;
                return Ok(term);
            }
            if matches!(
                term,
                Term::UnaryOperation { .. } | Term::BinaryOperation { .. } | Term::Absolute(_)
            ) {
                let mut variable = false;
                for subterm in term.subterms() {
                    self.budget
                        .charge(ExpansionResource::TermWork, 1, self.location)?;
                    variable |= matches!(
                        subterm,
                        Term::Variable(_) | Term::Interval { .. } | Term::Pool(_)
                    );
                }
                if variable {
                    return Ok(term);
                }
            }
            if matches!(term, Term::Tuple(_) | Term::Function { .. })
                && (!term.is_ground()
                    || term
                        .subterms()
                        .any(|node| matches!(node, Term::Interval { .. } | Term::Pool(_))))
            {
                return Ok(term);
            }
            extended::normalize_node(term, self.constants, self.budget, self.location)
        })();
        match result {
            Ok(term) => term,
            Err(error) => {
                self.failure = Some(error);
                Term::Symbolic(Symbol::Number(0))
            }
        }
    }
}

#[derive(Clone, Default)]
pub(super) struct Variables {
    pub(super) named: BTreeMap<String, usize>,
    pub(super) count: usize,
    pub(super) safe: BTreeSet<usize>,
    /// Diagnostic classification only; these reads never establish safety.
    pub(super) argument_inputs: BTreeSet<usize>,
}
impl Variables {
    pub(super) fn slot(&mut self, variable: &Variable) -> usize {
        if let Variable::Named(name) = variable {
            if let Some(index) = self.named.get(name.as_str()) {
                return *index;
            }
            let index = self.count;
            self.count += 1;
            self.named.insert(name.as_str().to_owned(), index);
            index
        } else {
            let index = self.count;
            self.count += 1;
            index
        }
    }
    pub(super) fn safety(&self, location: Location) -> Result<(), FormulaFailure> {
        for variable in 0..self.count {
            if !self.safe.contains(&variable) {
                if self.argument_inputs.contains(&variable) {
                    return Err(FormulaFailure::UnboundArgumentInput { variable, location });
                }
                return Err(FormulaFailure::UnsafeVariable { variable, location });
            }
        }
        Ok(())
    }
}

pub(super) struct Compiler<'a> {
    pub(super) options: AdmissionOptions,
    pub(super) limits: &'a FormulaLimits,
    pub(super) budget: &'a mut Budget,
    domain: BTreeSet<Value>,
    pub(super) next_aggregate: usize,
    pub(super) dependency_projection: bool,
    pub(super) location: Location,
}
impl Compiler<'_> {
    fn fact_rule(
        &mut self,
        head: AtomPattern,
        origins: &[Location],
    ) -> Result<RuleIr, FormulaFailure> {
        self.pattern_domain(&head)?;
        self.budget.charge(
            ExpansionResource::Origins,
            origins.len() as u128,
            self.location,
        )?;
        Ok(RuleIr {
            head: HeadIr::Normal(Some(head)),
            body: Vec::new(),
            bindings: None,
            variables: 0,
            origins: origins.to_vec(),
            location: self.location,
        })
    }
    fn objectives(
        &mut self,
        optimize: &Optimize,
        origins: &[Location],
        body_origins: &[Location],
        objectives: &mut Vec<ObjectiveIr>,
        declarations: &mut Vec<Location>,
    ) -> Result<(), FormulaFailure> {
        let polarity = match optimize.direction {
            Direction::Minimize => WeightPolarity::AsWritten,
            Direction::Maximize => WeightPolarity::Negated,
        };
        self.budget.charge(
            ExpansionResource::Origins,
            origins.len() as u128,
            self.location,
        )?;
        declarations.extend_from_slice(origins);
        for element in optimize.elements() {
            let mut evidence: Vec<_> = element
                .provenance()
                .origins()
                .filter_map(|origin| {
                    if let Origin::Parsed(location) = origin {
                        Some(*location)
                    } else {
                        None
                    }
                })
                .collect();
            // Statement-level origins retain equal directives merged across files,
            // even when nested element provenance keeps only its first occurrence.
            evidence.extend_from_slice(origins);
            evidence.extend_from_slice(body_origins);
            evidence.sort_unstable();
            evidence.dedup();
            self.budget.charge(
                ExpansionResource::Origins,
                evidence.len() as u128,
                self.location,
            )?;
            objectives.push(self.objective(element.get(), evidence, polarity)?);
        }
        Ok(())
    }
    fn objective(
        &mut self,
        element: &OptimizeElement,
        origins: Vec<Location>,
        polarity: WeightPolarity,
    ) -> Result<ObjectiveIr, FormulaFailure> {
        let mut variables = Variables::default();
        let mut positive = Vec::new();
        let mut filters = Vec::new();
        let mut condition = Vec::new();
        for literal in element.condition().literals() {
            if literal.get().negation != DefaultNegation::None {
                return Err(unsupported(ProfileFeature::Objective, self.location).into());
            }
            match &literal.get().inner {
                LiteralInner::Atom(atom) => {
                    let pattern = self.atom(atom.get(), &mut variables, true)?;
                    positive.push(pattern.clone());
                    condition.push(LiteralIr::Atom(DefaultNegation::None, pattern));
                }
                LiteralInner::Comparison(comparison) => {
                    let mut steps = comparison.get().steps();
                    let (relation, right) = steps.next().expect("comparison has step");
                    if steps.next().is_some() || !matches!(relation, Relation::Eq | Relation::Neq) {
                        return Err(unsupported(ProfileFeature::Objective, self.location).into());
                    }
                    let left = self.objective_term(comparison.get().first(), &mut variables)?;
                    let right = self.objective_term(right, &mut variables)?;
                    condition.push(LiteralIr::Compare(
                        scalar_expression(&left),
                        relation,
                        scalar_expression(&right),
                    ));
                    filters.push(if relation == Relation::Eq {
                        Filter::Eq(left, right)
                    } else {
                        Filter::Neq(left, right)
                    });
                }
                _ => return Err(unsupported(ProfileFeature::Objective, self.location).into()),
            }
        }
        let weight = self.objective_weight(element.weight().term(), &mut variables)?;
        // Nonnumeric literals follow the same resolved-value contract as bound
        // weights: they supply no contribution or numeric priority witness.
        // Still admit the whole element, including priority, tuple and safety,
        // before completed grounding determines objective presence.
        let priority = match element.weight().priority() {
            None => 0,
            Some(Term::Symbolic(Symbol::Number(priority))) => *priority,
            _ => return Err(unsupported(ProfileFeature::Objective, self.location).into()),
        };
        let tuple = element
            .terms()
            .map(|term| self.objective_term(term, &mut variables))
            .collect::<Result<Vec<_>, _>>()?;
        variables.safety(self.location)?;
        let count = variables.count;
        let template = ObjectiveTemplate::new(weight, priority, tuple, positive, filters)
            .with_weight_polarity(polarity);
        Ok(ObjectiveIr {
            template,
            condition,
            variables: count,
            origins,
            location: self.location,
        })
    }
    pub(super) fn objective_term(
        &mut self,
        term: &Term,
        variables: &mut Variables,
    ) -> Result<CoreTerm, FormulaFailure> {
        self.budget
            .charge(ExpansionResource::TermWork, 1, self.location)?;
        match term {
            Term::Variable(variable) => {
                let slot = variables.slot(variable);
                self.variable_limit(variables)?;
                Ok(CoreTerm::Variable(slot))
            }
            Term::Symbolic(symbol) => {
                Ok(CoreTerm::Constant(compile::scalar(symbol, self.location)?))
            }
            _ => Err(unsupported(ProfileFeature::Objective, self.location).into()),
        }
    }

    fn objective_weight(
        &mut self,
        term: &Term,
        variables: &mut Variables,
    ) -> Result<CoreTerm, FormulaFailure> {
        // Extrema are ignored logical weights, not finite arithmetic endpoints.
        // Other objective term contexts retain their own admission contract.
        let value = match term {
            Term::Symbolic(Symbol::Infimum) => Value::Infimum,
            Term::Symbolic(Symbol::Supremum) => Value::Supremum,
            _ => return self.objective_term(term, variables),
        };
        self.budget
            .charge(ExpansionResource::TermWork, 1, self.location)?;
        Ok(CoreTerm::Constant(value))
    }
    pub(super) fn rule(
        &mut self,
        rule: &Rule,
        origins: Vec<Location>,
        choice_source: Option<&Choice>,
    ) -> Result<RuleIr, FormulaFailure> {
        let mut variables = Variables::default();
        let mut body = Vec::new();
        for element in rule.body().get().elements() {
            match element.get() {
                BodyElement::Literal(literal) => {
                    self.literal_into(literal, &mut variables, &mut body)?;
                }
                BodyElement::Conditional(conditional) => {
                    self.conditional_syntax(conditional)?;
                }
                BodyElement::Aggregate { .. } => {}
                _ => return Err(unsupported(ProfileFeature::BodyElement, self.location).into()),
            }
        }
        // Declare every global before element-local scopes are cloned. In particular,
        // a head variable cannot accidentally become safe inside an aggregate.
        let ordinary = match rule.head().get() {
            Head::Falsum => Some(HeadIr::Normal(None)),
            Head::Verum => Some(self.verum_head()?),
            Head::Literal(literal)
                if literal.negation == DefaultNegation::None
                    && matches!(literal.inner, LiteralInner::Atom(_)) =>
            {
                Some(HeadIr::Normal(Some(self.generated_head(
                    literal,
                    &mut variables,
                    &mut body,
                )?)))
            }
            Head::Literal(literal) => {
                ceiling(
                    FormulaResource::DisjunctionElements,
                    1,
                    self.limits.max_disjunction_elements as u128,
                    self.location,
                )?;
                // The singleton retains its signed literal in the original
                // implication. Neither default-negation sign supplies support.
                Some(HeadIr::Disjunction(vec![self.head_literal(
                    literal,
                    &mut variables,
                    &mut body,
                )?]))
            }
            Head::Disjunction(disjunction) => {
                let mut heads = Vec::new();
                for element in disjunction.elements() {
                    ceiling(
                        FormulaResource::DisjunctionElements,
                        heads.len() as u128 + 1,
                        self.limits.max_disjunction_elements as u128,
                        self.location,
                    )?;
                    self.true_head_condition(element.get().condition())?;
                    // Generated arguments share outer bindings; each emitted
                    // rule retains its original disjunction, without shifting.
                    heads.push(self.head_literal(
                        element.get().literal(),
                        &mut variables,
                        &mut body,
                    )?);
                }
                Some(HeadIr::Disjunction(heads))
            }
            Head::Choice(_) | Head::Aggregate(_) => None,
            Head::TheoryAtom(_) => {
                return Err(unsupported(ProfileFeature::Head, self.location).into());
            }
        };
        let aggregate_guards = self.body_guards(rule, &mut variables)?;
        let choice_guards = self.head_guards(rule.head().get(), &mut variables)?;
        let assignments = self.assignment_targets(rule, &aggregate_guards, &mut variables)?;
        self.bindings(&mut body, &mut variables)?;
        variables.safety(self.location)?;
        self.body_aggregates(rule, aggregate_guards, assignments, &variables, &mut body)?;
        self.body_conditionals(rule, &variables, &mut body)?;
        let bindings = self.assignment_plan(&body, variables.count, &choice_guards)?;
        let head = if let Some(head) = ordinary {
            head
        } else {
            let (measure, elements) = match rule.head().get() {
                Head::Choice(choice) => (
                    HeadMeasure::Count,
                    self.choice_elements(choice, choice_source, &variables)?,
                ),
                Head::Aggregate(aggregate) => {
                    self.aggregate_head_elements(aggregate, &variables)?
                }
                _ => unreachable!("head classified"),
            };
            HeadIr::Choice(ChoiceIr {
                measure,
                guards: choice_guards,
                elements,
            })
        };
        self.variable_limit(&variables)?;
        Ok(RuleIr {
            head,
            body,
            bindings,
            variables: variables.count,
            origins,
            location: self.location,
        })
    }

    fn head_guards(
        &mut self,
        head: &Head,
        variables: &mut Variables,
    ) -> Result<Vec<AggregateGuard>, FormulaFailure> {
        match head {
            Head::Choice(choice) => self.choice_guards(choice, variables),
            Head::Aggregate(aggregate) => self.guards(
                aggregate
                    .left_guard()
                    .map(themelios_program::provenance::WithProvenance::get),
                aggregate
                    .right_guard()
                    .map(themelios_program::provenance::WithProvenance::get),
                variables,
            ),
            _ => Ok(Vec::new()),
        }
    }
    fn choice_elements(
        &mut self,
        choice: &Choice,
        source: Option<&Choice>,
        variables: &Variables,
    ) -> Result<Vec<Element>, FormulaFailure> {
        let mut elements = Vec::new();
        let mut source_booleans = source
            .into_iter()
            .flat_map(Choice::elements)
            .filter(|element| {
                matches!(
                    element.get().literal().inner,
                    themelios_program::program::LiteralInner::True
                        | themelios_program::program::LiteralInner::False
                )
            });
        for element in choice.elements() {
            let mut local = variables.clone();
            let mut condition = Vec::new();
            let head = self.head_literal(element.get().literal(), &mut local, &mut condition)?;
            let key = match &head.operand {
                HeadOperand::Atom(_) => HeadElementKey::Atom,
                HeadOperand::Boolean(_) => {
                    // The owned analysis set forgets Boolean multiplicity; it
                    // represents dependencies, not this group's exact measure.
                    self.dependency_projection = true;
                    HeadElementKey::BooleanOccurrences(
                        self.boolean_occurrences(element.get(), source_booleans.next())?,
                    )
                }
            };
            condition.extend(self.condition(element.get().condition(), &mut local)?);
            self.bindings(&mut condition, &mut local)?;
            self.variable_limit(&local)?;
            local.safety(self.location)?;
            elements.push(Element {
                key,
                head,
                condition,
                variables: local.count,
            });
        }
        if source_booleans.next().is_some() {
            return Err(FormulaFailure::ChoiceSource {
                location: self.location,
            });
        }
        Ok(elements)
    }
    pub(super) fn variable_limit(&self, variables: &Variables) -> Result<(), FormulaFailure> {
        ceiling(
            FormulaResource::Variables,
            variables.count as u128,
            self.options.core_limits.max_variables_per_template as u128,
            self.location,
        )
    }
    pub(super) fn literal(
        &mut self,
        literal: &Literal,
        variables: &mut Variables,
    ) -> Result<LiteralIr, FormulaFailure> {
        match &literal.inner {
            LiteralInner::Atom(atom) => {
                if literal.negation == DefaultNegation::None
                    && let Some(pattern) = self.positive_pattern(atom.get(), variables)?
                {
                    return Ok(LiteralIr::PatternAtom(pattern));
                }
                if let Some(projected) =
                    self.projected_atom(atom.get(), literal.negation, variables)?
                {
                    return Ok(projected);
                }
                Ok(LiteralIr::Atom(
                    literal.negation,
                    self.atom(
                        atom.get(),
                        variables,
                        literal.negation == DefaultNegation::None,
                    )?,
                ))
            }
            LiteralInner::Comparison(comparison) => {
                let mut steps = comparison.get().steps();
                let (relation, right) = steps.next().expect("comparison has a step");
                if literal.negation != DefaultNegation::None || steps.next().is_some() {
                    return self.comparison_guard(comparison.get(), literal.negation, variables);
                }
                self.binding_comparison(comparison.get().first(), relation, right, variables)
            }
            LiteralInner::True | LiteralInner::False => {
                self.budget
                    .charge(ExpansionResource::TermWork, 1, self.location)?;
                Ok(LiteralIr::Guard(crate::formula_guard::Guard::Boolean(
                    matches!(literal.inner, LiteralInner::True)
                        != (literal.negation == DefaultNegation::Not),
                )))
            }
        }
    }
    pub(super) fn atom(
        &mut self,
        atom: &themelios_program::program::Atom,
        variables: &mut Variables,
        binder: bool,
    ) -> Result<AtomPattern, FormulaFailure> {
        let Arguments::Single(arguments) = &atom.arguments else {
            return Err(unsupported(ProfileFeature::PooledArguments, self.location).into());
        };
        ceiling(
            FormulaResource::Arity,
            arguments.len() as u128,
            self.options.core_limits.max_predicate_arity as u128,
            self.location,
        )?;
        let mut terms = Vec::new();
        for term in arguments {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            terms.push(match term {
                Term::Variable(variable) => {
                    let slot = variables.slot(variable);
                    self.variable_limit(variables)?;
                    if binder {
                        variables.safe.insert(slot);
                    }
                    CoreTerm::Variable(slot)
                }
                Term::Symbolic(symbol) => {
                    let value = compile::scalar(symbol, self.location)?;
                    self.value(&value)?;
                    CoreTerm::Constant(value)
                }
                _ => return Err(unsupported(ProfileFeature::Term, self.location).into()),
            });
        }
        let predicate = Predicate::with_sign(
            atom.name.as_str(),
            arguments.len(),
            crate::coherence::core_sign(atom.sign),
        )
        .map_err(|error| AdmissionFailure::Construction {
            error,
            location: self.location,
        })?;
        AtomPattern::new(predicate, terms).map_err(|error| {
            AdmissionFailure::Construction {
                error,
                location: self.location,
            }
            .into()
        })
    }
    pub(super) fn expression(
        &mut self,
        term: &Term,
        variables: &mut Variables,
    ) -> Result<Expression, FormulaFailure> {
        self.value_plan_preflight(term)?;
        let mut nodes = Vec::new();
        term.clone()
            .try_fold(|parts| -> Result<usize, FormulaFailure> {
                self.budget
                    .charge(ExpansionResource::TermWork, 1, self.location)?;
                let node = match parts {
                    TermParts::Symbolic(symbol) => {
                        let value = compile::scalar(&symbol, self.location)?;
                        self.value(&value)?;
                        Operation::Constant(value)
                    }
                    TermParts::Variable(variable) => {
                        let slot = variables.slot(&variable);
                        self.variable_limit(variables)?;
                        Operation::Variable(slot)
                    }
                    TermParts::UnaryOperation { operator, argument } => {
                        if operator == UnaryOp::Negate
                            && let Operation::Constructor(constructor) = &mut nodes[argument]
                            && constructor.name.is_some()
                        {
                            constructor.sign = match constructor.sign {
                                zetesis_core::Sign::Positive => zetesis_core::Sign::Negative,
                                zetesis_core::Sign::Negative => zetesis_core::Sign::Positive,
                            };
                            return Ok(argument);
                        }
                        Operation::Unary(operator, argument)
                    }
                    TermParts::Function { name, arguments } => {
                        Operation::Constructor(Box::new(crate::formula_value::Constructor {
                            name: Some(name.as_str().to_owned()),
                            sign: zetesis_core::Sign::Positive,
                            arguments,
                        }))
                    }
                    TermParts::Tuple(arguments) => {
                        Operation::Constructor(Box::new(crate::formula_value::Constructor {
                            name: None,
                            sign: zetesis_core::Sign::Positive,
                            arguments,
                        }))
                    }
                    TermParts::BinaryOperation {
                        operator,
                        left,
                        right,
                    } => Operation::Binary(operator, left, right),
                    TermParts::Absolute(argument) => Operation::Absolute(argument),
                    _ => return Err(unsupported(ProfileFeature::Term, self.location).into()),
                };
                let index = nodes.len();
                nodes.push(node);
                Ok(index)
            })?;
        Ok(Expression { nodes })
    }
    pub(super) fn pattern_domain(&mut self, pattern: &AtomPattern) -> Result<(), FormulaFailure> {
        ceiling(
            FormulaResource::Arity,
            pattern.terms().len() as u128,
            self.options.core_limits.max_predicate_arity as u128,
            self.location,
        )?;
        for term in pattern.terms() {
            if let CoreTerm::Constant(value) = term {
                self.value(value)?;
            }
        }
        Ok(())
    }
    pub(super) fn value(&mut self, value: &Value) -> Result<(), FormulaFailure> {
        if !self.domain.contains(value) {
            ceiling(
                FormulaResource::DomainValues,
                self.domain.len() as u128 + 1,
                self.limits
                    .max_domain_values
                    .min(self.options.core_limits.max_domain_values) as u128,
                self.location,
            )?;
            self.budget.charge(
                ExpansionResource::ScalarBytes,
                value_bytes(value),
                self.location,
            )?;
            self.domain.insert(value.clone());
        }
        Ok(())
    }
}

pub(crate) fn value_bytes(value: &Value) -> u128 {
    match value {
        Value::Infimum | Value::Supremum | Value::Number(_) => 0,
        Value::Structured(value) => value.payload_bytes() as u128,
        Value::String(text) | Value::Symbol(text) => text.len() as u128,
    }
}

fn scalar_expression(term: &CoreTerm) -> Expression {
    Expression {
        nodes: vec![match term {
            CoreTerm::Variable(variable) => Operation::Variable(*variable),
            CoreTerm::Constant(value) => Operation::Constant(value.clone()),
        }],
    }
}

pub(crate) fn check_objectives(
    parsed: &Parse<ast::Program>,
    limits: &FormulaLimits,
    count: &mut usize,
) -> Result<(), FormulaFailure> {
    for statement in parsed.tree().statements() {
        if let ast::Statement::WeakConstraint(weak) = &statement {
            ceiling(
                FormulaResource::ObjectiveElements,
                *count as u128 + 1,
                limits.objective.max_templates as u128,
                parsed.location(weak.syntax().text_range()),
            )?;
            *count += 1;
            continue;
        }
        let ast::Statement::Optimize(optimize) = statement else {
            continue;
        };
        let location = parsed.location(optimize.syntax().text_range());
        if optimize.keyword_token().is_none_or(|token| {
            !matches!(
                token.kind(),
                SyntaxKind::KW_MINIMIZE | SyntaxKind::KW_MAXIMIZE
            )
        }) {
            return Err(unsupported(ProfileFeature::Objective, location).into());
        }
        for element in optimize.elements() {
            ceiling(
                FormulaResource::ObjectiveElements,
                *count as u128 + 1,
                limits.objective.max_templates as u128,
                parsed.location(element.syntax().text_range()),
            )?;
            *count += 1;
        }
    }
    Ok(())
}
