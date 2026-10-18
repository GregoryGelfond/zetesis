//! Differential closure and processed-event accounting for shared native rows.

use crate::regions::{
    Closure, Knowledge, Narrower, NarrowingScratch, NarrowingStatistics, RegionLimits, Step,
    Subject, Width, Work, bit,
};
use crate::{AdmissionLimits, FormulaNodes, FormulaParts, Node, Region, Theory};
use zetesis_cpu::{Cancellation, Stop};

use super::State;

fn family(atoms: usize, conditions: &[usize], weights: &[u64], bounds: &[i128]) -> Theory {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..atoms).map(Node::atom).collect(), vec![]).unwrap());
    nodes
        .push_threshold_family(conditions, weights, bounds, usize::MAX, usize::MAX)
        .unwrap();
    Theory::new(
        atoms,
        nodes.into_parts(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn decisions(mut code: usize, length: usize) -> Vec<Option<bool>> {
    (0..length)
        .map(|_| {
            let value = match code % 3 {
                0 => None,
                1 => Some(true),
                _ => Some(false),
            };
            code /= 3;
            value
        })
        .collect()
}

fn region(values: &[Option<bool>]) -> Region {
    let mut region = Region::all_open(values.len());
    for (atom, value) in values.iter().enumerate() {
        match value {
            Some(true) => assert!(region.hold(atom)),
            Some(false) => assert!(region.cut(atom)),
            None => {}
        }
    }
    region
}

fn readings(knowledge: &Knowledge, nodes: usize) -> Vec<Option<bool>> {
    fn read<C>(known: &crate::regions::Known<C>, nodes: usize) -> Vec<Option<bool>> {
        let masks = known.masks.slices();
        (0..nodes)
            .map(|node| {
                if bit(masks[0], node) {
                    Some(true)
                } else if bit(masks[1], node) {
                    Some(false)
                } else {
                    None
                }
            })
            .collect()
    }
    match &knowledge.width {
        Width::Compact16(known) => read(known, nodes),
        Width::Compact32(known) => read(known, nodes),
        Width::Native(known) => read(known, nodes),
    }
}

fn states(knowledge: &Knowledge) -> &[State] {
    match &knowledge.width {
        Width::Compact16(known) => &known.thresholds,
        Width::Compact32(known) => &known.thresholds,
        Width::Native(known) => &known.thresholds,
    }
}

fn record(values: &mut [Option<bool>], node: usize, value: bool) -> Result<bool, ()> {
    match values[node] {
        Some(existing) if existing != value => Err(()),
        Some(_) => Ok(false),
        None => {
            values[node] = Some(value);
            Ok(true)
        }
    }
}

/// A deliberately repeated full-row scan, independent of event counters and
/// cursor orders. It is only a reference for the same local propagation rules.
fn scalar(
    atoms: &[Option<bool>],
    guards: &[Option<bool>],
    weights: &[i128],
    bounds: &[i128],
    frozen: Option<&[bool]>,
) -> Result<Vec<Option<bool>>, ()> {
    let count = atoms.len();
    let mut values = atoms.iter().chain(guards).copied().collect::<Vec<_>>();
    if let Some(mask) = frozen {
        for node in 0..values.len() {
            if !mask[node] {
                if node < count {
                    values[node] = Some(false);
                } else {
                    record(&mut values, node, false)?;
                }
            }
        }
    }
    loop {
        let known: i128 = weights
            .iter()
            .enumerate()
            .filter(|&(node, _)| values[node] == Some(true))
            .map(|(_, weight)| weight)
            .sum();
        let possible: i128 = weights
            .iter()
            .enumerate()
            .filter(|&(node, _)| values[node] != Some(false))
            .map(|(_, weight)| weight)
            .sum();
        let mut lower = 0;
        let mut upper = None;
        for (guard, &bound) in bounds.iter().enumerate() {
            let node = count + guard;
            if frozen.is_some_and(|mask| !mask[node]) {
                continue;
            }
            match values[node] {
                Some(true) => lower = lower.max(bound),
                Some(false) => upper = Some(upper.map_or(bound, |prior: i128| prior.min(bound))),
                None => {}
            }
        }
        if lower > possible || upper.is_some_and(|upper| known.max(lower) >= upper) {
            return Err(());
        }
        let mut changed = false;
        for (guard, &bound) in bounds.iter().enumerate() {
            let node = count + guard;
            if frozen.is_some_and(|mask| !mask[node]) {
                continue;
            }
            if bound <= known.max(lower) {
                changed |= record(&mut values, node, true)?;
            }
            if bound > possible || upper.is_some_and(|upper| bound >= upper) {
                changed |= record(&mut values, node, false)?;
            }
        }
        for (node, &weight) in weights.iter().enumerate() {
            if values[node].is_some() {
                continue;
            }
            if weight > possible - lower {
                changed |= record(&mut values, node, true)?;
            }
            if upper.is_some_and(|upper| weight >= upper - known) {
                changed |= record(&mut values, node, false)?;
            }
        }
        if !changed {
            return Ok(values);
        }
    }
}

fn incremental(
    theory: &Theory,
    index: &Narrower,
    atoms: &[Option<bool>],
    guards: &[Option<bool>],
    frozen: Option<&[bool]>,
) -> Result<Vec<Option<bool>>, ()> {
    let mut knowledge = index.root_known::<usize>();
    let mut scratch = NarrowingScratch::default();
    scratch.prepare(theory.atom_count());
    let mut closure = Closure {
        known: knowledge.borrow(),
        lists: &mut scratch,
    };
    for (guard, value) in guards.iter().enumerate() {
        let node = atoms.len() + guard;
        match value {
            Some(true) => {
                assert!(closure.sure(node) != Step::Contradiction);
            }
            Some(false) => {
                assert!(closure.never(node) != Step::Contradiction);
            }
            None => {}
        }
    }
    let result = closure
        .close(
            Subject {
                theory,
                producers: None,
                frozen,
                choices: &crate::regions::NO_CHOICES,
                generator_order: None,
            },
            index,
            &region(atoms),
            &mut Work::new(u64::MAX),
            &mut NarrowingStatistics::default(),
        )
        .unwrap();
    if result == Step::Contradiction {
        return Err(());
    }
    Ok(readings(
        &Knowledge {
            width: Width::Native(knowledge),
        },
        theory.nodes().len(),
    ))
}

#[test]
fn counters_match_scalar_frozen_closure() {
    let bounds = [0, 5, 11];
    let theory = family(3, &[0, 0, 1, 2, 2], &[2, 3, 1, 0, 4], &bounds);
    let index = Narrower::new(&theory);
    assert_eq!(index.thresholds.rows.len(), 1);
    for mask in 0_usize..=8 {
        let weights = [5, 1, 4];
        let sum: i128 = weights
            .iter()
            .enumerate()
            .filter(|&(node, _)| mask & (1 << node) != 0)
            .map(|(_, weight)| weight)
            .sum();
        let truth: Vec<_> = (0..3)
            .map(|node| mask & (1 << node) != 0)
            .chain(bounds.iter().map(|&bound| sum >= bound))
            .collect();
        let frozen = (mask < 8).then_some(truth.as_slice());
        for atom_code in 0..27 {
            let atoms = decisions(atom_code, 3);
            for guard_code in 0..27 {
                let guards = decisions(guard_code, 3);
                assert_eq!(
                    incremental(&theory, &index, &atoms, &guards, frozen),
                    scalar(&atoms, &guards, &weights, &bounds, frozen),
                    "mask={mask}, atoms={atom_code}, guards={guard_code}"
                );
            }
        }
    }
}

fn close(
    theory: &Theory,
    index: &Narrower,
    region: &mut Region,
    knowledge: &mut Knowledge,
    scratch: &mut NarrowingScratch,
    limit: u64,
) -> Result<(crate::Narrowing, NarrowingStatistics), Stop> {
    index.narrow_known(
        crate::OriginalSubject::new(theory, None),
        region,
        knowledge,
        scratch,
        RegionLimits { max_work: limit },
        &Cancellation::default(),
    )
}

#[test]
fn queued_conditions_are_counted_once() {
    let theory = family(3, &[0, 0, 1, 2], &[2, 3, 1, 4], &[3, 5, 7]);
    let index = Narrower::new(&theory);
    let mut knowledge = index.knowledge();
    let mut region = region(&[Some(true), Some(false), Some(true)]);
    let mut scratch = NarrowingScratch::default();
    close(
        &theory,
        &index,
        &mut region,
        &mut knowledge,
        &mut scratch,
        u64::MAX,
    )
    .unwrap();
    assert_eq!(
        (states(&knowledge)[0].known, states(&knowledge)[0].possible),
        (9, 9)
    );
    let before = states(&knowledge).to_vec();
    close(
        &theory,
        &index,
        &mut region,
        &mut knowledge,
        &mut scratch,
        u64::MAX,
    )
    .unwrap();
    assert_eq!(states(&knowledge), before);
}

#[test]
fn copied_row_state_has_independent_branch_authority() {
    let theory = family(3, &[0, 0, 1, 2], &[2, 3, 1, 4], &[3, 5, 7]);
    let index = Narrower::new(&theory);
    let mut parent = index.knowledge();
    let mut root = Region::all_open(3);
    let mut scratch = NarrowingScratch::default();
    close(
        &theory,
        &index,
        &mut root,
        &mut parent,
        &mut scratch,
        u64::MAX,
    )
    .unwrap();
    let before = states(&parent).to_vec();
    let mut copied = parent.clone();
    let allocation = states(&copied).as_ptr();
    for code in 0..27 {
        copied.clone_from(&parent);
        assert_eq!(states(&copied).as_ptr(), allocation);
        let mut child = region(&decisions(code, 3));
        let mut fresh_region = child.clone();
        let mut fresh = index.knowledge();
        let inherited = close(
            &theory,
            &index,
            &mut child,
            &mut copied,
            &mut scratch,
            u64::MAX,
        )
        .unwrap()
        .0;
        let expected = close(
            &theory,
            &index,
            &mut fresh_region,
            &mut fresh,
            &mut NarrowingScratch::default(),
            u64::MAX,
        )
        .unwrap()
        .0;
        assert_eq!(inherited, expected);
        assert_eq!(child, fresh_region);
        assert_eq!(
            readings(&copied, theory.nodes().len()),
            readings(&fresh, theory.nodes().len())
        );
        assert_eq!(states(&copied), states(&fresh));
        assert_eq!(states(&parent), before);
    }
}

#[test]
fn stopped_numeric_work_leaves_no_scratch_authority() {
    let theory = family(3, &[0, 0, 1, 2], &[2, 3, 1, 4], &[3, 5, 7]);
    let index = Narrower::new(&theory);
    let original = region(&[Some(true), Some(false), Some(true)]);
    let mut expected_region = original.clone();
    let mut expected = index.knowledge();
    let receipt = close(
        &theory,
        &index,
        &mut expected_region,
        &mut expected,
        &mut NarrowingScratch::default(),
        u64::MAX,
    )
    .unwrap();
    let mut scratch = NarrowingScratch::default();
    for ceiling in 0..receipt.1.work {
        let mut discarded_region = original.clone();
        let stopped = index.narrow_with(
            (
                Subject {
                    theory: &theory,
                    producers: None,
                    frozen: None,
                    choices: &crate::regions::NO_CHOICES,
                    generator_order: None,
                },
                crate::regions::SplitSelection::Choose,
            ),
            &mut discarded_region,
            &mut index.knowledge(),
            &mut scratch,
            Work::new(ceiling),
            &Cancellation::default(),
        );
        assert_eq!(stopped.result, Err(Stop::WorkLimit));
        assert_eq!(stopped.statistics.work, ceiling);
        let mut actual_region = original.clone();
        let mut actual = index.knowledge();
        assert_eq!(
            close(
                &theory,
                &index,
                &mut actual_region,
                &mut actual,
                &mut scratch,
                u64::MAX
            )
            .unwrap(),
            receipt
        );
        assert_eq!(actual_region, expected_region);
        assert_eq!(states(&actual), states(&expected));
    }
}

#[test]
fn shared_row_charged_work_grows_linearly() {
    let measured = |atoms: usize| {
        let bounds: Vec<_> = (1..=atoms).map(|value| value as i128).collect();
        let theory = family(
            atoms,
            &(0..atoms).collect::<Vec<_>>(),
            &vec![1; atoms],
            &bounds,
        );
        let index = Narrower::new(&theory);
        assert_eq!(index.thresholds.rows.len(), 1);
        assert_eq!(index.thresholds.incidences.len(), atoms);
        let mut knowledge = index.knowledge();
        let mut region = region(&vec![Some(true); atoms]);
        let result = close(
            &theory,
            &index,
            &mut region,
            &mut knowledge,
            &mut NarrowingScratch::default(),
            u64::MAX,
        )
        .unwrap();
        assert_eq!(states(&knowledge)[0].known, atoms as i128);
        result.1.work
    };
    let small = measured(64);
    let large = measured(256);
    assert!(large <= 5 * small, "small={small}, large={large}");
    assert!(large < 16 * 256, "work={large}");
}

#[test]
fn retained_bytes_include_every_shared_row_state() {
    let theory = family(3, &[0, 0, 1, 2], &[2, 3, 1, 4], &[3, 5, 7]);
    let knowledge = Narrower::new(&theory).knowledge();
    let mut without = knowledge.clone();
    match &mut without.width {
        Width::Compact16(known) => known.thresholds = Box::default(),
        Width::Compact32(known) => known.thresholds = Box::default(),
        Width::Native(known) => known.thresholds = Box::default(),
    }
    assert_eq!(
        knowledge.retained_bytes() - without.retained_bytes(),
        size_of::<State>() as u128
    );
}

#[test]
fn copied_rows_preserve_arbitrary_frozen_children() {
    let bounds = [3, 5, 7];
    let theory = family(3, &[0, 0, 1, 2], &[2, 3, 1, 4], &bounds);
    let index = Narrower::new(&theory);
    let weights = [5, 1, 4];
    for mask in 0_usize..8 {
        let sum: i128 = weights
            .iter()
            .enumerate()
            .filter(|&(node, _)| mask & (1 << node) != 0)
            .map(|(_, weight)| weight)
            .sum();
        let truth: Vec<_> = (0..3)
            .map(|node| mask & (1 << node) != 0)
            .chain(bounds.iter().map(|&bound| sum >= bound))
            .collect();
        let close =
            |region: &mut Region, knowledge: &mut Knowledge, scratch: &mut NarrowingScratch| {
                index
                    .narrow_frozen_known(
                        crate::FrozenSubject::new(&theory, &truth),
                        region,
                        knowledge,
                        scratch,
                        RegionLimits::default(),
                        &Cancellation::default(),
                    )
                    .unwrap()
                    .0
            };
        let mut parent = index.knowledge();
        let mut scratch = NarrowingScratch::default();
        close(&mut Region::all_open(3), &mut parent, &mut scratch);
        let before = states(&parent).to_vec();
        let mut copied = parent.clone();
        for code in 0..27 {
            copied.clone_from(&parent);
            let mut child = region(&decisions(code, 3));
            let mut fresh_region = child.clone();
            let mut fresh = index.knowledge();
            assert_eq!(
                close(&mut child, &mut copied, &mut scratch),
                close(
                    &mut fresh_region,
                    &mut fresh,
                    &mut NarrowingScratch::default()
                )
            );
            assert_eq!(child, fresh_region);
            assert_eq!(
                readings(&copied, theory.nodes().len()),
                readings(&fresh, theory.nodes().len())
            );
            let values = readings(&copied, 3);
            let known: i128 = weights
                .iter()
                .enumerate()
                .filter(|&(node, _)| values[node] == Some(true))
                .map(|(_, weight)| weight)
                .sum();
            let possible: i128 = weights
                .iter()
                .enumerate()
                .filter(|&(node, _)| values[node] != Some(false))
                .map(|(_, weight)| weight)
                .sum();
            assert_eq!(
                (states(&copied)[0].known, states(&copied)[0].possible),
                (known, possible)
            );
            assert_eq!(states(&parent), before);
        }
    }
}

#[test]
fn zero_rows_keep_extreme_bounds_constant() {
    let theory = family(
        3,
        &[0, 0, 1, 2],
        &[0, 0, 0, 0],
        &[i128::MIN, 0, 1, i128::MAX],
    );
    let index = Narrower::new(&theory);
    assert!(index.thresholds.incidences.is_empty());
    let mut knowledge = index.knowledge();
    let mut region = Region::all_open(3);
    close(
        &theory,
        &index,
        &mut region,
        &mut knowledge,
        &mut NarrowingScratch::default(),
        u64::MAX,
    )
    .unwrap();
    assert!((0..3).all(|atom| region.is_open(atom)));
    assert_eq!(
        readings(&knowledge, 7),
        [
            None,
            None,
            None,
            Some(true),
            Some(true),
            Some(false),
            Some(false)
        ]
    );
    assert_eq!(
        (states(&knowledge)[0].known, states(&knowledge)[0].possible),
        (0, 0)
    );
}

#[test]
fn grouped_coefficients_exceed_u64_without_truncation() {
    let bound = 2 * i128::from(u64::MAX);
    let theory = family(2, &[0, 0, 1], &[u64::MAX; 3], &[bound]);
    let index = Narrower::new(&theory);
    assert_eq!(
        incremental(&theory, &index, &[None, Some(false)], &[Some(true)], None),
        Ok(vec![Some(true), Some(false), Some(true)])
    );
}

#[test]
fn frozen_false_guard_updates_its_parent_row() {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new(vec![Node::atom(0), Node::atom(1)], vec![]).unwrap());
    let inner = nodes
        .push_threshold_family(&[0], &[1], &[1], 100, 100)
        .unwrap()[0];
    let outer = nodes
        .push_threshold_family(&[inner, 1], &[10, 1], &[1], 100, 100)
        .unwrap()[0];
    let theory = Theory::new(
        2,
        nodes.into_parts(),
        vec![outer],
        AdmissionLimits::default(),
    )
    .unwrap();
    let index = Narrower::new(&theory);
    // M={1}; J is arbitrary. The masked inner guard cannot become true even
    // when J holds atom 0, so the outer root must obtain its weight from atom 1.
    let truth = [false, true, false, true];
    let mut knowledge = index.knowledge();
    let mut region = Region::all_open(2);
    index
        .narrow_frozen_known(
            crate::FrozenSubject::new(&theory, &truth),
            &mut region,
            &mut knowledge,
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(region.is_open(0));
    assert!(region.is_held(1));
    assert_eq!(
        (states(&knowledge)[1].known, states(&knowledge)[1].possible),
        (1, 1)
    );
}
