//! Descendant narrowing preserves the owned cube's pass and stop boundaries.

use super::*;
use zetesis_core::{AdmissionLimits, AtomPattern, Predicate, Template};

fn atom(name: &str) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap()
}

fn cycle() -> Program {
    Program::new(
        [("a", "b"), ("b", "c"), ("c", "a")]
            .into_iter()
            .map(|(head, gate)| {
                Template::new(Some(atom(head)), vec![], vec![], vec![atom(gate)], vec![])
            })
            .collect(),
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn bounded(program: &Program) -> Candidates<'_> {
    let mut candidates = Candidates::new(program, CandidateLimits::default(), Control::default());
    candidates.bounded(Limits::default());
    candidates.prepare_bounds().unwrap();
    assert!(matches!(candidates.narrowing, NarrowingState::Applied(_)));
    candidates
}

fn materialize(candidates: &Candidates<'_>, region: &Region) -> Cube {
    let mut cube = Cube {
        must: candidates.root_must.clone(),
        may: Some(candidates.root_must.clone()),
    };
    for (at, gate) in candidates.root.iter().enumerate() {
        if region.is_held(at) {
            cube.must.insert(gate.atom().clone());
        }
        if !region.is_cut(at) {
            cube.may.as_mut().unwrap().insert(gate.atom().clone());
        }
    }
    cube
}

fn transfer(root: &[Arc<GateAtom>], cube: &Cube, region: &mut Region) {
    for (at, gate) in root.iter().enumerate() {
        if region.is_open(at) {
            if cube.must.contains(gate.atom()) {
                region.hold(at);
            } else if !cube.may.as_ref().unwrap().contains(gate.atom()) {
                region.cut(at);
            }
        }
    }
}

fn closures<'a>(candidates: &'a mut Candidates<'_>) -> &'a mut Closures {
    let NarrowingState::Applied(closures) = &mut candidates.narrowing else {
        panic!("completed root closures");
    };
    closures
}

#[test]
fn borrowed_passes_match_materialized_passes() {
    let program = cycle();
    let mut candidates = bounded(&program);
    let mut reference = Closures::new(&program, Limits::default(), Control::default()).unwrap();
    let root = candidates.root.clone();
    let held = candidates.root_must.clone();
    assert_eq!(root.len(), 3);
    for mut digits in 0..27 {
        let mut region = Region::all_open(root.len());
        for at in 0..root.len() {
            match digits % 3 {
                1 => {
                    region.hold(at);
                }
                2 => {
                    region.cut(at);
                }
                _ => {}
            }
            digits /= 3;
        }
        // At most one pass per new decision, then a fixed/refuting pass.
        for _ in 0..=root.len() {
            let mut cube = materialize(&candidates, &region);
            let mut expected_region = region.clone();
            let expected = reference.narrow(&mut cube).unwrap();
            if expected != Pass::Refuted {
                transfer(&root, &cube, &mut expected_region);
            }
            let actual = closures(&mut candidates)
                .narrow_region(&held, &root, &mut region)
                .unwrap();
            assert_eq!(actual, expected);
            assert_eq!(region, expected_region);
            assert_eq!(region.decisions(), expected_region.decisions());
            if actual != Pass::Changed {
                break;
            }
        }
    }
}

#[test]
fn borrowed_passes_preserve_work_refusals() {
    let program = cycle();
    let candidates = bounded(&program);
    let mut original = Region::all_open(candidates.root.len());
    original.cut(0);
    let mut completed = false;
    // This finite three-atom control crosses both lower and upper work
    // boundaries. Fresh, identically prepared workspaces prevent prior capacity
    // histories from changing either side's accounting premise.
    for max_work in 0..=256 {
        let mut reference = Closures::new(&program, Limits::default(), Control::default()).unwrap();
        let mut borrowed = Closures::new(&program, Limits::default(), Control::default()).unwrap();
        reference.limits.max_work = max_work;
        borrowed.limits.max_work = max_work;
        let mut cube = materialize(&candidates, &original);
        let expected = reference.narrow(&mut cube);
        let mut expected_region = original.clone();
        if matches!(expected, Ok(Pass::Changed | Pass::Fixed)) {
            transfer(&candidates.root, &cube, &mut expected_region);
            completed = true;
        }
        let mut region = original.clone();
        let actual = borrowed.narrow_region(&candidates.root_must, &candidates.root, &mut region);
        assert_eq!(actual, expected, "work ceiling {max_work}");
        assert_eq!(region, expected_region, "work ceiling {max_work}");
        assert_eq!(region.decisions(), expected_region.decisions());
    }
    assert!(
        completed,
        "the sweep must include a completed pass, not only refusals"
    );
}

#[test]
fn both_closures_read_the_prepass_region() {
    let program = cycle();
    let mut candidates = bounded(&program);
    let root = candidates.root.clone();
    let held = candidates.root_must.clone();
    let mut region = Region::all_open(root.len());
    assert_eq!(
        root.iter()
            .map(|gate| gate.atom().predicate().name())
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    region.cut(0);
    assert_eq!(
        closures(&mut candidates).narrow_region(&held, &root, &mut region),
        Ok(Pass::Changed)
    );
    // Lower derives c. Upper must still read the old empty must, so it derives
    // b too. Holding c before the upper call would cut b one pass too early.
    assert_eq!(
        (region.decision(0), region.decision(1), region.decision(2)),
        (Some(false), None, Some(true))
    );
}

#[test]
fn a_stopped_upper_closure_commits_no_decisions() {
    let program = cycle();
    let mut candidates = bounded(&program);
    let root = candidates.root.clone();
    let held = candidates.root_must.clone();
    let mut region = Region::all_open(root.len());
    region.cut(0);
    let before = region.clone();
    let closures = closures(&mut candidates);
    closures.limits.max_derived_atoms = 1;
    let indexed = RegionBounds::new(&held, &root, &region);
    let bounds = Bounds::Region(&indexed);
    let mut probe = ClosureWorkspace::default();
    let lower = definite_closure(
        &closures.prepared,
        &mut probe,
        bounds,
        closures.limits,
        &closures.control,
    )
    .unwrap();
    assert_eq!(lower.atoms.atoms().len(), 1);
    assert!(!lower.constraint_violated);
    assert!(matches!(
        possible_closure(
            &closures.prepared,
            &mut probe,
            bounds,
            closures.limits,
            &closures.control
        ),
        Err(Stop::DerivedAtomLimit)
    ));
    assert_eq!(
        closures.narrow_region(&held, &root, &mut region),
        Err(Stop::DerivedAtomLimit)
    );
    assert_eq!(region, before);
    assert_eq!(region.decisions(), before.decisions());
}

#[test]
fn a_later_stop_keeps_the_completed_region() {
    let program = cycle();
    let mut candidates = bounded(&program);
    let root = candidates.root.clone();
    let held = candidates.root_must.clone();
    let mut region = Region::all_open(root.len());
    region.cut(0);
    assert_eq!(
        closures(&mut candidates).narrow_region(&held, &root, &mut region),
        Ok(Pass::Changed)
    );
    let completed = region.clone();
    closures(&mut candidates).limits.max_work = 0;
    assert_eq!(
        candidates.narrow_region(&mut region),
        Ok(Narrowing::Fixed { changed: false })
    );
    assert_eq!(region, completed);
    assert_eq!(region.decisions(), completed.decisions());
    assert_eq!(candidates.statistics.narrowing_stop, Some(Stop::WorkLimit));
}

#[test]
fn cancellation_keeps_the_completed_region() {
    let program = cycle();
    let mut candidates = bounded(&program);
    let root = candidates.root.clone();
    let held = candidates.root_must.clone();
    let mut region = Region::all_open(root.len());
    region.cut(0);
    assert_eq!(
        closures(&mut candidates).narrow_region(&held, &root, &mut region),
        Ok(Pass::Changed)
    );
    let completed = region.clone();
    candidates.control.cancel();
    assert_eq!(candidates.narrow_region(&mut region), Err(Stop::Cancelled));
    assert_eq!(region, completed);
    assert_eq!(region.decisions(), completed.decisions());
}

#[test]
fn an_upper_only_constraint_does_not_refute() {
    let a = atom("a");
    let program = Program::new(
        vec![
            Template::new(Some(a.clone()), vec![], vec![a.clone()], vec![], vec![]),
            Template::new(None, vec![a], vec![], vec![], vec![]),
        ],
        AdmissionLimits::default(),
    )
    .unwrap();
    let mut candidates = bounded(&program);
    let root = candidates.root.clone();
    let held = candidates.root_must.clone();
    let mut region = Region::all_open(root.len());
    let closures = closures(&mut candidates);
    let upper = possible_closure(
        &closures.prepared,
        &mut ClosureWorkspace::default(),
        Bounds::Region(&RegionBounds::new(&held, &root, &region)),
        closures.limits,
        &closures.control,
    )
    .unwrap();
    assert!(
        upper.constraint_violated,
        "the control must fire only in the upper reading"
    );
    assert_eq!(
        closures.narrow_region(&held, &root, &mut region),
        Ok(Pass::Fixed)
    );
}
