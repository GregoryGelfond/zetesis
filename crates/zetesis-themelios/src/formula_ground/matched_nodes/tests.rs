//! Exact graph/catalog comparisons select the new route in the same binary.
use super::{MatchedNodes, testing as route};
use crate::formula::Compiled;
use crate::formula_ground::{Builder, Purpose, ground, ground_hybrid};
use crate::formula_support::{Context, Join, testing};
use crate::test_support::compiled_atoms;
use crate::{FormulaFailure, FormulaLimits, FormulaResource, GroundingWork, ProgramSite};
use std::cell::Cell;

#[derive(Default)]
struct Observer(Cell<GroundingWork>);
impl crate::GroundingObserver for Observer {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        phase: crate::GroundingPhase,
        _: Option<ProgramSite>,
        _: crate::GroundingOutcome,
        work: GroundingWork,
    ) {
        if phase == crate::GroundingPhase::RuleInstantiation {
            self.0.set(self.0.get().checked_sum(work));
        }
    }
}

fn compile(source: &str, enabled: bool, hybrid: bool) -> (Compiled, GroundingWork, route::Visits) {
    // Disable continuation factoring in both runs. Component factoring is
    // unchanged: connected constraint scopes or nonnormal heads make these
    // route fixtures decline it before reaching the complete rule traversal.
    crate::formula_factor::testing::scoped(false, || {
        let observer = Observer::default();
        let (compiled, visits) = route::scoped(enabled, || {
            if hybrid {
                let (compiled, constraints) =
                    ground_hybrid(testing::prepare(source), Some(&observer)).unwrap();
                assert!(constraints.is_some(), "streamed capture selected");
                compiled
            } else {
                ground(testing::prepare(source), Some(&observer), None).unwrap()
            }
        });
        (compiled, observer.0.get(), visits)
    })
    .0
}

fn same(left: &Compiled, right: &Compiled) {
    assert_eq!(
        compiled_atoms(left),
        compiled_atoms(right),
        "catalog order and identities"
    );
    assert_eq!(left.theory.nodes(), right.theory.nodes());
    assert_eq!(left.theory.operands(), right.theory.operands());
    assert_eq!(left.theory.roots(), right.theory.roots());
    assert_eq!(left.origins, right.origins);
    assert_eq!(left.objective_origins, right.objective_origins);
}

fn pair(source: &str, hybrid: bool) -> (GroundingWork, route::Visits) {
    let (shared, after, visits) = compile(source, true, hybrid);
    let (reference, before, skipped) = compile(source, false, hybrid);
    same(&shared, &reference);
    assert!(skipped.literals.is_empty());
    assert_eq!(before.row_atoms, Some(0));
    assert_eq!(after.atoms_inserted, before.atoms_inserted);
    assert_eq!(after.nodes_inserted, before.nodes_inserted);
    assert_eq!(
        before.atom_lookups.unwrap() - after.atom_lookups.unwrap(),
        after.row_atom_reuses.unwrap()
    );
    assert_eq!(
        before.node_lookups.unwrap() - after.node_lookups.unwrap(),
        after.row_atom_reuses.unwrap()
    );
    assert_eq!(
        usize::try_from(after.row_atoms.unwrap()).unwrap(),
        visits.literals.len()
    );
    (after, visits)
}

#[test]
fn matched_rows_name_the_atoms_the_general_route_names() {
    let (work, _) = pair(
        "p(1;2). q(1,1). q(1,2). q(2,1). q(2,2). :- p(X),q(X,Y).",
        false,
    );
    assert_eq!(work.row_atoms, Some(8));
    assert_eq!(work.row_atom_reuses, Some(2));
}

#[test]
fn each_reuse_replaces_one_catalog_and_one_node_lookup() {
    let (work, _) = pair("p(1;2). q(1;2;3). {h(X,Y)} :- p(X),q(Y).", false);
    assert_eq!(work.row_atoms, Some(12));
    assert!(work.row_atom_reuses.unwrap() > 0);
}

#[test]
fn a_negative_literal_is_named_as_before() {
    let (work, _) = pair("{p}. :- not p.", false);
    assert_eq!(work.row_atoms, Some(0));
    assert_eq!(work.row_atom_reuses, Some(0));
}

#[test]
fn a_structural_pattern_names_its_captured_atom() {
    let (work, _) = pair("p(f(1);f(2)). q(1;2). :- p(f(X)),q(Y),not r(X,Y).", false);
    assert!(work.row_atoms.unwrap() > 0);
}

#[test]
fn literals_joined_out_of_order_keep_their_own_rows() {
    let (work, visits) = pair("p(1,1). p(2,1). p(3,1). q(1). :- p(X,Y),q(Y).", false);
    assert_eq!(work.row_atoms, Some(6));
    assert!(
        visits
            .literals
            .iter()
            .any(|&(literal, depth, _)| literal != depth),
        "the inverse is exercised"
    );
}

#[test]
fn a_generated_row_names_its_deepest_literal_by_the_general_route() {
    let (work, visits) = pair("p(1;2). q(1;2). h(X,Y,Z) :- p(X),q(Y), Z=1..2.", false);
    assert_eq!(
        work.row_atoms,
        Some(8),
        "eight owned completions retain exactly one of two depths"
    );
    assert!(visits.literals.iter().all(|&(_, depth, _)| depth == 0));
}

#[test]
fn a_head_slot_row_names_its_deepest_literal_by_the_general_route() {
    let (work, visits) = pair("p(1;2). q(1;2). h(X,Y,1..2) :- p(X),q(Y).", false);
    assert_eq!(work.row_atoms, Some(8));
    assert!(visits.literals.iter().all(|&(_, depth, _)| depth == 0));
}

