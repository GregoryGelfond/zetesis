//! Bounded normalization and explicit global/element-local variable scopes.

#[cfg(test)]
#[path = "formula_assignment_plan_tests.rs"]
mod assignment_plan_tests;

#[path = "formula_objective_scope.rs"]
mod objective_scope;

use std::collections::{BTreeMap, BTreeSet};

use themelios_base::span::Location;
use themelios_program::program::{
    Arguments, Body, BodyElement, Choice, DefaultNegation, Direction, HasGuards, Head, Literal,
    LiteralInner, Optimize, OptimizeElement, Program as SourceProgram, Relation, Rule, Statement,
};
use themelios_program::provenance::{Origin, TransformTag, WithProvenance};
use themelios_program::symbol::Symbol;
use themelios_program::term::{BinaryOp, Term, TermParts, UnaryOp, Variable};
use themelios_program::transform::{Rewrite, rewrite};
use themelios_syntax::ast;
use themelios_syntax::parse::Parse;
use themelios_syntax::tree::{AstNode, SyntaxKind};
use zetesis_core::{AtomPattern, Filter, Predicate, Term as CoreTerm, Value};
use zetesis_objective::{ObjectiveTemplate, WeightPolarity};

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
    pub projection: Vec<RuleIr>,
    pub project_selection: crate::ProjectSelection,
    pub objectives: Vec<ObjectiveIr>,
    pub objective_declarations: Vec<Location>,
    /// Extrema tuple carriers selected for an optional numeric-weight precision
    /// refinement after support completion.
    pub objective_extrema: BTreeSet<usize>,
    /// Written constraints over a keyed value that were asked as the one atom
    /// their key admits, before this preparation.
    pub keyed_constraints: usize,
}
pub(crate) struct ObjectiveIr {
    pub weight: ObjectiveField,
    pub priority: Expression,
    pub tuple: Vec<ObjectiveField>,
    pub positive: Vec<AtomPattern>,
    pub filters: Vec<Filter>,
    pub polarity: WeightPolarity,
    /// Positive conditions whose generated values affect priority evaluation or
    /// source eligibility through a literal, filter or repeated variable.
    /// Applicable completed refinements may exclude impossible source values.
    pub priority_sources: BTreeSet<usize>,
    /// Select finite source truth coverage and a closed original-model query.
    /// The query decides contribution in the model; it does not establish source
    /// eligibility or simultaneous realizability of possible source rows.
    pub needs_eligibility_query: bool,
    pub condition: ObjectiveCondition,
    pub variables: usize,
    pub origins: Vec<Location>,
    pub location: Location,
}
/// Literal objectives preserve the lifted path; scoped weak bodies own the
/// existing finite assignment schedule over the same literal sequence.
pub(crate) enum ObjectiveCondition {
    Literals(Vec<LiteralIr>),
    Body {
        literals: Vec<LiteralIr>,
        bindings: Option<crate::formula_assignment_plan::Plan>,
        /// Original scalar conditions; generated data instructions are not filters.
        filters: usize,
    },
}
impl ObjectiveCondition {
    pub(crate) fn literals(&self) -> &[LiteralIr] {
        match self {
            Self::Literals(literals) | Self::Body { literals, .. } => literals,
        }
    }
}

