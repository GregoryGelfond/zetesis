//! Unit-connective consequences preserve original models and leave frozen closure alone.

use crate::{
    AdmissionLimits, EvaluationLimits, EvaluationWorkspace, FormulaNodes, FormulaParts,
    FrozenSubject, Interpretation, Knowledge, Narrower, Narrowing, NarrowingQuota,
    NarrowingScratch, NarrowingStatistics, Node, NodeView, OriginalSubject, Region, RegionLimits,
    Theory,
};
use zetesis_cpu::{Cancellation, Stop};

/// Atom 4 reports the endpoint truth. All contributing atoms can stay open.
fn witness(native_source: bool, positive: bool) -> Theory {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..5).map(Node::atom).collect(), vec![]).unwrap());
    let bound = if native_source {
        2
    } else if positive {
        1
    } else {
        3
    };
    let native = nodes
        .push_threshold_family(&[0, 1, 2], &[1, 1, 1], &[bound], usize::MAX, usize::MAX)
        .unwrap()[0];
    let operands = match (native_source, positive) {
        (true, true) => vec![3, 2, 0, 1],
        (true, false) => vec![1, 0],
        (false, _) => vec![2, 0, 1],
    };
    let mut append = nodes.transaction();
    let boolean = append
        .push(
            if positive {
                NodeView::Or(&operands)
            } else {
                NodeView::And(&operands)
            },
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    let (source, target) = if native_source {
        (native, boolean)
    } else {
        (boolean, native)
    };
    let falsum = append
        .push(NodeView::False, usize::MAX, usize::MAX)
        .unwrap();
    let requirement = if positive {
        source
    } else {
        append
            .push(NodeView::Implies(source, falsum), usize::MAX, usize::MAX)
            .unwrap()
    };
    let consequence = append
        .push(
            if positive {
                NodeView::Implies(target, 4)
            } else {
                NodeView::Implies(4, target)
            },
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    append.commit();
    Theory::new(
        5,
        nodes.into_parts(),
        vec![requirement, consequence],
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn close(
    theory: &Theory,
    index: &Narrower,
    region: &mut Region,
    knowledge: &mut Knowledge,
    scratch: &mut NarrowingScratch,
) -> (Narrowing, NarrowingStatistics) {
    index
        .narrow_known(
            OriginalSubject::new(theory, None),
            region,
            knowledge,
            scratch,
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap()
}

#[test]
fn original_bounds_cross_the_connective_boundary() {
    for native_source in [false, true] {
        for positive in [false, true] {
            let theory = witness(native_source, positive);
            let index = Narrower::new(&theory);
            assert!(index.thresholds.connectives.is_some());
            let mut without = index.clone();
            without.thresholds.connectives = None;
            let mut baseline = Region::all_open(5);
            close(
                &theory,
                &without,
                &mut baseline,
                &mut without.knowledge(),
                &mut NarrowingScratch::default(),
            );
            assert!(baseline.is_open(4), "the witness requires the bridge");
            let mut region = Region::all_open(5);
            let mut scratch = NarrowingScratch::default();
            let result = close(
                &theory,
                &index,
                &mut region,
                &mut index.knowledge(),
                &mut scratch,
            );
            assert_eq!(result.0, Narrowing::Fixed { changed: true });
            assert!(!region.is_open(4));
            assert_eq!(region.is_held(4), positive);
            assert!((0..4).all(|atom| region.is_open(atom)));
            assert!(scratch.threshold_rows.is_empty());
            assert!(scratch.pending_thresholds.iter().all(|&word| word == 0));
            assert_eq!(
                index.knowledge().retained_bytes(),
                without.knowledge().retained_bytes()
            );
        }
    }
}

fn region(mut code: usize) -> Region {
    let mut region = Region::all_open(5);
    for atom in 0..5 {
        match code % 3 {
            1 => assert!(region.hold(atom)),
            2 => assert!(region.cut(atom)),
            _ => {}
        }
        code /= 3;
    }
    region
}

fn agrees(region: &Region, model: usize) -> bool {
    (0..5).all(|atom| {
        if model & (1 << atom) == 0 {
            !region.is_held(atom)
        } else {
            !region.is_cut(atom)
        }
    })
}

fn interpretation(theory: &Theory, mask: usize) -> Interpretation {
    Interpretation::new(theory, (0..5).filter(|atom| mask & (1 << atom) != 0)).unwrap()
}

#[test]
fn original_narrowing_keeps_every_compatible_model() {
    for native_source in [false, true] {
        for positive in [false, true] {
            let theory = witness(native_source, positive);
            let index = Narrower::new(&theory);
            let mut evaluation = EvaluationWorkspace::default();
            let models: Vec<_> = (0..32)
                .filter(|&mask| {
                    evaluation
                        .evaluate(
                            &interpretation(&theory, mask),
                            EvaluationLimits::default(),
                            &Cancellation::default(),
                        )
                        .result
                        .unwrap()
                        .is_model()
                })
                .collect();
            assert!(!models.is_empty());
            for code in 0..243 {
                let before = region(code);
                let mut after = before.clone();
                let result = close(
                    &theory,
                    &index,
                    &mut after,
                    &mut index.knowledge(),
                    &mut NarrowingScratch::default(),
                );
                for &model in &models {
                    if agrees(&before, model) {
                        assert_ne!(result.0, Narrowing::Refuted);
                        assert!(agrees(&after, model));
                    }
                }
            }
        }
    }
}

fn receipt(statistics: NarrowingStatistics) -> [u64; 4] {
    [
        statistics.work,
        statistics.propagations,
        statistics.held,
        statistics.cut,
    ]
}

#[test]
fn frozen_closure_ignores_connective_links() {
    for native_source in [false, true] {
        for positive in [false, true] {
            let theory = witness(native_source, positive);
            let index = Narrower::new(&theory);
            let mut without = index.clone();
            without.thresholds.connectives = None;
            let mut evaluation = EvaluationWorkspace::default();
            for candidate in 0..32 {
                let outer = interpretation(&theory, candidate);
                let evaluated = evaluation
                    .evaluate(
                        &outer,
                        EvaluationLimits::default(),
                        &Cancellation::default(),
                    )
                    .result
                    .unwrap();
                let truth: Vec<_> = (0..theory.nodes().len())
                    .map(|node| evaluated.node_truth(node).unwrap())
                    .collect();
                // These regions also contain interpretations not below the frozen candidate.
                for code in 0..243 {
                    let mut actual_region = region(code);
                    let mut expected_region = actual_region.clone();
                    let mut scratch = NarrowingScratch::default();
                    let run =
                        |index: &Narrower, region: &mut Region, scratch: &mut NarrowingScratch| {
                            index
                                .narrow_frozen_known(
                                    FrozenSubject::new(&theory, &truth),
                                    region,
                                    &mut index.knowledge(),
                                    scratch,
                                    RegionLimits::default(),
                                    &Cancellation::default(),
                                )
                                .unwrap()
                        };
                    let actual = run(&index, &mut actual_region, &mut scratch);
                    let expected = run(
                        &without,
                        &mut expected_region,
                        &mut NarrowingScratch::default(),
                    );
                    assert_eq!(actual.0, expected.0);
                    assert_eq!(actual_region, expected_region);
                    assert_eq!(receipt(actual.1), receipt(expected.1));
                    assert_eq!(scratch.threshold_rows.capacity(), 0);
                    assert_eq!(scratch.pending_thresholds.capacity(), 0);
                }
            }
        }
    }
}

struct Allowance {
    remaining: u64,
    failure: Stop,
}
impl NarrowingQuota for Allowance {
    fn reserve(&mut self, wanted: u64) -> Result<u64, Stop> {
        let granted = wanted.min(self.remaining);
        if granted == 0 {
            return Err(self.failure);
        }
        self.remaining -= granted;
        Ok(granted)
    }
    fn refund(&mut self, unspent: u64) {
        self.remaining += unspent;
    }
}

#[test]
fn interrupted_bridge_work_does_not_taint_reuse() {
    let theory = witness(true, false);
    let index = Narrower::new(&theory);
    let mut expected_region = Region::all_open(5);
    let expected = close(
        &theory,
        &index,
        &mut expected_region,
        &mut index.knowledge(),
        &mut NarrowingScratch::default(),
    );
    let mut scratch = NarrowingScratch::default();
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for remaining in 0..expected.1.work {
            let attempt = index.narrow_known_reserved(
                OriginalSubject::new(&theory, None),
                &mut Region::all_open(5),
                &mut index.knowledge(),
                &mut scratch,
                &Cancellation::default(),
                &mut Allowance { remaining, failure },
            );
            assert_eq!(attempt.result, Err(failure));
            assert_eq!(attempt.statistics.work, remaining);
            let mut region = Region::all_open(5);
            let actual = close(
                &theory,
                &index,
                &mut region,
                &mut index.knowledge(),
                &mut scratch,
            );
            assert_eq!(region, expected_region);
            assert_eq!(actual.0, expected.0);
            assert_eq!(receipt(actual.1), receipt(expected.1));
        }
    }
}

/// A standalone Boolean root and one numeric descriptor share an atom owner.
fn prepared(conditions: &[usize], weights: &[u64], leaves: &[usize]) -> Narrower {
    let mut nodes = FormulaNodes::new(
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::atom(2),
                Node::atom(3),
                Node::atom(0),
            ],
            vec![],
        )
        .unwrap(),
    );
    nodes
        .push_threshold_family(conditions, weights, &[2], usize::MAX, usize::MAX)
        .unwrap();
    let mut append = nodes.transaction();
    append
        .push(NodeView::And(leaves), usize::MAX, usize::MAX)
        .unwrap();
    append.commit();
    Narrower::new(&Theory::new(4, nodes.into_parts(), vec![], AdmissionLimits::default()).unwrap())
}

#[test]
fn discovery_authenticates_complete_unit_prefixes() {
    type RelationCase<'a> = (&'a [usize], &'a [u64], &'a [usize], bool);
    let cases: &[RelationCase<'_>] = &[
        (&[0, 1, 2], &[1, 1, 1], &[1, 0], true),
        (&[0, 1], &[1, 1], &[3, 1, 0, 2], true),
        (&[0, 1, 2], &[1, 1, 1], &[2, 0, 1], true),
        // Repeating a Boolean operand does not create a second contribution.
        (&[0, 1, 2], &[1, 1, 1], &[1, 0, 1], true),
        // A shared anchor does not authenticate a non-prefix subset or overlap.
        (&[0, 1, 2], &[1, 1, 1], &[0, 2], false),
        (&[0, 1, 2], &[1, 1, 1], &[0, 1, 3], false),
        // Grouped numeric multiplicity prevents a unit-row match.
        (&[0, 0, 1], &[1, 1, 1], &[0, 1], false),
        (&[0, 1], &[2, 1], &[0, 1], false),
        // Node 4 denotes atom 0 but is not condition node 0.
        (&[0, 1], &[1, 1], &[4, 1], false),
        (&[], &[], &[0, 1], false),
        (&[0, 1], &[1, 1], &[], false),
    ];
    for &(conditions, weights, leaves, expected) in cases {
        let index = prepared(conditions, weights, leaves);
        let matches = index.thresholds.rows.iter().any(|row| {
            super::unit_first(row).is_some()
                && index.chains.iter().any(|chain| {
                    super::exact_prefix(
                        &row.terms,
                        chain.ordinary_leaves(),
                        &mut crate::regions::Work::new(u64::MAX),
                    )
                    .unwrap()
                })
        });
        assert_eq!(
            matches, expected,
            "conditions={conditions:?}, weights={weights:?}, leaves={leaves:?}"
        );
        for row in &index.thresholds.rows {
            let Some(first) = super::unit_first(row) else {
                continue;
            };
            for chain in &index.chains {
                if super::exact_prefix(
                    &row.terms,
                    chain.ordinary_leaves(),
                    &mut crate::regions::Work::new(u64::MAX),
                )
                .unwrap()
                {
                    assert!(index.thresholds.connectives.is_some());
                    assert_eq!(chain.ordinary_leaves().iter().min(), Some(&first));
                    assert_eq!(
                        index.parents[first]
                            .iter()
                            .filter(|&&root| root == chain.root)
                            .count(),
                        1
                    );
                }
            }
        }
    }
}
