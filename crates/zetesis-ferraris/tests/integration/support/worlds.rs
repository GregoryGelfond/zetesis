//! The worlds of a two-atom theory: its interpretations chosen by bit mask, and
//! a node's truth in one of them.

use zetesis_ferraris::{FormulaView, Interpretation, NodeView, Theory};

/// The interpretation of `theory`'s two atoms holding those whose bits `world` sets.
pub(crate) fn interpretation(theory: &Theory, world: u8) -> Interpretation {
    Interpretation::new(theory, (0..2).filter(|atom| world & (1 << atom) != 0)).unwrap()
}

/// Whether node `root` holds in `world`: in the reduct by `frozen` when one is
/// given, where every subformula false in `frozen` is false.
pub(crate) fn eval(nodes: FormulaView<'_>, root: usize, world: u8, frozen: Option<u8>) -> bool {
    if frozen.is_some_and(|candidate| !eval(nodes, root, candidate, None)) {
        return false;
    }
    match nodes.node(root).unwrap() {
        NodeView::False => false,
        NodeView::Atom(atom) => world & (1 << atom) != 0,
        NodeView::And(operands) => operands
            .iter()
            .all(|&child| eval(nodes, child, world, frozen)),
        NodeView::Or(operands) => operands
            .iter()
            .any(|&child| eval(nodes, child, world, frozen)),
        NodeView::ExtremumCut(cut) => {
            use zetesis_ferraris::ExtremumCutRule::{Prefix, Suffix};
            match *cut.rule {
                Prefix(0) => false,
                Prefix(boundary) if boundary == cut.row.rank_count => true,
                Suffix(0) => true,
                Suffix(boundary) if boundary == cut.row.rank_count => false,
                rule => cut
                    .row
                    .conditions
                    .iter()
                    .zip(cut.row.ranks)
                    .any(|(&child, &rank)| {
                        let qualifies = match rule {
                            Prefix(boundary) => rank < boundary,
                            Suffix(boundary) => rank >= boundary,
                        };
                        qualifies && eval(nodes, child, world, frozen)
                    }),
            }
        }
        NodeView::Threshold(row) => {
            row.conditions
                .iter()
                .zip(row.weights)
                .filter(|(condition, _)| eval(nodes, **condition, world, frozen))
                .map(|(_, weight)| i128::from(*weight))
                .sum::<i128>()
                >= row.bound
        }
        NodeView::Implies(a, b) => !eval(nodes, a, world, frozen) || eval(nodes, b, world, frozen),
    }
}