/// A simple field stays lifted; evaluated fields share the existing scalar
/// expression semantics and are specialized only after a complete source join.
pub(crate) enum ObjectiveField {
    Term(CoreTerm),
    Expression(Expression),
}
impl ObjectiveField {
    pub(crate) fn term(&self) -> Option<&CoreTerm> {
        match self {
            Self::Term(term) => Some(term),
            Self::Expression(_) => None,
        }
    }
    pub(crate) fn uses(&self, variable: usize) -> bool {
        match self {
            Self::Term(term) => *term == CoreTerm::Variable(variable),
            Self::Expression(expression) => expression.inputs().any(|input| input == variable),
        }
    }
}
impl ObjectiveIr {
    /// Preserve the lifted evaluator when every data field is a simple term.
    pub(super) fn template(&self, priority: i32) -> Option<ObjectiveTemplate> {
        Some(
            ObjectiveTemplate::new(
                self.weight.term()?.clone(),
                priority,
                self.tuple
                    .iter()
                    .map(|field| field.term().cloned())
                    .collect::<Option<Vec<_>>>()?,
                self.positive.clone(),
                self.filters.clone(),
            )
            .with_weight_polarity(self.polarity),
        )
    }
}
pub(crate) struct RuleIr {
    pub head: HeadIr,
    pub body: Vec<LiteralIr>,
    /// Original body slots precede a suffix used only by generated head values.
    /// Local body scopes are compiled before that suffix exists.
    pub body_variables: usize,
    pub bindings: Option<crate::formula_assignment_plan::Plan>,
    pub variables: usize,
    pub origins: Vec<Location>,
    pub location: Location,
}
impl RuleIr {
    /// Body-local frames never receive the synthetic head-value suffix.
    pub(super) fn body_binding<'a>(
        &self,
        binding: &'a crate::formula_binding::Binding<'_>,
    ) -> crate::formula_binding::Binding<'a> {
        binding.prefix(self.body_variables)
    }
}
pub(crate) enum HeadIr {
    Normal(Option<AtomPattern>),
    Disjunction(Vec<HeadLiteral>),
    ConditionalDisjunction {
        ordinary: Vec<HeadLiteral>,
        elements: Vec<crate::formula_conditional_head_ir::ConditionalHeadIr>,
    },
    Choice(ChoiceIr),
}
impl HeadIr {
    /// Syntactic head occurrences only; local conditions keep their own scope.
    pub(crate) fn disjuncts(&self) -> impl Iterator<Item = &HeadLiteral> {
        let (ordinary, elements) = match self {
            Self::Disjunction(heads) => (heads.as_slice(), &[][..]),
            Self::ConditionalDisjunction { ordinary, elements } => {
                (ordinary.as_slice(), elements.as_slice())
            }
            Self::Normal(_) | Self::Choice(_) => (&[][..], &[][..]),
        };
        ordinary
            .iter()
            .chain(elements.iter().map(|element| &element.head))
    }
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
/// Aggregate measure used only for the bound constraint; every element retains
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
    /// Local condition frame; later slots are generated head arguments only.
    pub body_variables: usize,
    pub variables: usize,
}
impl Element {
    pub(super) fn body_binding<'a>(
        &self,
        binding: &'a crate::formula_binding::Binding<'_>,
    ) -> crate::formula_binding::Binding<'a> {
        binding.prefix(self.body_variables)
    }
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
/// Complete existential witnesses, with a flat relational fast path.
pub(crate) enum Projection {
    Arguments {
        predicate: Predicate,
        terms: Vec<Option<CoreTerm>>,
    },
    Witnesses {
        atom: AtomPattern,
        bindings: Vec<LiteralIr>,
        variables: usize,
        /// Only this prefix belongs to the outer frame. Later outer slots must
        /// never populate the private structural captures of this projection.
        inputs: usize,
    },
}
impl Projection {
    pub(crate) fn predicate(&self) -> &Predicate {
        match self {
            Self::Arguments { predicate, .. } => predicate,
            Self::Witnesses { atom, .. } => atom.predicate(),
        }
    }
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
    let mut projection = Vec::new();
    let mut project_selection = crate::ProjectSelection::default();
    let mut analyzed = Vec::new();
    let mut pool_projection_nodes = 0;
    let mut objectives = Vec::new();
    let mut objective_declarations = Vec::new();
    let mut compiler = Compiler {
        options,
        limits,
        budget,
        domain: BTreeSet::new(),
        predicates: BTreeSet::new(),
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
        let rewritten = compiler.normalize_statement(carrier, &constants)?;
        let statement = rewritten.statements().next().expect("rewrite keeps a rule");
        let origins = extended::parsed_origins(carrier);
        if compiler.project_statement(
            statement,
            &origins,
            &mut pool_projection_nodes,
            &mut projection,
            &mut project_selection,
        )? {
            continue;
        }
        if let Some(observation) = compiler.objective_statement(
            statement,
            &origins,
            &mut objectives,
            &mut objective_declarations,
            &mut pool_projection_nodes,
        )? {
            analyzed.extend(observation);
            continue;
        }
        if let Some(facts) = if generated_fact(statement.get()) {
            None
        } else {
            fact_expansion::facts(statement, compiler.budget, compiler.location)?
        } {
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
    let analyzed = SourceProgram::of_nodes(analyzed);
    let analysis = crate::formula_analysis::analyze(&analyzed, limits, compiler.budget, fallback)?;
    let objective_extrema =
        crate::formula_objective_dependencies::check(&rules, &mut objectives, &analysis);
    validate_objectives(&objectives, limits)?;
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
        projection,
        project_selection: project_selection.finish(),
        objectives,
        objective_declarations,
        objective_extrema,
        keyed_constraints: 0,
    })
}

