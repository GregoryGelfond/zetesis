//! Live external owners participate in admission without entering retained receipts.

use std::cell::Cell;

use super::{atom, insert, location};
use crate::formula::Preparation;
use crate::formula_ir::Prepared;
use crate::formula_support::producers::ProducerPlan;
use crate::formula_support::{Buffer, Computation, Counters, Support, SupportCatalog, testing};
use crate::grounding_observer::Profile;
use crate::{
    FormulaFailure, FormulaLimits, FormulaResource, GroundingObserver, GroundingOutcome,
    GroundingPhase, GroundingWork,
};

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

fn measure<T>(
    counters: &mut Counters,
    action: impl FnOnce(&mut Counters) -> Result<T, FormulaFailure>,
) -> (Result<T, FormulaFailure>, usize) {
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    let previous = std::mem::replace(&mut counters.observed, profile.work());
    let result = profile.phase(GroundingPhase::SupportCompletion, None, || action(counters));
    counters.observed = previous;
    let peak = usize::try_from(observer.0.get().support_peak_bytes.unwrap()).unwrap();
    (result, peak)
}

/// Keep real allocated metadata live after the temporary query view is gone.
fn workspace(catalog: &mut SupportCatalog, counters: &mut Counters) -> Buffer<u64> {
    let limits = FormulaLimits::default();
    let (relations, mut append) = catalog.split(&limits, counters, location()).unwrap();
    let support = Support::indexed(&relations, &limits, counters, location()).unwrap();
    let computation = Computation::new(&mut append, &support);
    let mut buffer = Buffer::new(&computation, &limits, counters, location()).unwrap();
    buffer
        .resize(64, 0, &computation, &limits, counters, location())
        .unwrap();
    buffer
}

fn producer() -> (Prepared, SupportCatalog, Counters) {
    let Preparation {
        program,
        catalog,
        accounting,
        ..
    } = testing::prepare("p(1).q(X):-p(X).r(X):-q(X).s(X):-r(X).t(X):-s(X).");
    let counters = Counters::resume(accounting, crate::grounding_observer::Work::default());
    (program, catalog, counters)
}

#[test]
fn producer_peak_includes_live_workspace_without_retaining_it() {
    let (prepared, mut catalog, mut counters) = producer();
    let limits = FormulaLimits::default();
    let (plain, plain_peak) = measure(&mut counters, |counters| {
        ProducerPlan::prepare(&prepared, &catalog, &limits, counters, location())
    });
    let retained = plain.unwrap().unwrap().bytes();
    let original = catalog.bytes(location()).unwrap();
    let previous = counters.accounting.workspace.bytes();
    let buffer = workspace(&mut catalog, &mut counters);
    let added = counters.accounting.workspace.bytes() - previous;
    assert!(added > 0);
    let (with_workspace, peak) = measure(&mut counters, |counters| {
        ProducerPlan::prepare(&prepared, &catalog, &limits, counters, location())
    });
    assert_eq!(with_workspace.unwrap().unwrap().bytes(), retained);
    assert_eq!(peak, plain_peak + added);
    catalog.prepared_bytes(retained);
    assert_eq!(catalog.bytes(location()).unwrap(), original + retained);
    drop(buffer);
    assert_eq!(counters.accounting.workspace.bytes(), previous);
}

#[test]
fn producer_scratch_peak_is_an_inclusive_composed_limit() {
    let (prepared, catalog, mut counters) = producer();
    let limits = FormulaLimits::default();
    let (plan, peak) = measure(&mut counters, |counters| {
        ProducerPlan::prepare(&prepared, &catalog, &limits, counters, location())
    });
    let retained = plan.unwrap().unwrap().bytes();
    let final_bytes =
        catalog.bytes(location()).unwrap() + counters.accounting.workspace.bytes() + retained;
    assert!(
        peak > final_bytes,
        "this fixture must exercise released preparation scratch"
    );
    let bounded = FormulaLimits {
        max_support_bytes: peak,
        ..limits
    };
    let (exact, actual) = measure(&mut counters, |counters| {
        ProducerPlan::prepare(&prepared, &catalog, &bounded, counters, location())
    });
    assert!(exact.unwrap().is_some());
    assert_eq!(actual, peak);
    let below = FormulaLimits {
        max_support_bytes: peak - 1,
        ..bounded
    };
    let (refused, observed_peak) = measure(&mut counters, |counters| {
        ProducerPlan::prepare(&prepared, &catalog, &below, counters, location())
    });
    assert!(matches!(refused, Err(FormulaFailure::Limit {
        resource: FormulaResource::SupportBytes, observed, limit, ..
    }) if observed == peak as u128 && limit == (peak - 1) as u128));
    assert!(
        observed_peak <= below.max_support_bytes,
        "a refused proposal is not an allocation"
    );
}

#[test]
fn snapshot_workspace_is_live_but_not_retained_in_its_receipt() {
    let mut catalog = SupportCatalog::default();
    insert(&mut catalog, &atom(&[1, 2]));
    let mut counters = Counters::default();
    let limits = FormulaLimits::default();
    let (plain, plain_peak) = measure(&mut counters, |counters| {
        catalog.snapshot(&limits, counters, location())
    });
    let receipt = plain.unwrap().bytes;
    let buffer = workspace(&mut catalog, &mut counters);
    let extra = counters.accounting.workspace.bytes();
    let bounded = FormulaLimits {
        max_support_bytes: plain_peak + extra,
        ..limits
    };
    let (snapshot, peak) = measure(&mut counters, |counters| {
        catalog.snapshot(&bounded, counters, location())
    });
    assert_eq!(snapshot.unwrap().bytes, receipt);
    assert_eq!(peak, plain_peak + extra);
    let below = FormulaLimits {
        max_support_bytes: peak - 1,
        ..bounded
    };
    assert!(
        matches!(catalog.snapshot(&below, &mut counters, location()),
        Err(FormulaFailure::Limit { resource: FormulaResource::SupportBytes, observed, limit, .. })
        if observed == peak as u128 && limit == (peak - 1) as u128)
    );
    drop(buffer);
    let exact = FormulaLimits {
        max_support_bytes: plain_peak,
        ..limits
    };
    assert!(catalog.snapshot(&exact, &mut counters, location()).is_ok());
}

