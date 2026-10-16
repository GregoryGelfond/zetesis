use crate::FormulaResource;
use std::cell::RefCell;

use super::*;
use crate::formula_ground::{Budget, Purpose};
use crate::formula_ir::{HeadIr, RuleIr};
use crate::formula_support::{Counters, testing};
use crate::grounding_observer::{GroundingOutcome, GroundingPhase, GroundingWork, Profile};
use crate::{ExpansionLimits, FormulaLimits, GroundingObserver};
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Span};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::Theory;

mod premises;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Point {
    Visit,
    Fill,
    Publish,
}
struct State {
    enabled: bool,
    published: usize,
    cancel: Option<(Point, Cancellation)>,
}
thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State { enabled: true, published: 0, cancel: None }) };
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
pub(super) fn scoped<T>(
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
    let value = action();
    (value, STATE.with(|state| state.borrow().published))
}
fn location(offset: u32) -> ProgramSite {
    ProgramSite::source(themelios_base::span::Location {
        source: SourceId::new(23),
        span: Span::empty(ByteOffset::new(offset)),
    })
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    RequiredPair,
    RequiredPrefix,
    NoPremise,
}

/// Different shared-AND paths reach a and b, but each head requires them once.
/// c is also an immediate body leaf and must not get a supplementary row.
fn fixture(
    limits: &FormulaLimits,
    cancellation: Option<&Cancellation>,
    ambiguous: bool,
) -> Attempt {
    fixture_with_shape(limits, cancellation, ambiguous, Shape::RequiredPair)
}

fn fixture_with_shape(
    limits: &FormulaLimits,
    cancellation: Option<&Cancellation>,
    ambiguous: bool,
    shape: Shape,
) -> Attempt {
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
            let mut atoms = Vec::new();
            let names = ["a", "b", "c", "h", "i", "j"];
            let count = if shape == Shape::RequiredPrefix { 6 } else { 5 };
            for (i, name) in names[..count].iter().enumerate() {
                let atom = zetesis_core::Atom::new(
                    zetesis_core::Predicate::new(*name, 0).unwrap(),
                    vec![],
                )
                .unwrap();
                atoms.push(
                    builder
                        .atom_ref((&atom).into(), location(u32::try_from(i).unwrap()))
                        .unwrap(),
                );
            }
            let leaf_rule = RuleIr {
                head: HeadIr::Normal(None),
                body: vec![],
                body_variables: 0,
                bindings: None,
                variables: 0,
                location: location(9),
                origins: vec![location(9)],
            };
            for &atom in &atoms[..3] {
                builder.producer(atom, VERUM, &leaf_rule).unwrap();
                let negative = builder.neg(atom, location(9)).unwrap();
                let choice = builder.or(atom, negative, location(9)).unwrap();
                builder.root(choice, &leaf_rule).unwrap();
            }
            let common = builder.node(Node::And(&atoms[..2]), location(0)).unwrap();
            let inner = builder
                .node(Node::And(&[common, atoms[2]]), location(0))
                .unwrap();
            let body = builder
                .node(Node::And(&[common, inner, atoms[2]]), location(0))
                .unwrap();
            for (i, &head) in atoms.iter().enumerate().skip(3) {
                let rule = RuleIr {
                    head: HeadIr::Normal(None),
                    body: vec![],
                    body_variables: 0,
                    bindings: None,
                    variables: 0,
                    location: location(10),
                    origins: vec![location(10 + u32::try_from(i).unwrap())],
                };
                builder.producer(head, body, &rule).unwrap();
                let original = builder
                    .node(Node::Implies(body, head), rule.location)
                    .unwrap();
                builder.root(original, &rule).unwrap();
                if ambiguous && i == 3 {
                    // Even an identical second entry is a two-producer family.
                    builder.producer(head, body, &rule).unwrap();
                }
            }
            if shape != Shape::NoPremise {
                let required = builder.or(atoms[3], atoms[4], location(9)).unwrap();
                let negative = builder.neg(required, location(9)).unwrap();
                let guarded = builder.neg(negative, location(9)).unwrap();
                builder.root(guarded, &leaf_rule).unwrap();
            }
            builder.limits = limits;
            let workspace = builder.counters.workspace_bytes();
            let result = profile.phase(GroundingPhase::SupportGuards, None, || {
                builder.support_guards()
            });
            assert_eq!(
                builder.counters.workspace_bytes(),
                workspace,
                "scratch owners release on every outcome"
            );
            let work = builder.counters.accounting.work;
            let result = result.map(|()| finish(&mut builder, limits));
            (result, work)
        });
    Attempt {
        result,
        work,
        peak: observer.0.get(),
    }
}

