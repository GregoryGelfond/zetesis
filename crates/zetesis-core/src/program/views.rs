//! Borrowed admitted-template reads, also usable over owned construction input.
//!
//! Views carry their immutable or borrowed canonical prefix where IDs need interpretation.
//! They never own values or reconstruct ingress templates.

use std::{iter::FusedIterator, ops::Range};

use crate::catalog::{PredicateRef, TermRef};
use crate::{AtomPattern, BindingView, Filter, InstantiationError, Template, Term};

use super::{PredicateId, ProgramData, TemplateData};
use crate::catalog::storage::Read;
use crate::template::catalog::{self, FilterData, PatternData, TermData};

/// A template argument borrowed from admitted metadata or construction input.
/// Variable coordinates are template-local; constants retain their read owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateTerm<'a> {
    /// An admitted dense variable slot, or a construction slot not yet admitted.
    Variable(usize),
    /// A closed typed value, without copying its payload.
    Constant(TermRef<'a>),
}
impl<'a> From<&'a Term> for TemplateTerm<'a> {
    fn from(term: &'a Term) -> Self {
        match term {
            Term::Variable(variable) => Self::Variable(*variable),
            Term::Constant(value) => Self::Constant(value.into()),
        }
    }
}
impl<'a> TemplateTerm<'a> {
    fn admitted(program: Read<'a>, term: TermData) -> Self {
        match term {
            TermData::Variable(variable) => Self::Variable(variable),
            TermData::Constant(id) => Self::Constant(catalog::term(program, id)),
        }
    }

    /// Resolve a constant or a present assignment slot without materialization.
    ///
    /// # Errors
    /// Returns the missing variable coordinate, including an unbound partial slot.
    pub fn resolve(
        self,
        values: impl Into<BindingView<'a>>,
    ) -> Result<TermRef<'a>, InstantiationError> {
        match self {
            Self::Constant(value) => Ok(value),
            Self::Variable(variable) => values
                .into()
                .get(variable)
                .ok_or(InstantiationError { variable }),
        }
    }
}

/// An arity-checked pattern borrowing its construction input or admitted owner.
#[derive(Clone, Copy, Debug)]
pub struct PatternRef<'a>(PatternSource<'a>);
#[derive(Clone, Copy, Debug)]
enum PatternSource<'a> {
    Ingress(&'a AtomPattern),
    Admitted(Read<'a>, &'a PatternData),
    Parts(PredicateRef<'a>, &'a [TemplateTerm<'a>]),
}
impl<'a> From<&'a AtomPattern> for PatternRef<'a> {
    fn from(pattern: &'a AtomPattern) -> Self {
        Self(PatternSource::Ingress(pattern))
    }
}
impl<'a> PatternRef<'a> {
    /// Borrow an arity-checked signature and ordered argument descriptions.
    /// Variable slots remain local; constants borrow their existing owners.
    ///
    /// # Errors
    /// Returns an arity mismatch without retaining any input.
    pub fn from_parts(
        predicate: PredicateRef<'a>,
        terms: &'a [TemplateTerm<'a>],
    ) -> Result<Self, crate::ConstructionError> {
        if predicate.arity() != terms.len() {
            return Err(crate::ConstructionError::ArityMismatch {
                expected: predicate.arity(),
                actual: terms.len(),
            });
        }
        Ok(Self(PatternSource::Parts(predicate, terms)))
    }

    pub(crate) fn is_ingress(self) -> bool {
        matches!(self.0, PatternSource::Ingress(_))
    }

    /// Borrow the complete signed signature.
    #[must_use]
    pub fn predicate(self) -> PredicateRef<'a> {
        match self.0 {
            PatternSource::Ingress(pattern) => pattern.predicate().into(),
            PatternSource::Parts(predicate, _) => predicate,
            PatternSource::Admitted(program, pattern) => {
                catalog::predicate(program, pattern.predicate)
            }
        }
    }
    /// Borrow arguments in their original order, including repeated variables.
    #[must_use]
    pub fn terms(self) -> PatternTerms<'a> {
        match self.0 {
            PatternSource::Ingress(pattern) => PatternTerms(TermsSource::Ingress(pattern.terms())),
            PatternSource::Parts(_, terms) => PatternTerms(TermsSource::Borrowed(terms)),
            PatternSource::Admitted(program, pattern) => {
                PatternTerms(TermsSource::Admitted(program, &pattern.terms))
            }
        }
    }
}
impl PartialEq for PatternRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.predicate() == other.predicate() && self.terms().iter().eq(other.terms())
    }
}
impl Eq for PatternRef<'_> {}
impl PartialEq<AtomPattern> for PatternRef<'_> {
    fn eq(&self, other: &AtomPattern) -> bool {
        *self == PatternRef::from(other)
    }
}

