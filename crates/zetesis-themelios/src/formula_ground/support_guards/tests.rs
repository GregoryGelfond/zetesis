//! Synchronous reference calls change only guard representation. The scoped
//! control restores on unwind and is not inherited by newly spawned threads.
use std::cell::RefCell;

use super::*;
use crate::formula_ground::producer::testing as producer;
use crate::formula_ground::{Budget, Purpose, ground};
use crate::formula_ir::{HeadIr, RuleIr};
use crate::formula_support::{Counters, testing};
use crate::grounding_observer::{GroundingOutcome, GroundingPhase, GroundingWork, Profile};
use crate::{ExpansionLimits, GroundingObserver};
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Span};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{Interpretation, Limits, Theory, check, models, models_reduct};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Point {
    Count,
    Fill,
    Evidence,
    Publish,
}
struct State {
    enabled: bool,
    accepted: usize,
    cancel: Option<(Point, Cancellation)>,
}
thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State { enabled: true, accepted: 0, cancel: None }) };
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
pub(super) fn accepted() {
    STATE.with(|state| state.borrow_mut().accepted += 1);
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
        accepted: 0,
        cancel,
    });
    let _restore = Restore(previous);
    let result = action();
    (result, STATE.with(|state| state.borrow().accepted))
}

#[test]
fn reference_control_restores_after_unwind() {
    let result = std::panic::catch_unwind(|| {
        scoped(false, None, || {
            assert!(!enabled());
            panic!("test unwind");
        })
    });
    assert!(result.is_err());
    assert!(enabled());
}

#[test]
fn reference_control_stays_on_its_thread() {
    scoped(false, None, || {
        assert!(!enabled());
        assert!(std::thread::spawn(enabled).join().unwrap());
    });
    assert!(enabled());
}

fn interpretation(theory: &Theory, bits: usize) -> Interpretation {
    Interpretation::new(
        theory,
        (0..theory.atom_count()).filter(|atom| bits & (1 << atom) != 0),
    )
    .unwrap()
}
fn same_truth(left: &Theory, right: &Theory) {
    assert_eq!(left.atom_count(), right.atom_count());
    assert!(left.atom_count() <= 7);
    let cancellation = Cancellation::default();
    for candidate in 0..1 << left.atom_count() {
        let (m, other_m) = (
            interpretation(left, candidate),
            interpretation(right, candidate),
        );
        assert_eq!(
            models(left, &m, Limits::default(), &cancellation).unwrap(),
            models(right, &other_m, Limits::default(), &cancellation).unwrap(),
            "M={candidate}"
        );
        assert_eq!(
            check(left, &m, Limits::default(), &cancellation)
                .unwrap()
                .accepted(),
            check(right, &other_m, Limits::default(), &cancellation)
                .unwrap()
                .accepted(),
            "stable M={candidate}"
        );
        for tested in 0..1 << left.atom_count() {
            assert_eq!(
                models_reduct(
                    left,
                    &m,
                    &interpretation(left, tested),
                    Limits::default(),
                    &cancellation
                )
                .unwrap(),
                models_reduct(
                    right,
                    &other_m,
                    &interpretation(right, tested),
                    Limits::default(),
                    &cancellation
                )
                .unwrap(),
                "M={candidate}, J={tested}"
            );
        }
    }
}

#[test]
fn transposed_sources_preserve_formula_semantics() {
    // This comparison changes only the guard representation. Producer grouping
    // can change the immediate incidences and legitimately decline the price.
    producer::scoped(false, || {
        for source in [
            "{a;b}. x :- a,b. y :- a,b. z :- a,b.",
            "{a}. b :- a. x :- b. y :- b. z :- b.",
            "{a}. x :- not a. y :- not a. z :- not a.",
            "{a;b}. x | not y :- a,b. z :- a,b. w :- a,b.",
            "{a;b}. x :- a,b. x :- a,b. y :- a,b. z :- a,b. w :- a,b.",
            "{a}. x :- not not a. y :- not not a. z :- not not a.",
            "{a}. x :- a,not not x. y :- a,not not x. z :- a,not not x.",
            "{a;b}. {x;y;z} :- a,b.",
            "{a;b}. x:a | y:a | z:a :- b.",
            "{a;b;c}. x :- a,b,c. y :- a,b,c. z :- a,b,c.",
        ] {
            let (shared, used) = scoped(true, None, || {
                ground(testing::prepare(source), None, None).unwrap()
            });
            let (prior, skipped) = scoped(false, None, || {
                ground(testing::prepare(source), None, None).unwrap()
            });
            assert_eq!(used, 1, "{source}");
            assert_eq!(skipped, 0);
            assert_eq!(shared.atoms.atoms(), prior.atoms.atoms());
            same_truth(&shared.theory, &prior.theory);
            assert!(
                shared.theory.nodes().len() < prior.theory.nodes().len(),
                "{source}"
            );
        }
    });
}

