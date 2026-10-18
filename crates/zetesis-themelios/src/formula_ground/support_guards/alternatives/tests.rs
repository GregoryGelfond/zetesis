use std::cell::RefCell;

use super::*;
use crate::formula_ground::{Budget, Purpose, ground};
use crate::formula_ir::{HeadIr, RuleIr};
use crate::formula_support::{Counters, testing};
use crate::grounding_observer::{GroundingOutcome, GroundingPhase, GroundingWork, Profile};
use crate::{ExpansionLimits, FormulaLimits, FormulaResource, GroundingObserver};
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Span};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{Region, Theory};

mod boundaries;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Point {
    Eligibility,
    Walk,
    Intersection,
    Publish,
}

struct State {
    enabled: bool,
    published: usize,
    cancel: Option<(Point, Cancellation)>,
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State {
        enabled: true, published: 0, cancel: None,
    }) };
}

struct Restore(State);

impl Drop for Restore {
    fn drop(&mut self) {
        STATE.with(|state| std::mem::swap(&mut *state.borrow_mut(), &mut self.0));
    }
}

pub(super) fn enabled() -> bool {
    STATE.with(|state| state.borrow().enabled)
}

pub(super) fn published() {
    STATE.with(|state| state.borrow_mut().published += 1);
}

pub(super) fn point(point: Point) {
    STATE.with(|state| {
        if let Some((at, cancellation)) = &state.borrow().cancel
            && *at == point
        {
            cancellation.cancel();
        }
    });
}

fn scoped<T>(
    enabled: bool,
    cancel: Option<(Point, Cancellation)>,
    action: impl FnOnce() -> T,
) -> (T, usize) {
    let previous = STATE.replace(State {
        enabled,
        published: 0,
        cancel,
    });
    let _restore = Restore(previous);
    let result = action();
    (result, STATE.with(|state| state.borrow().published))
}

fn location(offset: u32) -> ProgramSite {
    ProgramSite::source(themelios_base::span::Location {
        source: SourceId::new(29),
        span: Span::empty(ByteOffset::new(offset)),
    })
}

fn rule(offset: u32) -> RuleIr {
    RuleIr {
        head: HeadIr::Normal(None),
        body: vec![],
        body_variables: 0,
        bindings: None,
        variables: 0,
        location: location(offset),
        origins: vec![location(offset)],
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Shared,
    Nested,
    Fact,
    False,
    Repeated,
    Atomic,
    Disjoint,
    Opaque,
    Choice,
    Competing,
    Cycle,
}

#[derive(Default)]
struct Peak(std::cell::Cell<usize>);

impl GroundingObserver for Peak {
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
            .set(usize::try_from(work.support_peak_bytes.unwrap_or(0)).unwrap());
    }
}

struct Attempt {
    result: Result<(Theory, Vec<Vec<ProgramSite>>), FormulaFailure>,
    work: u64,
    peak: usize,
}

fn fixture(shape: Shape, limits: &FormulaLimits, cancellation: Option<&Cancellation>) -> Attempt {
    let observer = Peak::default();
    let profile = Profile::new(Some(&observer));
    let defaults = FormulaLimits::default();
    let (result, work) =
        testing::Fixture::default().with(location(0), |_, computation, counters| {
            *counters = Counters::resume(std::mem::take(&mut counters.accounting), profile.work())
                .with_cancellation(cancellation);
            let mut budget = Budget::new(ExpansionLimits::default(), 0);
            let mut builder = Builder::empty(
                computation,
                &defaults,
                &mut budget,
                counters,
                Purpose::Theory,
                None,
                location(0),
            )
            .unwrap();
            builder.initialize(location(0)).unwrap();
            let atoms = atoms(&mut builder);
            for &atom in &atoms[..3] {
                choice(&mut builder, atom);
            }
            if matches!(shape, Shape::Disjoint | Shape::Opaque) {
                choice(&mut builder, atoms[4]);
            }
            producers(&mut builder, atoms, shape);
            let negative = builder.neg(atoms[3], location(30)).unwrap();
            let required = builder.neg(negative, location(30)).unwrap();
            builder.root(required, &rule(30)).unwrap();
            builder.limits = limits;
            let workspace = builder.counters.workspace_bytes();
            let result = profile.phase(GroundingPhase::SupportGuards, None, || {
                builder.support_guards()
            });
            assert_eq!(builder.counters.workspace_bytes(), workspace);
            let work = builder.counters.accounting.work;
            let result = result.map(|()| {
                let theory = Theory::new(
                    builder.catalog.len(),
                    std::mem::take(&mut builder.nodes).into_parts(),
                    std::mem::take(&mut builder.roots),
                    limits.theory,
                )
                .unwrap();
                (theory, std::mem::take(&mut builder.origins))
            });
            (result, work)
        });
    Attempt {
        result,
        work,
        peak: observer.0.get(),
    }
}