/// A complete normalized template borrowed without copying its patterns.
#[derive(Clone, Copy, Debug)]
pub struct TemplateRef<'a>(TemplateSource<'a>);
#[derive(Clone, Copy, Debug)]
enum TemplateSource<'a> {
    Ingress(&'a Template),
    Admitted(&'a ProgramData, &'a TemplateData),
    Partition {
        head: Option<PatternRef<'a>>,
        positive: Patterns<'a>,
        gate_true: Patterns<'a>,
        gate_false: Patterns<'a>,
        filters: Filters<'a>,
        variable_count: usize,
    },
}
impl<'a> From<&'a Template> for TemplateRef<'a> {
    fn from(template: &'a Template) -> Self {
        Self(TemplateSource::Ingress(template))
    }
}
impl<'a> TemplateRef<'a> {
    /// Borrow a query-specific partition of positive and true-gate patterns.
    /// The head, false gates, filters and admitted variable coordinates remain
    /// those of this template. The supplied patterns must use those coordinates.
    #[must_use]
    pub fn with_patterns(
        self,
        positive: &'a [PatternRef<'a>],
        gate_true: &'a [PatternRef<'a>],
    ) -> Self {
        Self(TemplateSource::Partition {
            head: self.head(),
            positive: Patterns(PatternsSource::Borrowed(positive)),
            gate_true: Patterns(PatternsSource::Borrowed(gate_true)),
            gate_false: self.gate_false(),
            filters: self.filters(),
            variable_count: self.variable_count(),
        })
    }

    /// Consequence pattern, or no consequence for a constraint.
    #[must_use]
    pub fn head(self) -> Option<PatternRef<'a>> {
        match self.0 {
            TemplateSource::Partition { head, .. } => head,
            TemplateSource::Ingress(template) => template.head().map(PatternRef::from),
            TemplateSource::Admitted(program, template) => template
                .head
                .and_then(|at| program.row(template).patterns.get(at))
                .map(|pattern| {
                    PatternRef(PatternSource::Admitted(program.catalog.read(), pattern))
                }),
        }
    }
    /// Ordinary positive support patterns in source order.
    #[must_use]
    pub fn positive(self) -> Patterns<'a> {
        match self.0 {
            TemplateSource::Partition { positive, .. } => positive,
            TemplateSource::Ingress(template) => {
                Patterns(PatternsSource::Ingress(template.positive()))
            }
            TemplateSource::Admitted(program, template) => {
                program.patterns(template, template.positive.clone())
            }
        }
    }
    /// Patterns required true in the frozen candidate.
    #[must_use]
    pub fn gate_true(self) -> Patterns<'a> {
        match self.0 {
            TemplateSource::Partition { gate_true, .. } => gate_true,
            TemplateSource::Ingress(template) => {
                Patterns(PatternsSource::Ingress(template.gate_true()))
            }
            TemplateSource::Admitted(program, template) => {
                program.patterns(template, template.gate_true.clone())
            }
        }
    }
    /// Patterns required false in the frozen candidate.
    #[must_use]
    pub fn gate_false(self) -> Patterns<'a> {
        match self.0 {
            TemplateSource::Partition { gate_false, .. } => gate_false,
            TemplateSource::Ingress(template) => {
                Patterns(PatternsSource::Ingress(template.gate_false()))
            }
            TemplateSource::Admitted(program, template) => {
                program.patterns(template, template.gate_false.clone())
            }
        }
    }
    /// Exact comparisons in their original order.
    #[must_use]
    pub fn filters(self) -> Filters<'a> {
        match self.0 {
            TemplateSource::Partition { filters, .. } => filters,
            TemplateSource::Ingress(template) => {
                Filters(FiltersSource::Ingress(template.filters()))
            }
            TemplateSource::Admitted(program, template) => {
                Filters::admitted(program.catalog.read(), &program.row(template).filters)
            }
        }
    }
    /// Number of distinct template-local variables.
    #[must_use]
    pub fn variable_count(self) -> usize {
        match self.0 {
            TemplateSource::Partition { variable_count, .. } => variable_count,
            TemplateSource::Ingress(template) => template.variable_count(),
            TemplateSource::Admitted(_, template) => template.variable_count,
        }
    }
}
impl PartialEq for TemplateRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.head() == other.head()
            && self.positive().iter().eq(other.positive())
            && self.gate_true().iter().eq(other.gate_true())
            && self.gate_false().iter().eq(other.gate_false())
            && self.filters().iter().eq(other.filters())
    }
}
impl Eq for TemplateRef<'_> {}
impl PartialEq<Template> for TemplateRef<'_> {
    fn eq(&self, other: &Template) -> bool {
        *self == TemplateRef::from(other)
    }
}

