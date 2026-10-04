//! One original source and parse, transferable between admission profiles.

use std::fmt;

use themelios_base::source::Source;
use themelios_base::span::{ByteOffset, Location, Span};
use themelios_syntax::ast;
use themelios_syntax::dialect::Dialect;
use themelios_syntax::parse::{Parse, parse};

use crate::{
    AdmissionFailure, AdmissionOptions, Admitted, ExpansionFailure, ExpansionLimits,
    FormulaFailure, FormulaLimits, InputLimit, PreparedFormula, SyntaxFailure, extended, formula,
};

/// One syntax-checked input with fixed source-admission options.
///
/// Construction takes ownership of the original UTF-8 bytes and parses once.
/// The source-byte ceiling bounds that input; each later profile operation
/// checks its syntax-node/depth and language restrictions in their existing
/// order. Successful parsing therefore does not certify a language profile,
/// finite grounding, or existence of an answer set.
///
/// Retaining this owner retains the source and upstream syntax tree, with
/// O(source bytes + syntax nodes) space. Moving it between profile attempts
/// neither clones the source nor reparses it. Each attempt still performs its
/// own checked raising and normalization with the supplied expansion limits.
///
/// ```
/// use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, ParsedSource};
/// let source = ParsedSource::new("1{a;b}1.".into(), AdmissionOptions::default())?;
/// let refused = source.admit_extended(ExpansionLimits::default()).unwrap_err();
/// assert!(refused.error().needs_formula_admission());
/// let prepared = refused.into_source()
///     .prepare_formula(ExpansionLimits::default(), FormulaLimits::default())?;
/// let admitted = prepared.ground()?;
/// assert_eq!(admitted.atoms().len(), 2);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct ParsedSource {
    source: Source,
    parsed: Parse<ast::Program>,
    options: AdmissionOptions,
}

impl ParsedSource {
    /// Parse original bytes under the clingo dialect, retaining their identity.
    /// Parsing is linear in the admitted input; no filesystem access occurs.
    ///
    /// # Errors
    /// Returns the original source/byte refusal or complete parser diagnostics.
    /// Syntax failure owns the rejected source; no recovered owner is returned.
    pub fn new(text: String, options: AdmissionOptions) -> Result<Self, AdmissionFailure> {
        let start = Location {
            source: options.source_id,
            span: Span::empty(ByteOffset::new(0)),
        };
        if text.len() > options.max_source_bytes {
            return Err(AdmissionFailure::Limit {
                resource: InputLimit::SourceBytes,
                limit: options.max_source_bytes,
                observed: text.len(),
                location: start.into(),
            });
        }
        let source =
            Source::new(options.source_id, text).map_err(|error| AdmissionFailure::Source {
                error,
                location: start,
            })?;
        let parsed = parse(&source, Dialect::Clingo);
        if !parsed.diagnostics().is_empty() {
            let diagnostics = parsed.diagnostics().to_vec();
            return Err(AdmissionFailure::Syntax(SyntaxFailure::new(
                source,
                diagnostics,
            )));
        }
        Ok(Self {
            source,
            parsed,
            options,
        })
    }

    /// Original source identity and bytes, borrowed without reconstruction.
    #[must_use]
    pub fn source(&self) -> &Source {
        &self.source
    }

    /// Original successful parse; locations refer to [`Self::source`].
    #[must_use]
    pub fn parsed(&self) -> &Parse<ast::Program> {
        &self.parsed
    }

    /// The immutable source/profile limits used by each admission attempt.
    #[must_use]
    pub fn options(&self) -> AdmissionOptions {
        self.options
    }

    /// Admit relational templates through the extended source profile.
    /// Costs and refusals are those of [`crate::admit_extended`] after parsing.
    /// This computes no answer sets and performs no static grounding.
    ///
    /// # Errors
    /// Retains this exact owner beside the typed expansion refusal. A caller
    /// may explicitly recover it to attempt formula preparation; recovery does
    /// not classify the error as eligible for that alternative profile.
    pub fn admit_extended(
        self,
        limits: ExpansionLimits,
    ) -> Result<Admitted, SourceFailure<ExpansionFailure>> {
        extended::admit_parsed(self, limits)
    }