fn atoms(builder: &mut Builder<'_, '_, '_>) -> [usize; 5] {
    std::array::from_fn(|position| {
        let name = ["a", "b", "c", "h", "k"][position];
        let atom = zetesis_core::Atom::new(zetesis_core::Predicate::new(name, 0).unwrap(), vec![])
            .unwrap();
        builder
            .atom_ref((&atom).into(), location(u32::try_from(position).unwrap()))
            .unwrap()
    })
}

fn choice(builder: &mut Builder<'_, '_, '_>, atom: usize) {
    builder.producer(atom, VERUM, &rule(10)).unwrap();
    let negative = builder.neg(atom, location(10)).unwrap();
    let choice = builder.or(atom, negative, location(10)).unwrap();
    builder.root(choice, &rule(10)).unwrap();
}

fn bodies(builder: &mut Builder<'_, '_, '_>, atoms: [usize; 5], shape: Shape) -> [usize; 2] {
    let [left, right, guard, head, peer] = atoms;
    if shape == Shape::Atomic {
        return [guard, left];
    }
    if shape == Shape::Opaque {
        let common = builder
            .node(Node::Or(&[left, right]), location(20))
            .unwrap();
        return [
            builder
                .node(Node::And(&[common, guard]), location(20))
                .unwrap(),
            builder
                .node(Node::And(&[common, peer]), location(21))
                .unwrap(),
        ];
    }
    let first = builder
        .node(Node::And(&[guard, left]), location(20))
        .unwrap();
    let second = builder
        .node(Node::And(&[guard, right]), location(21))
        .unwrap();
    match shape {
        Shape::Nested => [
            builder
                .node(Node::And(&[first, left, first]), location(20))
                .unwrap(),
            builder
                .node(Node::And(&[second, right, second]), location(21))
                .unwrap(),
        ],
        Shape::Cycle => [
            builder
                .node(Node::And(&[first, head]), location(20))
                .unwrap(),
            builder
                .node(Node::And(&[second, head]), location(21))
                .unwrap(),
        ],
        Shape::Repeated => [first, first],
        Shape::Disjoint => [
            first,
            builder
                .node(Node::And(&[right, peer]), location(21))
                .unwrap(),
        ],
        _ => [first, second],
    }
}

fn producers(builder: &mut Builder<'_, '_, '_>, atoms: [usize; 5], shape: Shape) {
    let [_, _, _, head, other] = atoms;
    let bodies = bodies(builder, atoms, shape);
    let consequent = match shape {
        Shape::Choice => {
            let negative = builder.neg(head, location(20)).unwrap();
            builder.or(head, negative, location(20)).unwrap()
        }
        Shape::Competing => builder.or(head, other, location(20)).unwrap(),
        _ => head,
    };
    for (position, body) in bodies.into_iter().enumerate() {
        let rule = rule(20 + u32::try_from(position).unwrap());
        builder.producer(head, body, &rule).unwrap();
        if shape == Shape::Competing {
            builder.producer(other, body, &rule).unwrap();
        }
        let root = builder
            .node(Node::Implies(body, consequent), rule.location)
            .unwrap();
        builder.root(root, &rule).unwrap();
    }
    match shape {
        Shape::Fact => {
            builder.producer(head, VERUM, &rule(22)).unwrap();
            builder.root(head, &rule(22)).unwrap();
        }
        Shape::False => {
            builder.producer(head, FALSUM, &rule(22)).unwrap();
            let root = builder
                .node(Node::Implies(FALSUM, head), location(22))
                .unwrap();
            builder.root(root, &rule(22)).unwrap();
        }
        _ => {}
    }
}