fn pending(with_workspace: bool) -> (SupportCatalog, Counters, Option<Buffer<u64>>) {
    let mut catalog = SupportCatalog::default();
    insert(&mut catalog, &atom(&[1]));
    let mut counters = Counters::default();
    let buffer = with_workspace.then(|| workspace(&mut catalog, &mut counters));
    let limits = FormulaLimits::default();
    {
        let (relations, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
        let support = Support::indexed(&relations, &limits, &counters, location()).unwrap();
        let mut computation = Computation::new(&mut append, &support);
        let next = atom(&[2]);
        let source = computation
            .atom_ref((&next).into(), &limits, &mut counters, location())
            .unwrap();
        computation
            .support(&source, &limits, &mut counters, location())
            .unwrap();
    }
    (catalog, counters, buffer)
}

#[test]
fn publication_workspace_is_live_but_not_retained_in_the_catalog() {
    let limits = FormulaLimits::default();
    let (mut plain, mut plain_counters, _) = pending(false);
    let (result, plain_peak) = measure(&mut plain_counters, |counters| {
        plain.publish(&limits, counters, location())
    });
    result.unwrap();
    let (mut catalog, mut counters, _buffer) = pending(true);
    let extra = counters.accounting.workspace.bytes();
    let bounded = FormulaLimits {
        max_support_bytes: plain_peak + extra,
        ..limits
    };
    let (result, peak) = measure(&mut counters, |counters| {
        catalog.publish(&bounded, counters, location())
    });
    result.unwrap();
    assert_eq!(peak, plain_peak + extra);
    assert_eq!(
        catalog.bytes(location()).unwrap(),
        plain.bytes(location()).unwrap()
    );
    let snapshot = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    assert_eq!(snapshot.row_count(atom(&[0]).predicate()), 2);
}

#[test]
fn publication_refuses_below_its_composed_peak() {
    let limits = FormulaLimits::default();
    let (mut admitted, mut counters, _buffer) = pending(true);
    let (result, peak) = measure(&mut counters, |counters| {
        admitted.publish(&limits, counters, location())
    });
    result.unwrap();
    let (mut refused, mut counters, _buffer) = pending(true);
    let below = FormulaLimits {
        max_support_bytes: peak - 1,
        ..limits
    };
    let (result, observed_peak) = measure(&mut counters, |counters| {
        refused.publish(&below, counters, location())
    });
    assert!(matches!(result, Err(FormulaFailure::Limit {
        resource: FormulaResource::SupportBytes, observed, limit, ..
    }) if observed == peak as u128 && limit == (peak - 1) as u128));
    assert!(observed_peak <= below.max_support_bytes);
}

#[test]
fn external_storage_requirement_remains_wide() {
    let limits = FormulaLimits {
        max_support_bytes: usize::MAX,
        ..FormulaLimits::default()
    };
    let counters = Counters::default();
    let mut memory = super::Memory::new(1, usize::MAX as u128, &limits, &counters, location());
    assert!(matches!(memory.add(0), Err(FormulaFailure::Limit {
        resource: FormulaResource::SupportBytes, observed, limit, ..
    }) if observed == usize::MAX as u128 + 1 && limit == usize::MAX as u128));
}

fn probe(
    catalog: &SupportCatalog,
    counters: &mut Counters,
    limits: &FormulaLimits,
) -> (Result<Option<usize>, FormulaFailure>, usize) {
    let setup = FormulaLimits::default();
    let relations = catalog.snapshot(&setup, counters, location()).unwrap();
    let support = Support::indexed(&relations, &setup, counters, location()).unwrap();
    let pattern = zetesis_core::AtomPattern::new(
        zetesis_core::Predicate::new("row", 1).unwrap(),
        vec![zetesis_core::Term::Constant(zetesis_core::Value::Number(1))],
    )
    .unwrap();
    measure(counters, |counters| {
        support
            .probe(
                (&pattern).into(),
                (&[] as &[Option<zetesis_core::catalog::TermRef<'_>>]).into(),
                limits,
                counters,
                location(),
            )
            .map(|rows| rows.map(<[usize]>::len))
    })
}

#[test]
fn probe_counts_explicit_workspace_once_at_the_exact_limit() {
    let mut catalog = SupportCatalog::default();
    insert(&mut catalog, &atom(&[1]));
    let mut counters = Counters::default();
    let limits = FormulaLimits::default();
    let (plain, plain_peak) = probe(&catalog, &mut counters, &limits);
    assert_eq!(plain.unwrap(), Some(1));
    let _buffer = workspace(&mut catalog, &mut counters);
    let external = counters.accounting.workspace.bytes();
    let (observed, peak) = probe(&catalog, &mut counters, &limits);
    assert_eq!(observed.unwrap(), Some(1));
    assert_eq!(peak, plain_peak + external);
    let exact = FormulaLimits {
        max_support_bytes: peak,
        ..limits
    };
    assert_eq!(probe(&catalog, &mut counters, &exact).0.unwrap(), Some(1));
}