fn validate_objectives(
    objectives: &[ObjectiveIr],
    limits: &FormulaLimits,
) -> Result<(), FormulaFailure> {
    for (index, objective) in objectives.iter().enumerate() {
        if let ObjectiveCondition::Body { filters, .. } = objective.condition {
            ObjectiveTemplate::validate_shape(
                objective.tuple.len(),
                &objective.positive,
                filters,
                limits.objective,
                index,
            )
        } else {
            ObjectiveTemplate::validate_scope(
                objective.tuple.len(),
                &objective.positive,
                &objective.filters,
                limits.objective,
                index,
            )
            .map(|_| ())
        }
        .map_err(|error| FormulaFailure::Objective {
            error,
            location: objective.location,
        })?;
        if matches!(objective.condition, ObjectiveCondition::Body { .. })
            && objective.variables > limits.objective.max_variables_per_template
        {
            return Err(FormulaFailure::Objective {
                error: zetesis_objective::AdmissionError::Limit {
                    resource: zetesis_objective::AdmissionResource::Variables,
                    template: Some(index),
                    actual: objective.variables,
                    limit: limits.objective.max_variables_per_template,
                },
                location: objective.location,
            });
        }
    }
    Ok(())
}

struct Normalizer<'a> {
    constants: &'a BTreeMap<String, Symbol>,
    budget: &'a mut Budget,
    location: Location,
    failure: Option<ExpansionFailure>,
}

