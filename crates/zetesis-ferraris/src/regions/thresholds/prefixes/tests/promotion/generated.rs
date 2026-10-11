//! Small generated prefix forests compare pending schedules with fresh closure.

use proptest::prelude::*;
use zetesis_cpu::Cancellation;

use crate::{Interpretation, Limits, models};

use super::{
    AdmissionLimits, FormulaNodes, FormulaParts, Knowledge, Narrower, Narrowing, NarrowingScratch,
    Node, NodeView, Region, Theory, Width, Work, assert_same_choice, finish_pending, narrow,
};

#[derive(Debug)]
struct Case {
    weights: [u64; 3],
    selected: u8,
    decisions: u8,
    optional: u8,
    guards: [(u8, u8, u8); 6],
    requirements: u8,
    reversed: bool,
}

impl Case {
    /// Canonical rows form a branching forest, with optional equal grouped rows.
    /// Input order and duplicate occurrences need not be canonical themselves.
    fn rows(&self) -> Vec<(usize, Vec<(usize, u64)>)> {
        let mut weights = self.weights;
        weights.sort_unstable_by(|left, right| right.cmp(left));
        let core: Vec<_> = weights.into_iter().enumerate().collect();
        let extended = [core.as_slice(), &[(3, 1), (4, 1)]].concat();
        let middle = [core.as_slice(), &[(3, 1)]].concat();
        let branch = [core.as_slice(), &[(4, 1)]].concat();
        let equal = [&[(0, weights[0] - 1), (0, 1)][..], &core[1..], &[(4, 0)]].concat();
        let short = core[..2].to_vec();
        [core, extended, middle, branch, equal, short]
            .into_iter()
            .enumerate()
            .filter(|&(kind, _)| kind < 2 || self.optional & (1 << (kind - 2)) != 0)
            .map(|(kind, mut terms)| {
                if self.reversed {
                    terms.reverse();
                }
                (kind, terms)
            })
            .collect()
    }

