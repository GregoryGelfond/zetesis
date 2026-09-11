use std::fmt;
use std::sync::Arc;

use zetesis_core::{AtomPattern, Filter, Term};

use crate::Condition;

/// Numeric normalization applied before constructing a global contribution key.
/// All scores remain minimization costs, including negated maximize weights.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WeightPolarity {
    /// Retain the resolved numeric weight (minimize and weak constraints).
    #[default]
    AsWritten,
    /// Negate the resolved numeric weight using checked i32 arithmetic.
    Negated,
}
impl WeightPolarity {
    /// Normalize a numeric contribution. `None` means negating `i32::MIN`
    /// cannot be represented; it never means an ignored nonnumeric term.
    #[must_use]
    pub const fn normalize(self, weight: i32) -> Option<i32> {
        match self {
            Self::AsWritten => Some(weight),
            Self::Negated => weight.checked_neg(),
        }
    }
}

/// A lifted minimization element with positive relational bindings and an
/// optional closed model query. Variable IDs are local; comparisons and closed
/// conditions never establish variable safety.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectiveTemplate {
    weight: Term,
    polarity: WeightPolarity,
    priority: i32,
    tuple: Vec<Term>,
    positive: Vec<AtomPattern>,
    filters: Vec<Filter>,
    condition: Condition,
}
impl ObjectiveTemplate {
    /// Assemble a template. Program admission checks dimensions and safety.
    /// An omitted source priority is represented by the caller as zero.
    #[must_use]
    pub fn new(
        weight: Term,
        priority: i32,
        tuple: Vec<Term>,
        positive: Vec<AtomPattern>,
        filters: Vec<Filter>,
    ) -> Self {
        Self {
            weight,
            polarity: WeightPolarity::AsWritten,
            priority,
            tuple,
            positive,
            filters,
            condition: Condition::default(),
        }
    }
    /// Set the numeric normalization while retaining the lifted source term.
    /// Normalization precedes global key deduplication in every evaluator.
    #[must_use]
    pub fn with_weight_polarity(mut self, polarity: WeightPolarity) -> Self {
        self.polarity = polarity;
        self
    }
    /// Require this closed query in addition to the positive bindings and
    /// scalar filters. It reads the original supplied model and has no effect
    /// on this element's admitted priority slot or complete contribution key.
    #[must_use]
    pub fn with_condition(mut self, condition: Condition) -> Self {
        self.condition = condition;
        self
    }
    /// Closed model query; an empty query is true.
    #[must_use]
    pub const fn condition(&self) -> &Condition {
        &self.condition
    }
    /// Numeric normalization applied to the resolved source weight.
    #[must_use]
    pub const fn weight_polarity(&self) -> WeightPolarity {
        self.polarity
    }
    /// Original scalar weight, required numeric only for an active contribution.
    /// Apply [`Self::weight_polarity`] before interpreting a numeric key or cost.
    #[must_use]
    pub const fn weight(&self) -> &Term {
        &self.weight
    }
    /// Fixed priority; larger priorities are compared first.
    #[must_use]
    pub const fn priority(&self) -> i32 {
        self.priority
    }
    /// Explicit tuple following the weight and priority.
    #[must_use]
    pub fn tuple(&self) -> &[Term] {
        &self.tuple
    }
    /// Ordinary positive conditions joined against supplied model atoms.
    #[must_use]
    pub fn positive(&self) -> &[AtomPattern] {
        &self.positive
    }
    /// Exact equality or inequality comparisons on bound scalar values.
    #[must_use]
    pub fn filters(&self) -> &[Filter] {
        &self.filters
    }
    /// Validate scoped data fields before a source priority has been resolved.
    /// Returns the number of densely numbered, positively bound variables.
    /// Numeric priority values do not affect these shape and safety obligations.
    /// `index` identifies the caller's original element in indexed errors.
    ///
    /// A source frontend may call this before evaluating its priority expression.
    /// That frontend owns expression safety, numeric evaluation and the complete
    /// priority carrier; every expression input must have an independent binder.
    /// The fields are borrowed and remain caller-owned. After resolving a numeric
    /// priority, construct a template and admit it through [`ObjectiveProgram::new`].
    ///
    /// # Errors
    /// Refuses shape limits, unsafe or sparse variable IDs, overflow or allocation.
    pub fn validate_fields(
        weight: &Term,
        tuple: &[Term],
        positive: &[AtomPattern],
        filters: &[Filter],
        limits: AdmissionLimits,
        index: usize,
    ) -> Result<usize, AdmissionError> {
        admit_fields(weight, tuple, positive, filters, limits, index)
    }