fn finish(
    builder: &mut Builder<'_, '_, '_>,
    limits: &FormulaLimits,
) -> (Theory, Vec<Vec<ProgramSite>>) {
    let theory = Theory::new(
        builder.catalog.len(),
        std::mem::take(&mut builder.nodes).into_parts(),
        std::mem::take(&mut builder.roots),
        limits.theory,
    )
    .unwrap();
    (theory, std::mem::take(&mut builder.origins))
}

#[test]
fn nested_requirements_preserve_arbitrary_frozen_truth() {
    let (candidate, added) = scoped(true, None, || {
        fixture(&FormulaLimits::default(), None, false)
    });
    let (baseline, skipped) = scoped(false, None, || {
        fixture(&FormulaLimits::default(), None, false)
    });
    assert_eq!(
        added, 2,
        "a and b occur once per head despite shared paths; c is direct"
    );
    assert_eq!(skipped, 0);
    let (candidate, origins) = candidate.result.unwrap();
    let (baseline, old_origins) = baseline.result.unwrap();
    super::super::tests::same_truth(&candidate, &baseline);
    assert!(
        zetesis_ferraris::check(
            &baseline,
            &crate::test_support::interpretation(&baseline, 31),
            zetesis_ferraris::Limits::default(),
            &Cancellation::default(),
        )
        .unwrap()
        .accepted(),
        "the baseline admits both heads and their conditions"
    );
    assert_eq!(
        &candidate.roots()[..baseline.roots().len()],
        baseline.roots()
    );
    assert_eq!(&origins[..old_origins.len()], old_origins);
    let expected = vec![location(3), location(4), location(13), location(14)];
    assert_eq!(&origins[old_origins.len()..], [expected.clone(), expected]);
}

#[test]
fn repeated_producers_do_not_authorize_singleton_requirements() {
    let (candidate, added) = scoped(true, None, || {
        fixture(&FormulaLimits::default(), None, true)
    });
    let (baseline, _) = scoped(false, None, || {
        fixture(&FormulaLimits::default(), None, true)
    });
    assert_eq!(added, 0);
    let (candidate, origins) = candidate.result.unwrap();
    let (baseline, old_origins) = baseline.result.unwrap();
    assert_eq!(candidate.nodes(), baseline.nodes());
    assert_eq!(candidate.operands(), baseline.operands());
    assert_eq!(candidate.roots(), baseline.roots());
    assert_eq!(origins, old_origins);
}

#[test]
fn absent_disjunction_adds_no_supplemental_guards() {
    let defaults = FormulaLimits::default();
    let (candidate, added) = scoped(true, None, || {
        fixture_with_shape(&defaults, None, false, Shape::NoPremise)
    });
    let (baseline, _) = scoped(false, None, || {
        fixture_with_shape(&defaults, None, false, Shape::NoPremise)
    });
    assert_eq!(added, 0);
    let (candidate, origins) = candidate.result.unwrap();
    let (baseline, old_origins) = baseline.result.unwrap();
    assert_eq!(candidate.nodes(), baseline.nodes());
    assert_eq!(candidate.operands(), baseline.operands());
    assert_eq!(candidate.roots(), baseline.roots());
    assert_eq!(origins, old_origins);
}

