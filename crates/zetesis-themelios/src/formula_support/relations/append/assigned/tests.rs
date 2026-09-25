use std::sync::atomic::Ordering;

use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Span};

use super::*;
use crate::FormulaResource;
use crate::formula_support::SupportCatalog;
use zetesis_core::{Atom, Predicate};

fn location() -> Location {
    Location {
        source: SourceId::new(0),
        span: Span::empty(ByteOffset::new(0)),
    }
}

fn atom() -> Atom {
    Atom::new(Predicate::new("discovered", 0).unwrap(), Vec::new()).unwrap()
}

#[test]
fn discovery_does_not_establish_support() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let (_, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    let ingress = atom();
    let source = append
        .discover_ref((&ingress).into(), 0, &limits, &mut counters, location())
        .unwrap();
    assert!(!append.is_supported(source.position));
    assert!(append.is_empty());
    assert_eq!(
        append
            .source_at(
                &source.scope,
                source.position,
                &limits,
                &mut counters,
                location()
            )
            .unwrap(),
        AtomRef::from(&ingress)
    );
}

#[test]
fn an_existing_identity_can_enter_support_once() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let ingress = atom();
    {
        let (_, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
        append
            .discover_ref((&ingress).into(), 0, &limits, &mut counters, location())
            .unwrap();
        // The legacy producer entry must distinguish identity from membership too.
        append
            .atom((&ingress).into(), &limits, &mut counters, location())
            .unwrap();
        append
            .atom((&ingress).into(), &limits, &mut counters, location())
            .unwrap();
        assert_eq!(append.pending.len(), 1);
        assert_eq!(append.owner.len(), 1);
    }
    catalog.publish(&limits, &mut counters, location()).unwrap();
    let (_, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    append
        .atom((&ingress).into(), &limits, &mut counters, location())
        .unwrap();
    assert!(append.is_empty());
}

#[test]
fn foreign_discoveries_cannot_select_support() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut left = SupportCatalog::default();
    let mut right = SupportCatalog::default();
    let ingress = atom();
    let (_, mut append) = left.split(&limits, &mut counters, location()).unwrap();
    let foreign = append
        .discover_ref((&ingress).into(), 0, &limits, &mut counters, location())
        .unwrap();
    let (_, mut other) = right.split(&limits, &mut counters, location()).unwrap();
    other
        .discover_ref((&ingress).into(), 0, &limits, &mut counters, location())
        .unwrap();
    assert!(matches!(
        other.retain(&foreign, 0, &limits, &mut counters, location()),
        Err(FormulaFailure::SupportRelation {
            error: Failure::Owner,
            ..
        })
    ));
    assert!(other.is_empty());
}

#[test]
fn every_stopped_selection_can_retry_once() {
    let limits = FormulaLimits::default();
    let ingress = atom();
    let work = {
        let mut counters = Counters::default();
        let mut catalog = SupportCatalog::default();
        let (_, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
        let source = append
            .discover_ref((&ingress).into(), 0, &limits, &mut counters, location())
            .unwrap();
        let before = counters.accounting.work;
        append
            .retain(&source, 0, &limits, &mut counters, location())
            .unwrap();
        counters.accounting.work - before
    };
    assert!(work > 1);
    for cutoff in 0..work {
        let mut counters = Counters::default();
        let mut catalog = SupportCatalog::default();
        let (_, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
        let source = append
            .discover_ref((&ingress).into(), 0, &limits, &mut counters, location())
            .unwrap();
        let stopped = FormulaLimits {
            max_work: counters.accounting.work + cutoff,
            ..limits
        };
        assert!(matches!(
            append.retain(&source, 0, &stopped, &mut counters, location()),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            })
        ));
        assert!(append.is_empty());
        assert!(!append.is_supported(source.position));
        let actual = append.owner.storage_bytes() - append.base.owner
            + ((append.pending.capacity() * size_of::<usize>() - append.base.pending)
                + (append.supported.capacity() * size_of::<u64>() - append.base.supported))
                as u128;
        assert_eq!(append.growth.load(Ordering::Relaxed) as u128, actual);
        append
            .retain(&source, 0, &limits, &mut counters, location())
            .unwrap();
        append
            .retain(&source, 0, &limits, &mut counters, location())
            .unwrap();
        assert_eq!(append.pending.len(), 1);
    }
}
