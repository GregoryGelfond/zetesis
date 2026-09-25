use std::fmt;
use std::sync::Arc;

use std::{iter::FusedIterator, ops::Range};
use zetesis_core::{
    AtomPattern, Filter, FilterRef, Filters, PatternRef, PatternTerms, Patterns, TemplateCatalog,
    TemplateRow, TemplateTerm, Term,
};

mod admission;
#[cfg(test)]
mod tests;

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

    /// Check tuple width, positive body count, filter count and predicate arity.
    /// This does not establish variable safety or dense IDs. A source frontend
    /// with additional binding forms must establish their scope independently;
    /// resolved templates still require [`ObjectiveProgram::new`].
    ///
    /// # Errors
    /// Refuses a configured shape ceiling before source values are evaluated.
    pub fn validate_shape(
        tuple_width: usize,
        positive: &[AtomPattern],
        filter_count: usize,
        limits: AdmissionLimits,
        index: usize,
    ) -> Result<(), AdmissionError> {
        validate_shape(
            tuple_width,
            positive.iter().map(PatternRef::from),
            filter_count,
            limits,
            index,
        )
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
                }))
                .map(TemplateTerm::from),
            positive.iter().map(PatternRef::from),
            filters.len(),
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

/// An objective occurrence referencing a row of an existing template catalog.
/// The row's first scalar field is the weight; the remaining fields are its
/// tuple. Conditions must already be canonical, or be the empty true query.
/// This descriptor retains no atom, term or predicate payload of its own.
#[derive(Clone, Debug)]
pub struct ObjectiveElement {
    row: usize,
    priority: i32,
    polarity: WeightPolarity,
    condition: Condition,
}
impl ObjectiveElement {
    /// Reference one catalog row at a fixed priority. Admission checks the row.
    #[must_use]
    pub fn new(row: usize, priority: i32) -> Self {
        Self {
            row,
            priority,
            polarity: WeightPolarity::AsWritten,
            condition: Condition::default(),
        }
    }
    /// Select checked numeric normalization before global key deduplication.
    #[must_use]
    pub fn with_weight_polarity(mut self, polarity: WeightPolarity) -> Self {
        self.polarity = polarity;
        self
    }
    /// Attach an already canonical closed model query, or the empty true query.
    #[must_use]
    pub fn with_condition(mut self, condition: Condition) -> Self {
        self.condition = condition;
        self
    }
}

