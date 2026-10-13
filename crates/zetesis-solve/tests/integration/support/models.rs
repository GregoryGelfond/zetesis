//! Families of models, as the session propositions compare them.

use std::collections::BTreeSet;

use zetesis_core::Model;

/// A family of models, compared as a set.
pub(crate) type Family = BTreeSet<Model>;
