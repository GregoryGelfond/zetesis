//! Reusable corpus integrity, bounded process capture, and reported-answer checks.
#![forbid(unsafe_code)]

pub mod answers;
pub mod curated;
pub mod process;
pub mod selected;

/// Canonical native atoms and closed values used by structured report views.
pub use zetesis_core as core;

/// Canonical pinned identifier vocabulary used by validated ASP spelling views.
pub use themelios_program;