    /// Validate the relational scope and output width before evaluating source
    /// expressions. The source frontend must independently establish that every
    /// expression input has a binder in `positive`; expressions never add binders.
    /// This shares shape, dense-variable and filter checks with `validate_fields`.
    /// Resolved output values must still pass through [`ObjectiveProgram::new`].
    ///
    /// # Errors
    /// Refuses shape limits, unsafe or sparse variable IDs, overflow or allocation.
    pub fn validate_scope(
        tuple_width: usize,
        positive: &[AtomPattern],
        filters: &[Filter],
        limits: AdmissionLimits,
        index: usize,
    ) -> Result<usize, AdmissionError> {
        admit_scope(
            tuple_width,
            positive
                .iter()
                .flat_map(AtomPattern::terms)
                .chain(filters.iter().flat_map(|filter| {
                    let (left, right) = filter.terms();
                    [left, right]
                })),
            positive,
            filters,
            limits,
            index,
        )
    }
}

fn terms<'a>(
    weight: &'a Term,
    tuple: &'a [Term],
    positive: &'a [AtomPattern],
    filters: &'a [Filter],
) -> impl Iterator<Item = &'a Term> + Clone {
    std::iter::once(weight)
        .chain(tuple)
        .chain(positive.iter().flat_map(AtomPattern::terms))
        .chain(filters.iter().flat_map(|filter| {
            let (left, right) = filter.terms();
            [left, right]
        }))
}

/// Admission ceilings. Zero is a real ceiling, never an unlimited sentinel.
#[derive(Clone, Copy, Debug)]
pub struct AdmissionLimits {
    /// Maximum lifted elements across all objective directives.
    pub max_templates: usize,
    /// Maximum explicit scalar tuple components per element.
    pub max_tuple_width: usize,
    /// Maximum distinct, densely numbered local variables per element.
    pub max_variables_per_template: usize,
    /// Maximum positive conditions per element before any deduplication.
    pub max_positive_body: usize,
    /// Maximum predicate arity in an objective condition.
    pub max_predicate_arity: usize,
    /// Maximum comparisons per element.
    pub max_filters: usize,
    /// Closed model-query operations per template; zero permits empty queries.
    pub max_condition_nodes: usize,
}
impl Default for AdmissionLimits {
    fn default() -> Self {
        Self {
            max_templates: 100_000,
            max_tuple_width: 64,
            max_variables_per_template: 64,
            max_positive_body: 1_024,
            max_predicate_arity: 32,
            max_filters: 1_024,
            max_condition_nodes: 65_536,
        }
    }
}

/// The objective admission dimension that was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionResource {
    /// Lifted objective elements.
    Templates,
    /// Explicit tuple components.
    TupleWidth,
    /// Local variable indices.
    Variables,
    /// Positive relational conditions.
    PositiveBody,
    /// Predicate arguments.
    PredicateArity,
    /// Scalar comparisons.
    Filters,
    /// Closed model-query operations.
    ConditionNodes,
}

