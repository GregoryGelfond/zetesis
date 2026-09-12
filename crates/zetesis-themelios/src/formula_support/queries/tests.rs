//! Distinguishing workspace ownership, live masks and refused work prefixes.

use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Span};
use zetesis_core::{Atom, Predicate};

use super::*;
use crate::grounding_observer::Profile;
use crate::{GroundingObserver, GroundingOutcome, GroundingPhase, GroundingWork};

fn location() -> Location {
    Location {
        source: SourceId::new(0),
        span: Span::empty(ByteOffset::new(0)),
    }
}

fn catalog() -> super::super::SupportCatalog {
    let mut catalog = super::super::SupportCatalog::default();
    for value in 0..70 {
        let atom = Atom::new(
            Predicate::new("r", 2).unwrap(),
            vec![Value::Number(value % 7), Value::Number(value)],
        )
        .unwrap();
        catalog = catalog
            .insert(
                atom,
                &FormulaLimits::default(),
                &mut Counters::default(),
                location(),
            )
            .unwrap();
    }
    catalog
}

fn pattern(alias: bool) -> AtomPattern {
    AtomPattern::new(
        Predicate::new("r", 2).unwrap(),
        vec![Term::Variable(0), Term::Variable(usize::from(!alias))],
    )
    .unwrap()
}

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
        _: Option<Location>,
        _: GroundingOutcome,
        work: GroundingWork,
    ) {
        self.0.set(work);
    }
}

#[test]
fn a_selection_survives_new_cache_entries() {
    let catalog = catalog();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let support = Support::completed(
        &relations,
        JoinStrategy::Table,
        &limits,
        &counters,
        location(),
    )
    .unwrap();
    let flat = pattern(false);
    let alias = pattern(true);
    let first = support
        .select(
            PositivePattern::Flat(&flat),
            &[Some(Value::Number(1)), None],
            &limits,
            &mut counters,
            location(),
        )
        .unwrap()
        .unwrap();
    let second = support
        .select(
            PositivePattern::Flat(&alias),
            &[None],
            &limits,
            &mut counters,
            location(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(support.tables.as_ref().unwrap().indices.borrow().len(), 2);
    assert_eq!(
        first.selection.rows().collect::<Vec<_>>(),
        (1..70).step_by(7).collect::<Vec<_>>()
    );
    assert_eq!(
        second.selection.rows().collect::<Vec<_>>(),
        (0..7).collect::<Vec<_>>()
    );
    drop(second);
    let restored = support
        .select(
            PositivePattern::Flat(&flat),
            &[None, None],
            &limits,
            &mut counters,
            location(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        restored.selection.rows().collect::<Vec<_>>(),
        (0..70).collect::<Vec<_>>()
    );
    assert_eq!(
        first.selection.rows().collect::<Vec<_>>(),
        (1..70).step_by(7).collect::<Vec<_>>()
    );
}

#[test]
fn every_simultaneous_mask_consumes_live_capacity() {
    let catalog = catalog();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let support = Support::completed(
        &relations,
        JoinStrategy::Table,
        &limits,
        &counters,
        location(),
    )
    .unwrap();
    let pattern = pattern(false);
    drop(
        support
            .select(
                PositivePattern::Flat(&pattern),
                &[None, None],
                &limits,
                &mut counters,
                location(),
            )
            .unwrap(),
    );
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    let mut counters = Counters::observed(profile.work());
    let baseline = support.live.get();
    let first = profile
        .phase(GroundingPhase::RuleInstantiation, None, || {
            support.select(
                PositivePattern::Flat(&pattern),
                &[None, None],
                &limits,
                &mut counters,
                location(),
            )
        })
        .unwrap()
        .unwrap();
    assert_eq!(support.live.get(), baseline + first.bytes);
    let peak = usize::try_from(observer.0.get().support_peak_bytes.unwrap()).unwrap();
    let bounded = FormulaLimits {
        max_support_bytes: peak,
        ..limits
    };
    let failed = support.select(
        PositivePattern::Flat(&pattern),
        &[None, None],
        &bounded,
        &mut counters,
        location(),
    );
    assert!(
        matches!(failed, Err(FormulaFailure::Limit { resource: FormulaResource::SupportBytes, observed, limit, .. }) if observed > limit && limit == peak as u128)
    );
    assert_eq!(support.live.get(), baseline + first.bytes);
    assert_eq!(first.selection.rows().count(), 70);
    drop(first);
    assert_eq!(support.live.get(), baseline);
    assert!(
        support
            .select(
                PositivePattern::Flat(&pattern),
                &[None, None],
                &bounded,
                &mut counters,
                location()
            )
            .unwrap()
            .is_some()
    );
}

#[test]
fn failed_selection_retains_its_charged_work_prefix() {
    let catalog = catalog();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let support = Support::completed(
        &relations,
        JoinStrategy::Table,
        &limits,
        &counters,
        location(),
    )
    .unwrap();
    let pattern = pattern(false);
    let values = [Some(Value::Number(1)), None];
    drop(
        support
            .select(
                PositivePattern::Flat(&pattern),
                &values,
                &limits,
                &mut counters,
                location(),
            )
            .unwrap(),
    );
    let before = counters.work;
    drop(
        support
            .select(
                PositivePattern::Flat(&pattern),
                &values,
                &limits,
                &mut counters,
                location(),
            )
            .unwrap(),
    );
    let complete = counters.work - before;
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    let mut checked = Counters::observed(profile.work());
    checked.work = counters.work;
    let bounded = FormulaLimits {
        max_work: checked.work + complete - 1,
        ..limits
    };
    let live = support.live.get();
    let failed = profile.phase(GroundingPhase::RuleInstantiation, None, || {
        support.select(
            PositivePattern::Flat(&pattern),
            &values,
            &bounded,
            &mut checked,
            location(),
        )
    });
    assert!(
        matches!(failed, Err(FormulaFailure::Limit { resource: FormulaResource::Work, observed, limit, .. }) if observed > limit)
    );
    assert!(observer.0.get().table_query_work.unwrap() > 0);
    assert!(checked.work > counters.work);
    assert!(checked.work <= bounded.max_work);
    assert_eq!(support.live.get(), live);
}

#[test]
fn refused_capacity_is_not_a_retained_peak() {
    let catalog = catalog();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let support = Support::completed(
        &relations,
        JoinStrategy::Table,
        &limits,
        &counters,
        location(),
    )
    .unwrap();
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    let mut checked = Counters::observed(profile.work());
    let bounded = FormulaLimits {
        max_support_bytes: support.live.get(),
        ..limits
    };
    let pattern = pattern(false);
    let failed = profile.phase(GroundingPhase::RuleInstantiation, None, || {
        support.select(
            PositivePattern::Flat(&pattern),
            &[None, None],
            &bounded,
            &mut checked,
            location(),
        )
    });
    assert!(matches!(
        failed,
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            ..
        })
    ));
    assert_eq!(
        observer.0.get().support_peak_bytes,
        Some(u64::try_from(support.live.get()).unwrap())
    );
}

#[test]
fn workspace_size_overflow_is_a_typed_failure() {
    let mut relations = Relations::default();
    relations.bytes = usize::MAX;
    let limits = FormulaLimits {
        max_support_bytes: usize::MAX,
        ..FormulaLimits::default()
    };
    assert!(matches!(
        Support::completed(
            &relations,
            JoinStrategy::Table,
            &limits,
            &Counters::default(),
            location()
        ),
        Err(FormulaFailure::SupportTable {
            error: table::Failure {
                cause: Cause::Overflow,
                ..
            },
            ..
        })
    ));
}