    /// Every required interval contains the planted complete interpretation.
    /// Thus partial decisions drawn from it cannot make the seed vacuously fail.
    fn theory(&self) -> Theory {
        let mut nodes =
            FormulaNodes::new(FormulaParts::new((0..6).map(Node::atom).collect(), vec![]).unwrap());
        let mut families = Vec::new();
        for (kind, terms) in self.rows() {
            let total: u64 = terms.iter().map(|&(_, weight)| weight).sum();
            let selected: u64 = terms
                .iter()
                .filter(|&&(atom, _)| self.selected & (1 << atom) != 0)
                .map(|&(_, weight)| weight)
                .sum();
            let (lower_slack, upper_slack, probe) = self.guards[kind];
            let bounds = [
                i128::from(selected.saturating_sub(u64::from(lower_slack) % (total + 1))),
                i128::from((selected + 1 + u64::from(upper_slack) % (total + 1)).min(total + 1)),
                i128::from(u64::from(probe) % (total + 2)),
            ];
            let (conditions, weights): (Vec<_>, Vec<_>) = terms.into_iter().unzip();
            let guards = nodes
                .push_threshold_family(&conditions, &weights, &bounds, usize::MAX, usize::MAX)
                .unwrap();
            families.push((kind, guards));
        }
        let mut append = nodes.transaction();
        let falsum = append
            .push(NodeView::False, usize::MAX, usize::MAX)
            .unwrap();
        let mut roots = Vec::new();
        for (kind, guards) in families {
            if kind == 0 || self.requirements & (1 << kind) != 0 {
                roots.push(guards[0]);
            }
            if kind == 1 || self.requirements.rotate_left(1) & (1 << kind) != 0 {
                roots.push(
                    append
                        .push(NodeView::Implies(guards[1], falsum), usize::MAX, usize::MAX)
                        .unwrap(),
                );
            }
            // A guard's newly established truth can decide observer atom 5;
            // its threshold occurrence also participates in ordinary split scores.
            roots.push(
                append
                    .push(NodeView::Implies(guards[2], 5), usize::MAX, usize::MAX)
                    .unwrap(),
            );
        }
        append.commit();
        Theory::new(6, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap()
    }

    fn region(&self) -> Region {
        let mut region = Region::all_open(6);
        for atom in 0..5 {
            if self.decisions & (1 << atom) != 0 {
                assert!(if self.selected & (1 << atom) != 0 {
                    region.hold(atom)
                } else {
                    region.cut(atom)
                });
            }
        }
        region
    }
}

fn queue(
    theory: &Theory,
    index: &Narrower,
    order: &[usize],
    promoted: Option<usize>,
) -> NarrowingScratch {
    let mut scratch = NarrowingScratch::default();
    scratch.prepare(theory.atom_count());
    scratch
        .prepare_thresholds(index.thresholds.rows.len())
        .unwrap();
    if let Some(row) = promoted {
        scratch.queue_residual(row);
    }
    for &row in order {
        scratch.queue_threshold(row);
    }
    scratch
}

fn pop_order(mut scratch: NarrowingScratch) -> Vec<usize> {
    let mut rows = Vec::new();
    while let Some((row, transfer)) = scratch.next_threshold() {
        assert!(transfer);
        rows.push(row);
    }
    rows
}

fn schedules(index: &Narrower, knowledge: &Knowledge) -> Vec<(Vec<usize>, Option<usize>)> {
    let forward: Vec<_> = (0..index.thresholds.rows.len()).collect();
    let reverse = forward.iter().rev().copied().collect();
    let mut result = vec![(forward.clone(), None), (reverse, None)];
    let Width::Compact16(known) = &knowledge.width else {
        panic!("generated small theories use compact counters");
    };
    if let Some(row) = index
        .thresholds
        .rows
        .iter()
        .enumerate()
        .find_map(|(row, data)| {
            (data.has_nonunit_coefficient() && known.thresholds[row].has_residual_interval())
                .then_some(row)
        })
    {
        let mut order = forward;
        // Promotion changes the pop order only when the row was not already
        // the first mandatory insertion. Preserve that discriminating premise.
        if order[0] == row {
            order.swap(0, 1);
        }
        result.push((order, Some(row)));
    }
    result
}

fn branches(theory: &Theory, index: &Narrower, region: &Region, inherited: &Knowledge) {
    let Some(atom) = inherited
        .preferred_atom(region, &mut Work::new(u64::MAX))
        .unwrap()
    else {
        return;
    };
    for value in [false, true] {
        let mut actual_region = region.clone();
        assert!(if value {
            actual_region.hold(atom)
        } else {
            actual_region.cut(atom)
        });
        let mut expected_region = actual_region.clone();
        let mut actual = inherited.clone();
        let mut expected = index.knowledge();
        let actual_result = narrow(
            theory,
            index,
            &mut actual_region,
            &mut actual,
            &mut NarrowingScratch::default(),
        )
        .0;
        let expected_result = narrow(
            theory,
            index,
            &mut expected_region,
            &mut expected,
            &mut NarrowingScratch::default(),
        )
        .0;
        assert_eq!(
            actual_result == Narrowing::Refuted,
            expected_result == Narrowing::Refuted
        );
        if actual_result != Narrowing::Refuted {
            assert_eq!(actual_region, expected_region);
            assert_same_choice(index, &actual_region, &actual, &expected);
        }
    }
}

fn verify(case: &Case) {
    let theory = case.theory();
    let planted = Interpretation::new(
        &theory,
        (0..6).filter(|&atom| atom == 5 || case.selected & (1 << atom) != 0),
    )
    .unwrap();
    assert!(
        models(
            &theory,
            &planted,
            Limits::default(),
            &Cancellation::default()
        )
        .unwrap()
    );
    let index = Narrower::new(&theory);
    assert!((2..=6).contains(&index.thresholds.rows.len()));
    assert!(index.thresholds.relations.is_some());
    let initial = case.region();
    let mut seed_region = initial.clone();
    let mut seed = index.knowledge();
    let mut local = index.clone();
    local.thresholds.relations = None;
    assert_ne!(
        narrow(
            &theory,
            &local,
            &mut seed_region,
            &mut seed,
            &mut NarrowingScratch::default()
        )
        .0,
        Narrowing::Refuted
    );

    let mut expected_region = initial;
    let mut expected = index.knowledge();
    assert_ne!(
        narrow(
            &theory,
            &index,
            &mut expected_region,
            &mut expected,
            &mut NarrowingScratch::default()
        )
        .0,
        Narrowing::Refuted
    );
    let schedules = schedules(&index, &seed);
    assert_ne!(
        pop_order(queue(&theory, &index, &schedules[0].0, None)),
        pop_order(queue(&theory, &index, &schedules[1].0, None)),
    );
    for (order, promoted) in schedules {
        let actual_order = pop_order(queue(&theory, &index, &order, promoted));
        if promoted.is_some() {
            assert_ne!(
                actual_order,
                pop_order(queue(&theory, &index, &order, None))
            );
        }
        assert_eq!(actual_order.len(), index.thresholds.rows.len());
        let mut actual_region = seed_region.clone();
        let mut actual = seed.clone();
        assert!(!finish_pending(
            &theory,
            &index,
            &mut actual_region,
            &mut actual,
            &mut queue(&theory, &index, &order, promoted)
        ));
        assert_eq!(actual_region, expected_region);
        assert_same_choice(&index, &actual_region, &actual, &expected);
        branches(&theory, &index, &actual_region, &actual);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn prefix_schedules_preserve_closed_branch_knowledge(
        weights in prop::array::uniform3(2u64..=6),
        selected in any::<u8>(),
        decisions in any::<u8>(),
        optional in any::<u8>(),
        guards in any::<[(u8, u8, u8); 6]>(),
        requirements in any::<u8>(),
        reversed in any::<bool>(),
    ) {
        verify(&Case { weights, selected, decisions, optional, guards, requirements, reversed });
    }
}