fn closed(theory: &Theory) -> Region {
    let narrower = zetesis_ferraris::Narrower::new(theory);
    let mut region = Region::all_open(theory.atom_count());
    let (outcome, _) = narrower
        .narrow_known(
            zetesis_ferraris::OriginalSubject::new(theory, None),
            &mut region,
            &mut narrower.knowledge(),
            &mut zetesis_ferraris::NarrowingScratch::default(),
            zetesis_ferraris::RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(matches!(outcome, zetesis_ferraris::Narrowing::Fixed { .. }));
    region
}

#[test]
fn common_condition_is_forced_before_an_alternative() {
    let defaults = FormulaLimits::default();
    for shape in [
        Shape::Shared,
        Shape::Nested,
        Shape::Choice,
        Shape::Competing,
    ] {
        let (baseline, _) = scoped(false, None, || fixture(shape, &defaults, None));
        let (candidate, added) = scoped(true, None, || fixture(shape, &defaults, None));
        assert!(added > 0, "{shape:?}");
        let before = closed(&baseline.result.unwrap().0);
        let after = closed(&candidate.result.unwrap().0);
        assert!(before.is_held(3) && before.is_open(2), "{shape:?}");
        assert!(after.is_held(2), "{shape:?}");
        assert!(after.is_open(0) && after.is_open(1), "{shape:?}");
    }
}

#[test]
fn alternative_guards_preserve_original_and_frozen_truth() {
    let defaults = FormulaLimits::default();
    for shape in [
        Shape::Shared,
        Shape::Nested,
        Shape::Fact,
        Shape::False,
        Shape::Repeated,
        Shape::Atomic,
        Shape::Disjoint,
        Shape::Opaque,
        Shape::Choice,
        Shape::Competing,
        Shape::Cycle,
    ] {
        let (baseline, _) = scoped(false, None, || fixture(shape, &defaults, None));
        let (candidate, _) = scoped(true, None, || fixture(shape, &defaults, None));
        let (baseline, origins) = baseline.result.unwrap();
        let (candidate, new_origins) = candidate.result.unwrap();
        super::super::tests::same_truth(&baseline, &candidate);
        assert_eq!(
            &candidate.roots()[..baseline.roots().len()],
            baseline.roots()
        );
        assert_eq!(&new_origins[..origins.len()], origins);
    }
}

#[test]
fn nested_duplicate_paths_emit_one_requirement() {
    let (attempt, added) = scoped(true, None, || {
        fixture(Shape::Nested, &FormulaLimits::default(), None)
    });
    attempt.result.unwrap();
    assert_eq!(added, 1);
}

#[test]
fn ineligible_families_need_no_intersection_storage() {
    for shape in [Shape::Fact, Shape::Repeated, Shape::Atomic] {
        let (baseline, _) = scoped(false, None, || {
            fixture(shape, &FormulaLimits::default(), None)
        });
        let (attempt, added) = scoped(true, None, || {
            fixture(shape, &FormulaLimits::default(), None)
        });
        baseline.result.unwrap();
        attempt.result.unwrap();
        assert_eq!(added, 0);
        assert_eq!(attempt.peak, baseline.peak);
    }
}

#[test]
fn disjoint_alternatives_add_no_guard() {
    let (attempt, added) = scoped(true, None, || {
        fixture(Shape::Disjoint, &FormulaLimits::default(), None)
    });
    attempt.result.unwrap();
    assert_eq!(added, 0);
    assert!(
        attempt.peak > 0,
        "the eligible intersection was actually examined"
    );
}

#[test]
fn an_opaque_disjunction_does_not_force_its_operands() {
    let (attempt, added) = scoped(true, None, || {
        fixture(Shape::Opaque, &FormulaLimits::default(), None)
    });
    assert_eq!(added, 1);
    let (theory, _) = attempt.result.unwrap();
    let region = closed(&theory);
    assert!(region.is_held(3));
    assert!(region.is_open(0) && region.is_open(1));
}

#[test]
fn cyclic_self_requirements_add_no_tautological_guard() {
    let (attempt, added) = scoped(true, None, || {
        fixture(Shape::Cycle, &FormulaLimits::default(), None)
    });
    attempt.result.unwrap();
    assert_eq!(added, 1, "c is common; the head itself is skipped");
}

#[test]
fn every_producer_origin_remains_on_the_shared_guard() {
    let defaults = FormulaLimits::default();
    for (shape, expected) in [
        (Shape::Shared, vec![location(3), location(20), location(21)]),
        (
            Shape::False,
            vec![location(3), location(20), location(21), location(22)],
        ),
    ] {
        let (baseline, _) = scoped(false, None, || fixture(shape, &defaults, None));
        let (candidate, added) = scoped(true, None, || fixture(shape, &defaults, None));
        assert_eq!(added, 1);
        let (_, prior) = baseline.result.unwrap();
        let (_, current) = candidate.result.unwrap();
        assert_eq!(&current[..prior.len()], prior);
        assert_eq!(&current[prior.len()..], [expected]);
    }
}

#[test]
fn source_choices_cycles_and_disjunctions_preserve_answers() {
    for source in [
        "{a;b;c}. h :- c,a. h :- c,b. :- not h.",
        "{a;b;c}. {h} :- c,a. {h} :- c,b. :- not h.",
        "{a;b;c}. h | k :- c,a. h | k :- c,b. :- not h.",
        "{a;b;c}. h :- h,c,a. h :- h,c,b. :- not h.",
        "{a;b;c}. h :- c,a. h :- c,b. h.",
        "{a;b;c}. h :- c,a. h :- c,b. {h}.",
        "{a;b;c}. h :- a, 1 <= #count{1:c}. h :- b, 1 <= #count{1:c}.",
    ] {
        let (baseline, _) = scoped(false, None, || {
            ground(testing::prepare(source), None, None).unwrap()
        });
        let (candidate, _) = scoped(true, None, || {
            ground(testing::prepare(source), None, None).unwrap()
        });
        assert_eq!(baseline.atoms.atoms(), candidate.atoms.atoms());
        super::super::tests::same_truth(&baseline.theory, &candidate.theory);
    }
}
