//! The hybrid schedule grounds a streamable constraint in its core when the
//! constraint's instances fit the share the limits grant, and streams it
//! otherwise; either way the original family is unchanged.

use super::*;
use std::collections::BTreeSet;
use zetesis_reference_support as reference;
use zetesis_reference_support::prepared;
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, prepare_formula};

/// `{p(1..3)}`, `{q(1..3)}` and one streamable constraint with three instances.
/// The double negation keeps pair capture from claiming the constraint.
const SINGLE: &str = "{p(1..3)}. {q(1..3)}. :- p(X), not not q(X).";

/// Per instance of SINGLE's constraint, the bound counts the implication to
/// falsum (1 node), `p(X)` (atom and conjunction: 2) and `not not q(X)` (atom,
/// two negations and conjunction: 4); its three instances bound 21 nodes.
const SINGLE_BOUND: usize = 21;

fn grounded(source: &str, limits: &FormulaLimits) -> HybridFormula {
    prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        *limits,
    )
    .unwrap()
    .ground_hybrid()
    .unwrap()
}

fn sharing(source: &str, share: usize) -> HybridFormula {
    grounded(
        source,
        &FormulaLimits {
            max_retained_constraint_nodes: share,
            ..FormulaLimits::default()
        },
    )
}

fn eager_family(source: &str) -> BTreeSet<BTreeSet<String>> {
    reference::exhaustive(&prepared(source).ground().unwrap())
        .into_iter()
        .map(|(atoms, cost)| {
            assert!(cost.is_none());
            atoms
        })
        .collect()
}

#[test]
fn a_granted_share_grounds_a_fitting_constraint_in_the_core() {
    let owner = sharing(SINGLE, FormulaLimits::default().theory.max_nodes);
    assert_eq!(owner.streamed_templates(), 0);
    assert_eq!(owner.streamed_instances(), 0);
    assert_eq!(owner.retained_templates(), 1);
    assert_eq!(owner.retained_instances(), 3);
    assert_eq!(full_family(&owner), eager_family(SINGLE));
}

#[test]
fn no_share_streams_every_eligible_constraint() {
    let owner = sharing(SINGLE, 0);
    assert_eq!(owner.streamed_templates(), 1);
    assert_eq!(owner.streamed_instances(), 3);
    assert_eq!(owner.retained_templates(), 0);
    assert_eq!(full_family(&owner), eager_family(SINGLE));
}

#[test]
fn templates_are_grounded_in_program_order_while_the_share_holds_them() {
    // The first constraint's instances bound 21 nodes; the second's one
    // instance bounds 6: the implication, `p(1)` (2) and `not q(2)` (3).
    let source = "{p(1..3)}. {q(1..3)}. :- p(X), not not q(X). :- p(1), not q(2).";
    let expected = eager_family(source);
    for (share, streamed, retained, retained_instances) in [
        // Neither fits.
        (5, 2, 0, 0),
        // The first does not fit; the later, smaller one still does.
        (20, 1, 1, 1),
        // The first fits and leaves 5 nodes, too few for the second.
        (26, 1, 1, 3),
        // Both fit exactly.
        (27, 0, 2, 4),
    ] {
        let owner = sharing(source, share);
        assert_eq!(owner.streamed_templates(), streamed, "share {share}");
        assert_eq!(owner.retained_templates(), retained, "share {share}");
        assert_eq!(
            owner.retained_instances(),
            retained_instances,
            "share {share}"
        );
        assert_eq!(full_family(&owner), expected, "share {share}");
    }
}

#[test]
fn a_template_that_exactly_fills_the_node_ceiling_is_grounded_without_refusal() {
    // The streamed core is the core retention starts from: capture and
    // completion are the same either way.
    let core = sharing(SINGLE, 0).core_theory().nodes().len();
    let limits = |max_nodes| FormulaLimits {
        max_retained_constraint_nodes: usize::MAX,
        theory: zetesis_ferraris::AdmissionLimits {
            max_nodes,
            ..FormulaLimits::default().theory
        },
        ..FormulaLimits::default()
    };
    let filled = grounded(SINGLE, &limits(core + SINGLE_BOUND));
    assert_eq!(filled.retained_templates(), 1);
    assert!(filled.core_theory().nodes().len() <= core + SINGLE_BOUND);
    let short = grounded(SINGLE, &limits(core + SINGLE_BOUND - 1));
    assert_eq!(short.retained_templates(), 0);
    assert_eq!(short.streamed_templates(), 1);
    assert_eq!(short.core_theory().nodes().len(), core);
}

#[test]
fn a_grounded_constraint_reads_fixed_facts_after_completion() {
    // `d/1` holds fixed facts, which grounding reads as true; retention runs
    // after the support guards and must still find them.
    let source = "d(1..2). {p(1..2)}. {q(1..2)}. :- p(X), d(X), not q(X).";
    let owner = sharing(source, usize::MAX);
    assert_eq!(owner.retained_templates(), 1);
    assert_eq!(owner.streamed_templates(), 0);
    assert_eq!(full_family(&owner), eager_family(source));
}
