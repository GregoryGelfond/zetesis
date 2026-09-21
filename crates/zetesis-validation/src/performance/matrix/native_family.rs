//! Exact native families, independent of publication and catalog ordering.
//!
//! One decoded qualification family is retained per workload. Comparisons borrow
//! atom/value payloads and sort only pointer vectors outside the measured interval.
//! Duplicate records and shown terms remain occurrences, never set entries.

use crate::answers::native_json::{ModelRecord, NativeAnswers};
use zetesis_core::{Atom, Value};

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Record<'a> {
    atoms: Vec<&'a Atom>,
    shown: Vec<&'a Atom>,
    terms: Vec<&'a Value>,
    costs: Option<&'a [(i32, i64)]>,
}

impl<'a> From<&'a ModelRecord> for Record<'a> {
    fn from(model: &'a ModelRecord) -> Self {
        let mut atoms: Vec<_> = model.full_model().iter().collect();
        let mut shown: Vec<_> = model
            .shown_atom_indices()
            .iter()
            .map(|&index| &model.full_model()[index])
            .collect();
        let mut terms: Vec<_> = model.shown_terms().iter().collect();
        atoms.sort_unstable();
        shown.sort_unstable();
        terms.sort_unstable();
        Self {
            atoms,
            shown,
            terms,
            costs: model.costs(),
        }
    }
}

fn records(family: &NativeAnswers) -> Vec<Record<'_>> {
    let mut records: Vec<_> = family.records().iter().map(Record::from).collect();
    records.sort_unstable();
    records
}

/// Record the first successful native census, then require exact native parity.
/// The caller supplies only completely decoded, reference-qualified answers.
/// Retained population is bounded by the existing per-report decoder/capture
/// limits and finite case count; it is not an allocator-RSS bound.
pub(super) fn accept(
    baseline: &mut Option<NativeAnswers>,
    observed: NativeAnswers,
) -> Result<(), (super::Decision, String)> {
    if let Some(expected) = baseline {
        if expected.costs() != observed.costs() || records(expected) != records(&observed) {
            return Err((super::Decision::ParityMismatch,
                "complete native atoms, shown values, multiplicities or costs differ from the native census".into()));
        }
    } else {
        *baseline = Some(observed);
    }
    Ok(())
}