/// An objective program cannot enter the evaluator unless these checks pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionError {
    /// A configured shape ceiling was exceeded.
    Limit {
        /// Refused dimension.
        resource: AdmissionResource,
        /// Original input template, or `None` for the program-wide limit.
        template: Option<usize>,
        /// Observed or proposed count.
        actual: usize,
        /// Inclusive ceiling.
        limit: usize,
    },
    /// A weight/tuple/filter variable has no positive binding condition.
    UnsafeVariable {
        /// Original input template.
        template: usize,
        /// Unbound local variable.
        variable: usize,
    },
    /// Local variable indices have a gap below a used index.
    NonDenseVariable {
        /// Original input template.
        template: usize,
        /// Missing local index.
        variable: usize,
    },
    /// A variable index or count cannot be incremented on this host.
    Overflow {
        /// Original input template, if known.
        template: Option<usize>,
    },
    /// Fallible admission storage reservation failed.
    Allocation,
    /// The internal priority registry lacks a previously collected template.
    MissingPriority {
        /// Original input template whose priority could not be resolved.
        template: usize,
    },
    /// A closed model query references its own or a later operation.
    ConditionReference {
        /// Original input template.
        template: usize,
        /// Operation containing the invalid reference.
        node: usize,
        /// Operand that must precede the operation.
        operand: usize,
    },
}
impl AdmissionError {
    /// Resolve a refusal into the caller's parallel source-origin catalog.
    #[must_use]
    pub const fn template_index(&self) -> Option<usize> {
        match *self {
            Self::Limit { template, .. } | Self::Overflow { template } => template,
            Self::UnsafeVariable { template, .. }
            | Self::NonDenseVariable { template, .. }
            | Self::MissingPriority { template }
            | Self::ConditionReference { template, .. } => Some(template),
            Self::Allocation => None,
        }
    }
}
impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit {
                resource,
                template,
                actual,
                limit,
            } => write!(
                f,
                "objective {resource:?} count {actual} exceeds {limit} (template {template:?})"
            ),
            Self::UnsafeVariable { template, variable } => write!(
                f,
                "objective template {template} variable {variable} has no positive binding"
            ),
            Self::NonDenseVariable { template, variable } => write!(
                f,
                "objective template {template} is missing local variable {variable}"
            ),
            Self::Overflow { template } => write!(
                f,
                "objective admission arithmetic overflow (template {template:?})"
            ),
            Self::Allocation => f.write_str("objective admission storage reservation failed"),
            Self::MissingPriority { template } => {
                write!(f, "objective priority registry lacks template {template}")
            }
            Self::ConditionReference {
                template,
                node,
                operand,
            } => write!(
                f,
                "objective template {template} condition node {node} references nonpreceding operand {operand}"
            ),
        }
    }
}
impl std::error::Error for AdmissionError {}

#[derive(Debug)]
struct Data {
    templates: Vec<ObjectiveTemplate>,
    variables: Vec<usize>,
    priorities: Vec<i32>,
    slots: Vec<usize>,
    present: bool,
}

/// An immutable collection of all objective elements. Equal tuple keys from
/// separate templates/directives are globally coalesced during evaluation.
#[derive(Clone, Debug)]
pub struct ObjectiveProgram(Arc<Data>);
impl ObjectiveProgram {
    /// Admit templates in original order and mark the objective present even
    /// for an empty list. Use [`Self::none`] for an absent objective. Determining
    /// source-level objective presence belongs to the frontend, not this evaluator.
    ///
    /// # Errors
    /// Refuses shape limits, unsafe or sparse variable IDs, overflow or allocation.
    pub fn new(
        templates: Vec<ObjectiveTemplate>,
        limits: AdmissionLimits,
    ) -> Result<Self, AdmissionError> {
        check_bound(
            AdmissionResource::Templates,
            templates.len(),
            limits.max_templates,
            None,
        )?;
        let mut variables = reserved(templates.len())?;
        let mut priorities = reserved(templates.len())?;
        for (index, template) in templates.iter().enumerate() {
            variables.push(admit_template(template, limits, index)?);
            priorities.push(template.priority);
        }
        priorities.sort_unstable_by(|left, right| right.cmp(left));
        priorities.dedup();
        let mut slots = reserved(templates.len())?;
        for (index, template) in templates.iter().enumerate() {
            slots.push(
                priorities
                    .binary_search_by(|value| template.priority.cmp(value))
                    .map_err(|_| AdmissionError::MissingPriority { template: index })?,
            );
        }
        Ok(Self(Arc::new(Data {
            templates,
            variables,
            priorities,
            slots,
            present: true,
        })))
    }
    /// An explicitly absent objective, distinct from a present zero-cost objective.
    #[must_use]
    pub fn none() -> Self {
        Self(Arc::new(Data {
            templates: Vec::new(),
            variables: Vec::new(),
            priorities: Vec::new(),
            slots: Vec::new(),
            present: false,
        }))
    }
    /// Whether the caller admitted an objective, independently of current cost.
    #[must_use]
    pub fn is_present(&self) -> bool {
        self.0.present
    }
    /// Original template order for source-origin correlation.
    #[must_use]
    pub fn templates(&self) -> &[ObjectiveTemplate] {
        &self.0.templates
    }
    /// Fixed, distinct descending priority slots, including inactive templates.
    #[must_use]
    pub fn priorities(&self) -> &[i32] {
        &self.0.priorities
    }
    pub(crate) fn variables(&self, index: usize) -> usize {
        self.0.variables[index]
    }
    pub(crate) fn slot(&self, index: usize) -> usize {
        self.0.slots[index]
    }
}

