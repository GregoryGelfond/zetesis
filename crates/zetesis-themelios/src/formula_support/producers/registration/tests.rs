//! A registration classifies each producer once, reads every frame a rich
//! producer's traversal reads, and skips only traversals that would derive
//! nothing new.

use std::cell::Cell;

use super::{Producer, Registration, testing};
use crate::formula::{Compiled, Preparation};
use crate::formula_support::facts::rounds;
use crate::formula_support::{Counters, testing::prepare};
use crate::grounding_observer::{GroundingOutcome, GroundingPhase, GroundingWork};
use crate::{FormulaLimits, GroundingObserver, ProgramSite};
use zetesis_core::{Predicate, ValueLimits};

/// The registration of every producer of `preparation`, through its catalog's
/// component view.
fn registered(preparation: &Preparation) -> Registration<'_> {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let view = preparation
        .catalog
        .component_view(&limits, &mut counters, preparation.location)
        .unwrap();
    Registration::prepare(
        preparation.program.rules_without_facts(),
        view,
        0,
        &limits,
        &mut counters,
        preparation.location,
    )
    .unwrap()
}

/// The kinds of `source`'s producers, in rule order.
fn kinds(source: &str) -> Vec<Producer> {
    registered(&prepare(source)).producers
}

/// The predicates the rich producer at `index` reads, as signed signatures.
fn reads(source: &str, index: usize) -> Vec<Predicate> {
    let preparation = prepare(source);
    let registration = registered(&preparation);
    let Producer::Rich(range) = registration.producers[index].clone() else {
        panic!("producer {index} is not rich");
    };
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let view = preparation
        .catalog
        .component_view(&limits, &mut counters, preparation.location)
        .unwrap()
        .unwrap();
    let mut predicates: Vec<_> = registration.reads[range]
        .iter()
        .map(|&read| {
            let predicate =
                super::predicate(read, view, &limits, &mut counters, preparation.location).unwrap();
            Predicate::with_sign(predicate.name(), predicate.arity(), predicate.sign()).unwrap()
        })
        .collect();
    predicates.sort();
    predicates
}

fn predicate(name: &str, arity: usize) -> Predicate {
    Predicate::new(name, arity).unwrap()
}

#[test]
fn a_flat_normal_rule_is_certified_with_its_positive_occurrences() {
    let preparation = prepare("q(1). t(1). r(X) :- q(X), not s(X), X > 0, t(X).");
    let registration = registered(&preparation);
    let rule = &preparation.program.rules_without_facts()[0];
    let positive: Vec<_> = (0..rule.body.len())
        .filter(|&occurrence| super::positive(&rule.body[occurrence]))
        .collect();
    assert_eq!(positive.len(), 2);
    assert_eq!(registration.producers, [Producer::Certified(0..2)]);
    assert_eq!(registration.inputs, positive);
}

#[test]
fn rich_bodies_and_heads_make_a_rule_rich() {
    for source in [
        "q(1). r(N) :- N = #count{X : q(X)}.",
        "q(1). r :- q(X) : q(X).",
        "q(1). {r(X)} :- q(X).",
        "q(1). r(X) ; s(X) :- q(X).",
        "q(1). r(X) : q(X) :- q(1).",
    ] {
        assert!(
            matches!(kinds(source)[..], [Producer::Rich(_)]),
            "{source}: {:?}",
            kinds(source)
        );
    }
}

#[test]
fn a_constraint_is_registered_as_a_constraint() {
    assert_eq!(kinds("q(1). :- q(X), X > 2."), [Producer::Constraint]);
}

#[test]
fn a_projection_makes_a_normal_producer_rich() {
    let preparation = prepare("d(1). p(1,2). q(X) :- d(X), not p(X,_).");
    let rules = preparation.program.rules_without_facts();
    assert!(
        rules[0]
            .body
            .iter()
            .any(|literal| matches!(literal, crate::formula_ir::LiteralIr::ProjectedAtom(..)))
    );
    assert!(matches!(
        registered(&preparation).producers[..],
        [Producer::Rich(_)]
    ));
}

