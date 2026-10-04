use crate::ProgramSite;
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Span};
use zetesis_core::{Atom, Predicate};

use super::*;
use crate::expansion::Budget;
use crate::formula_support::{Computation, Counters, Support, SupportCatalog};
use crate::{ExpansionLimits, FormulaLimits};

#[test]
fn round_reservation_counts_the_live_source_owner() {
    let limits = FormulaLimits::default();
    let mut budget = Budget::new(ExpansionLimits::default(), 0);
    let mut counters = Counters::default();
    let location = ProgramSite::source(themelios_base::span::Location {
        source: SourceId::new(0),
        span: Span::empty(ByteOffset::new(0)),
    });
    let mut catalog = SupportCatalog::default();
    let (relations, mut append) = catalog.split(&limits, &mut counters, location).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location).unwrap();
    let mut computation = Computation::new(&mut append, &support);
    let mut context = Context {
        computation: &mut computation,
        limits: &limits,
        budget: &mut budget,
        counters: &mut counters,
        location,
    };
    let atom = Atom::new(Predicate::new("p", 0).unwrap(), vec![]).unwrap();
    let mut owner = SourceEligibility::new(&context).unwrap();
    owner.round = Some(Round::new(&context));
    owner
        .retain_atom((&atom).into(), Activity::Optional, 0, &mut context)
        .unwrap();
    owner.publish_predicate(&mut context).unwrap();
    let selected = owner
        .atoms
        .source(0, &limits, context.counters, location)
        .unwrap();
    owner.round = Some(Round::new(&context));
    let unused = context.computation.lease();
    let current = limits.max_support_bytes
        - context
            .computation
            .allowance(&unused, &limits, location)
            .unwrap();
    // Stage the admitted source identity directly: re-entering atom admission
    // would test its prepared-entry scratch before this round reservation.
    // The classification cell coexists with every live source/selection byte.
    let required = current + size_of::<Option<Activity>>();
    let tight = FormulaLimits {
        max_support_bytes: required - 1,
        ..limits
    };
    let mut refused = Context {
        computation: context.computation,
        limits: &tight,
        budget: context.budget,
        counters: context.counters,
        location,
    };
    assert!(
        matches!(owner.stage(&selected, Activity::Required, 1, &mut refused),
        Err(FormulaFailure::Limit { resource: FormulaResource::SupportBytes, observed, .. })
            if observed == required as u128)
    );
    let round = owner.round.as_ref().unwrap();
    assert!(round.updates.is_empty());
    assert_eq!(round.updates.capacity(), 0);
    assert!(owner.activity[0] == Activity::Optional);
}