/// Admission ceilings. Zero is a real ceiling, never an unlimited sentinel.
#[derive(Clone, Copy, Debug)]
pub struct AdmissionLimits {
    /// Inclusive ID-only node metadata per closed condition, including its
    /// shared condition envelope. Shared canonical payload is counted once
    /// under `max_bytes`. The independent default is 128 MiB; zero is a real limit.
    pub max_condition_node_bytes: usize,
    /// Inclusive combined template vocabulary, metadata, condition nodes and
    /// retained catalog allowance, including admission/publication overlap.
    /// The independent default is 128 MiB; input descriptions, allocator
    /// bookkeeping and Arc counters are excluded.
    pub max_bytes: usize,
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
            max_condition_node_bytes: 134_217_728,
            max_bytes: 134_217_728,
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdmissionError {
    /// An objective occurrence names no row of the supplied catalog.
    CatalogRow {
        /// Original objective occurrence.
        template: usize,
        /// Requested catalog row.
        row: usize,
    },
    /// A referenced catalog row has no scalar weight field.
    MissingWeight {
        /// Original objective occurrence.
        template: usize,
        /// Referenced catalog row.
        row: usize,
    },
    /// The catalog entry point cannot import owned condition descriptions.
    ConditionNotCanonical {
        /// Original objective occurrence.
        template: usize,
    },
    /// Combined canonical vocabulary or retained metadata could not be admitted.
    Storage {
        /// Original template where known.
        template: Option<usize>,
        /// Exact canonical/storage refusal.
        error: zetesis_core::catalog::Error,
    },
    /// A closed condition's canonical representation could not be admitted.
    ConditionStorage {
        /// Original input template.
        template: usize,
        /// Exact coordinate or canonical storage cause.
        error: crate::ConditionError,
    },
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
            Self::Storage { template, .. }
            | Self::Limit { template, .. }
            | Self::Overflow { template } => template,
            Self::CatalogRow { template, .. }
            | Self::MissingWeight { template, .. }
            | Self::ConditionNotCanonical { template }
            | Self::UnsafeVariable { template, .. }
            | Self::NonDenseVariable { template, .. }
            | Self::MissingPriority { template }
            | Self::ConditionStorage { template, .. }
            | Self::ConditionReference { template, .. } => Some(template),
            Self::Allocation => None,
        }
    }
}
impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CatalogRow { template, row } => write!(
                f,
                "objective template {template} references missing catalog row {row}"
            ),
            Self::MissingWeight { template, row } => write!(
                f,
                "objective template {template} catalog row {row} has no weight field"
            ),
            Self::ConditionNotCanonical { template } => write!(
                f,
                "objective template {template} requires a canonical condition for catalog admission"
            ),
            Self::Storage { template, error } => {
                write!(f, "objective storage (template {template:?}): {error}")
            }
            Self::ConditionStorage { template, error } => {
                write!(f, "objective template {template} condition: {error}")
            }
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
impl std::error::Error for AdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ConditionStorage { error, .. } => Some(error),
            Self::Storage { error, .. } => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct Data {
    catalog: Option<TemplateCatalog>,
    templates: Vec<AdmittedTemplate>,
    priorities: Vec<i32>,
    present: bool,
    storage_bytes: u128,
}
#[derive(Debug)]
struct AdmittedTemplate {
    row: usize,
    polarity: WeightPolarity,
    condition: Condition,
    variables: usize,
    slot: usize,
}
impl Data {
    fn row(&self, template: &AdmittedTemplate) -> TemplateRow<'_> {
        self.catalog
            .as_ref()
            .and_then(|catalog| catalog.at(template.row))
            .expect("objective metadata names its complete canonical component row")
    }
    fn fields(&self, template: &AdmittedTemplate) -> (TemplateTerm<'_>, PatternTerms<'_>) {
        let fields = self.row(template).terms();
        (
            fields
                .at(0)
                .expect("objective admission stores a weight before its tuple"),
            fields
                .slice(1..fields.len())
                .expect("the remaining scalar fields are the objective tuple"),
        )
    }
    fn priority(&self, template: &AdmittedTemplate) -> i32 {
        self.priorities[template.slot]
    }
}

/// An immutable collection of objective metadata over canonical template storage.
/// It supplies costs, never logical support. Equal complete contribution keys
/// from separate templates/directives are globally coalesced during evaluation.
#[derive(Clone, Debug)]
pub struct ObjectiveProgram(Arc<Data>);
impl ObjectiveProgram {
    /// Consume original-order descriptions and admit one canonical vocabulary.
    /// Owned condition input shares that vocabulary; supplied canonical condition
    /// catalogs retain their original authority. Source presence and complete
    /// priority discovery remain the frontend's responsibility.
    ///
    /// # Errors
    /// Refuses shape/safety, invalid condition references, combined named storage,
    /// overflow or fallible reservation. No partial `ObjectiveProgram` escapes.
    pub fn new(
        templates: Vec<ObjectiveTemplate>,
        limits: AdmissionLimits,
    ) -> Result<Self, AdmissionError> {
        admission::admit(templates, limits).map(|data| Self(Arc::new(data)))
    }
    /// Admit objective metadata over existing canonical components without
    /// importing or copying their payload. Row occurrences remain in descriptor
    /// order and may repeat. Each row stores its weight before its tuple fields.
    /// Positive patterns alone establish dense variable safety; filters and
    /// closed conditions do not. Empty conditions mean true; nonempty conditions
    /// must already have a canonical atom catalog.
    ///
    /// The byte ceiling covers the retained catalog, occurrence/priority buffers,
    /// unique condition allocations and simultaneous safety scratch. Exact shared
    /// snapshots are counted once; partially shared prefixes are conservative.
    /// Caller descriptor storage and allocator/Arc bookkeeping are excluded.
    ///
    /// # Errors
    /// Refuses missing rows or weights, owned nonempty conditions, shape/safety,
    /// condition references, named storage, overflow or fallible reservation.
    pub fn from_catalog(
        catalog: TemplateCatalog,
        rows: Vec<ObjectiveElement>,
        limits: AdmissionLimits,
    ) -> Result<Self, AdmissionError> {
        admission::from_catalog(catalog, rows, limits).map(|data| Self(Arc::new(data)))
    }