/// An exact equality or inequality filter over borrowed template arguments.
#[derive(Clone, Copy, Debug)]
pub struct FilterRef<'a>(FilterSource<'a>);
#[derive(Clone, Copy, Debug)]
enum FilterSource<'a> {
    Ingress(&'a Filter),
    Admitted(Read<'a>, FilterData),
    Parts(bool, TemplateTerm<'a>, TemplateTerm<'a>),
}
impl<'a> From<&'a Filter> for FilterRef<'a> {
    fn from(filter: &'a Filter) -> Self {
        Self(FilterSource::Ingress(filter))
    }
}
impl<'a> FilterRef<'a> {
    /// Borrow exact equality (`equal = true`) or inequality operands.
    #[must_use]
    pub fn from_parts(equal: bool, left: TemplateTerm<'a>, right: TemplateTerm<'a>) -> Self {
        Self(FilterSource::Parts(equal, left, right))
    }

    /// Borrow the ordered operands without resolving variables.
    #[must_use]
    pub fn terms(self) -> (TemplateTerm<'a>, TemplateTerm<'a>) {
        match self.0 {
            FilterSource::Parts(_, left, right) => (left, right),
            FilterSource::Ingress(filter) => {
                let (left, right) = filter.terms();
                (left.into(), right.into())
            }
            FilterSource::Admitted(program, filter) => (
                TemplateTerm::admitted(program, filter.left),
                TemplateTerm::admitted(program, filter.right),
            ),
        }
    }
    /// Whether this requires equality rather than inequality.
    #[must_use]
    pub fn is_equality(self) -> bool {
        match self.0 {
            FilterSource::Parts(equal, ..) => equal,
            FilterSource::Ingress(filter) => matches!(filter, Filter::Eq(..)),
            FilterSource::Admitted(_, filter) => filter.equal,
        }
    }
    /// Evaluate exact typed equality after substitution, without copying values.
    ///
    /// # Errors
    /// Returns the first missing operand variable.
    pub fn evaluate(self, values: impl Into<BindingView<'a>>) -> Result<bool, InstantiationError> {
        let values = values.into();
        let (left, right) = self.terms();
        let equal = left.resolve(values)? == right.resolve(values)?;
        Ok(equal == self.is_equality())
    }
}
impl PartialEq for FilterRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.is_equality() == other.is_equality() && self.terms() == other.terms()
    }
}
impl Eq for FilterRef<'_> {}