#[test]
fn registration_rejects_a_foreign_rule_occurrence() {
    let source = "p(1). {q(X)} :- p(X).";
    let owner = prepare(source);
    let foreign = prepare(source);
    let registration = registered(&owner);
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let snapshot = owner
        .catalog
        .snapshot(&limits, &mut counters, owner.location)
        .unwrap();
    let support = super::Support::indexed(&snapshot, &limits, &counters, owner.location).unwrap();
    for (index, rule) in [
        (0, &foreign.program.rules_without_facts()[0]),
        (1, &owner.program.rules_without_facts()[0]),
    ] {
        let Err(error) = registration.variants(index, rule, &support, true, &limits, &mut counters)
        else {
            panic!("only the registered rule at its own position may use this registration");
        };
        assert!(matches!(error, crate::FormulaFailure::SupportRelation {
            error: zetesis_core::relation::Failure::Owner, location,
        } if location == rule.location));
    }
}

#[test]
fn registration_refuses_every_insufficient_work_prefix() {
    let owner = prepare("p(1). q(2). {r(X,Y)} :- p(X), q(Y), not p(Y).");
    let defaults = FormulaLimits::default();
    let view = owner
        .catalog
        .component_view(&defaults, &mut Counters::default(), owner.location)
        .unwrap();
    let run = |maximum| {
        let limits = FormulaLimits {
            max_work: maximum,
            ..defaults
        };
        let mut counters = Counters::default();
        let result = Registration::prepare(
            owner.program.rules_without_facts(),
            view,
            0,
            &limits,
            &mut counters,
            owner.location,
        );
        (result, counters.accounting.work)
    };
    let (complete, required) = run(defaults.max_work);
    assert!(complete.is_ok());
    assert!(required > 0);
    for maximum in 0..required {
        let (result, work) = run(maximum);
        assert!(matches!(result, Err(crate::FormulaFailure::Limit {
            resource: crate::FormulaResource::Work, observed, limit, ..
        }) if observed > limit && limit == u128::from(maximum)));
        assert!(work <= maximum);
    }
    assert!(run(required).0.is_ok());
}

#[test]
fn registration_admits_its_header_beside_existing_storage() {
    let owner = prepare("p(1). {q(X)} :- p(X).");
    let external = owner.catalog.bytes(owner.location).unwrap() as u128;
    let required = external + std::mem::size_of::<Registration<'_>>() as u128;
    let limits = FormulaLimits {
        max_support_bytes: usize::try_from(required - 1).unwrap(),
        ..FormulaLimits::default()
    };
    let mut counters = Counters::default();
    let result = Registration::prepare(
        owner.program.rules_without_facts(),
        None,
        external,
        &limits,
        &mut counters,
        owner.location,
    );
    assert!(matches!(result, Err(crate::FormulaFailure::Limit {
        resource: crate::FormulaResource::SupportBytes, observed, limit, ..
    }) if observed == required && limit == required - 1));
    assert_eq!(counters.accounting.work, 0);
}

#[test]
fn an_unchanged_visit_charges_each_registered_read() {
    let owner = prepare("p(1). q(2). {r(X)} :- p(X), not q(X).");
    let registration = registered(&owner);
    let Producer::Rich(range) = &registration.producers[0] else {
        panic!("the choice must be a rich producer");
    };
    assert_eq!(range.len(), 2);
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    // Before facts are published, both read relations are unchanged and empty.
    let snapshot = owner
        .catalog
        .snapshot(&limits, &mut counters, owner.location)
        .unwrap();
    let support = super::Support::indexed(&snapshot, &limits, &counters, owner.location).unwrap();
    let rule = &owner.program.rules_without_facts()[0];
    let before = counters.accounting.work;
    let mut variants = registration
        .variants(0, rule, &support, false, &limits, &mut counters)
        .unwrap();
    // One unit to test an entry and one to resolve its predicate.
    assert_eq!(counters.accounting.work - before, 4);
    assert!(variants.next(&limits, &mut counters).unwrap().is_none());
}

