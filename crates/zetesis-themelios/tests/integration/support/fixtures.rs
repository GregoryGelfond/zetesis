//! The fixture files several modules read.

/// Objective sources whose extrema admission refuses, one JSON case per line.
pub const OBJECTIVE_EXTREMA_REFUSALS: &str =
    include_str!("../../fixtures/objective-extrema-refusals.jsonl");

/// Objectives at the language's boundaries, one JSON case per line.
pub const OBJECTIVE_LANGUAGE_BOUNDARIES: &str =
    include_str!("../../fixtures/objective-language-boundaries.jsonl");