/// Ordered, possibly repeated arguments in a pattern.
#[derive(Clone, Copy, Debug)]
pub struct PatternTerms<'a>(TermsSource<'a>);
#[derive(Clone, Copy, Debug)]
enum TermsSource<'a> {
    Borrowed(&'a [TemplateTerm<'a>]),
    Ingress(&'a [Term]),
    Admitted(Read<'a>, &'a [TermData]),
}
impl<'a> PatternTerms<'a> {
    /// Borrow a contiguous range of scalar fields, preserving the same owner.
    /// Returns None when either endpoint is outside this view or reversed.
    #[must_use]
    pub fn slice(self, range: Range<usize>) -> Option<Self> {
        match self.0 {
            TermsSource::Borrowed(terms) => terms
                .get(range)
                .map(|terms| Self(TermsSource::Borrowed(terms))),
            TermsSource::Ingress(terms) => terms
                .get(range)
                .map(|terms| Self(TermsSource::Ingress(terms))),
            TermsSource::Admitted(owner, terms) => terms
                .get(range)
                .map(|terms| Self(TermsSource::Admitted(owner, terms))),
        }
    }
    pub(crate) fn admitted(vocabulary: Read<'a>, records: &'a [TermData]) -> Self {
        Self(TermsSource::Admitted(vocabulary, records))
    }
    /// Variable slots in argument order, including repetitions. This scans
    /// metadata only; constants do not read their canonical payload.
    pub fn variables(self) -> impl Iterator<Item = usize> + 'a {
        (0..self.len()).filter_map(move |index| match self.0 {
            TermsSource::Borrowed(terms) => match terms[index] {
                TemplateTerm::Variable(variable) => Some(variable),
                TemplateTerm::Constant(_) => None,
            },
            TermsSource::Ingress(terms) => match &terms[index] {
                Term::Variable(variable) => Some(*variable),
                Term::Constant(_) => None,
            },
            TermsSource::Admitted(_, terms) => match terms[index] {
                TermData::Variable(variable) => Some(variable),
                TermData::Constant(_) => None,
            },
        })
    }
    /// Number of argument columns.
    #[must_use]
    pub fn len(self) -> usize {
        match self.0 {
            TermsSource::Borrowed(terms) => terms.len(),
            TermsSource::Ingress(terms) => terms.len(),
            TermsSource::Admitted(_, terms) => terms.len(),
        }
    }
    /// Borrow a column, or return None outside the pattern's arity.
    #[must_use]
    pub fn at(self, index: usize) -> Option<TemplateTerm<'a>> {
        match self.0 {
            TermsSource::Borrowed(terms) => terms.get(index).copied(),
            TermsSource::Ingress(terms) => terms.get(index).map(TemplateTerm::from),
            TermsSource::Admitted(program, terms) => terms
                .get(index)
                .map(|term| TemplateTerm::admitted(program, *term)),
        }
    }
}

/// Ordered patterns borrowed from one normalized template.
#[derive(Clone, Copy, Debug)]
pub struct Patterns<'a>(PatternsSource<'a>);
#[derive(Clone, Copy, Debug)]
enum PatternsSource<'a> {
    Borrowed(&'a [PatternRef<'a>]),
    Ingress(&'a [AtomPattern]),
    Admitted(Read<'a>, &'a [PatternData]),
}
impl<'a> Patterns<'a> {
    pub(crate) fn admitted(vocabulary: Read<'a>, records: &'a [PatternData]) -> Self {
        Self(PatternsSource::Admitted(vocabulary, records))
    }
    /// Number of occurrences, including duplicates.
    #[must_use]
    pub fn len(self) -> usize {
        match self.0 {
            PatternsSource::Borrowed(patterns) => patterns.len(),
            PatternsSource::Ingress(patterns) => patterns.len(),
            PatternsSource::Admitted(_, patterns) => patterns.len(),
        }
    }
    /// Borrow an original occurrence by position.
    #[must_use]
    pub fn at(self, index: usize) -> Option<PatternRef<'a>> {
        match self.0 {
            PatternsSource::Borrowed(patterns) => patterns.get(index).copied(),
            PatternsSource::Ingress(patterns) => patterns.get(index).map(PatternRef::from),
            PatternsSource::Admitted(program, patterns) => patterns
                .get(index)
                .map(|pattern| PatternRef(PatternSource::Admitted(program, pattern))),
        }
    }
}