    /// Prepare the finite formula profile without reparsing original bytes.
    /// Costs and budgets are those of [`crate::prepare_formula`] after parsing.
    /// The returned preparation retains the original source until grounding.
    ///
    /// # Errors
    /// Retains this owner beside the complete typed preparation refusal. No
    /// partial preparation or completed grounding is returned on failure.
    pub fn prepare_formula(
        self,
        expansion: ExpansionLimits,
        limits: FormulaLimits,
    ) -> Result<PreparedFormula, SourceFailure<FormulaFailure>> {
        formula::prepare_parsed(self, expansion, &limits)
    }

    /// Recover original source bytes and identity, discarding the syntax tree.
    #[must_use]
    pub fn into_source(self) -> Source {
        self.source
    }
}

/// A profile refusal retaining its exact parsed source for inspection or reuse.
/// One allocation retains the original parsed owner and typed error together,
/// without copying source bytes or the syntax tree. The public wrapper holds
/// only that owner handle. Retained space includes the source and cause. No
/// partial compiled program or preparation is retained as a successful result.
pub struct SourceFailure<E> {
    retained: Box<RetainedFailure<E>>,
}

struct RetainedFailure<E> {
    source: ParsedSource,
    error: E,
}

impl<E> SourceFailure<E> {
    pub(crate) fn new(source: ParsedSource, error: E) -> Self {
        Self {
            retained: Box::new(RetainedFailure { source, error }),
        }
    }

    /// The original source and syntax owner; no clone or reparse occurs.
    #[must_use]
    pub fn source(&self) -> &ParsedSource {
        &self.retained.source
    }

    /// The unchanged typed refusal from the attempted profile.
    #[must_use]
    pub fn error(&self) -> &E {
        &self.retained.error
    }

    /// Recover the source owner, discarding the refusal.
    #[must_use]
    pub fn into_source(self) -> ParsedSource {
        self.retained.source
    }

    /// Recover the refusal, discarding the source and syntax tree.
    #[must_use]
    pub fn into_error(self) -> E {
        self.retained.error
    }

    /// Recover both original owners without cloning either.
    #[must_use]
    pub fn into_parts(self) -> (ParsedSource, E) {
        (self.retained.source, self.retained.error)
    }
}

impl<E: fmt::Debug> fmt::Debug for SourceFailure<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceFailure")
            .field("source", &self.retained.source)
            .field("error", &self.retained.error)
            .finish()
    }
}

impl<E: fmt::Display> fmt::Display for SourceFailure<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.retained.error.fmt(formatter)
    }
}

impl<E: std::error::Error + 'static> std::error::Error for SourceFailure<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.retained.error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusal_recovery_transfers_original_owners() {
        struct Cause(Box<u8>);

        let source = ParsedSource::new("p.".into(), AdmissionOptions::default()).unwrap();
        let source_bytes = source.source().text().as_ptr();
        let error = Cause(Box::new(7));
        let error_payload = std::ptr::from_ref(error.0.as_ref());
        let failure = SourceFailure::new(source, error);
        assert_eq!(failure.source().source().text().as_ptr(), source_bytes);
        assert_eq!(
            std::ptr::from_ref(failure.error().0.as_ref()),
            error_payload
        );
        let (source, error) = failure.into_parts();
        assert_eq!(source.source().text().as_ptr(), source_bytes);
        assert_eq!(std::ptr::from_ref(error.0.as_ref()), error_payload);
        let error = SourceFailure::new(source, error).into_error();
        assert_eq!(std::ptr::from_ref(error.0.as_ref()), error_payload);
        assert_eq!(*error.0, 7);
    }
}
