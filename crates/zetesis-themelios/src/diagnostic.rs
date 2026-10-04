//! Typed refusal values and their located diagnostic views.

use std::fmt;

use themelios_base::diagnostic::{Diagnostic, DiagnosticId, Label, Severity, ToDiagnostic};
use themelios_base::source::{Source, TooLarge};
use themelios_base::span::Location;
use themelios_program::raise::LowerError;
use themelios_syntax::diagnostic::SyntaxError;
use zetesis_core::{AdmissionError, ConstructionError};

use crate::ProgramSite;

/// A physical resource counted before template admission.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputLimit {
    /// Source UTF-8 bytes, checked before parsing.
    SourceBytes,
    /// Syntax nodes visited in the pre-raise walk.
    SyntaxNodes,
    /// Syntax nesting, including nested term syntax.
    SyntaxDepth,
    /// Body elements in a single source rule.
    BodyElements,
}

impl fmt::Display for InputLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SourceBytes => "source bytes",
            Self::SyntaxNodes => "syntax nodes",
            Self::SyntaxDepth => "syntax depth",
            Self::BodyElements => "body elements",
        })
    }
}

/// A construct excluded by the admission operation that reports it.
///
/// Relational and formula admission share these feature names. The enclosing
/// operation identifies the profile; this value does not imply an S0 refusal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProfileFeature {
    /// Any non-rule statement, including an erased program delimiter.
    Statement,
    /// A named or parameterized program part.
    ProgramPart,
    /// An unsupported head, including disjunction, theory, or an aggregate.
    Head,
    /// A disjunction element with its own condition, including an explicit empty one.
    ConditionalDisjunction,
    /// A finite function-head group aliases complete tuple and atom identities.
    HeadAggregateAlias,
    /// A choice with a lower or upper bound.
    BoundedChoice,
    /// A choice containing other than one source element.
    ChoiceCardinality,
    /// A choice element carrying a condition, including an explicit empty one.
    ConditionalChoice,
    /// Default negation in a normal or choice head.
    NegatedHead,
    /// An atom argument-list pool.
    PooledArguments,
    /// An anonymous atom position projected before default negation.
    AnonymousProjection,
    /// An aggregate, conditional literal, theory atom, or unknown body element.
    BodyElement,
    /// A Boolean literal in a body or non-atom head.
    BooleanLiteral,
    /// Default negation applied to a comparison.
    NegatedComparison,
    /// More than one relation in a comparison literal.
    ComparisonChain,
    /// A comparison other than equality or disequality.
    ComparisonRelation,
    /// Equality needs a value not already supplied by a positive relational atom.
    ScalarBinding,
    /// A non-scalar term, unsupported operator, external call, pool, or interval.
    Term,
    /// Checked numeric negation could not fit the scalar width.
    NumericOverflow,
    /// A NUL byte in a scalar string; clingo cannot preserve this identity.
    NulString,
    /// A term-valued or conditional display directive rather than a signature.
    ShowTerm,
    /// An optimization form outside the first bounded minimization profile.
    Objective,
    /// An aggregate form outside the bounded finite aggregate profile.
    Aggregate,
    /// A new-variable assignment outside the finite supported binder profile.
    AggregateAssignment,
    /// Pooling survived the bounded normalized analysis boundary.
    AnalysisPool,
    /// An objective producer cone lacks the completed finite source-eligibility profile.
    ObjectiveSourceEligibility,
}

