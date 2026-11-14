//! The statements that keep an admitted owner's zero-divisor warnings.
//!
//! An instance that divides by zero is omitted, and its statement keeps one
//! warning however many of its instances are omitted. These helpers read the
//! text of such a statement from the source the owner was admitted from.

use themelios_base::source::{Source, SourceId};
use themelios_base::span::Location;
use zetesis_themelios::{AdmittedFormula, FormulaWarning, HybridFormula};

/// An admitted owner: the warnings its admission kept and the source they are
/// located in.
pub(crate) trait Warned {
    /// The warnings the admission kept.
    fn warnings(&self) -> &[FormulaWarning];

    /// The source the owner was admitted from, absent for a constructed
    /// program.
    fn source(&self) -> Option<&Source>;
}

impl Warned for AdmittedFormula {
    fn warnings(&self) -> &[FormulaWarning] {
        AdmittedFormula::warnings(self)
    }

    fn source(&self) -> Option<&Source> {
        AdmittedFormula::source(self)
    }
}

impl Warned for HybridFormula {
    fn warnings(&self) -> &[FormulaWarning] {
        HybridFormula::warnings(self)
    }

    fn source(&self) -> Option<&Source> {
        HybridFormula::source(self)
    }
}

/// The source `owner` was admitted from.
fn admitted_source(owner: &impl Warned) -> &Source {
    owner.source().expect("source input")
}

/// The text `location` spans in the source `owner` was admitted from.
fn text_at(owner: &impl Warned, location: Location) -> &str {
    admitted_source(owner)
        .slice(location.span)
        .expect("a span of the admitted source")
}

/// The location of `owner`'s one warning, a zero divisor.
fn warning_location(owner: &impl Warned) -> Location {
    let [FormulaWarning::ZeroDivisor { location: site }] = owner.warnings() else {
        panic!(
            "{}: one zero-divisor warning: {:?}",
            admitted_source(owner).text(),
            owner.warnings()
        );
    };
    site.location().expect("parsed source")
}

/// The statement that keeps `owner`'s one warning: a zero divisor reached by
/// an instance of that statement.
///
/// # Panics
/// Panics unless `owner` keeps exactly one warning, located in the source it
/// was admitted from.
pub(crate) fn zero_divisor_statement(owner: &impl Warned) -> &str {
    text_at(owner, warning_location(owner))
}

/// [`zero_divisor_statement`], for a warning that must also name the source
/// `source_id` identifies.
///
/// # Panics
/// Panics as [`zero_divisor_statement`] does, and if the warning names another
/// source.
pub(crate) fn zero_divisor_statement_in(source_id: SourceId, owner: &impl Warned) -> &str {
    let location = warning_location(owner);
    assert_eq!(
        location.source,
        source_id,
        "{}",
        admitted_source(owner).text()
    );
    text_at(owner, location)
}

/// The statements that keep `owner`'s warnings, each a zero divisor, in the
/// order `owner` keeps them.
///
/// # Panics
/// Panics unless every warning is located in the source `owner` was admitted
/// from.
pub(crate) fn zero_divisor_statements(owner: &impl Warned) -> Vec<&str> {
    owner
        .warnings()
        .iter()
        .map(|warning| {
            let FormulaWarning::ZeroDivisor { location: site } = warning;
            text_at(owner, site.location().expect("parsed source"))
        })
        .collect()
}
