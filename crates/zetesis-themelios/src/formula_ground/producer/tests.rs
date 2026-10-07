//! Original and frozen truth compare semantic atoms across different DAG shapes.

use super::testing::{self as route, Counts, Point};
use crate::formula::Compiled;
use crate::formula_ground::{Builder, Purpose, ground, ground_retained};
use crate::formula_support::testing;
use crate::{ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource, ProgramSite};
use zetesis_core::{Atom, Value, ValueLimits};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{Interpretation, Limits, NodeView, models, models_reduct};
use zetesis_test_support::programs::atom;

const WIDE: &str = "{a(1);b(1);c(1);d(1);e(1);f(1)}. \
    h(X) :- a(X),b(X),c(X),d(X),e(X),f(X).";

const SEMANTIC_CASES: &[&str] = &[
    WIDE,
    "{p(1,2);p(1,3);q(2,4);q(3,5)}. h(X,Y,Z) :- p(X,Y),q(Y,Z).",
    "{p(1);q(1);r(1)}. h(X) :- p(X),q(X). \
        h(X) :- p(X),r(X). h(X) :- p(X),q(X).",
    "{p(1);q(1)}. h(X,X) :- p(X),p(X),q(X).",
    "{-p(1);q(1)}. -h(X,7,X) :- -p(X),q(X).",
];

fn atoms(compiled: &Compiled) -> Vec<Atom> {
    compiled
        .atoms
        .atoms()
        .iter()
        .map(|atom| atom.to_atom(ValueLimits::default()).unwrap())
        .collect()
}

struct Pair {
    shared: Compiled,
    reference: Compiled,
    shared_coordinates: Vec<usize>,
    reference_coordinates: Vec<usize>,
    population: usize,
}

impl Pair {
    fn compile(source: &str) -> Self {
        let (shared, counts) = route::scoped(true, || {
            ground(testing::prepare(source), None, None).unwrap()
        });
        let (reference, skipped) = route::scoped(false, || {
            ground(testing::prepare(source), None, None).unwrap()
        });
        assert!(counts.runs > 0, "producer route: {source}");
        assert!(counts.published > 0, "selected complete witness: {source}");
        assert_eq!(skipped, Counts::default());
        let result = Self::from_compiled(shared, reference, source);
        assert!(
            result.population <= 8,
            "bounded exhaustive truth table: {source}"
        );
        result
    }

    fn from_compiled(shared: Compiled, reference: Compiled, source: &str) -> Self {
        let shared_atoms = atoms(&shared);
        let reference_atoms = atoms(&reference);
        let mut vocabulary = shared_atoms.clone();
        vocabulary.sort();
        let mut other = reference_atoms.clone();
        other.sort();
        assert_eq!(vocabulary, other, "complete vocabulary: {source}");
        assert!(
            vocabulary.len() < usize::BITS as usize,
            "bounded sampled masks: {source}"
        );
        let coordinates = |values: &[Atom]| {
            values
                .iter()
                .map(|atom| vocabulary.binary_search(atom).unwrap())
                .collect()
        };
        Self {
            shared_coordinates: coordinates(&shared_atoms),
            reference_coordinates: coordinates(&reference_atoms),
            population: vocabulary.len(),
            shared,
            reference,
        }
    }