#[test]
fn prefix_guards_use_only_covered_producer_evidence() {
    let defaults = FormulaLimits::default();
    let (candidate, added) = scoped(true, None, || {
        fixture_with_shape(&defaults, None, false, Shape::RequiredPrefix)
    });
    let (baseline, _) = scoped(false, None, || {
        fixture_with_shape(&defaults, None, false, Shape::RequiredPrefix)
    });
    assert_eq!(added, 2);
    let (candidate, origins) = candidate.result.unwrap();
    let (baseline, old_origins) = baseline.result.unwrap();
    super::super::tests::same_truth(&candidate, &baseline);
    assert!(
        zetesis_ferraris::check(
            &baseline,
            &crate::test_support::interpretation(&baseline, 63),
            zetesis_ferraris::Limits::default(),
            &Cancellation::default(),
        )
        .unwrap()
        .accepted(),
        "the baseline admits the covered heads and the untouched tail"
    );
    let expected = vec![location(3), location(4), location(13), location(14)];
    assert_eq!(&origins[old_origins.len()..], [expected.clone(), expected]);
    assert_eq!(&origins[..old_origins.len()], old_origins);
    let before = closed(&baseline);
    assert!(before.is_open(0) && before.is_open(1));
    let after = closed(&candidate);
    assert!(after.is_held(0) && after.is_held(1));
}

#[test]
fn nested_work_cutoffs_preserve_refusal() {
    let defaults = FormulaLimits::default();
    let exact = fixture(&defaults, None, false).work;
    for cap in 0..=exact {
        let attempt = fixture(
            &FormulaLimits {
                max_work: cap,
                ..defaults
            },
            None,
            false,
        );
        if cap == exact {
            attempt.result.unwrap();
        } else {
            assert!(matches!(
                attempt.result,
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    ..
                })
            ));
        }
    }
}

#[test]
fn nested_storage_boundaries_are_inclusive() {
    let defaults = FormulaLimits::default();
    let baseline = fixture(&defaults, None, false);
    let (theory, origins) = baseline.result.unwrap();
    for (resource, exact) in [
        (FormulaResource::Nodes, theory.nodes().len()),
        (FormulaResource::Operands, theory.parts().occurrences()),
        (FormulaResource::Roots, theory.roots().len()),
        (FormulaResource::Origins, origins.iter().map(Vec::len).sum()),
        (FormulaResource::SupportBytes, baseline.peak),
    ] {
        for cap in [exact - 1, exact] {
            let mut limits = defaults;
            match resource {
                FormulaResource::Nodes => limits.theory.max_nodes = cap,
                FormulaResource::Operands => limits.theory.max_operands = cap,
                FormulaResource::Roots => limits.theory.max_roots = cap,
                FormulaResource::Origins => limits.max_origin_locations = cap,
                FormulaResource::SupportBytes => limits.max_support_bytes = cap,
                _ => unreachable!("selected storage limits"),
            }
            let attempt = fixture(&limits, None, false);
            if cap == exact {
                attempt.result.unwrap();
            } else {
                assert!(
                    matches!(attempt.result, Err(FormulaFailure::Limit { resource: found, .. }) if found == resource),
                    "{resource:?}"
                );
            }
        }
    }
}

#[test]
fn nested_cancellation_keeps_typed_interruption() {
    for point in [Point::Visit, Point::Fill, Point::Publish] {
        let cancellation = Cancellation::default();
        let (attempt, _) = scoped(true, Some((point, cancellation.clone())), || {
            fixture(&FormulaLimits::default(), Some(&cancellation), false)
        });
        assert!(matches!(
            attempt.result,
            Err(FormulaFailure::Interrupted {
                reason: Stop::Cancelled,
                ..
            })
        ));
    }
}

#[test]
fn shared_nested_conditions_follow_a_required_head_disjunction() {
    let (candidate, added) = scoped(true, None, || {
        fixture(&FormulaLimits::default(), None, false)
    });
    let (baseline, _) = scoped(false, None, || {
        fixture(&FormulaLimits::default(), None, false)
    });
    assert_eq!(added, 2);
    let before = closed(&baseline.result.unwrap().0);
    assert!(before.is_open(0) && before.is_open(1));
    let after = closed(&candidate.result.unwrap().0);
    assert!(after.is_held(0) && after.is_held(1));
}

fn closed(theory: &Theory) -> zetesis_ferraris::Region {
    let narrower = zetesis_ferraris::Narrower::new(theory);
    let mut region = zetesis_ferraris::Region::all_open(theory.atom_count());
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
