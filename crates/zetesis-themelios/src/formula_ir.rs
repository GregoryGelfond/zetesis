//! Bounded normalization and explicit global/element-local variable scopes.

#[cfg(test)]
mod assignment_plan_tests;

#[cfg(test)]
mod shared_names_tests;

mod objective_scope;

mod domain;

use std::collections::{BTreeMap, BTreeSet};

use crate::formula_support::components::{
    Filter, Pattern as AtomPattern, Predicate, Term as CoreTerm,
};
use crate::{ProgramSite, StatementId};
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
use zetesis_core::Value;
use zetesis_objective::{ObjectiveTemplateRef, WeightPolarity};

use crate::diagnostic::unsupported;
use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_support::{Counters, GroundingWork, SupportCatalog, components};
use crate::{
    AdmissionFailure, AdmissionOptions, ExpansionFailure, ExpansionResource, FormulaFailure,
    FormulaLimits, FormulaResource, ProfileFeature, compile, extended, fact_expansion,
};
use zetesis_core::catalog::TermKey;

pub(crate) struct Prepared {
    pub analysis: themelios_analysis::Analysis,
    pub analysis_basis: crate::AnalysisBasis,
    pub analyzed: SourceProgram,
    pub rules: Vec<RuleIr>,
    pub projection: Vec<RuleIr>,
    pub project_selection: crate::ProjectSelection,
    pub objectives: Vec<ObjectiveIr>,
    pub objective_declarations: Vec<ProgramSite>,
    /// Extrema tuple carriers selected for an optional numeric-weight precision
    /// refinement after support completion.
    pub objective_extrema: BTreeSet<usize>,
    /// Written constraints over a keyed value that were asked as the one atom
    /// their key admits, in place, during this preparation.
    pub keyed_constraints: usize,
    /// How the key analysis that asked them ended.
    pub key_analysis: crate::KeyAnalysis,
}
/// One original objective element, before its pooled or weak-body alternatives.
/// Its first lowered position is unique within the prepared objective vector;
/// all alternatives remain contiguous. Diagnostic spans do not define identity.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ObjectiveFamily(usize);