impl fmt::Display for ProfileFeature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Statement => "non-rule statement",
            Self::ProgramPart => "program part",
            Self::Head => "head form",
            Self::ConditionalDisjunction => "conditional disjunction element",
            Self::HeadAggregateAlias => "aliased function aggregate head tuple and atom",
            Self::BoundedChoice => "bounded choice",
            Self::ChoiceCardinality => "choice with other than one element",
            Self::ConditionalChoice => "conditional choice element",
            Self::NegatedHead => "default-negated head",
            Self::PooledArguments => "pooled arguments",
            Self::AnonymousProjection => "anonymous projection under default negation",
            Self::BodyElement => "body element form",
            Self::BooleanLiteral => "Boolean literal",
            Self::NegatedComparison => "default-negated comparison",
            Self::ComparisonChain => "comparison chain",
            Self::ComparisonRelation => "comparison relation",
            Self::ScalarBinding => "scalar equality requiring binding analysis",
            Self::Term => "non-scalar term or unsupported operator",
            Self::NumericOverflow => "numeric negation overflow",
            Self::NulString => "a string containing NUL",
            Self::ShowTerm => "a term-valued or conditional show directive",
            Self::Objective => "an unsupported optimization objective",
            Self::AggregateAssignment => "an unsupported aggregate assignment",
            Self::AnalysisPool => "a pool at the normalized analysis boundary",
            Self::Aggregate => "an unsupported finite aggregate",
            Self::ObjectiveSourceEligibility => {
                "an objective dependency without complete source eligibility"
            }
        })
    }
}

/// Parser diagnostics together with the original source they describe.
///
/// Admission transfers the rejected source here without rereading or reparsing
/// it. Consumers can derive their own views from the source and diagnostics;
/// the plain human view uses `<input>` for this unnamed source.
#[derive(Debug)]
pub struct SyntaxFailure {
    source: Source,
    diagnostics: Vec<SyntaxError>,
}

impl SyntaxFailure {
    pub(crate) fn new(source: Source, diagnostics: Vec<SyntaxError>) -> Self {
        Self {
            source,
            diagnostics,
        }
    }

    /// Original source identity and UTF-8 bytes, retained only on refusal.
    #[must_use]
    pub fn source(&self) -> &Source {
        &self.source
    }

    /// Every parser diagnostic, in the order reported by themelios.
    #[must_use]
    pub fn diagnostics(&self) -> &[SyntaxError] {
        &self.diagnostics
    }
}

impl fmt::Display for SyntaxFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "source parsing reported {} diagnostic(s)",
            self.diagnostics.len()
        )?;
        crate::source_diagnostics::write(f, "<input>", &self.source, &self.diagnostics)
    }
}

impl std::error::Error for SyntaxFailure {}

/// A typed admission refusal. Logical failures retain their program subject and
/// any real source evidence; parser failures retain complete located diagnostics.
#[derive(Debug)]
pub enum AdmissionFailure {
    /// A source or traversal ceiling was exceeded; this is not semantic UNSAT.
    Limit {
        /// The resource being counted.
        resource: InputLimit,
        /// Configured inclusive ceiling.
        limit: usize,
        /// Count observed when admission stopped.
        observed: usize,
        /// Logical or source scope associated with the refusal.
        location: ProgramSite,
    },
    /// The source model's own coordinate ceiling was exceeded.
    Source {
        /// The dependency's typed source refusal.
        error: TooLarge,
        /// Start of the source that could not be admitted.
        location: Location,
    },
    /// Original source and every parser diagnostic; none is silently ignored.
    Syntax(SyntaxFailure),
    /// Every diagnostic returned by the best-effort raiser.
    Raise(Vec<LowerError>),
    /// A form excluded by the selected admission profile.
    Profile {
        /// The excluded construct.
        feature: ProfileFeature,
        /// Logical subject and any real source position.
        location: ProgramSite,
    },
    /// A finite numeric endpoint excluded by the extremum representation guard.
    /// This is a zetesis admission policy, not a parser or resource failure.
    ExtremumEndpoint {
        /// The evaluated endpoint, either `i32::MIN` or `i32::MAX`.
        value: i32,
        /// Source position of the guarded extremum expression.
        location: ProgramSite,
    },
    /// A core atom or predicate constructor refused its checked shape.
    Construction {
        /// The typed core refusal.
        error: ConstructionError,
        /// Source of the value being constructed.
        location: ProgramSite,
    },
    /// Canonical source metadata could not be admitted or published.
    Metadata {
        /// Typed canonical identity, allocation or storage refusal.
        error: crate::MetadataStorageError,
        /// Original declaration identity and any real source evidence.
        location: ProgramSite,
    },
    /// The normalized program failed the independent core admission door.
    Core {
        /// The typed core refusal.
        error: AdmissionError,
        /// The affected rule, or the full source for program-wide failures.
        location: ProgramSite,
    },
}

