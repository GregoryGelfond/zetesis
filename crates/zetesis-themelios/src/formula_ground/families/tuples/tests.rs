//! Exact live payload release after a later element lacks a replay certificate.
use super::*;
use crate::formula_support::testing::Fixture;
use crate::formula_support::{Counters, components::Term};
use crate::grounding_observer::Profile;
use crate::test_support::location;
use crate::{FormulaLimits, GroundingObserver, GroundingOutcome, GroundingPhase, ProgramSite};

#[derive(Default)]
struct Peak(std::cell::Cell<Option<u64>>);
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
        work: crate::GroundingWork,
    ) {
        self.0.set(work.support_peak_bytes);
    }
}

fn live_bytes(computation: &Computation<'_, '_>) -> usize {
    let limits = FormulaLimits::default();
    limits.max_support_bytes
        - computation
            .allowance(&computation.lease(), &limits, location())
            .unwrap()
}

fn singleton_tuple(value: i32, context: Context<'_, &mut Computation<'_, '_>>) -> TermKey {
    let Context { computation, work } = context;
    let GroundingWork {
        limits,
        counters,
        location,
    } = work;
    let value = computation
        .number(value, limits, counters, location)
        .unwrap();
    let mut frame = Binding::new(computation, limits, counters, location).unwrap();
    frame
        .extend_scope(1, computation, limits, counters, location)
        .unwrap();
    frame.set(0, &value, limits, counters, location).unwrap();
    crate::formula_assignment::tuple(
        &[Term::Variable(0)],
        &frame,
        computation,
        limits,
        counters,
        location,
    )
    .unwrap()
}

fn discard_populated(
    trace: &mut Trace<'_, '_, '_>,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> usize {
    let draft = trace.draft.as_ref().unwrap();
    assert_eq!(draft.occurrences.len(), 1);
    assert_eq!(draft.segments.len(), 1);
    assert_eq!(draft.segments.slice()[0].end, 1);
    let released = draft.occurrences.capacity() * size_of::<usize>()
        + draft.segments.capacity() * size_of::<Segment<'_>>();
    assert!(released > 0);
    let before_discard = live_bytes(computation);
    let work = counters.accounting.work;
    trace
        .begin(
            None,
            1,
            false,
            Context::new(computation, limits, counters, location()),
        )
        .unwrap();
    assert_eq!(counters.accounting.work, work + 1);
    assert_eq!(live_bytes(computation) + released, before_discard);
    assert!(trace.draft.is_none());
    assert!(!trace.retaining());
    assert_eq!(trace.lease.bytes(), size_of::<Trace<'_, '_, '_>>());
    before_discard
}

fn assert_inactive(
    mut trace: Trace<'_, '_, '_>,
    first: &TermKey,
    second: &TermKey,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) {
    let inactive_live = live_bytes(computation);
    let work = counters.accounting.work;
    trace
        .row(
            second,
            Context::new(computation, limits, counters, location()),
        )
        .unwrap();
    trace
        .begin(
            None,
            1,
            true,
            Context::new(computation, limits, counters, location()),
        )
        .unwrap();
    trace
        .end(Context::new(computation, limits, counters, location()))
        .unwrap();
    assert_eq!(counters.accounting.work, work + 3);
    assert!(trace.draft.is_none());
    assert_eq!(live_bytes(computation), inactive_live);
    let terms = trace.owner.terms.borrow();
    assert_eq!(
        terms
            .find(first, computation, limits, counters, location())
            .unwrap(),
        Some(0)
    );
    assert_eq!(
        terms
            .find(second, computation, limits, counters, location())
            .unwrap(),
        None
    );
    drop(terms);
    drop(trace);
    assert_eq!(
        live_bytes(computation) + size_of::<Trace<'_, '_, '_>>(),
        inactive_live
    );
}

#[test]
fn missing_later_plan_releases_trace_payload_and_stops_coordinate_growth() {
    Fixture::default().with(location(), |_support, computation, original| {
        let limits = FormulaLimits::default();
        let observer = Peak::default();
        let profile = Profile::new(Some(&observer));
        let mut counters =
            Counters::resume(std::mem::take(&mut original.accounting), profile.work());
        let before_discard = profile
            .phase(GroundingPhase::RuleInstantiation, None, || {
                let first = singleton_tuple(
                    1,
                    Context::new(&mut *computation, &limits, &mut counters, location()),
                );
                let second = singleton_tuple(
                    2,
                    Context::new(&mut *computation, &limits, &mut counters, location()),
                );
                let owner = Workspace::new(Context::new(
                    &*computation,
                    &limits,
                    &mut counters,
                    location(),
                ))?;
                let mut trace = Trace::new(
                    &owner,
                    Context::new(&*computation, &limits, &mut counters, location()),
                )?;
                // An empty condition is a certified one-row segment. A later
                // nonempty condition without a shared plan then disables replay.
                trace.begin(
                    None,
                    1,
                    true,
                    Context::new(&*computation, &limits, &mut counters, location()),
                )?;
                trace.row(
                    &first,
                    Context::new(&*computation, &limits, &mut counters, location()),
                )?;
                trace.end(Context::new(
                    &*computation,
                    &limits,
                    &mut counters,
                    location(),
                ))?;
                let before_discard =
                    discard_populated(&mut trace, computation, &limits, &mut counters);
                assert_inactive(trace, &first, &second, computation, &limits, &mut counters);
                Ok::<_, FormulaFailure>(before_discard)
            })
            .unwrap();
        assert!(
            observer.0.get().unwrap() >= before_discard as u64,
            "releasing payload does not erase the earlier actual peak"
        );
    });
}
