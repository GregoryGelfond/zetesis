//! Refusals after policy buffers are populated retain work but publish no plan.

use std::cell::Cell;

use super::{Context, Counters, FormulaFailure, FormulaResource, policy, prepare, selection};
use crate::grounding_observer::Profile;
use crate::{FormulaLimits, GroundingObserver, GroundingOutcome, GroundingPhase, GroundingWork};

#[derive(Default)]
struct Observer(Cell<GroundingWork>);

impl GroundingObserver for Observer {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        _: GroundingPhase,
        _: Option<themelios_base::span::Location>,
        _: GroundingOutcome,
        work: GroundingWork,
    ) {
        self.0.set(work);
    }
}

struct Capture {
    result: Result<usize, FormulaFailure>,
    initial_work: u64,
    work: u64,
    peak: u64,
    certified: Vec<u8>,
    selected: Vec<u8>,
}

fn capture(limits: &FormulaLimits) -> Capture {
    let mut prepared = prepare("closed(7). d(2). d(X):-p(X). p(1). z:-p(1).");
    let source = prepared.program.analyzed.clone();
    let rules = prepared.program.rules.as_ptr();
    let expansion = prepared.budget.usage();
    let analysis = zetesis_domain::terminal::analyze(
        &prepared.program.analyzed,
        zetesis_domain::Limits::default(),
    );
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    let mut counters = Counters::resume(std::mem::take(&mut prepared.accounting), profile.work());
    let admission = prepared
        .catalog
        .component_admission(&prepared.limits, &mut counters, prepared.location)
        .unwrap();
    let components = admission
        .components(&prepared.limits, &mut counters, prepared.location)
        .unwrap();
    let mut context = Context {
        admission: &admission,
        components,
        limits: &prepared.limits,
        counters: &mut counters,
        location: prepared.location,
    };
    let mut selected = selection::select(&prepared.program, analysis.definitions(), &mut context)
        .unwrap()
        .expect("the complete certificate precedes the bounded policy");
    let certified = selected.values.clone();
    let workspace = context.counters.workspace_bytes();
    let initial_work = context.counters.accounting.work;
    context.limits = limits;
    let result = profile.phase(GroundingPhase::DomainAnalysis, None, || {
        policy::select(
            &prepared.program,
            analysis.definitions(),
            &mut selected.values,
            &mut context,
        )
        .map(|definitions| definitions.values.len())
    });
    // Neither an output buffer nor either private policy buffer may leak after
    // its result is consumed. A partly refined mask never rewrites its owners.
    assert_eq!(context.counters.workspace_bytes(), workspace);
    assert_eq!(prepared.program.rules.as_ptr(), rules);
    assert_eq!(prepared.program.analyzed, source);
    assert_eq!(prepared.budget.usage(), expansion);
    Capture {
        result,
        initial_work,
        work: context.counters.accounting.work,
        peak: observer.0.get().support_peak_bytes.unwrap(),
        certified,
        selected: selected.into_values(),
    }
}

#[test]
fn late_policy_refusals_release_populated_scratch() {
    let complete = capture(&FormulaLimits::default());
    assert!(matches!(complete.result, Ok(2)));
    assert!(complete.work > complete.initial_work);
    assert_ne!(complete.selected, complete.certified);

    // Stop at the final successful policy operation. Refining the mask happens
    // only after both the signature and source-carrier buffers are populated.
    let work_limit = complete.work.checked_sub(1).unwrap();
    let work = capture(&FormulaLimits {
        max_work: work_limit,
        ..FormulaLimits::default()
    });
    assert!(matches!(work.result, Err(FormulaFailure::Limit {
        resource: FormulaResource::Work, observed, limit, ..
    }) if limit == u128::from(work_limit) && observed == limit + 1));
    assert_eq!(work.initial_work, complete.initial_work);
    assert_eq!(work.work, work_limit);
    assert!(work.work > work.initial_work);
    assert_eq!(work.peak, complete.peak);
    assert_ne!(work.selected, work.certified);
    assert_ne!(work.selected, complete.selected);

    // This fixture's last source-carrier growth establishes the measured peak.
    // One byte less reaches that growth with earlier carriers already retained.
    let storage_limit = usize::try_from(complete.peak.checked_sub(1).unwrap()).unwrap();
    let storage = capture(&FormulaLimits {
        max_support_bytes: storage_limit,
        ..FormulaLimits::default()
    });
    assert!(matches!(storage.result, Err(FormulaFailure::Limit {
        resource: FormulaResource::SupportBytes, observed, limit, ..
    }) if limit == storage_limit as u128 && observed == u128::from(complete.peak)));
    assert_eq!(storage.initial_work, complete.initial_work);
    assert!(storage.work > storage.initial_work);
    assert!(storage.work < complete.work);
    assert!(storage.peak < complete.peak);
    assert_eq!(storage.selected, storage.certified);
}