impl AdmissionFailure {
    /// Logical or source scope carried by this typed refusal. Parser and raiser
    /// diagnostic collections can describe several sites and have no single site.
    #[must_use]
    pub fn site(&self) -> Option<ProgramSite> {
        match self {
            Self::Source { location, .. } => Some((*location).into()),
            Self::Limit { location, .. }
            | Self::Profile { location, .. }
            | Self::ExtremumEndpoint { location, .. }
            | Self::Construction { location, .. }
            | Self::Metadata { location, .. }
            | Self::Core { location, .. } => Some(*location),
            Self::Syntax(_) | Self::Raise(_) => None,
        }
    }
    /// Derive source diagnostics without discarding typed refusal data.
    /// An unlocated logical refusal returns no source diagnostic; [`Self::site`]
    /// still exposes its program scope.
    #[must_use]
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        match self {
            Self::Syntax(error) => error
                .diagnostics()
                .iter()
                .map(ToDiagnostic::to_diagnostic)
                .collect(),
            Self::Raise(errors) => errors.iter().map(ToDiagnostic::to_diagnostic).collect(),
            Self::Limit { location, .. } => {
                site_diagnostics("input-limit", self.to_string(), *location)
            }
            Self::Source { location, .. } => {
                vec![diagnostic("source-limit", self.to_string(), *location)]
            }
            Self::Profile { location, .. } => {
                site_diagnostics("unsupported-profile", self.to_string(), *location)
            }
            Self::ExtremumEndpoint { location, .. } => {
                site_diagnostics("extremum-endpoint", self.to_string(), *location)
            }
            Self::Construction { location, .. } => {
                site_diagnostics("invalid-shape", self.to_string(), *location)
            }
            Self::Metadata { location, .. } => {
                site_diagnostics("metadata-storage", self.to_string(), *location)
            }
            Self::Core { location, .. } => {
                site_diagnostics("core-admission", self.to_string(), *location)
            }
        }
    }
}

impl fmt::Display for AdmissionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit {
                resource,
                limit,
                observed,
                ..
            } => {
                write!(
                    f,
                    "{resource} exceeded the admission limit {limit} (observed {observed})"
                )
            }
            Self::Source { error, .. } => error.fmt(f),
            Self::Syntax(error) => error.fmt(f),
            Self::Raise(errors) => {
                write!(f, "source raising reported {} diagnostic(s)", errors.len())
            }
            Self::Profile { feature, .. } => write!(f, "source profile does not admit {feature}"),
            Self::ExtremumEndpoint { value, .. } => write!(
                f,
                "numeric extremum endpoint {value} is excluded by the zetesis endpoint guard"
            ),
            Self::Construction { error, .. } => error.fmt(f),
            Self::Metadata { error, .. } => error.fmt(f),
            Self::Core { error, .. } => error.fmt(f),
        }
    }
}

impl std::error::Error for AdmissionFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source { error, .. } => Some(error),
            Self::Syntax(error) => Some(error),
            Self::Construction { error, .. } => Some(error),
            Self::Metadata { error, .. } => Some(error),
            Self::Core { error, .. } => Some(error),
            _ => None,
        }
    }
}

pub(crate) fn diagnostic(name: &'static str, message: String, location: Location) -> Diagnostic {
    Diagnostic::new(
        DiagnosticId::new("zetesis", name),
        Severity::Error,
        message,
        Label {
            location,
            message: None,
        },
    )
    .expect("every typed refusal renders a nonempty diagnostic message")
}

pub(crate) fn unsupported(
    feature: ProfileFeature,
    location: impl Into<ProgramSite>,
) -> AdmissionFailure {
    AdmissionFailure::Profile {
        feature,
        location: location.into(),
    }
}

fn site_diagnostics(name: &'static str, message: String, site: ProgramSite) -> Vec<Diagnostic> {
    site.location()
        .map(|location| diagnostic(name, message, location))
        .into_iter()
        .collect()
}
