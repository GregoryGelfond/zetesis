//! Raise source occurrences once, preserving diagnostics and metadata.

use themelios_program::program::Program;
use themelios_program::raise::raise_occurrences;
use themelios_syntax::ast;
use themelios_syntax::parse::Parse;

use crate::{AdmissionFailure, FormulaFailure, metadata};

/// Metadata reads the occurrence stream before equal statements merge. Counted
/// choice elements retain their own multiplicity in the resulting program.
pub(crate) fn raise(
    parsed: &Parse<ast::Program>,
    metadata: &mut metadata::Builder,
) -> Result<Program, FormulaFailure> {
    let occurrences = raise_occurrences(parsed);
    if !occurrences.diagnostics().is_empty() {
        return Err(AdmissionFailure::Raise(occurrences.diagnostics().to_vec()).into());
    }
    metadata::collect_occurrences(&occurrences, metadata)?;
    Ok(occurrences.into_raised().into_program())
}

#[cfg(test)]
mod tests;
