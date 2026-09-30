//! A session's record of one model: its atoms and, when it optimizes, its costs.

use std::collections::BTreeSet;

use zetesis_core::Atom;

/// A model's atoms, with its (priority, cost) pairs when the session optimizes.
pub type Record = (BTreeSet<Atom>, Option<Vec<(i32, i64)>>);
