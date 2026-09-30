//! The comparisons, extrema and node prefix from which the aggregate
//! propositions build their theories.

use zetesis_ferraris::{AggregateExtremum as Extremum, Node};

pub use zetesis_theory_support::aggregate::{COMPARISONS, ferraris_prefix as prefix};

/// Both extrema.
pub const EXTREMA: [Extremum; 2] = [Extremum::Min, Extremum::Max];

/// Appends `node` to `nodes`, returning its index.
pub fn push(nodes: &mut Vec<Node>, node: Node) -> usize {
    let index = nodes.len();
    nodes.push(node);
    index
}
