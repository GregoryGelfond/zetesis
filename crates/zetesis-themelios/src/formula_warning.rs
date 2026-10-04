//! Bounded warnings retained by successful formula admission.

use std::fmt;

use themelios_base::{
    diagnostic::{Diagnostic, DiagnosticId, Label, Severity},
    source::Source,
    span::Location,
};

use crate::{ProgramSite, SourceBundle};

const ZERO_DIVISOR: &str =
    "instances omitted because an evaluated division or remainder had a zero divisor";

/// A nonfatal condition observed during successful formula admission.
/// Warnings describe omitted instances without retaining substitutions or
/// counting repeated evaluations during support completion and grounding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormulaWarning {
    /// A zero divisor occurred in a family that also had defined instances.
    ZeroDivisor {
        /// Original statement containing the evaluated operation.
        location: ProgramSite,
    },
}

impl FormulaWarning {
    /// Original statement identity, resolved in the admitted owner.
    #[must_use]
    pub const fn site(self) -> ProgramSite {
        match self {
            Self::ZeroDivisor { location } => location,
        }
    }

    /// Actual source coordinate, absent for constructed statements.
    #[must_use]
    pub const fn location(self) -> Option<Location> {
        self.site().location()
    }

    /// A source diagnostic when an actual parsed coordinate is available.
    /// Returns `None` only when the warning has no source coordinate.
    #[must_use]
    pub fn diagnostic(self) -> Option<Diagnostic> {
        self.location().map(zero_divisor_diagnostic)
    }
}

/// The diagnostic constructor can refuse only an empty headline. This fixed
/// warning message is nonempty independently of every caller-supplied value.
fn zero_divisor_diagnostic(location: Location) -> Diagnostic {
    Diagnostic::new(
        DiagnosticId::new("zetesis", "zero-divisor"),
        Severity::Warning,
        ZERO_DIVISOR.into(),
        Label {
            location,
            message: None,
        },
    )
    .expect("the zero-divisor warning has a nonempty diagnostic message")
    .with_help("guard the denominator to exclude zero".into())
}

pub(crate) fn source_view<'a>(
    warnings: &'a [FormulaWarning],
    source: &'a Source,
) -> impl fmt::Display + 'a {
    View {
        warnings,
        sources: Sources::Single(source),
    }
}

pub(crate) fn bundle_view<'a>(
    warnings: &'a [FormulaWarning],
    bundle: &'a SourceBundle,
) -> impl fmt::Display + 'a {
    View {
        warnings,
        sources: Sources::Bundle(bundle),
    }
}

pub(crate) fn program_view(warnings: &[FormulaWarning]) -> impl fmt::Display + '_ {
    View {
        warnings,
        sources: Sources::Program,
    }
}

enum Sources<'a> {
    Single(&'a Source),
    Bundle(&'a SourceBundle),
    Program,
}
struct View<'a> {
    warnings: &'a [FormulaWarning],
    sources: Sources<'a>,
}

impl fmt::Display for View<'_> {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.sources {
            Sources::Single(source) => crate::source_diagnostics::write_diagnostics(
                output,
                "<input>",
                source,
                self.warnings
                    .iter()
                    .filter_map(|warning| warning.diagnostic()),
            ),
            Sources::Bundle(bundle) => {
                for diagnostic in self
                    .warnings
                    .iter()
                    .filter_map(|warning| warning.diagnostic())
                {
                    crate::source_diagnostics::write_diagnostic(output, &diagnostic, *bundle)?;
                }
                Ok(())
            }
            Sources::Program => {
                for warning in self.warnings {
                    if let Some(statement) = warning.site().statement_id() {
                        write!(output, "statement {}: ", statement.index())?;
                    }
                    writeln!(
                        output,
                        "warning: {ZERO_DIVISOR}; guard the denominator to exclude zero"
                    )?;
                }
                Ok(())
            }
        }
    }
}