/// Ordered filters borrowed from one normalized template.
#[derive(Clone, Copy, Debug)]
pub struct Filters<'a>(FiltersSource<'a>);
#[derive(Clone, Copy, Debug)]
enum FiltersSource<'a> {
    Ingress(&'a [Filter]),
    Admitted(Read<'a>, &'a [FilterData]),
}
impl<'a> Filters<'a> {
    pub(crate) fn admitted(vocabulary: Read<'a>, records: &'a [FilterData]) -> Self {
        Self(FiltersSource::Admitted(vocabulary, records))
    }
    /// Number of filter occurrences.
    #[must_use]
    pub fn len(self) -> usize {
        match self.0 {
            FiltersSource::Ingress(filters) => filters.len(),
            FiltersSource::Admitted(_, filters) => filters.len(),
        }
    }
    /// Borrow an original filter occurrence.
    #[must_use]
    pub fn at(self, index: usize) -> Option<FilterRef<'a>> {
        match self.0 {
            FiltersSource::Ingress(filters) => filters.get(index).map(FilterRef::from),
            FiltersSource::Admitted(program, filters) => filters
                .get(index)
                .map(|filter| FilterRef(FilterSource::Admitted(program, *filter))),
        }
    }
}

/// Original admitted template occurrences over one immutable Program.
#[derive(Clone, Copy, Debug)]
pub struct Templates<'a> {
    pub(super) program: &'a ProgramData,
}
impl<'a> Templates<'a> {
    /// Original template count, including duplicates.
    #[must_use]
    pub fn len(self) -> usize {
        self.program.templates.len()
    }
    /// Borrow one original template occurrence.
    #[must_use]
    pub fn at(self, index: usize) -> Option<TemplateRef<'a>> {
        self.program
            .templates
            .get(index)
            .map(|template| TemplateRef(TemplateSource::Admitted(self.program, template)))
    }
}

/// The exact admitted domain, independently of canonical subterm storage.
#[derive(Clone, Copy, Debug)]
pub struct Domain<'a> {
    pub(super) program: &'a ProgramData,
}
impl<'a> Domain<'a> {
    /// Number of distinct explicitly occurring closed constants.
    #[must_use]
    pub fn len(self) -> usize {
        self.program.domain.len()
    }
    /// Borrow a value by its semantic-storage-order coordinate.
    #[must_use]
    pub fn at(self, index: usize) -> Option<TermRef<'a>> {
        self.program
            .domain
            .get(index)
            .map(|id| self.program.term(*id))
    }
    /// Locate a typed value using the order that admitted this domain.
    ///
    /// # Errors
    /// Returns the insertion coordinate when the value is absent.
    pub fn binary_search<'q>(self, value: impl Into<TermRef<'q>>) -> Result<usize, usize> {
        let value = value.into();
        self.program
            .domain
            .binary_search_by(|id| self.program.term(*id).cmp(&value))
    }

    /// Checked domain lookup, charging before each probe and typed comparison.
    ///
    /// # Errors
    /// Returns the first callback refusal; the inner error is the insertion rank
    /// of a missing value, not an incomplete search.
    pub fn binary_search_with<'q, E>(
        self,
        value: impl Into<TermRef<'q>>,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Result<usize, usize>, E> {
        let value = value.into();
        locate(&self.program.domain, |id| {
            before()?;
            self.program.term(*id).compare_ref_with(value, &mut before)
        })
    }
}