    /// An absent objective, distinct from a present empty or zero-cost objective.
    #[must_use]
    pub fn none() -> Self {
        Self(Arc::new(Data {
            catalog: None,
            templates: Vec::new(),
            priorities: Vec::new(),
            present: false,
            storage_bytes: size_of::<Data>() as u128,
        }))
    }
    /// Whether an objective was admitted, independently of current cost.
    #[must_use]
    pub fn is_present(&self) -> bool {
        self.0.present
    }
    /// Original template occurrences as borrowed canonical views.
    #[must_use]
    pub fn templates(&self) -> ObjectiveTemplates<'_> {
        ObjectiveTemplates(&self.0)
    }
    /// Fixed distinct descending priority slots, including inactive templates.
    #[must_use]
    pub fn priorities(&self) -> &[i32] {
        &self.0.priorities
    }
    /// Named retained canonical and metadata storage. Shared clones name the
    /// same allocations. Independent source catalogs can share segments beyond
    /// their exact occurrence owner and are conservatively counted separately.
    #[must_use]
    pub fn storage_bytes(&self) -> u128 {
        self.0.storage_bytes
    }
    pub(crate) fn variables(&self, index: usize) -> usize {
        self.0.templates[index].variables
    }
    pub(crate) fn slot(&self, index: usize) -> usize {
        self.0.templates[index].slot
    }
}

/// One admitted objective element, borrowing all typed terms and patterns.
#[derive(Clone, Copy, Debug)]
pub struct ObjectiveTemplateRef<'a> {
    data: &'a Data,
    template: &'a AdmittedTemplate,
}
impl PartialEq for ObjectiveTemplateRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.weight() == other.weight()
            && self.weight_polarity() == other.weight_polarity()
            && self.priority() == other.priority()
            && self.tuple().iter().eq(other.tuple())
            && self.positive().iter().eq(other.positive())
            && self.filters().iter().eq(other.filters())
            && self.condition() == other.condition()
    }
}
impl Eq for ObjectiveTemplateRef<'_> {}
impl<'a> ObjectiveTemplateRef<'a> {
    /// Check a borrowed source scope's shape without copying logical terms.
    /// This checks tuple width, body size, filters and predicate arity; the
    /// frontend remains responsible for the safety of additional binding forms.
    /// # Errors
    /// Refuses a configured shape ceiling.
    pub fn validate_shape(
        tuple_width: usize,
        positive: impl Iterator<Item = PatternRef<'a>> + Clone,
        filter_count: usize,
        limits: AdmissionLimits,
        index: usize,
    ) -> Result<(), AdmissionError> {
        validate_shape(tuple_width, positive, filter_count, limits, index)
    }

    /// Validate borrowed positive patterns and filters before source expression
    /// evaluation. Constants stay in the caller's canonical vocabulary.
    /// The caller establishes that expression inputs have independent binders.
    /// # Errors
    /// Refuses shape limits, unsafe or sparse variable IDs, storage or allocation.
    pub fn validate_scope(
        tuple_width: usize,
        positive: impl Iterator<Item = PatternRef<'a>> + Clone,
        filters: impl Iterator<Item = FilterRef<'a>> + Clone,
        limits: AdmissionLimits,
        index: usize,
    ) -> Result<usize, AdmissionError> {
        let filter_count = filters.clone().count();
        let terms = positive
            .clone()
            .flat_map(|pattern| pattern.terms().iter())
            .chain(filters.flat_map(|filter| {
                let (left, right) = filter.terms();
                [left, right]
            }));
        admit_scope(tuple_width, terms, positive, filter_count, limits, index)
    }

    /// Original scalar weight; numeric normalization follows substitution.
    #[must_use]
    pub fn weight(self) -> TemplateTerm<'a> {
        self.data.fields(self.template).0
    }
    /// Checked normalization applied before contribution-key deduplication.
    #[must_use]
    pub fn weight_polarity(self) -> WeightPolarity {
        self.template.polarity
    }
    /// Admitted priority, retained even when this element has no active binding.
    #[must_use]
    pub fn priority(self) -> i32 {
        self.data.priority(self.template)
    }
    /// Original tuple fields after the weight, including repeated occurrences.
    #[must_use]
    pub fn tuple(self) -> PatternTerms<'a> {
        self.data.fields(self.template).1
    }
    /// Positive relational conditions, which alone establish variable safety.
    #[must_use]
    pub fn positive(self) -> Patterns<'a> {
        self.data.row(self.template).patterns()
    }
    /// Exact comparison conditions; they do not establish variable safety.
    #[must_use]
    pub fn filters(self) -> Filters<'a> {
        self.data.row(self.template).filters()
    }
    /// Closed query against the supplied original model; empty means true.
    #[must_use]
    pub fn condition(self) -> &'a Condition {
        &self.template.condition
    }
}