    /// These larger hit-path fixtures use an explicit finite sample, not a
    /// claim of exhaustive truth tables. Empty/full masks ensure tested J can
    /// contain atoms outside M; singleton/complement/mixed masks vary every atom.
    fn sampled_interpretations(&self) -> Vec<(Interpretation, Interpretation)> {
        let all = (1_usize << self.population) - 1;
        let mut masks = vec![0, all];
        for index in 0..self.population {
            masks.push(1 << index);
            masks.push(all ^ (1 << index));
        }
        let mut mixed = 0x5a5a_5a5a_usize;
        for _ in 0..16 {
            masks.push(mixed & all);
            mixed = mixed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        }
        masks.sort_unstable();
        masks.dedup();
        masks
            .into_iter()
            .map(|bits| {
                let interpretation = |compiled: &Compiled, coordinates: &[usize]| {
                    Interpretation::new(
                        &compiled.theory,
                        coordinates
                            .iter()
                            .enumerate()
                            .filter_map(|(local, semantic)| {
                                (bits & (1 << *semantic) != 0).then_some(local)
                            }),
                    )
                    .unwrap()
                };
                (
                    interpretation(&self.shared, &self.shared_coordinates),
                    interpretation(&self.reference, &self.reference_coordinates),
                )
            })
            .collect()
    }

    fn interpretations(&self) -> Vec<(Interpretation, Interpretation)> {
        (0..1 << self.population)
            .map(|bits| {
                let interpretation = |compiled: &Compiled, coordinates: &[usize]| {
                    Interpretation::new(
                        &compiled.theory,
                        coordinates
                            .iter()
                            .enumerate()
                            .filter_map(|(local, semantic)| {
                                (bits & (1 << *semantic) != 0).then_some(local)
                            }),
                    )
                    .unwrap()
                };
                (
                    interpretation(&self.shared, &self.shared_coordinates),
                    interpretation(&self.reference, &self.reference_coordinates),
                )
            })
            .collect()
    }
}

#[test]
fn shared_bodies_preserve_original_truth() {
    let cancellation = Cancellation::default();
    for source in SEMANTIC_CASES {
        let pair = Pair::compile(source);
        for (candidate, (shared, reference)) in pair.interpretations().iter().enumerate() {
            assert_eq!(
                models(
                    &pair.shared.theory,
                    shared,
                    Limits::default(),
                    &cancellation
                )
                .unwrap(),
                models(
                    &pair.reference.theory,
                    reference,
                    Limits::default(),
                    &cancellation
                )
                .unwrap(),
                "M={candidate}: {source}"
            );
        }
    }
}

#[test]
fn shared_bodies_preserve_arbitrary_frozen_truth() {
    let cancellation = Cancellation::default();
    for source in SEMANTIC_CASES {
        let pair = Pair::compile(source);
        let interpretations = pair.interpretations();
        let mut outside = 0;
        for (candidate, (shared, reference)) in interpretations.iter().enumerate() {
            for (tested, (shared_tested, reference_tested)) in interpretations.iter().enumerate() {
                outside += usize::from(tested & !candidate != 0);
                assert_eq!(
                    models_reduct(
                        &pair.shared.theory,
                        shared,
                        shared_tested,
                        Limits::default(),
                        &cancellation
                    )
                    .unwrap(),
                    models_reduct(
                        &pair.reference.theory,
                        reference,
                        reference_tested,
                        Limits::default(),
                        &cancellation
                    )
                    .unwrap(),
                    "M={candidate}, J={tested}: {source}"
                );
            }
        }
        assert!(
            outside > 0,
            "tested interpretations are not restricted to subsets"
        );
    }
}

#[test]
fn wide_bodies_use_native_shared_groups() {
    let pair = Pair::compile(WIDE);
    assert!((0..pair.shared.theory.nodes().len()).any(|index| {
        matches!(pair.shared.theory.view().node(index).unwrap(), NodeView::And(row) if row.len() == 3)
    }));
}

struct Fragment {
    atoms: Vec<Atom>,
    origins: Vec<Vec<ProgramSite>>,
    source_origins: Vec<ProgramSite>,
    nodes: usize,
    roots: usize,
}

