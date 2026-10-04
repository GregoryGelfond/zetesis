//! Relational admission from the canonical logical program, without source text.

use std::fmt;

use themelios_program::program::{PartKey, Program as LogicalProgram, Statement};
use themelios_program::provenance::WithProvenance;
use zetesis_core::{AdmissionError, AdmissionLimits, ConstructionError, Program};

use crate::{AdmissionFailure, ProfileFeature};

pub use crate::program_limits::{Limit as ProgramLimit, Resource as ProgramResource};

/// Limits for an already constructed logical program. Source-byte and syntax
/// limits belong to source admission; these bounds inspect canonical structure.
///
/// [`admit_program`] visits relational rule structure without copying or counting
/// its borrowed provenance. [`crate::prepare_program_formula`] also visits
/// admitted directives, nested conditions, provenance entries and annotations
/// before normalization can copy them. Neither operation accounts for the
/// caller's earlier program construction, allocator overhead or process RSS.
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
    /// Native template admission limits, also used for formula template,
    /// variable, arity and value ceilings.
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
    /// A construct outside the strict relational profile.
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
            Self::Profile(feature) => write!(f, "relational admission does not support {feature}"),
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
pub enum ProgramSubject<'a> {
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
    /// The independently checked native program failed admission.
    Core(AdmissionError),
}

impl fmt::Display for ProgramFailureKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit(error) => error.fmt(f),
            Self::Compilation(error) => error.fmt(f),
            Self::Core(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ProgramFailureKind {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Limit(error) => Some(error),
            Self::Compilation(error) => Some(error),
            Self::Core(error) => Some(error),
        }
    }
}

/// A refusal borrowing the offending logical object instead of cloning it or
/// manufacturing a source span. No partially admitted program escapes.
#[derive(Debug)]
pub struct ProgramAdmissionFailure<'a> {
    /// The input object responsible for the refusal.
    pub subject: ProgramSubject<'a>,
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

/// Checked relational templates and their original logical statements.
/// Compilation does not copy the caller's program. Consuming `into_program`
/// releases the provenance borrow and leaves an independently owned native value.
#[derive(Debug)]
pub struct AdmittedProgram<'a> {
    program: Program,
    statements: Vec<Vec<&'a WithProvenance<Statement>>>,
}

impl<'a> AdmittedProgram<'a> {
    /// The same canonical relational representation used by source admission.
    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }

    /// Original statements in native template order. A generated strong-negation
    /// coherence constraint names both statements that introduced its signatures.
    #[must_use]
    pub fn template_statements(&self) -> &[Vec<&'a WithProvenance<Statement>>] {
        &self.statements
    }

    /// Release the input borrow while retaining the checked native program.
    #[must_use]
    pub fn into_program(self) -> Program {
        self.program
    }
}

/// Compile a canonical logical program under the strict relational (S0) profile.
///
/// This is the typed counterpart of [`crate::admit`]. It accepts base-part rules
/// with scalar relational joins,
/// default negation, equality/disequality filters and unbounded single-atom
/// choices. Use [`crate::prepare_program_formula`] for the general formula
/// profile, including arithmetic generators, aggregates and directives.
/// No rendering, parsing, grounding or answer enumeration occurs here.
///
/// The compiler and strong-negation coherence operation are shared with source
/// admission. Source admission additionally checks authored syntax before
/// canonicalization; those source checks are not replaced by this operation.
/// Logical traversal is bounded and iterative. Retained input evidence consists
/// of borrowed statement references; the native program owns its canonical data.
///
/// # Errors
/// Returns a typed limit, capability, construction or native admission failure,
/// naming the caller's actual statement or part when applicable.
pub fn admit_program(
    source: &LogicalProgram,
    options: ProgramAdmissionOptions,
) -> Result<AdmittedProgram<'_>, ProgramAdmissionFailure<'_>> {
    let mut budget = crate::program_limits::Budget::new(crate::program_limits::Limits {
        nodes: options.max_nodes,
        depth: options.max_depth,
        body_elements: options.max_body_elements,
        text_bytes: options.max_text_bytes,
    });
    let mut templates = Vec::new();
    let mut statements = Vec::new();
    for part in source.parts() {
        if part.key().name.as_str() != "base" || !part.key().formals.is_empty() {
            return Err(ProgramAdmissionFailure {
                subject: ProgramSubject::Part(part.key()),
                kind: ProgramFailureKind::Compilation(CompilationFailure::Profile(
                    ProfileFeature::ProgramPart,
                )),
            });
        }
        for carrier in part.statements() {
            let subject = ProgramSubject::Statement(carrier);
            if templates.len() >= options.core_limits.max_templates {
                return Err(core_failure(
                    AdmissionError::LimitExceeded {
                        resource: zetesis_core::AdmissionResource::Templates,
                        limit: options.core_limits.max_templates,
                        actual: templates.len().saturating_add(1),
                        template: Some(templates.len()),
                    },
                    Some(carrier),
                ));
            }
            if let Statement::Rule(rule) = carrier.get() {
                budget
                    .check_rule(rule)
                    .map_err(|error| ProgramAdmissionFailure {
                        subject,
                        kind: ProgramFailureKind::Limit(error),
                    })?;
            }
            let template = crate::compile::checked_statement(carrier.get()).map_err(|error| {
                ProgramAdmissionFailure {
                    subject,
                    kind: ProgramFailureKind::Compilation(error),
                }
            })?;
            templates.push(template);
            statements.push(vec![carrier]);
        }
    }
    crate::coherence::append_with(
        &mut templates,
        &mut statements,
        options.core_limits,
        |_, _, _| Ok(()),
        |error, origins| core_failure(error, origins.first().copied()),
    )?;
    let program = Program::new(templates, options.core_limits).map_err(|error| {
        let statement = error
            .template_index()
            .and_then(|index| statements.get(index))
            .and_then(|origins| origins.first().copied());
        core_failure(error, statement)
    })?;
    Ok(AdmittedProgram {
        program,
        statements,
    })
}

fn core_failure(
    error: AdmissionError,
    statement: Option<&WithProvenance<Statement>>,
) -> ProgramAdmissionFailure<'_> {
    ProgramAdmissionFailure {
        subject: statement.map_or(ProgramSubject::Program, ProgramSubject::Statement),
        kind: ProgramFailureKind::Core(error),
    }
}
