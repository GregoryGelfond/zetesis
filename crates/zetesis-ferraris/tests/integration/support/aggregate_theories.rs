//! Shared aggregate fixtures retain nodes and native operand rows together.

use zetesis_ferraris::{AggregateExtremum as Extremum, FormulaNodes, FormulaParts, Node, NodeView};

pub use zetesis_theory_support::aggregate::{COMPARISONS, ferraris_prefix as prefix};

/// Both extrema.
pub const EXTREMA: [Extremum; 2] = [Extremum::Min, Extremum::Max];

/// Copies both fixture buffers into a new, unchecked owner.
pub fn copy(nodes: &FormulaNodes) -> FormulaNodes {
    let (nodes, operands) = snapshot(nodes);
    FormulaNodes::new(FormulaParts::new(nodes, operands).unwrap())
}

/// Captures the complete represented graph, including native operand content.
pub fn snapshot(nodes: &FormulaNodes) -> (Vec<Node>, Vec<usize>) {
    (
        nodes.parts().nodes().to_vec(),
        nodes.parts().operands().to_vec(),
    )
}

/// Admits pair-only fixture storage through the ordinary raw ingress.
pub fn raw(nodes: Vec<Node>) -> FormulaNodes {
    FormulaNodes::new(FormulaParts::new(nodes, vec![]).unwrap())
}

/// Appends a checked fixture row, retaining the paired owner.
pub fn push(nodes: &mut FormulaNodes, node: NodeView<'_>) -> usize {
    let mut transaction = nodes.transaction();
    let index = transaction.push(node, usize::MAX, usize::MAX).unwrap();
    transaction.commit();
    index
}