#[test]
fn transposed_native_groups_preserve_formula_semantics() {
    // Four heads share a native triple with two distinct conditions. Its twelve
    // incidences plus twelve guard operands meet the old twenty-four-operand
    // price, while eight proposed nodes improve on twelve old guard nodes.
    // Distinct source patterns survive Body's literal-set admission, then p(X)
    // and p(1) denote the same atom in the sole complete binding.
    let source = "{p(1);q(1)}. w(X) :- p(X),q(X),p(1). \
        x(X) :- p(X),q(X),p(1). y(X) :- p(X),q(X),p(1). \
        z(X) :- p(X),q(X),p(1).";
    let ((shared, used), counts) = producer::scoped(true, || {
        scoped(true, None, || {
            ground(testing::prepare(source), None, None).unwrap()
        })
    });
    let ((prior, skipped), reference) = producer::scoped(true, || {
        scoped(false, None, || {
            ground(testing::prepare(source), None, None).unwrap()
        })
    });
    assert_eq!(counts.runs, 4);
    assert_eq!(counts.published, 4);
    assert_eq!(counts, reference);
    assert_eq!(used, 1);
    assert_eq!(skipped, 0);
    assert!((0..shared.theory.nodes().len()).any(|index| {
        matches!(shared.theory.view().node(index).unwrap(), Node::And(row) if row.len() == 3)
    }));
    assert_eq!(shared.atoms.atoms(), prior.atoms.atoms());
    same_truth(&shared.theory, &prior.theory);
}

#[test]
fn unprofitable_sources_keep_the_original_graph() {
    let source = "{a;b}. x :- a. y :- b.";
    let (shared, used) = scoped(true, None, || {
        ground(testing::prepare(source), None, None).unwrap()
    });
    let (prior, _) = scoped(false, None, || {
        ground(testing::prepare(source), None, None).unwrap()
    });
    assert_eq!(used, 0);
    assert_eq!(shared.theory.nodes(), prior.theory.nodes());
    assert_eq!(shared.theory.operands(), prior.theory.operands());
    assert_eq!(shared.theory.roots(), prior.theory.roots());
    assert_eq!(shared.origins, prior.origins);
}

