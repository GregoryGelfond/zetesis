//! Replaying captured rows grounds exactly what joining the template again does.

use super::testing;
use crate::{AdmissionOptions, ExpansionLimits, FormulaLimits, HybridFormula, prepare_formula};

const PROGRAMS: &[&str] = &[
    "{p(1..3)}. {q(1..3)}. :- p(X), not not q(X).",
    "{p(1..3)}. {q(1..3)}. :- p(X), not not q(X). :- p(1), not q(2).",
    "#const n=6. 1 { q(X,1..n) } 1 :- X=1..n. :- q(X1,Y), q(X2,Y), X1 < X2. \
     :- q(X1,Y1), q(X2,Y2), X1 < X2, X2-X1 = |Y2-Y1|.",
    "{a(1..4)}. b(X) :- a(X), X > 2. :- a(X), not b(X), X < 4.",
    "{p(1..5)}. :- p(X), p(Y), X+Y = 6, X < Y.",
    "r(1). r(2). {p(1..3)}. :- p(X), r(X), not not p(X).",
    "{p(1..3)}. :- p(X), X > 10. :- p(X), not not p(X), X < 3.",
    "{p(1..3)}. {q(1;2)}. :- p(X), q(Y), X != Y, not q(X).",
    "{e(1,2);e(2,3);e(3,1);e(1,3)}. :- e(X,Y), e(Y,Z), e(Z,X), X < Y, Y < Z.",
];

fn grounded(source: &str, share: usize, recording: bool) -> (HybridFormula, u64) {
    let limits = FormulaLimits {
        max_retained_constraint_nodes: share,
        ..FormulaLimits::default()
    };
    testing::scoped(recording, || {
        prepare_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits,
        )
        .unwrap()
        .ground_hybrid()
        .unwrap()
    })
}

#[test]
fn replayed_rows_ground_the_same_core_as_a_second_join() {
    let shares = [
        0,
        1,
        5,
        20,
        26,
        27,
        64,
        FormulaLimits::default().theory.max_nodes,
    ];
    for source in PROGRAMS {
        for share in shares {
            let (joined, none) = grounded(source, share, false);
            let (replayed, _) = grounded(source, share, true);
            assert_eq!(none, 0);
            let (a, b) = (joined.core_theory(), replayed.core_theory());
            let context = format!("{source} at share {share}");
            assert_eq!(a.atom_count(), b.atom_count(), "{context}");
            assert_eq!(a.nodes(), b.nodes(), "{context}");
            assert_eq!(a.operands(), b.operands(), "{context}");
            assert_eq!(a.roots(), b.roots(), "{context}");
            assert_eq!(
                joined.retained_templates(),
                replayed.retained_templates(),
                "{context}"
            );
            assert_eq!(
                joined.retained_instances(),
                replayed.retained_instances(),
                "{context}"
            );
            assert_eq!(
                joined.streamed_templates(),
                replayed.streamed_templates(),
                "{context}"
            );
            assert_eq!(
                joined.streamed_instances(),
                replayed.streamed_instances(),
                "{context}"
            );
        }
    }
}

#[test]
fn every_template_retained_under_the_default_share_is_replayed() {
    // The default share grounds every streamed constraint of these programs in
    // the core, and capture kept the rows of each.
    for source in PROGRAMS {
        let (replayed, count) = grounded(source, FormulaLimits::default().theory.max_nodes, true);
        assert_eq!(replayed.streamed_templates(), 0, "{source}");
        assert_eq!(count, replayed.retained_templates() as u64, "{source}");
    }
}

#[test]
fn a_share_too_small_for_every_template_keeps_no_rows() {
    // No template of SINGLE fits 20 nodes, so none is recorded or replayed.
    let (owner, count) = grounded(PROGRAMS[0], 20, true);
    assert_eq!(owner.retained_templates(), 0);
    assert_eq!(count, 0);
}
