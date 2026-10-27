//! Borrowed admitted-template reads, also usable over owned construction input.
//!
//! Views carry their immutable or borrowed canonical prefix where IDs need interpretation.
//! They never own values or reconstruct ingress templates.

use std::{iter::FusedIterator, ops::Range};

use crate::catalog::{PredicateRef, TermRef};
use crate::{AtomPattern, BindingView, Filter, InstantiationError, Term};

use crate::catalog::storage::Read;
use crate::template::catalog::{self, FilterData, PatternData, TermData};

/// A template argument borrowed from admitted metadata or construction input.
/// Variable coordinates are template-local; constants retain their read owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateTerm<'a> {
    /// A template-local variable slot; the semantic owner validates its domain.
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
    pub(crate) fn admitted(read: Read<'a>, term: TermData) -> Self {
        match term {
            TermData::Variable(variable) => Self::Variable(variable),
            TermData::Constant(id) => Self::Constant(catalog::term(read, id)),
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

    pub(crate) fn admitted_parts(self) -> Option<(Read<'a>, &'a PatternData)> {
        match self.0 {
            PatternSource::Admitted(read, pattern) => Some((read, pattern)),
            PatternSource::Ingress(_) | PatternSource::Parts(_, _) => None,
        }
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
            PatternSource::Admitted(read, pattern) => catalog::predicate(read, pattern.predicate),
        }
    }
    /// Borrow arguments in their original order, including repeated variables.
    #[must_use]
    pub fn terms(self) -> PatternTerms<'a> {
        match self.0 {
            PatternSource::Ingress(pattern) => PatternTerms(TermsSource::Ingress(pattern.terms())),
            PatternSource::Parts(_, terms) => PatternTerms(TermsSource::Borrowed(terms)),
            PatternSource::Admitted(read, pattern) => {
                PatternTerms(TermsSource::Admitted(read, &pattern.terms))
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
            FilterSource::Admitted(read, filter) => (
                TemplateTerm::admitted(read, filter.left),
                TemplateTerm::admitted(read, filter.right),
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
            TermsSource::Admitted(read, terms) => terms
                .get(index)
                .map(|term| TemplateTerm::admitted(read, *term)),
        }
    }
}

/// Ordered pattern occurrences borrowed from one canonical component row.
#[derive(Clone, Copy, Debug)]
pub struct Patterns<'a> {
    vocabulary: Read<'a>,
    records: &'a [PatternData],
}
impl<'a> Patterns<'a> {
    pub(crate) fn admitted(vocabulary: Read<'a>, records: &'a [PatternData]) -> Self {
        Self {
            vocabulary,
            records,
        }
    }
    /// Number of occurrences, including duplicates.
    #[must_use]
    pub fn len(self) -> usize {
        self.records.len()
    }
    /// Borrow an original occurrence by position.
    #[must_use]
    pub fn at(self, index: usize) -> Option<PatternRef<'a>> {
        self.records
            .get(index)
            .map(|pattern| PatternRef(PatternSource::Admitted(self.vocabulary, pattern)))
    }
}

/// Ordered filter occurrences borrowed from one canonical component row.
#[derive(Clone, Copy, Debug)]
pub struct Filters<'a> {
    vocabulary: Read<'a>,
    records: &'a [FilterData],
}
impl<'a> Filters<'a> {
    pub(crate) fn admitted(vocabulary: Read<'a>, records: &'a [FilterData]) -> Self {
        Self {
            vocabulary,
            records,
        }
    }
    /// Number of filter occurrences.
    #[must_use]
    pub fn len(self) -> usize {
        self.records.len()
    }
    /// Borrow an original filter occurrence.
    #[must_use]
    pub fn at(self, index: usize) -> Option<FilterRef<'a>> {
        self.records
            .get(index)
            .map(|filter| FilterRef(FilterSource::Admitted(self.vocabulary, *filter)))
    }
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