/// Original-order objective rows; coordinates are local to this immutable owner.
#[derive(Clone, Copy, Debug)]
pub struct ObjectiveTemplates<'a>(&'a Data);
impl<'a> ObjectiveTemplates<'a> {
    /// Number of original occurrences.
    #[must_use]
    pub fn len(self) -> usize {
        self.0.templates.len()
    }
    /// Whether the objective has no element rows.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.0.templates.is_empty()
    }
    /// Borrow one original occurrence or return None outside the view.
    #[must_use]
    pub fn at(self, index: usize) -> Option<ObjectiveTemplateRef<'a>> {
        self.0
            .templates
            .get(index)
            .map(|template| ObjectiveTemplateRef {
                data: self.0,
                template,
            })
    }
    /// Same checked occurrence read as at.
    #[must_use]
    pub fn get(self, index: usize) -> Option<ObjectiveTemplateRef<'a>> {
        self.at(index)
    }
    /// Exact double-ended traversal borrowing this owner.
    #[must_use]
    pub fn iter(self) -> ObjectiveTemplateIter<'a> {
        ObjectiveTemplateIter {
            view: self,
            range: 0..self.len(),
        }
    }
}
/// Exact double-ended cursor over admitted objective occurrences.
#[derive(Clone, Debug)]
pub struct ObjectiveTemplateIter<'a> {
    view: ObjectiveTemplates<'a>,
    range: Range<usize>,
}
impl<'a> Iterator for ObjectiveTemplateIter<'a> {
    type Item = ObjectiveTemplateRef<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        self.range.next().and_then(|index| self.view.at(index))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}
impl DoubleEndedIterator for ObjectiveTemplateIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.range.next_back().and_then(|index| self.view.at(index))
    }
}
impl ExactSizeIterator for ObjectiveTemplateIter<'_> {}
impl FusedIterator for ObjectiveTemplateIter<'_> {}
impl<'a> IntoIterator for ObjectiveTemplates<'a> {
    type Item = ObjectiveTemplateRef<'a>;
    type IntoIter = ObjectiveTemplateIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
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
    template.condition.validate(limits, index)?;
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
        terms(weight, tuple, positive, filters).map(TemplateTerm::from),
        positive.iter().map(PatternRef::from),
        filters.len(),
        limits,
        index,
    )
}

fn admit_scope<'a>(
    tuple_width: usize,
    terms: impl Iterator<Item = TemplateTerm<'a>> + Clone,
    positive: impl Iterator<Item = PatternRef<'a>> + Clone,
    filter_count: usize,
    limits: AdmissionLimits,
    index: usize,
) -> Result<usize, AdmissionError> {
    validate_shape(tuple_width, positive.clone(), filter_count, limits, index)?;
    let mut count = 0;
    for term in terms.clone() {
        if let TemplateTerm::Variable(variable) = term {
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
    let headers = 2 * size_of::<Vec<bool>>() as u128;
    admission::check_storage(headers + 2 * count as u128, limits.max_bytes, Some(index))?;
    let mut used = reserved(count)?;
    admission::check_storage(
        headers + used.capacity() as u128 + count as u128,
        limits.max_bytes,
        Some(index),
    )?;
    used.resize(count, false);
    let mut bound = reserved(count)?;
    admission::check_storage(
        headers + used.capacity() as u128 + bound.capacity() as u128,
        limits.max_bytes,
        Some(index),
    )?;
    bound.resize(count, false);
    for term in terms {
        if let TemplateTerm::Variable(variable) = term {
            used[variable] = true;
        }
    }
    for term in positive.flat_map(PatternRef::terms) {
        if let TemplateTerm::Variable(variable) = term {
            bound[variable] = true;
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

fn validate_shape<'a>(
    tuple_width: usize,
    positive: impl Iterator<Item = PatternRef<'a>> + Clone,
    filter_count: usize,
    limits: AdmissionLimits,
    index: usize,
) -> Result<(), AdmissionError> {
    for (resource, actual, limit) in [
        (
            AdmissionResource::TupleWidth,
            tuple_width,
            limits.max_tuple_width,
        ),
        (
            AdmissionResource::PositiveBody,
            positive.clone().count(),
            limits.max_positive_body,
        ),
        (AdmissionResource::Filters, filter_count, limits.max_filters),
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
    Ok(())
}