/// Complete support normally, then emit only this fixture's non-fact rules.
/// Support rows remain possible inputs; none is asserted as a formula fact here.
fn fragment(source: &str) -> (Fragment, Counts) {
    route::scoped(true, || {
        let mut result = None;
        testing::with_completed_source(source, |prepared, support, computation, counters| {
            let limits = FormulaLimits::default();
            let mut budget = crate::expansion::Budget::new(ExpansionLimits::default(), usize::MAX);
            let location = prepared.rules.last().unwrap().location;
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
            builder.initialize(location).unwrap();
            let mut source_origins = Vec::new();
            for (index, rule) in prepared.rules.iter().enumerate() {
                if !rule.body.is_empty() {
                    source_origins.extend_from_slice(&rule.origins);
                    builder
                        .instantiate_rule(rule, index, None, support)
                        .unwrap();
                }
            }
            source_origins.sort();
            source_origins.dedup();
            let mut atoms = Vec::new();
            let mut origins = Vec::new();
            for index in 0..builder.catalog.len() {
                atoms.push(
                    builder
                        .catalog
                        .get(
                            index,
                            builder.computation,
                            &mut builder.counters,
                            &limits,
                            location,
                        )
                        .unwrap()
                        .to_atom(ValueLimits::default())
                        .unwrap(),
                );
                origins.push(builder.metadata.origins(index).collect());
            }
            result = Some(Fragment {
                atoms,
                origins,
                source_origins,
                nodes: builder.nodes.view().len(),
                roots: builder.roots.len(),
            });
        });
        result.unwrap()
    })
}

#[test]
fn dead_families_publish_no_formula_vocabulary() {
    let (fragment, counts) = fragment("p(1). q(2). h(X) :- p(X),q(X).");
    assert_eq!(counts.runs, 1);
    assert_eq!(counts.witnesses, 0);
    assert_eq!(counts.atoms, 0);
    assert_eq!(counts.groups, 0);
    assert_eq!(counts.published, 0);
    assert!(fragment.atoms.is_empty());
    assert_eq!(fragment.nodes, 2, "only the initialized Boolean constants");
    assert_eq!(fragment.roots, 0);
}

#[test]
fn unused_prefixes_do_not_enter_formula_vocabulary() {
    let (mut fragment, counts) = fragment("p(1;2). q(2;3;4). h(X) :- p(X),q(X).");
    assert_eq!(counts.runs, 1);
    assert_eq!(counts.witnesses, 1);
    assert_eq!(counts.atoms, 2);
    fragment.atoms.sort();
    let mut expected = ["p", "q", "h"]
        .map(|name| atom(name, vec![Value::Number(2)]))
        .to_vec();
    expected.sort();
    assert_eq!(fragment.atoms, expected);
}

#[test]
fn shared_heads_keep_every_source_origin() {
    let (fragment, counts) = fragment("p(1). q(1). r(1). h(X) :- p(X),q(X). h(X) :- p(X),r(X).");
    assert_eq!(counts.runs, 2);
    assert_eq!(counts.published, 2);
    let head = atom("h", vec![Value::Number(1)]);
    let index = fragment
        .atoms
        .iter()
        .position(|atom| *atom == head)
        .unwrap();
    assert_eq!(fragment.origins[index], fragment.source_origins);
    assert_eq!(fragment.source_origins.len(), 2);
}

#[test]
fn shared_family_work_limit_is_inclusive() {
    let source = "p(1;2). q(1;2). h(X,Y) :- p(X),q(Y).";
    let (completed, counts) = route::scoped(true, || {
        ground_retained(testing::prepare(source), None, false).unwrap()
    });
    assert_eq!(counts.runs, 1);
    assert_eq!(counts.published, 4);
    let exact = completed.accounting.work;
    assert!(exact > 0);
    for allowance in [exact - 1, exact] {
        let mut preparation = testing::prepare(source);
        preparation.limits.max_work = allowance;
        let (result, _) = route::scoped(true, || ground_retained(preparation, None, false));
        if allowance == exact {
            assert_eq!(result.unwrap().accounting.work, exact);
        } else {
            assert!(matches!(result, Err(FormulaFailure::Limit {
                resource: FormulaResource::Work, observed, limit, ..
            }) if limit == u128::from(allowance) && observed > limit));
        }
    }
}

