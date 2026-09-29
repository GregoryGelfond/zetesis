//! The answer sets a comparison holds, from either side of it.
//!
//! A comparison of zetesis with clingo reads each side's answer sets into
//! [`Records`]: plain data, so neither side's crate is needed to hold them.

use std::collections::BTreeSet;

/// Every answer set of one program, each as the spellings of its atoms and
/// its costs, which are absent when the program has no objective.
pub type Records = BTreeSet<(BTreeSet<String>, Option<Vec<i64>>)>;