impl Normalizer<'_> {
    fn normalize_node(&mut self, term: Term) -> Result<Term, ExpansionFailure> {
        (|| {
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
        })()
    }
}
impl Rewrite for Normalizer<'_> {
    fn tag(&self) -> TransformTag {
        TransformTag::new("zetesis-finite-formula")
    }
    fn rewrite_term(&mut self, term: Term) -> Term {
        if self.failure.is_some() {
            return term;
        }
        let result =
            crate::formula_pool::distribute(term, self.budget, self.location).and_then(|term| {
                match term.into_parts() {
                    TermParts::Pool(items) => {
                        let mut normalized = Vec::with_capacity(items.len());
                        for item in items {
                            normalized.push(self.normalize_node(item)?);
                        }
                        Ok(Term::pool(normalized).expect("source alternatives remain nonempty"))
                    }
                    parts => self.normalize_node(Term::from(parts)),
                }
            });
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
    pub(super) domain: BTreeSet<Value>,
    /// The one shared name per predicate this compilation mints patterns for.
    pub(super) predicates: BTreeSet<Predicate>,
    pub(super) next_aggregate: usize,
    pub(super) dependency_projection: bool,
    pub(super) location: Location,
}
impl Compiler<'_> {
    /// The compilation's shared name for the predicate: every pattern of one
    /// predicate refers to one allocation, as an admitted program's do.
    fn shared(&mut self, predicate: Predicate) -> Predicate {
        if let Some(shared) = self.predicates.get(&predicate) {
            return shared.clone();
        }
        self.predicates.insert(predicate.clone());
        predicate
    }

    /// Keep the rewritten owner private until every normalization step has succeeded.
    fn normalize_statement(
        &mut self,
        statement: &WithProvenance<Statement>,
        constants: &BTreeMap<String, Symbol>,
    ) -> Result<SourceProgram, ExpansionFailure> {
        let mut normalizer = Normalizer {
            constants,
            budget: self.budget,
            location: self.location,
            failure: None,
        };
        let rewritten = rewrite(
            SourceProgram::of_nodes([statement.clone()]),
            &mut normalizer,
        );
        match normalizer.failure {
            Some(error) => Err(error),
            None => Ok(rewritten),
        }
    }

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
            body_variables: 0,
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
            let source = element.get();
            let fields: Vec<_> = std::iter::once(source.weight().term())
                .chain(source.weight().priority())
                .chain(source.terms())
                .collect();
            for (terms, condition) in self.local_alternatives(&fields, source.condition())? {
                let mut terms = terms.into_iter();
                let mut weight =
                    themelios_program::program::weight(terms.next().expect("weight field"));
                if source.weight().priority().is_some() {
                    weight = weight.at_priority(terms.next().expect("priority field"));
                }
                let alternative = OptimizeElement::new(weight, terms, condition);
                self.budget.charge(
                    ExpansionResource::Origins,
                    evidence.len() as u128,
                    self.location,
                )?;
                objectives.push(self.objective(&alternative, evidence.clone(), polarity)?);
            }
        }
        Ok(())
    }
    fn objective(
        &mut self,
        element: &OptimizeElement,
        origins: Vec<Location>,
        polarity: WeightPolarity,
    ) -> Result<ObjectiveIr, FormulaFailure> {
        if self.element_needs_scope(element)? {
            return self.scoped_element(element, origins, polarity);
        }
        let mut variables = Variables::default();
        let mut positive = Vec::new();
        let mut filters = Vec::new();
        let mut condition = Vec::new();
        for literal in element.condition().literals() {
            match &literal.get().inner {
                LiteralInner::Atom(atom) => {
                    let negation = literal.get().negation;
                    let pattern = self.atom(
                        atom.get(),
                        &mut variables,
                        negation == DefaultNegation::None,
                    )?;
                    if negation == DefaultNegation::None {
                        positive.push(pattern.clone());
                    }
                    condition.push(LiteralIr::Atom(negation, pattern));
                }
                LiteralInner::Comparison(comparison) => {
                    let mut steps = comparison.get().steps();
                    let (relation, right) = steps.next().expect("comparison has step");
                    if literal.get().negation != DefaultNegation::None
                        || steps.next().is_some()
                        || !matches!(relation, Relation::Eq | Relation::Neq)
                        || !matches!(
                            comparison.get().first(),
                            Term::Variable(_) | Term::Symbolic(_)
                        )
                        || !matches!(right, Term::Variable(_) | Term::Symbolic(_))
                    {
                        condition.push(self.comparison_guard(
                            comparison.get(),
                            literal.get().negation,
                            &mut variables,
                        )?);
                        continue;
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
                LiteralInner::True | LiteralInner::False => {
                    condition.push(self.literal(literal.get(), &mut variables)?);
                }
            }
        }
        let weight = self.objective_field(element.weight().term(), &mut variables)?;
        // Nonnumeric literals follow the same resolved-value contract as bound
        // weights: they supply no contribution or numeric priority witness.
        // Still admit the whole element, including priority, tuple and safety,
        // before completed grounding determines objective presence.
        let priority = self.objective_priority(element.weight().priority(), &mut variables)?;
        let tuple = element
            .terms()
            .map(|term| self.objective_field(term, &mut variables))
            .collect::<Result<Vec<_>, _>>()?;
        variables.safety(self.location)?;
        let count = variables.count;
        Ok(ObjectiveIr {
            weight,
            priority,
            tuple,
            positive,
            filters,
            polarity,
            priority_sources: BTreeSet::new(),
            needs_eligibility_query: false,
            condition: ObjectiveCondition::Literals(condition),
            variables: count,
            origins,
            location: self.location,
        })
    }
    fn objective_priority(
        &mut self,
        priority: Option<&Term>,
        variables: &mut Variables,
    ) -> Result<Expression, FormulaFailure> {
        match priority {
            None => Ok(scalar_expression(&CoreTerm::Constant(Value::Number(0)))),
            Some(Term::Symbolic(Symbol::Number(priority))) => Ok(scalar_expression(
                &CoreTerm::Constant(Value::Number(*priority)),
            )),
            Some(Term::Symbolic(Symbol::Infimum)) => {
                Ok(scalar_expression(&CoreTerm::Constant(Value::Infimum)))
            }
            Some(Term::Symbolic(Symbol::Supremum)) => {
                Ok(scalar_expression(&CoreTerm::Constant(Value::Supremum)))
            }
            Some(term) => self.expression(term, variables),
        }
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

    fn objective_field(
        &mut self,
        term: &Term,
        variables: &mut Variables,
    ) -> Result<ObjectiveField, FormulaFailure> {
        match term {
            Term::Symbolic(Symbol::Infimum | Symbol::Supremum) => {
                self.budget
                    .charge(ExpansionResource::TermWork, 1, self.location)?;
                Ok(ObjectiveField::Term(CoreTerm::Constant(
                    if matches!(term, Term::Symbolic(Symbol::Infimum)) {
                        Value::Infimum
                    } else {
                        Value::Supremum
                    },
                )))
            }
            Term::Variable(_) | Term::Symbolic(_) => self
                .objective_term(term, variables)
                .map(ObjectiveField::Term),
            _ => self
                .expression(term, variables)
                .map(ObjectiveField::Expression),
        }
    }
    pub(super) fn rule(
        &mut self,
        rule: &Rule,
        origins: Vec<Location>,
        choice_source: Option<&Choice>,
    ) -> Result<RuleIr, FormulaFailure> {
        let mut variables = Variables::default();
        let mut body = Vec::new();
        self.body_literals(rule.body().get(), &mut variables, &mut body)?;
        // Source head names are global even when their value is only needed
        // after body selection. Declare them before cloning element-local scopes.
        self.head_globals(rule.head().get(), &mut variables)?;
        let aggregate_guards = self.body_guards(rule.body().get(), &mut variables, &mut body)?;
        let choice_guards = self.head_guards(rule.head().get(), &mut variables, &mut body)?;
        let assignments =
            self.assignment_targets(rule.body().get(), &aggregate_guards, &mut variables)?;
        self.bindings(&mut body, &mut variables)?;
        variables.safety(self.location)?;
        self.body_aggregates(
            rule.body().get(),
            aggregate_guards,
            assignments,
            &variables,
            &mut body,
        )?;
        self.body_conditionals(rule.body().get(), &variables, &mut body)?;
        let body_variables = variables.count;
        let mut head_values = Vec::new();
        let ordinary = self.ordinary_head(rule.head().get(), &mut variables, &mut head_values)?;
        // Only fresh head-value targets are allocated after the body frame.
        // Reuse the binding scheduler; no original body expression can name this suffix.
        self.bindings(&mut head_values, &mut variables)?;
        variables.safety(self.location)?;
        body.extend(head_values);
        let bindings =
            self.assignment_plan(&body, variables.count, body_variables, &choice_guards)?;
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
            body_variables,
            bindings,
            variables: variables.count,
            origins,
            location: self.location,
        })
    }

    fn body_literals(
        &mut self,
        source: &Body,
        variables: &mut Variables,
        body: &mut Vec<LiteralIr>,
    ) -> Result<(), FormulaFailure> {
        for element in source.elements() {
            match element.get() {
                BodyElement::Literal(literal) => {
                    self.literal_into(literal, variables, body)?;
                }
                BodyElement::Conditional(conditional) => {
                    self.conditional_syntax(conditional)?;
                }
                BodyElement::Aggregate { .. } => {}
                _ => return Err(unsupported(ProfileFeature::BodyElement, self.location).into()),
            }
        }
        Ok(())
    }

    fn head_guards(
        &mut self,
        head: &Head,
        variables: &mut Variables,
        values: &mut Vec<LiteralIr>,
    ) -> Result<Vec<AggregateGuard>, FormulaFailure> {
        match head {
            Head::Choice(choice) => self.choice_guards(choice, variables, values),
            Head::Aggregate(aggregate) => self.guards(
                aggregate
                    .left_guard()
                    .map(themelios_program::provenance::WithProvenance::get),
                aggregate
                    .right_guard()
                    .map(themelios_program::provenance::WithProvenance::get),
                variables,
                values,
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
            let boolean = if matches!(
                element.get().literal().inner,
                LiteralInner::True | LiteralInner::False
            ) {
                self.dependency_projection = true;
                Some(self.boolean_occurrences(element.get(), source_booleans.next())?)
            } else {
                None
            };
            for alternative in self.condition_alternatives(element.get().condition())? {
                let mut local = variables.clone();
                self.head_global_literal(element.get().literal(), &mut local)?;
                let mut condition = self.condition(&alternative, &mut local)?;
                let (head, body_variables) =
                    self.element_head(element.get().literal(), &mut local, &mut condition)?;
                let key = match &boolean {
                    None => HeadElementKey::Atom,
                    Some(origins) => {
                        self.budget.charge(
                            ExpansionResource::Origins,
                            origins.len() as u128,
                            self.location,
                        )?;
                        HeadElementKey::BooleanOccurrences(origins.clone())
                    }
                };
                self.variable_limit(&local)?;
                local.safety(self.location)?;
                elements.push(Element {
                    key,
                    head,
                    condition,
                    body_variables,
                    variables: local.count,
                });
            }
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
        let predicate = self.shared(predicate);
        AtomPattern::new(predicate, terms).map_err(|error| {
            AdmissionFailure::Construction {
                error,
                location: self.location,
            }
            .into()
        })
    }
    /// Compile a source term after `prepare`'s bottom-up normalization: closed
    /// arithmetic has already become a value or produced a located failure.
    /// Admitted pool selections preserve this invariant. Bound source slots
    /// remain variable nodes; interval lowering supplies separate variable plans.
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

/// Closed scalar/flat range facts keep their direct finite expansion. A
/// constructor containing an interval uses the same scoped generator as heads.
fn generated_fact(statement: &Statement) -> bool {
    let Statement::Rule(rule) = statement else {
        return false;
    };
    let Head::Literal(literal) = rule.head().get() else {
        return false;
    };
    let LiteralInner::Atom(atom) = &literal.inner else {
        return false;
    };
    atom.get()
        .argument_terms()
        .flat_map(Term::subterms)
        .any(|term| {
            matches!(
                term,
                Term::Function { .. }
                    | Term::Tuple(_)
                    | Term::UnaryOperation { .. }
                    | Term::BinaryOperation { .. }
                    | Term::Absolute(_)
            )
        })
}