#[test]
fn interrupted_families_publish_no_theory() {
    for point in [
        Point::Run,
        Point::Witness,
        Point::Atom,
        Point::Group,
        Point::Publish,
    ] {
        let cancellation = Cancellation::default();
        let mut preparation = testing::prepare(WIDE);
        preparation.budget = preparation
            .budget
            .with_cancellation(Some(cancellation.clone()));
        let (result, counts) = route::controlled(true, Some((point, 1, cancellation)), || {
            ground(preparation, None, None)
        });
        assert_eq!(counts.runs, 1, "{point:?}");
        assert!(
            matches!(
                result,
                Err(FormulaFailure::Interrupted {
                    reason: Stop::Cancelled,
                    ..
                })
            ),
            "{point:?}"
        );
    }
}

const REPEATED_ROWS: &str = "{p(1);p(2);q(1);q(2);r(1);r(2);s(1);s(2);t(1);t(2);u(1);u(2)}. \
    h(X,Y) :- p(X),q(X),r(X),s(Y),t(Y),u(Y).";

fn sharing_pair(source: &str) -> Pair {
    let compile = |reuse| {
        route::sharing(reuse, || {
            route::scoped(true, || {
                ground(testing::prepare(source), None, None).unwrap()
            })
        })
    };
    let (shared, counts) = compile(true);
    let (reference, old) = compile(false);
    assert!(counts.reused > 0, "reached reuse: {source}");
    assert_eq!(old.reused, 0);
    assert_eq!(counts.published, old.published);
    Pair::from_compiled(shared, reference, source)
}

#[test]
fn reached_subformulas_preserve_sampled_truth() {
    let sources = [
        REPEATED_ROWS,
        // Nine distinct predicates survive source normalization. The two
        // non-prefix leaf groups have equal local positions but different atoms.
        "{p(1);p(2);q(1);q(2);r(1);r(2);s(1);s(2);t(1);t(2);u(1);u(2);v(1);v(2);w(1);w(2);z(1);z(2)}. \
         h(X,Y) :- p(X),q(X),r(X),s(Y),t(Y),u(Y),v(Y),w(Y),z(Y).",
        // Equal local positions in different rule owners cannot identify an
        // existing group's operands. Both permutations retain every producer.
        "{p(1);p(2);q(1);q(2);r(1);r(2);s(1);s(2);t(1);t(2);u(1);u(2)}. \
         h(X,Y) :- p(X),q(X),r(X),s(Y),t(Y),u(Y). \
         h(X,Y) :- s(X),t(X),u(X),p(Y),q(Y),r(Y).",
    ];
    let cancellation = Cancellation::default();
    for source in sources {
        let pair = sharing_pair(source);
        let interpretations = pair.sampled_interpretations();
        for (shared, reference) in &interpretations {
            assert_eq!(
                models(
                    &pair.shared.theory,
                    shared,
                    Limits::default(),
                    &cancellation
                )
                .unwrap(),
                models(
                    &pair.reference.theory,
                    reference,
                    Limits::default(),
                    &cancellation
                )
                .unwrap(),
                "original: {source}",
            );
            for (shared_tested, reference_tested) in &interpretations {
                assert_eq!(
                    models_reduct(
                        &pair.shared.theory,
                        shared,
                        shared_tested,
                        Limits::default(),
                        &cancellation
                    )
                    .unwrap(),
                    models_reduct(
                        &pair.reference.theory,
                        reference,
                        reference_tested,
                        Limits::default(),
                        &cancellation
                    )
                    .unwrap(),
                    "sampled arbitrary frozen pair: {source}",
                );
            }
        }
    }
}

