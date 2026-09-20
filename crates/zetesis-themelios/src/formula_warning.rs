//! Bounded located warnings retained by successful formula admission.

use std::fmt;

use themelios_base::{
    diagnostic::{Diagnostic, DiagnosticId, Label, Severity, ToDiagnostic},
    source::Source,
    span::Location,
};

use crate::SourceBundle;

/// A nonfatal source condition observed during successful formula admission.
/// Warnings describe omitted instances without retaining substitutions or
/// counting repeated evaluations during support completion and grounding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormulaWarning {
    /// An evaluated division or remainder had a zero divisor in a family that
    /// also had defined instances. Instances reaching that operation were omitted.
    ZeroDivisor {
        /// Original source construct containing the evaluated operation.
        location: Location,
    },
}

impl FormulaWarning {
    /// Original source location; identities resolve in the admitted owner.
    #[must_use]
    pub const fn location(self) -> Location {
        match self {
            Self::ZeroDivisor { location } => location,
        }
    }
}

impl ToDiagnostic for FormulaWarning {
    fn to_diagnostic(&self) -> Diagnostic {
        let Self::ZeroDivisor { location } = *self;
        Diagnostic::new(
            DiagnosticId::new("zetesis", "zero-divisor"),
            Severity::Warning,
            "instances omitted because an evaluated division or remainder had a zero divisor"
                .into(),
            Label {
                location,
                message: None,
            },
        )
        .expect("the zero-divisor warning has a nonempty diagnostic message")
        .with_help("guard the denominator to exclude zero".into())
    }
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

enum Sources<'a> {
    Single(&'a Source),
    Bundle(&'a SourceBundle),
}

struct View<'a> {
    warnings: &'a [FormulaWarning],
    sources: Sources<'a>,
}

impl fmt::Display for View<'_> {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.warnings.is_empty() {
            return Ok(());
        }
        match &self.sources {
            Sources::Single(source) => crate::source_diagnostics::write_diagnostics(
                output,
                "<input>",
                source,
                self.warnings.iter().map(ToDiagnostic::to_diagnostic),
            ),
            Sources::Bundle(bundle) => {
                for warning in self.warnings {
                    crate::source_diagnostics::write_diagnostic(
                        output,
                        &warning.to_diagnostic(),
                        *bundle,
                    )?;
                }
                Ok(())
            }
        }
    }
}