#[test]
fn a_rich_rule_reads_every_frame_of_either_sign() {
    // Body atoms of both signs, a choice element's condition, and an
    // aggregate element's condition, but not the derived heads.
    let source = "a(1). b(1). c(1). d(1). \
        {h(X) : c(X)} :- a(X), not b(X), N = #count{Y : d(Y)}, N > 0.";
    assert_eq!(
        reads(source, 0),
        [
            predicate("a", 1),
            predicate("b", 1),
            predicate("c", 1),
            predicate("d", 1)
        ]
    );
}

#[test]
fn distinct_predicates_sharing_a_name_stay_distinct_reads() {
    let source = "p(1). p(1,2). -p(3). {e(X,Y) : p(X,Y)} :- p(X), not -p(X).";
    let mut expected = vec![
        Predicate::with_sign("p", 1, zetesis_core::Sign::Negative).unwrap(),
        predicate("p", 1),
        predicate("p", 2),
    ];
    expected.sort();
    assert_eq!(reads(source, 0), expected);
}

#[test]
fn repeated_reads_of_one_predicate_are_one_entry() {
    let source = "q(1). {r(X,Y)} :- q(X), q(Y), not q(3).";
    assert_eq!(reads(source, 0), [predicate("q", 1)]);
}

/// The unchanged visits every phase reports, summed: support completion
/// reports once per completion it runs.
#[derive(Default)]
struct Unchanged(Cell<u64>);

impl GroundingObserver for Unchanged {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        _: GroundingPhase,
        _: Option<ProgramSite>,
        _: GroundingOutcome,
        work: GroundingWork,
    ) {
        self.0
            .set(self.0.get() + work.support_unchanged_visits.unwrap());
    }
}

/// `source` grounded eagerly: the result, the atoms each support round
/// published, in order, and the unchanged visits.
fn ground(source: &str) -> (Compiled, Vec<Vec<String>>, u64) {
    let observer = Unchanged::default();
    let (compiled, published) = rounds::capture(|| {
        crate::formula_ground::ground(prepare(source), Some(&observer), None).unwrap()
    });
    (compiled, published, observer.0.get())
}

