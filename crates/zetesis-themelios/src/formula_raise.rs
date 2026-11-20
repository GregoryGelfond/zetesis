//! Raise source occurrences once, preserving diagnostics and metadata.

use themelios_program::program::Program;
use themelios_program::program::Statement;
use themelios_program::provenance::WithProvenance;
use themelios_program::raise::{StatementOccurrence, raise_occurrences};
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

/// The raised statements of one source, owned and unmerged, for a caller that
/// collects several sources into one program: each part's occurrences in
/// source order, the parts in key order. Collecting them merges equal
/// statements as [`raise`]'s program and a later collection of its statements
/// together would: the merge keeps the last of the equal statements and
/// unions their provenance, so merging once over this order equals merging
/// each part and then the flattened parts. Nothing is cloned.
pub(crate) fn raise_statements(
    parsed: &Parse<ast::Program>,
    metadata: &mut metadata::Builder,
) -> Result<Vec<WithProvenance<Statement>>, FormulaFailure> {
    let occurrences = raise_occurrences(parsed);
    if !occurrences.diagnostics().is_empty() {
        return Err(AdmissionFailure::Raise(occurrences.diagnostics().to_vec()).into());
    }
    metadata::collect_occurrences(&occurrences, metadata)?;
    let mut occurrences = occurrences.into_occurrences();
    // Stable: equal parts keep their source order.
    occurrences.sort_by(|left, right| left.part().cmp(right.part()));
    Ok(occurrences
        .into_iter()
        .map(StatementOccurrence::into_statement)
        .collect())
}

#[cfg(test)]
mod tests;