/// Unique signed predicate signatures in semantic storage order.
#[derive(Clone, Copy, Debug)]
pub struct Predicates<'a> {
    pub(super) program: &'a ProgramData,
    pub(super) ids: &'a [PredicateId],
}
impl<'a> Predicates<'a> {
    /// Number of unique signatures.
    #[must_use]
    pub fn len(self) -> usize {
        self.ids.len()
    }
    /// Borrow one signature by its declared order coordinate.
    #[must_use]
    pub fn at(self, index: usize) -> Option<PredicateRef<'a>> {
        self.ids.get(index).map(|id| self.program.predicate(*id))
    }
    /// Locate a signed signature using this view's admitted order.
    ///
    /// # Errors
    /// Returns the insertion coordinate when the signature is absent.
    pub fn binary_search<'q>(self, predicate: impl Into<PredicateRef<'q>>) -> Result<usize, usize> {
        let predicate = predicate.into();
        self.ids
            .binary_search_by(|id| self.program.predicate(*id).cmp(&predicate))
    }

    /// Checked signed-signature lookup without copying text.
    ///
    /// # Errors
    /// Returns the first callback refusal; an inner error is the absent
    /// signature's insertion rank after a complete search.
    pub fn binary_search_with<'q, E>(
        self,
        predicate: impl Into<PredicateRef<'q>>,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Result<usize, usize>, E> {
        let predicate = predicate.into();
        locate(self.ids, |id| {
            before()?;
            self.program
                .predicate(*id)
                .compare_ref_with(predicate, &mut before)
        })
    }
}

fn locate<T, E>(
    values: &[T],
    mut compare: impl FnMut(&T) -> Result<std::cmp::Ordering, E>,
) -> Result<Result<usize, usize>, E> {
    let (mut low, mut high) = (0, values.len());
    while low < high {
        let middle = low + (high - low) / 2;
        match compare(&values[middle])? {
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Equal => return Ok(Ok(middle)),
            std::cmp::Ordering::Greater => high = middle,
        }
    }
    Ok(Err(low))
}

// Each sequence has the same exact immutable range contract. The macro only
// supplies cursor mechanics; element resolution remains in its concrete view.
macro_rules! sequence {
    ($view:ident, $iterator:ident, $item:ident) => {
        impl<'a> $view<'a> {
            /// Whether the sequence contains no element.
            #[must_use]
            pub fn is_empty(self) -> bool {
                self.len() == 0
            }
            /// Borrow an element, with the same meaning as `at`.
            #[must_use]
            pub fn get(self, index: usize) -> Option<$item<'a>> {
                self.at(index)
            }
            /// Exact-size double-ended traversal; cloning copies only cursor state.
            #[must_use]
            pub fn iter(self) -> $iterator<'a> {
                $iterator {
                    view: self,
                    range: 0..self.len(),
                }
            }
        }
        /// Exact-size, double-ended borrowed view traversal.
        #[derive(Clone, Debug)]
        pub struct $iterator<'a> {
            view: $view<'a>,
            range: Range<usize>,
        }
        impl<'a> Iterator for $iterator<'a> {
            type Item = $item<'a>;
            fn next(&mut self) -> Option<Self::Item> {
                self.range.next().map(|at| {
                    self.view
                        .at(at)
                        .expect("iterator range belongs to immutable view")
                })
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.range.size_hint()
            }
        }
        impl DoubleEndedIterator for $iterator<'_> {
            fn next_back(&mut self) -> Option<Self::Item> {
                self.range.next_back().map(|at| {
                    self.view
                        .at(at)
                        .expect("iterator range belongs to immutable view")
                })
            }
        }
        impl ExactSizeIterator for $iterator<'_> {}
        impl FusedIterator for $iterator<'_> {}
        impl<'a> IntoIterator for $view<'a> {
            type Item = $item<'a>;
            type IntoIter = $iterator<'a>;
            fn into_iter(self) -> Self::IntoIter {
                self.iter()
            }
        }
    };
}
sequence!(PatternTerms, PatternTermIter, TemplateTerm);
sequence!(Patterns, PatternIter, PatternRef);
sequence!(Filters, FilterIter, FilterRef);
sequence!(Templates, TemplateIter, TemplateRef);
sequence!(Domain, DomainIter, TermRef);
sequence!(Predicates, PredicateIter, PredicateRef);