#[test]
fn a_generator_over_one_planned_literal_names_no_atom_by_its_row() {
    let (work, _) = pair("p(1;2). h(X,Y) :- p(X),Y=1..2.", false);
    assert_eq!(work.row_atoms, Some(0));
}

#[test]
fn a_streamed_constraint_retains_the_same_identities() {
    let (work, _) = pair("{p(1);p(2);q(1);q(2)}. :- p(X),q(Y),not r(X,Y).", true);
    assert_eq!(work.row_atoms, Some(8));
    assert!(work.row_atom_reuses.unwrap() > 0);
}

#[test]
fn cache_currency_survives_aggregate_suffixes_and_rule_changes() {
    let (work, _) = pair(
        "{p(1);p(2);q(1);q(2);r(1);r(2)}. \
        {h(X,Y)} :- p(X), #count{Z:r(Z)} >= 1, q(Y). \
        {k(X,Y)} :- q(Y), #count{Z:r(Z)} >= 2, p(X).",
        false,
    );
    assert!(work.row_atom_reuses.unwrap() > 0);
}

#[test]
fn the_producer_route_keeps_its_compiled_groups_unrecorded() {
    let source = "p(1;2). q(1;2). r(1;2). s(1;2). t(1;2). u(1;2). \
        h(A,B,C,D,E,F) :- p(A),q(B),r(C),s(D),t(E),u(F).";
    let ((work, visits), producer) =
        crate::formula_ground::producer::testing::scoped(true, || pair(source, false));
    assert!(producer.reused > 0, "compiled producer groups are reached");
    assert_eq!(work.row_atoms, Some(0));
    assert!(visits.literals.is_empty());
}

#[test]
fn two_predicate_variations_keep_exact_catalog_and_graph_order() {
    // Bounded cross-product of unequal populations, literal order, negative,
    // structural, and generated bodies. Every fixture asserts the row route.
    for first in ["p(X)", "p(f(X))"] {
        for reverse in [false, true] {
            for tail in ["", ",not r(X,Y)", ",Z=1..2"] {
                let facts = if first.contains('f') {
                    "p(f(1);f(2);f(3))."
                } else {
                    "p(1;2;3)."
                };
                let body = if reverse {
                    format!("q(Y),{first}")
                } else {
                    format!("{first},q(Y)")
                };
                let source = format!("{facts} q(1;2). :- {body},X<=Y+3{tail}.");
                let (work, _) = pair(&source, false);
                assert!(work.row_atoms.unwrap() > 0, "{source}");
            }
        }
    }
}

#[test]
fn disconnected_component_factoring_keeps_its_catalog_and_graph() {
    let (work, visits) = pair("p(1;2;3). q(1;2). :- p(X),q(Y).", false);
    assert_eq!(
        work.row_atoms,
        Some(0),
        "component factoring retains its own route"
    );
    assert!(visits.literals.is_empty());
}

#[test]
fn current_nodes_and_inverse_have_one_live_lease() {
    testing::with_completed_source(
        "p(1;2). q(1). :- p(X),q(Y).",
        |prepared, support, computation, counters| {
            let rule = prepared
                .rules_without_facts()
                .iter()
                .find(|rule| !rule.body.is_empty())
                .unwrap();
            let location = rule.location;
            let limits = FormulaLimits::default();
            let mut budget = testing::budget();
            let mut join =
                Join::rule(rule, support, computation, &limits, &mut budget, counters).unwrap();
            join.retain_witnesses(Context::new(computation, &limits, counters, location))
                .unwrap();
            let row = join
                .next_matched_row(
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            let mut builder = Builder::empty(
                computation,
                &limits,
                &mut budget,
                counters,
                Purpose::Theory,
                None,
                location,
            )
            .unwrap();
            let before = testing::live_bytes(support);
            let nodes = MatchedNodes::for_body(
                row.witnesses.as_ref().unwrap(),
                rule.body.len(),
                &mut builder,
                location,
            )
            .unwrap();
            let inverse = nodes.literal_depths.as_ref().expect("out-of-order mapping");
            let expected = size_of::<crate::formula_support::Buffer<Option<usize>>>()
                + nodes.nodes.capacity() * size_of::<Option<usize>>()
                + size_of::<crate::formula_support::Buffer<usize>>()
                + inverse.capacity() * size_of::<usize>();
            assert_eq!(testing::live_bytes(support) - before, expected);
            drop(nodes);
            assert_eq!(testing::live_bytes(support), before);
        },
    );
}

#[test]
fn node_cache_capacity_refuses_before_publication() {
    testing::with_completed_source("p(1).", |_, support, computation, counters| {
        let location = crate::test_support::location();
        let limits = FormulaLimits::default();
        let mut budget = testing::budget();
        let mut builder = Builder::empty(
            computation,
            &limits,
            &mut budget,
            counters,
            Purpose::Theory,
            None,
            location,
        )
        .unwrap();
        let before = testing::live_bytes(support);
        let restricted = FormulaLimits {
            max_support_bytes: before,
            ..limits
        };
        builder.limits = &restricted;
        assert!(matches!(
            MatchedNodes::new(2, &mut builder, location),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                ..
            })
        ));
        assert_eq!(testing::live_bytes(support), before);
    });
}

#[test]
fn reference_control_restores_on_unwind_and_stays_on_its_thread() {
    assert!(route::enabled());
    let result = std::panic::catch_unwind(|| {
        route::scoped(false, || {
            assert!(!route::enabled());
            assert!(std::thread::spawn(route::enabled).join().unwrap());
            panic!("restore reference route");
        })
    });
    assert!(result.is_err());
    assert!(route::enabled());
}
