//! Generated theories: the anchored-cone summary never suppresses a deduction.
//!
//! Each case builds Boolean chains of both kinds, shared and nested, with unit
//! native rows over atoms and connectives, and checks the summary against the
//! discoveries it lets connective transport skip.

use proptest::prelude::*;

use crate::regions::Work;
use crate::{
    AdmissionLimits, FormulaNodes, FormulaParts, Narrower, NarrowingScratch, Node, NodeView, Theory,
};

const ATOMS: usize = 6;

#[derive(Debug, Clone)]
struct Case {
    /// Connectives in creation order: (disjunction, operand picks).
    connectives: Vec<(bool, Vec<usize>)>,
    /// Unit rows: two condition picks each, over atoms and connectives.
    rows: Vec<(usize, usize)>,
}

fn case() -> impl Strategy<Value = Case> {
    (
        prop::collection::vec(
            (any::<bool>(), prop::collection::vec(any::<usize>(), 2..4)),
            1..9,
        ),
        prop::collection::vec((any::<usize>(), any::<usize>()), 1..3),
    )
        .prop_map(|(connectives, rows)| Case { connectives, rows })
}

impl Case {
    /// The theory. A connective whose picks name fewer than two distinct
    /// operands, and a row whose two picks coincide, are left out; every other
    /// generated shape is admissible.
    fn theory(&self) -> Theory {
        let mut nodes = FormulaNodes::new(
            FormulaParts::new((0..ATOMS).map(Node::atom).collect(), vec![]).unwrap(),
        );
        let mut built: Vec<usize> = (0..ATOMS).collect();
        let mut roots = Vec::new();
        let mut append = nodes.transaction();
        for (disjunction, picks) in &self.connectives {
            let mut operands: Vec<usize> =
                picks.iter().map(|pick| built[pick % built.len()]).collect();
            operands.sort_unstable();
            operands.dedup();
            if operands.len() < 2 {
                continue;
            }
            let view = if *disjunction {
                NodeView::Or(&operands)
            } else {
                NodeView::And(&operands)
            };
            let node = append
                .push(view, usize::MAX, usize::MAX)
                .expect("generated connectives are admissible");
            built.push(node);
            roots.push(node);
        }
        append.commit();
        for &(first, second) in &self.rows {
            let mut conditions = [built[first % built.len()], built[second % built.len()]];
            conditions.sort_unstable();
            if conditions[0] == conditions[1] {
                continue;
            }
            let row = nodes
                .push_threshold_family(&conditions, &[1, 1], &[1], usize::MAX, usize::MAX)
                .expect("generated unit rows are admissible");
            roots.extend(row);
        }
        Theory::new(ATOMS, nodes.into_parts(), roots, AdmissionLimits::default())
            .expect("generated theories are admissible")
    }
}

proptest! {
    /// The skip's soundness, chain by chain: for every chain the summary
    /// leaves unmarked, neither discovery reaches a row. The flat route reads
    /// the chain's least-leaf posting; the expanded route reads the anchor
    /// postings of every node its cone walk collects.
    #[test]
    fn unmarked_chains_reach_no_native_row(case in case()) {
        let theory = case.theory();
        let index = Narrower::new(&theory);
        // Without a unit row there is no connective index, and nothing to check.
        prop_assume!(index.thresholds.connectives.is_some());
        let anchors = index.thresholds.connectives.as_ref().unwrap();
        let mut scratch = NarrowingScratch::default();
        scratch.prepare_thresholds(index.thresholds.rows.len()).unwrap();
        let mut work = Work::new(u64::MAX);
        for chain in 0..index.chains.len() {
            if anchors.anchored_cone(chain) {
                continue;
            }
            prop_assert!(
                anchors.rows(chain, &mut work).unwrap().is_empty(),
                "chain {} posts a row under its least leaf", chain
            );
            scratch.witnesses.cone(&index, chain, &mut work).unwrap();
            for &node in scratch.witnesses.prepared() {
                prop_assert!(
                    anchors.rows_at(node, &mut work).unwrap().is_empty(),
                    "node {} in the cone of chain {} anchors a row", node, chain
                );
            }
        }
    }
}