fn reserved<T>(count: usize) -> Result<Vec<T>, AdmissionError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| AdmissionError::Allocation)?;
    Ok(result)
}

pub(crate) fn check_bound(
    resource: AdmissionResource,
    actual: usize,
    limit: usize,
    template: Option<usize>,
) -> Result<(), AdmissionError> {
    if actual > limit {
        Err(AdmissionError::Limit {
            resource,
            template,
            actual,
            limit,
        })
    } else {
        Ok(())
    }
}

fn admit_template(
    template: &ObjectiveTemplate,
    limits: AdmissionLimits,
    index: usize,
) -> Result<usize, AdmissionError> {
    template.condition.admit(limits, index)?;
    ObjectiveTemplate::validate_fields(
        &template.weight,
        &template.tuple,
        &template.positive,
        &template.filters,
        limits,
        index,
    )
}

fn admit_fields(
    weight: &Term,
    tuple: &[Term],
    positive: &[AtomPattern],
    filters: &[Filter],
    limits: AdmissionLimits,
    index: usize,
) -> Result<usize, AdmissionError> {
    admit_scope(
        tuple.len(),
        terms(weight, tuple, positive, filters),
        positive,
        filters,
        limits,
        index,
    )
}

fn admit_scope<'a>(
    tuple_width: usize,
    terms: impl Iterator<Item = &'a Term> + Clone,
    positive: &[AtomPattern],
    filters: &[Filter],
    limits: AdmissionLimits,
    index: usize,
) -> Result<usize, AdmissionError> {
    for (resource, actual, limit) in [
        (
            AdmissionResource::TupleWidth,
            tuple_width,
            limits.max_tuple_width,
        ),
        (
            AdmissionResource::PositiveBody,
            positive.len(),
            limits.max_positive_body,
        ),
        (
            AdmissionResource::Filters,
            filters.len(),
            limits.max_filters,
        ),
    ] {
        check_bound(resource, actual, limit, Some(index))?;
    }
    for pattern in positive {
        check_bound(
            AdmissionResource::PredicateArity,
            pattern.terms().len(),
            limits.max_predicate_arity,
            Some(index),
        )?;
    }
    let mut count = 0;
    for term in terms.clone() {
        if let Term::Variable(variable) = term {
            let proposed = variable.checked_add(1).ok_or(AdmissionError::Overflow {
                template: Some(index),
            })?;
            check_bound(
                AdmissionResource::Variables,
                proposed,
                limits.max_variables_per_template,
                Some(index),
            )?;
            count = count.max(proposed);
        }
    }
    let mut used = reserved(count)?;
    used.resize(count, false);
    let mut bound = reserved(count)?;
    bound.resize(count, false);
    for term in terms {
        if let Term::Variable(variable) = term {
            used[*variable] = true;
        }
    }
    for term in positive.iter().flat_map(AtomPattern::terms) {
        if let Term::Variable(variable) = term {
            bound[*variable] = true;
        }
    }
    for variable in 0..count {
        if !used[variable] {
            return Err(AdmissionError::NonDenseVariable {
                template: index,
                variable,
            });
        }
        if !bound[variable] {
            return Err(AdmissionError::UnsafeVariable {
                template: index,
                variable,
            });
        }
    }
    Ok(count)
}