#[derive(Default)]
struct SharingPeak(std::cell::Cell<u64>);
impl crate::GroundingObserver for SharingPeak {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        _: crate::GroundingPhase,
        _: Option<ProgramSite>,
        _: crate::GroundingOutcome,
        work: crate::GroundingWork,
    ) {
        self.0
            .set(self.0.get().max(work.support_peak_bytes.unwrap()));
    }
}

fn sharing_receipt(source: &str, reuse: bool) -> (u64, u64, Counts) {
    let peak = SharingPeak::default();
    let (result, counts) = route::sharing(reuse, || {
        route::scoped(true, || {
            ground_retained(testing::prepare(source), Some(&peak), false).unwrap()
        })
    });
    (result.accounting.work, peak.0.get(), counts)
}

#[test]
fn reached_subformulas_reduce_repeated_product_work() {
    let product = "p(1;2). q(1;2). r(1;2). s(1;2). t(1;2). u(1;2). \
        h(A,B,C,D,E,F) :- p(A),q(B),r(C),s(D),t(E),u(F).";
    let before = sharing_receipt(product, false);
    let after = sharing_receipt(product, true);
    eprintln!("shared-product before={before:?} after={after:?}");
    assert!(after.2.reused > 0);
    assert_eq!(before.2.published, after.2.published);
    assert!(
        after.0 < before.0,
        "entire admitted work, including map shifts"
    );
    assert!(after.2.atoms < before.2.atoms);
}

#[test]
fn unrepeated_subformula_work_is_reported() {
    // The first case has no eligible group. The following live cases have six
    // distinct predicates and genuinely prepare a non-prefix leaf map, but each
    // complete row has a new key. The last case has no selected descendant.
    for (source, eligible) in [
        ("p(1;2). q(1;2). h(X,Y) :- p(X),q(Y).", false),
        (
            "p(1). q(1). r(1). s(1). t(1). u(1). h(X,Y) :- p(X),q(X),r(X),s(Y),t(Y),u(Y).",
            true,
        ),
        (
            "p(1;2). q(1;2). r(1;2). s(1;2). t(1;2). u(1;2). h(X) :- p(X),q(X),r(X),s(X),t(X),u(X).",
            true,
        ),
        (
            "p(1). q(1). r(1). s(2). t(2). u(2). h(X) :- p(X),q(X),r(X),s(X),t(X),u(X).",
            false,
        ),
    ] {
        let before = sharing_receipt(source, false);
        let after = sharing_receipt(source, true);
        eprintln!("unrepeated source={source:?} before={before:?} after={after:?}");
        assert_eq!(after.2.reused, 0);
        assert_eq!(after.2.lookups > 0, eligible, "actual map route: {source}");
        assert_eq!(before.2.published, after.2.published);
    }
}

#[test]
fn reused_subformulas_observe_cancellation() {
    for (point, after) in [(Point::Lookup, 1), (Point::Reuse, 1), (Point::Group, 1)] {
        let cancellation = Cancellation::default();
        let mut preparation = testing::prepare(REPEATED_ROWS);
        preparation.budget = preparation
            .budget
            .with_cancellation(Some(cancellation.clone()));
        let (result, counts) = route::controlled(true, Some((point, after, cancellation)), || {
            ground(preparation, None, None)
        });
        assert!(
            matches!(
                result,
                Err(FormulaFailure::Interrupted {
                    reason: Stop::Cancelled,
                    ..
                })
            ),
            "{point:?}: {counts:?}"
        );
    }
}

#[test]
fn reached_subformula_work_bound_is_inclusive() {
    let (exact, _, counts) = sharing_receipt(REPEATED_ROWS, true);
    assert!(counts.reused > 0);
    for allowance in [exact - 1, exact] {
        let mut preparation = testing::prepare(REPEATED_ROWS);
        preparation.limits.max_work = allowance;
        let result = ground_retained(preparation, None, false);
        if allowance == exact {
            assert_eq!(result.unwrap().accounting.work, exact);
        } else {
            assert!(matches!(
                result,
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    ..
                })
            ));
        }
    }
}