/// Ground `source` with the registration's test and with the reference that
/// traverses every rich producer in every round. Each support round must
/// publish the same atoms in the same order, and the emitted theory must
/// agree; the test must have skipped at least one visit, and the reference
/// none. Emission alone could not show a missing traversal: it names a choice
/// head that support never derived. Returns the skipped visits.
fn equivalent(source: &str) -> u64 {
    let (actual, published, skipped) = ground(source);
    let (reference, expected, traversed) = testing::with_every_round_traversed(|| ground(source));
    assert_eq!(published, expected);
    let atoms = |compiled: &Compiled| {
        compiled
            .atoms
            .atoms()
            .iter()
            .map(|atom| atom.to_atom(ValueLimits::default()).unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(atoms(&actual), atoms(&reference));
    assert_eq!(actual.theory.nodes(), reference.theory.nodes());
    assert_eq!(actual.theory.operands(), reference.theory.operands());
    assert_eq!(actual.theory.roots(), reference.theory.roots());
    assert_eq!(actual.origins, reference.origins);
    assert_eq!(actual.objective_origins, reference.objective_origins);
    assert!(
        actual
            .objectives
            .templates()
            .iter()
            .eq(reference.objectives.templates().iter())
    );
    assert_eq!(actual.warnings, reference.warnings);
    assert_eq!(traversed, 0);
    assert!(skipped > 0, "the test never skipped a visit");
    skipped
}

/// `s` gains one row in each of four rounds.
const GROWING: &str = "s(0). s(Y) :- s(X), X < 3, Y = X + 1.";

#[test]
fn a_rich_producer_wakes_on_its_flat_body_atom() {
    equivalent(&format!("{GROWING} {{a(X)}} :- s(X)."));
}

#[test]
fn a_rich_producer_wakes_on_its_structural_body_occurrence() {
    equivalent("p(f(0)). p(f(Y)) :- p(f(X)), X < 3, Y = X + 1. {b(X)} :- p(f(X)).");
}

#[test]
fn a_rich_producer_wakes_on_its_assignment_element_condition() {
    equivalent(&format!("{GROWING} m(N) :- N = #count{{X : s(X)}}."));
}

#[test]
fn a_rich_producer_wakes_on_its_choice_element_condition() {
    equivalent(&format!("{GROWING} go. {{c(X) : s(X)}} :- go."));
}

#[test]
fn a_rich_producer_wakes_on_its_conditional_head_condition() {
    equivalent(&format!("{GROWING} go. d(X) : s(X) :- go."));
}

#[test]
fn a_rich_producer_wakes_on_a_predicate_sharing_another_reads_name() {
    // The body reads p/1, which never changes after the first round; only the
    // element condition's p/2 keeps growing.
    equivalent("p(0). p(0,0). p(0,Y) :- p(0,X), X < 3, Y = X + 1. {e(X,Y) : p(X,Y)} :- p(X).");
}

#[test]
fn a_rich_producer_over_facts_skips_every_round_after_the_second() {
    // The facts' rows are new only in the second round's snapshot; the choice
    // rule is unaffected from the third round while the recursion grows.
    let skipped = equivalent(&format!("q(1). q(2). {{a(X)}} :- q(X). {GROWING}"));
    assert!(skipped >= 3, "{skipped}");
}

/// A maintained scenario whose encoding its `#include` names, with the
/// encoding's text where that line stood.
fn scenario(instance: &str, include: &str, encoding: &str) -> String {
    assert!(instance.contains(include));
    instance.replace(include, encoding)
}

#[test]
fn nested_mastermind_skips_without_changing_support_or_theory() {
    equivalent(include_str!(
        "../../../../../../examples/scalability/mastermind-nested.lp"
    ));
}

#[test]
fn sudoku_skips_without_changing_support_or_theory() {
    equivalent(include_str!("../../../../../../examples/sudoku.lp"));
}

#[test]
fn n_queens_skips_without_changing_support_or_theory() {
    equivalent(include_str!(
        "../../../../../../examples/scalability/n-queens.lp"
    ));
}

#[test]
fn a_traveling_salesman_scenario_skips_without_changing_support_or_theory() {
    equivalent(&scenario(
        include_str!(
            "../../../../../../examples/correctness/scenarios/traveling-salesman/variant-05/05-three-depots-mixed.lp"
        ),
        "#include \"../../../encodings/traveling-salesman/variant-05.lp\".",
        include_str!(
            "../../../../../../examples/correctness/encodings/traveling-salesman/variant-05.lp"
        ),
    ));
}

#[test]
fn a_shortest_path_scenario_skips_without_changing_support_or_theory() {
    equivalent(&scenario(
        include_str!(
            "../../../../../../examples/correctness/scenarios/shortest-path/variant-04/06-layered-dag-ordering-cap.lp"
        ),
        "#include \"../../../encodings/shortest-path/variant-04.lp\".",
        include_str!(
            "../../../../../../examples/correctness/encodings/shortest-path/variant-04.lp"
        ),
    ));
}

#[test]
fn the_reference_restores_after_unwind() {
    assert!(!super::every_round_traversed());
    let result = std::panic::catch_unwind(|| {
        testing::with_every_round_traversed(|| panic!("test unwind"));
    });
    assert!(result.is_err());
    assert!(!super::every_round_traversed());
}

#[test]
fn the_reference_stays_on_its_thread() {
    testing::with_every_round_traversed(|| {
        assert!(super::every_round_traversed());
        assert!(
            !std::thread::spawn(super::every_round_traversed)
                .join()
                .unwrap()
        );
    });
}
