//! Logical-program admission limits and typed refusal subjects.

use std::fmt;

use themelios_program::program::{PartKey, Statement};
use themelios_program::provenance::WithProvenance;
use zetesis_core::{AdmissionLimits, ConstructionError};

use crate::{AdmissionFailure, ProfileFeature};

pub use crate::program_limits::{Limit as ProgramLimit, Resource as ProgramResource};

/// Limits for an already constructed logical program. Source-byte and syntax
/// limits belong to source admission; these bounds inspect canonical structure.
///
/// [`crate::prepare_program_formula`] visits admitted directives, nested
/// conditions, provenance entries and annotations before normalization can copy
/// them. It does not account for the caller's earlier program construction,
/// allocator overhead or process RSS.
#[derive(Clone, Copy, Debug)]
pub struct ProgramAdmissionOptions {
    /// Cumulative visited logical nodes, including closed symbol interiors.
    /// Formula preparation also counts provenance entries and annotation fields.
    pub max_nodes: usize,
    /// Logical nesting, including closed symbol interiors.
    pub max_depth: usize,
    /// Canonical elements in one body; formula preparation also checks conditions.
    pub max_body_elements: usize,
    /// Total UTF-8 bytes in visited logical names and strings. Formula preparation
    /// additionally counts annotation text and transformation tags.
    pub max_text_bytes: usize,
    /// Formula template, variable, arity and value ceilings.
    pub core_limits: AdmissionLimits,
}

impl Default for ProgramAdmissionOptions {
    fn default() -> Self {
        Self {
            max_nodes: 262_144,
            max_depth: 128,
            max_body_elements: 1_024,
            max_text_bytes: 1_048_576,
            core_limits: AdmissionLimits::default(),
        }
    }
}

/// A compilation cause without an invented source coordinate.
#[derive(Debug)]
pub enum CompilationFailure {
    /// A construct outside the requested admission profile.
    Profile(ProfileFeature),
    /// An invalid native predicate, atom or closed value.
    Construction(ConstructionError),
}

impl CompilationFailure {
    pub(crate) fn at(self, location: impl Into<crate::ProgramSite>) -> AdmissionFailure {
        let location = location.into();
        match self {
            Self::Profile(feature) => AdmissionFailure::Profile { feature, location },
            Self::Construction(error) => AdmissionFailure::Construction { error, location },
        }
    }
}

impl fmt::Display for CompilationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(feature) => write!(f, "program admission does not support {feature}"),
            Self::Construction(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for CompilationFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Construction(error) => Some(error),
            Self::Profile(_) => None,
        }
    }
}

/// The logical object associated with a refusal. A constructed statement need
/// not have a parsed location; its actual identity remains available here.
#[derive(Clone, Copy, Debug)]
pub enum ProgramOriginal<'a> {
    /// A whole-program validation failure.
    Program,
    /// A named or parameterized part excluded by this single-shot profile.
    Part(&'a PartKey),
    /// The caller's original statement, with its complete provenance.
    Statement(&'a WithProvenance<Statement>),
}

/// Why a typed logical program was not admitted.
#[derive(Debug)]
pub enum ProgramFailureKind {
    /// Logical structure exceeded a configured ceiling.
    Limit(ProgramLimit),
    /// A statement cannot be compiled by the requested admission profile.
    Compilation(CompilationFailure),
}

impl fmt::Display for ProgramFailureKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit(error) => error.fmt(f),
            Self::Compilation(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ProgramFailureKind {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Limit(error) => Some(error),
            Self::Compilation(error) => Some(error),
        }
    }
}

/// A refusal borrowing the offending logical object instead of cloning it or
/// manufacturing a source span. No partially admitted program escapes.
#[derive(Debug)]
pub struct ProgramAdmissionFailure<'a> {
    /// The input object responsible for the refusal.
    pub original: ProgramOriginal<'a>,
    /// The typed cause.
    pub kind: ProgramFailureKind,
}

impl fmt::Display for ProgramAdmissionFailure<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}

impl std::error::Error for ProgramAdmissionFailure<'_> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.kind)
    }
}
