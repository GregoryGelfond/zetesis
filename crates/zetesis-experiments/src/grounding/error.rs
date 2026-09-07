//! Refusal data distinguish measurement failure from logical inconsistency.

use std::{fmt, io};

use serde::{Serialize, Serializer, ser::SerializeStruct};

use super::CaptureRefusal;

/// A refused or incomplete experiment; none of these variants means UNSAT.
#[derive(Debug)]
pub enum Error {
    /// The round count is outside the finite command contract.
    Configuration(&'static str),
    /// Retained experiment storage could not be reserved.
    Allocation,
    /// Original source loading or parsing failed.
    Source(zetesis_themelios::BundleError),
    /// Native admission failed, retaining its original source evidence.
    Admission(Box<zetesis_themelios::FormulaBundleFailure>),
    /// Objectives are outside this full-model enumeration experiment.
    Objective,
    /// A complete callback record could not be retained.
    Capture(CaptureRefusal),
    /// Native membership enumeration stopped without exhaustion.
    Search(zetesis_sat::Incomplete),
    /// A retained diagnostic dimension exceeded its inclusive ceiling.
    Limit {
        /// Name of the refused diagnostic dimension.
        resource: &'static str,
        /// Inclusive configured ceiling.
        limit: usize,
    },
    /// Original loaded source identities or bytes changed between admissions.
    SourceChanged,
    /// Ordered native subject, provenance or display metadata changed.
    SubjectChanged,
    /// A complete native model multiset differed from the reference.
    ModelsChanged,
    /// The native iterator ended without reporting exhaustion.
    NotExhausted,
    /// Admission nanoseconds could not be represented.
    ClockOverflow,
    /// Formatting did not produce the expected UTF-8 atom spelling.
    Formatting,
    /// Bounded report serialization failed.
    Serialization(serde_json::Error),
    /// The external writer failed; it may retain a byte prefix.
    Output(io::Error),
}

impl Error {
    /// Stable schema-one refusal identifier, independent of diagnostic wording.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Configuration(_) => "configuration",
            Self::Allocation => "allocation",
            Self::Source(_) => "source",
            Self::Admission(_) => "admission",
            Self::Objective => "objective_unsupported",
            Self::Capture(_) => "capture",
            Self::Search(_) => "search_incomplete",
            Self::Limit { .. } => "capture_limit",
            Self::SourceChanged => "source_changed",
            Self::SubjectChanged => "subject_changed",
            Self::ModelsChanged => "models_changed",
            Self::NotExhausted => "not_exhausted",
            Self::ClockOverflow => "clock_overflow",
            Self::Formatting => "atom_formatting",
            Self::Serialization(_) => "serialization",
            Self::Output(_) => "output",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(detail) => f.write_str(detail),
            Self::Allocation => f.write_str("experiment storage allocation refused"),
            Self::Source(error) => error.fmt(f),
            Self::Admission(error) => error.fmt(f),
            Self::Objective => f.write_str("grounding experiment requires objective-free source"),
            Self::Capture(reason) => write!(f, "phase capture incomplete: {reason:?}"),
            Self::Search(error) => error.fmt(f),
            Self::Limit { resource, limit } => write!(f, "{resource} exceeded {limit}"),
            Self::SourceChanged => f.write_str("original source catalog changed"),
            Self::SubjectChanged => f.write_str("ordered native subject or provenance changed"),
            Self::ModelsChanged => f.write_str("complete native model multiset changed"),
            Self::NotExhausted => f.write_str("native model iterator did not establish exhaustion"),
            Self::ClockOverflow => f.write_str("admission nanoseconds overflowed"),
            Self::Formatting => f.write_str("atom spelling did not produce valid UTF-8"),
            Self::Serialization(error) => error.fmt(f),
            Self::Output(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Admission(error) => Some(error),
            Self::Search(error) => Some(error),
            Self::Serialization(error) => Some(error),
            Self::Output(error) => Some(error),
            _ => None,
        }
    }
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Streaming Display avoids allocating an unbounded diagnostic string.
        struct Detail<'a>(&'a Error);
        impl Serialize for Detail<'_> {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(self.0)
            }
        }
        let mut fields = serializer.serialize_struct("Refusal", 2)?;
        fields.serialize_field("code", self.code())?;
        fields.serialize_field("detail", &Detail(self))?;
        fields.end()
    }
}