fn location(offset: u32) -> ProgramSite {
    ProgramSite::source(themelios_base::span::Location {
        source: SourceId::new(17),
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

#[derive(Clone, Copy)]
enum Shape {
    Pair,
    Repeated,
    Wide,
    Empty,
}
impl Shape {
    fn names(self) -> &'static [&'static str] {
        match self {
            Self::Pair | Self::Empty => &["a", "b", "h", "i", "j"],
            Self::Repeated => &["a", "b", "h", "i", "j", "k"],
            Self::Wide => &["a", "b", "c", "d", "e", "f", "h", "i"],
        }
    }
    fn head_start(self) -> usize {
        if matches!(self, Self::Wide) { 6 } else { 2 }
    }
    fn row(self, atoms: &[usize]) -> Vec<usize> {
        match self {
            Self::Pair => atoms[..2].to_vec(),
            Self::Repeated => vec![atoms[0], atoms[0], atoms[1]],
            Self::Wide => atoms[..6].to_vec(),
            Self::Empty => vec![],
        }
    }
}
fn raw(
    limits: &crate::FormulaLimits,
    repeated: bool,
    cancellation: Option<&Cancellation>,
) -> Attempt {
    fixture(
        limits,
        if repeated {
            Shape::Repeated
        } else {
            Shape::Pair
        },
        cancellation,
    )
}
fn fixture(
    limits: &crate::FormulaLimits,
    shape: Shape,
    cancellation: Option<&Cancellation>,
) -> Attempt {
    let observer = Peak::default();
    let profile = Profile::new(Some(&observer));
    let defaults = crate::FormulaLimits::default();
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
            for (index, name) in shape.names().iter().enumerate() {
                let atom = zetesis_core::Atom::new(
                    zetesis_core::Predicate::new(*name, 0).unwrap(),
                    vec![],
                )
                .unwrap();
                atoms.push(
                    builder
                        .atom_ref((&atom).into(), location(u32::try_from(index).unwrap()))
                        .unwrap(),
                );
            }
            let row = shape.row(&atoms);
            let body = if row.is_empty() {
                builder.group(&row, true, location(0)).unwrap()
            } else {
                builder.node(Node::And(&row), location(0)).unwrap()
            };
            for (index, &head) in atoms.iter().enumerate().skip(shape.head_start()) {
                let rule = RuleIr {
                    head: HeadIr::Normal(None),
                    body: vec![],
                    body_variables: 0,
                    bindings: None,
                    variables: 0,
                    location: location(10),
                    origins: vec![location(10), location(20 + u32::try_from(index).unwrap())],
                };
                builder.producer(head, body, &rule).unwrap();
            }
            builder.limits = limits;
            let workspace = builder.counters.workspace_bytes();
            let result = profile.phase(GroundingPhase::SupportGuards, None, || {
                builder.support_guards()
            });
            assert_eq!(
                builder.counters.workspace_bytes(),
                workspace,
                "plan scratch must release its sole leases on every outcome"
            );
            let work = builder.counters.accounting.work;
            let result = result.map(|()| {
                let theory = Theory::new(
                    atoms.len(),
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

#[test]
fn repeated_children_preserve_frozen_truth() {
    let (shared, used) = scoped(true, None, || {
        raw(&crate::FormulaLimits::default(), true, None)
    });
    let (prior, _) = scoped(false, None, || {
        raw(&crate::FormulaLimits::default(), true, None)
    });
    assert_eq!(used, 1);
    let shared = shared.result.unwrap().0;
    let prior = prior.result.unwrap().0;
    same_truth(&shared, &prior);
    assert!((0..shared.nodes().len()).any(
        |index| matches!(shared.view().node(index).unwrap(), Node::Or(row) if row.len() == 8)
    ));
}

#[test]
fn transposed_origins_union_every_head() {
    let attempt = raw(&crate::FormulaLimits::default(), false, None);
    let (_, origins) = attempt.result.unwrap();
    // Unsupported a/b stay first. Both condition groups receive every selected
    // head's first occurrence and both written origins, sorted without repeats.
    let expected = vec![
        location(2),
        location(3),
        location(4),
        location(10),
        location(22),
        location(23),
        location(24),
    ];
    assert_eq!(
        origins,
        vec![
            vec![location(0)],
            vec![location(1)],
            expected.clone(),
            expected
        ]
    );
}

#[test]
fn transposed_storage_limits_are_inclusive() {
    let defaults = crate::FormulaLimits::default();
    let baseline = raw(&defaults, false, None);
    let (theory, origins) = baseline.result.unwrap();
    let origin_count = origins.iter().map(Vec::len).sum::<usize>();
    for (resource, exact) in [
        (FormulaResource::Nodes, theory.nodes().len()),
        (FormulaResource::Operands, theory.parts().occurrences()),
        (FormulaResource::Roots, theory.roots().len()),
        (FormulaResource::Origins, origin_count),
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
            let attempt = raw(&limits, false, None);
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
fn every_guard_work_cutoff_remains_a_refusal() {
    let defaults = crate::FormulaLimits::default();
    let exact = raw(&defaults, false, None).work;
    for cap in 0..=exact {
        let limits = crate::FormulaLimits {
            max_work: cap,
            ..defaults
        };
        let attempt = raw(&limits, false, None);
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
fn guard_cancellation_preserves_typed_interruption() {
    for point in [Point::Count, Point::Fill, Point::Evidence, Point::Publish] {
        let cancellation = Cancellation::default();
        let (attempt, _) = scoped(true, Some((point, cancellation.clone())), || {
            raw(&crate::FormulaLimits::default(), false, Some(&cancellation))
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
fn impossible_prices_allocate_no_plan_workspace() {
    let limits = crate::FormulaLimits {
        max_support_bytes: 0,
        ..crate::FormulaLimits::default()
    };
    // Six direct conditions for each of two heads cannot fit the old twelve
    // guard operands, even with perfect sharing. The source owner normalizes
    // empty conjunctions to VERUM; those producers retain their original path.
    for shape in [Shape::Wide, Shape::Empty] {
        let (shared, used) = scoped(true, None, || fixture(&limits, shape, None));
        let (prior, _) = scoped(false, None, || fixture(&limits, shape, None));
        assert_eq!(used, 0);
        let (shared, shared_origins) = shared.result.unwrap();
        let (prior, prior_origins) = prior.result.unwrap();
        assert_eq!(shared.nodes(), prior.nodes());
        assert_eq!(shared.operands(), prior.operands());
        assert_eq!(shared.roots(), prior.roots());
        assert_eq!(shared_origins, prior_origins);
    }
}
