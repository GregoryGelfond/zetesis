//! Extraction entry point for the unchanged solver's membership operation.

#![forbid(unsafe_code)]

/// Read the actual library implementation; this harness supplies no replacement.
#[must_use]
pub fn membership(candidate: &zetesis_ferraris::Interpretation, atom: usize) -> bool {
    candidate.contains(atom)
}