pub(crate) struct ObjectiveIr {
    pub family: ObjectiveFamily,
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
    pub origins: Vec<ProgramSite>,
    pub location: ProgramSite,
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
pub(crate) struct RuleIr {
    pub head: HeadIr,
    pub body: Vec<LiteralIr>,
    /// Original body slots precede a suffix used only by generated head values.
    /// Local body scopes are compiled before that suffix exists.
    pub body_variables: usize,
    pub bindings: Option<crate::formula_assignment_plan::Plan>,
    pub variables: usize,
    pub origins: Vec<ProgramSite>,
    pub location: ProgramSite,
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
/// One original element within its enclosing local collection. Alternatives
/// remain contiguous and share this identity. It is not comparable across
/// collections or outer bindings, and diagnostic spans do not determine it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct LocalFamily(pub(crate) usize);

pub(crate) struct Element {
    pub family: LocalFamily,
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
/// Boolean elements identify each pool-expanded entry before local grounding.
pub(crate) enum HeadElementKey {
    Atom,
    Occurrence(usize),
    Tuple(Vec<CoreTerm>),
}
impl HeadElementKey {
    pub(crate) fn tuple(&self) -> Option<&[CoreTerm]> {
        match self {
            Self::Tuple(terms) => Some(terms),
            Self::Atom | Self::Occurrence(_) => None,
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
    /// The complement of an ordinary singleton comparison head. Its truth
    /// selects emitted constraints, but cannot exclude original body scopes
    /// from source-family validation. It never supplies a binding instruction.
    HeadGuard(crate::formula_guard::Guard),
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
    pub(crate) fn predicate<'a>(
        &self,
        view: zetesis_core::TemplateComponentsRef<'a>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<zetesis_core::catalog::PredicateRef<'a>, FormulaFailure> {
        match self {
            Self::Arguments { predicate, .. } => predicate.get(view, limits, counters, location),
            Self::Witnesses { atom, .. } => atom
                .get(view, limits, counters, location)
                .map(zetesis_core::PatternRef::predicate),
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
    pub family: LocalFamily,
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
    Constant(components::Scalar),
    Variable(usize),
    Unary(UnaryOp, usize),
    Binary(BinaryOp, usize, usize),
    Absolute(usize),
    Constructor(Box<crate::formula_value::Constructor>),
}

/// Logical compilation limits shared by source and canonical-program admission.
/// Source identity and syntax limits stay at their respective input boundaries.
#[derive(Clone, Copy)]
pub(crate) struct CompilationOptions {
    pub(crate) max_body_elements: usize,
    pub(crate) core_limits: zetesis_core::AdmissionLimits,
}

impl From<AdmissionOptions> for CompilationOptions {
    fn from(options: AdmissionOptions) -> Self {
        Self {
            max_body_elements: options.max_body_elements,
            core_limits: options.core_limits,
        }
    }
}

impl From<crate::BundleAdmissionOptions> for CompilationOptions {
    fn from(options: crate::BundleAdmissionOptions) -> Self {
        Self {
            max_body_elements: options.max_body_elements,
            core_limits: options.core_limits,
        }
    }
}

impl From<crate::ProgramAdmissionOptions> for CompilationOptions {
    fn from(options: crate::ProgramAdmissionOptions) -> Self {
        Self {
            max_body_elements: options.max_body_elements,
            core_limits: options.core_limits,
        }
    }
}

/// Mutable logical admission capabilities, borrowed only until the compiled
/// program is ready to return to its owning preparation receipt.
pub(crate) struct PreparationContext<'a> {
    pub(crate) options: CompilationOptions,
    pub(crate) budget: &'a mut Budget,
    pub(crate) catalog: &'a mut SupportCatalog,
    pub(crate) work: GroundingWork<'a>,
}

impl PreparationContext<'_> {
    pub(crate) fn prepare(
        self,
        source: &SourceProgram,
        project_selection: crate::ProjectSelection,
    ) -> Result<Prepared, FormulaFailure> {
        let Self {
            options,
            budget,
            catalog,
            work,
        } = self;
        let GroundingWork {
            limits,
            counters,
            location: fallback,
        } = work;
        let constants = extended::resolve(source, budget, fallback)?;
        let mut parts = Parts::default();
        let admission = catalog.component_admission(limits, counters, fallback)?;
        let domain = domain::Domain::new(&admission, limits, counters, fallback)?;
        let mut compiler = Compiler {
            options,
            limits,
            budget,
            source: admission,
            counters,
            domain,
            next_aggregate: 0,
            dependency_projection: false,
            location: fallback,
        };
        let emitted_owners = compiler.compile_program(source, &constants, &mut parts, fallback)?;
        let (mut analyzed, owners) = crate::formula_keys::owners::Owners::collect(
            std::mem::take(&mut parts.analyzed),
            emitted_owners,
            compiler.budget,
            fallback,
        )?;
        let asked =
            crate::formula_keys::ask_all(&analyzed, &owners, limits, compiler.budget, fallback)?;
        let keyed_constraints = asked.rules.len();
        if keyed_constraints > 0 {
            analyzed = replace_asked(
                &mut compiler,
                &constants,
                &mut parts,
                &analyzed,
                &owners,
                asked.rules,
                fallback,
            )?;
        }
        let analysis =
            crate::formula_analysis::analyze(&analyzed, limits, compiler.budget, fallback)?;
        let components = compiler
            .source
            .components(limits, compiler.counters, fallback)?;
        let objective_extrema = crate::formula_objective_dependencies::check(
            &parts.rules,
            &mut parts.objectives,
            &analysis,
            components,
            limits,
            compiler.counters,
        )?;
        validate_objectives(
            &parts.objectives,
            components,
            limits,
            compiler.budget,
            compiler.counters,
        )?;
        let analysis_basis = if compiler.dependency_projection {
            crate::AnalysisBasis::DependencyProjection
        } else {
            crate::AnalysisBasis::NormalizedProgram
        };
        drop(compiler.domain);
        compiler
            .source
            .finish(limits, compiler.counters, fallback)?;
        Ok(Prepared {
            analysis,
            analysis_basis,
            analyzed,
            rules: parts.rules,
            projection: parts.projection,
            project_selection,
            objectives: parts.objectives,
            objective_declarations: parts.objective_declarations,
            objective_extrema,
            keyed_constraints,
            key_analysis: asked.analysis,
        })
    }
}

/// What the statements compile to, in source order: the rules and the
/// projection's, the analyzed statements, the objectives and their
/// declarations, and the projection nodes charged so far.
#[derive(Default)]
struct Parts {
    rules: Vec<RuleIr>,
    projection: Vec<RuleIr>,
    analyzed: Vec<WithProvenance<Statement>>,
    pool_projection_nodes: u128,
    objectives: Vec<ObjectiveIr>,
    objective_declarations: Vec<ProgramSite>,
}

/// Replace each written constraint's rules and analyzed statement by those
/// of the constraints asked in its place, compiled as any statement is,
/// under the written constraint's provenance and the transformation's tag.
/// The analyzed statements of the rest are kept as they are; the program is
/// rebuilt once. The written constraint's charges stay charged: it was
/// compiled.
fn replace_asked(
    compiler: &mut Compiler<'_>,
    constants: &BTreeMap<String, Symbol>,
    parts: &mut Parts,
    analyzed: &SourceProgram,
    owners: &crate::formula_keys::owners::Owners,
    asked: BTreeMap<StatementId, (themelios_program::provenance::Provenance, Vec<Rule>)>,
    fallback: ProgramSite,
) -> Result<SourceProgram, FormulaFailure> {
    parts.rules.retain(|rule| {
        rule.location
            .statement_id()
            .is_none_or(|owner| !asked.contains_key(&owner))
    });
    parts.analyzed = analyzed
        .statements()
        .enumerate()
        .filter(|(index, _)| {
            owners
                .at(*index)
                .is_none_or(|owner| !asked.contains_key(&owner))
        })
        .map(|(_, carrier)| carrier.clone())
        .collect();
    let tag = themelios_program::provenance::Provenance::from(Origin::Transformed(
        TransformTag::new("zetesis-keyed-constraint"),
    ));
    for (owner, (provenance, rules)) in asked {
        for rule in rules {
            let carrier =
                WithProvenance::new(Statement::Rule(rule), provenance.clone().merge(tag.clone()));
            compiler.compile(&carrier, constants, parts, fallback.with_statement(owner))?;
        }
    }
    Ok(SourceProgram::of_nodes(std::mem::take(&mut parts.analyzed)))
}

fn validate_objectives(
    objectives: &[ObjectiveIr],
    components: zetesis_core::TemplateComponentsRef<'_>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<(), FormulaFailure> {
    for (index, objective) in objectives.iter().enumerate() {
        let mut positive = Vec::new();
        crate::formula_pattern::reserve(
            &mut positive,
            objective.positive.len(),
            budget,
            objective.location,
        )?;
        for pattern in &objective.positive {
            positive.push(pattern.get(components, limits, counters, objective.location)?);
        }
        let mut filters = Vec::new();
        crate::formula_pattern::reserve(
            &mut filters,
            objective.filters.len(),
            budget,
            objective.location,
        )?;
        for filter in &objective.filters {
            filters.push(filter.get(components, limits, counters, objective.location)?);
        }
        if let ObjectiveCondition::Body { filters, .. } = objective.condition {
            ObjectiveTemplateRef::validate_shape(
                objective.tuple.len(),
                positive.iter().copied(),
                filters,
                limits.objective,
                index,
            )
        } else {
            ObjectiveTemplateRef::validate_scope(
                objective.tuple.len(),
                positive.iter().copied(),
                filters.iter().copied(),
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
    location: ProgramSite,
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
    pub(super) fn safety(&self, location: ProgramSite) -> Result<(), FormulaFailure> {
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
    pub(super) options: CompilationOptions,
    pub(super) limits: &'a FormulaLimits,
    pub(super) budget: &'a mut Budget,
    pub(super) source: components::Admission<'a>,
    pub(super) counters: &'a mut Counters,
    pub(super) domain: domain::Domain,
    pub(super) next_aggregate: usize,
    pub(super) dependency_projection: bool,
    pub(super) location: ProgramSite,
}
impl Compiler<'_> {
    /// Compile each original statement and record its emitted analysis family
    /// before canonical collection can reorder or merge those carriers.
    fn compile_program(
        &mut self,
        source: &SourceProgram,
        constants: &BTreeMap<String, Symbol>,
        parts: &mut Parts,
        fallback: ProgramSite,
    ) -> Result<Vec<Option<StatementId>>, FormulaFailure> {
        let mut emitted_owners = Vec::new();
        for (index, carrier) in source.statements().enumerate() {
            if matches!(
                carrier.get(),
                Statement::Const(_) | Statement::Defined(_) | Statement::Show(_)
            ) {
                continue;
            }
            let owner = StatementId::new(index);
            let site = fallback.with_statement(owner);
            let start = parts.analyzed.len();
            self.compile(carrier, constants, parts, site)?;
            crate::formula_keys::owners::record(
                &mut emitted_owners,
                owner,
                parts.analyzed.len() - start,
                self.budget,
                site,
            )?;
        }
        Ok(emitted_owners)
    }

    /// Compile one statement into the parts: a projection or objective
    /// declaration, expanded facts, or its rules, after normalizing it.
    fn compile(
        &mut self,
        carrier: &WithProvenance<Statement>,
        constants: &BTreeMap<String, Symbol>,
        parts: &mut Parts,
        fallback: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        self.location = extended::origin(carrier, fallback);
        let rewritten = self.normalize_statement(carrier, constants)?;
        let statement = rewritten.statements().next().expect("rewrite keeps a rule");
        let origins = extended::program_sites(carrier, self.location);
        if self.project_statement(
            statement,
            &origins,
            &mut parts.pool_projection_nodes,
            &mut parts.projection,
        )? {
            return Ok(());
        }
        if let Some(observation) = self.objective_statement(
            statement,
            &origins,
            &mut parts.objectives,
            &mut parts.objective_declarations,
            &mut parts.pool_projection_nodes,
        )? {
            parts.analyzed.extend(observation);
            return Ok(());
        }
        if let Some(facts) = if generated_fact(statement.get()) {
            None
        } else {
            fact_expansion::facts(statement, self.budget, self.location)?
        } {
            self.budget.charge(
                ExpansionResource::Origins,
                (facts.len() as u128).saturating_mul(origins.len() as u128),
                self.location,
            )?;
            for fact in facts {
                let head = fact.head().expect("expanded facts have a head").clone();
                parts.analyzed.push(crate::formula_analysis::fact(
                    &head,
                    statement,
                    self.location,
                )?);
                parts.rules.push(self.fact_rule(&head, &origins)?);
            }
        } else {
            self.source_rules(
                statement,
                &origins,
                &mut parts.pool_projection_nodes,
                &mut parts.rules,
                &mut parts.analyzed,
            )?;
        }
        Ok(())
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
        head: &zetesis_core::AtomPattern,
        origins: &[ProgramSite],
    ) -> Result<RuleIr, FormulaFailure> {
        let head = self.admit_fact_pattern(head)?;
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
        origins: &[ProgramSite],
        body_origins: &[ProgramSite],
        objectives: &mut Vec<ObjectiveIr>,
        declarations: &mut Vec<ProgramSite>,
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
            let family = ObjectiveFamily(objectives.len());
            let mut evidence: Vec<_> = element
                .provenance()
                .origins()
                .filter_map(|origin| {
                    if let Origin::Parsed(location) = origin {
                        Some(self.location.with_location(*location))
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
                objectives.push(self.objective(
                    &alternative,
                    evidence.clone(),
                    polarity,
                    family,
                )?);
            }
        }
        Ok(())
    }
    fn objective(
        &mut self,
        element: &OptimizeElement,
        origins: Vec<ProgramSite>,
        polarity: WeightPolarity,
        family: ObjectiveFamily,
    ) -> Result<ObjectiveIr, FormulaFailure> {
        if self.element_needs_scope(element)? {
            return self.scoped_element(element, origins, polarity, family);
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
                        positive.push(pattern);
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
                        Self::scalar_expression(&left),
                        relation,
                        Self::scalar_expression(&right),
                    ));
                    filters.push(self.source.filter(
                        relation == Relation::Eq,
                        left,
                        right,
                        self.limits,
                        self.counters,
                        self.location,
                    )?);
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
            family,
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
            None => self.constant_expression(0),
            Some(Term::Symbolic(Symbol::Number(priority))) => self.constant_expression(*priority),
            Some(Term::Symbolic(Symbol::Infimum)) => {
                let value = self.scalar(&Value::Infimum)?;
                Ok(Self::scalar_expression(&CoreTerm::Constant(value)))
            }
            Some(Term::Symbolic(Symbol::Supremum)) => {
                let value = self.scalar(&Value::Supremum)?;
                Ok(Self::scalar_expression(&CoreTerm::Constant(value)))
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
            Term::Symbolic(symbol) => Ok(CoreTerm::Constant(
                self.scalar(&compile::scalar(symbol, self.location)?)?,
            )),
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
                let value = if matches!(term, Term::Symbolic(Symbol::Infimum)) {
                    Value::Infimum
                } else {
                    Value::Supremum
                };
                Ok(ObjectiveField::Term(CoreTerm::Constant(
                    self.scalar(&value)?,
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
        origins: Vec<ProgramSite>,
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
                    CoreTerm::Constant(self.root_scalar(&value)?)
                }
                _ => return Err(unsupported(ProfileFeature::Term, self.location).into()),
            });
        }
        let predicate = self.predicate(
            atom.name.as_str(),
            arguments.len(),
            crate::coherence::core_sign(atom.sign),
        )?;
        self.pattern_from_parts(predicate, &terms)
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
                        let key = self.value(&value)?;
                        Operation::Constant(self.source.scalar_key(
                            &key,
                            self.limits,
                            self.counters,
                            self.location,
                        )?)
                    }
                    TermParts::Variable(variable) => {
                        let slot = variables.slot(&variable);
                        self.variable_limit(variables)?;
                        Operation::Variable(slot)
                    }
                    TermParts::UnaryOperation { operator, argument } => {
                        if operator == UnaryOp::Negate
                            && let Operation::Constructor(constructor) = &mut nodes[argument]
                            && let Some(shape) = self.source.negate_constructor(
                                constructor.shape,
                                self.limits,
                                self.counters,
                                self.location,
                            )?
                        {
                            constructor.shape = shape;
                            return Ok(argument);
                        }
                        Operation::Unary(operator, argument)
                    }
                    TermParts::Function { name, arguments } => {
                        Operation::Constructor(Box::new(crate::formula_value::Constructor {
                            shape: self.source.constructor(
                                zetesis_core::ValueNodeRef::Function {
                                    name: name.as_str(),
                                    sign: zetesis_core::Sign::Positive,
                                    arity: arguments.len(),
                                },
                                self.limits,
                                self.counters,
                                self.location,
                            )?,
                            arguments,
                        }))
                    }
                    TermParts::Tuple(arguments) => {
                        Operation::Constructor(Box::new(crate::formula_value::Constructor {
                            shape: self.source.constructor(
                                zetesis_core::ValueNodeRef::Tuple {
                                    arity: arguments.len(),
                                },
                                self.limits,
                                self.counters,
                                self.location,
                            )?,
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
    fn admit_fact_pattern(
        &mut self,
        pattern: &zetesis_core::AtomPattern,
    ) -> Result<AtomPattern, FormulaFailure> {
        ceiling(
            FormulaResource::Arity,
            pattern.terms().len() as u128,
            self.options.core_limits.max_predicate_arity as u128,
            self.location,
        )?;
        let terms = pattern
            .terms()
            .iter()
            .map(|term| match term {
                zetesis_core::Term::Variable(slot) => Ok(CoreTerm::Variable(*slot)),
                zetesis_core::Term::Constant(value) => {
                    self.root_scalar(value).map(CoreTerm::Constant)
                }
            })
            .collect::<Result<Vec<_>, FormulaFailure>>()?;
        let predicate = self.source.predicate(
            pattern.predicate().into(),
            self.limits,
            self.counters,
            self.location,
        )?;
        self.pattern_from_parts(predicate, &terms)
    }

    pub(super) fn predicate(
        &mut self,
        name: &str,
        arity: usize,
        sign: zetesis_core::Sign,
    ) -> Result<Predicate, FormulaFailure> {
        let predicate = zetesis_core::Predicate::with_sign(name, arity, sign).map_err(|error| {
            AdmissionFailure::Construction {
                error,
                location: self.location,
            }
        })?;
        self.source.predicate(
            (&predicate).into(),
            self.limits,
            self.counters,
            self.location,
        )
    }

    pub(super) fn pattern_from_parts(
        &mut self,
        predicate: Predicate,
        terms: &[CoreTerm],
    ) -> Result<AtomPattern, FormulaFailure> {
        self.source
            .pattern(predicate, terms, self.limits, self.counters, self.location)
    }

    pub(super) fn scalar(&mut self, value: &Value) -> Result<components::Scalar, FormulaFailure> {
        self.source
            .scalar(value.into(), self.limits, self.counters, self.location)
    }

    pub(super) fn root_scalar(
        &mut self,
        value: &Value,
    ) -> Result<components::Scalar, FormulaFailure> {
        let key = self.value(value)?;
        self.source
            .scalar_key(&key, self.limits, self.counters, self.location)
    }

    /// Admit one explicit source root. Interned descendants do not enlarge this
    /// scope's domain; objective and projection scopes own separate selections.
    pub(super) fn value(&mut self, value: &Value) -> Result<TermKey, FormulaFailure> {
        let key = self
            .source
            .import(value.into(), self.limits, self.counters, self.location)?;
        if self.domain.insert(
            &key,
            &self.source,
            self.limits
                .max_domain_values
                .min(self.options.core_limits.max_domain_values),
            self.limits,
            self.counters,
            self.location,
        )? {
            self.budget.charge(
                ExpansionResource::ScalarBytes,
                value_bytes(value),
                self.location,
            )?;
        }
        Ok(key)
    }

    pub(super) fn empty_domain(&self) -> Result<domain::Domain, FormulaFailure> {
        domain::Domain::new(&self.source, self.limits, self.counters, self.location)
    }

    pub(super) fn scalar_expression(term: &CoreTerm) -> Expression {
        let node = match term {
            CoreTerm::Variable(variable) => Operation::Variable(*variable),
            CoreTerm::Constant(value) => Operation::Constant(*value),
        };
        Expression { nodes: vec![node] }
    }

    pub(super) fn constant_expression(&mut self, value: i32) -> Result<Expression, FormulaFailure> {
        let scalar = self.scalar(&Value::Number(value))?;
        Ok(Self::scalar_expression(&CoreTerm::Constant(scalar)))
    }

    pub(super) fn scalar_number(
        &mut self,
        scalar: components::Scalar,
    ) -> Result<Option<i32>, FormulaFailure> {
        let value = self
            .source
            .scalar_ref(scalar, self.limits, self.counters, self.location)?;
        Ok(match value.descriptor() {
            zetesis_core::ValueNodeRef::Number(number) => Some(number),
            _ => None,
        })
    }
}

pub(crate) fn value_bytes(value: &Value) -> u128 {
    match value {
        Value::Infimum | Value::Supremum | Value::Number(_) => 0,
        Value::Structured(value) => value.payload_bytes() as u128,
        Value::String(text) | Value::Symbol(text) => text.len() as u128,
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
                parsed.location(weak.syntax().text_range()).into(),
            )?;
            *count += 1;
            continue;
        }
        let ast::Statement::Optimize(optimize) = statement else {
            continue;
        };
        let location: ProgramSite = parsed.location(optimize.syntax().text_range()).into();
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
                parsed.location(element.syntax().text_range()).into(),
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
