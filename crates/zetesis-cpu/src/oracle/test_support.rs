//! Helpers the oracle's unit tests share across its modules.

use zetesis_core::{AtomPattern, Template};

/// The rule `head :- body.`, whose body is positive.
pub(crate) fn rule(head: AtomPattern, body: Vec<AtomPattern>) -> Template {
    Template::new(Some(head), body, vec![], vec![], vec![])
}
