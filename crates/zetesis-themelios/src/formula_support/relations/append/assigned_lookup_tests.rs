use std::cell::RefCell;

use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Span};
use zetesis_core::{AtomPattern, Predicate, Term, Value};

use super::*;
use crate::formula_support::{Accounting, Support, SupportCatalog, testing};
use crate::grounding_observer::Profile;
use crate::{FormulaResource, GroundingObserver, GroundingOutcome, GroundingPhase};

#[derive(Default)]
struct Observer(RefCell<Option<crate::GroundingWork>>);

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
        work: crate::GroundingWork,
    ) {
        *self.0.borrow_mut() = Some(work);
    }
}

fn location() -> Location {
    Location {
        source: SourceId::new(0),
        span: Span::empty(ByteOffset::new(0)),
    }
}

fn refused_lookup(short: bool) -> (u128, crate::GroundingWork, FormulaFailure) {
    let limits = FormulaLimits::default();
    let mut setup = Counters::default();
    let mut catalog = SupportCatalog::default();
    let pattern = testing::admit_pattern(
        &mut catalog,
        &AtomPattern::new(
            Predicate::new("receipt", 1).unwrap(),
            vec![Term::Constant(Value::Number(1))],
        )
        .unwrap(),
        &mut setup,
        location(),
    );
    let (relations, append) = catalog.split(&limits, &mut setup, location()).unwrap();
    let support = Support::indexed(&relations, &limits, &setup, location()).unwrap();
    let pattern = pattern
        .get(
            support.components().unwrap(),
            &limits,
            &mut setup,
            location(),
        )
        .unwrap();
    let values = append.read().assignment();
    let workspace = support.workspace_bytes();
    let required = append.owner.pattern_lookup_bytes() + append.outer_bytes(workspace);
    let checked = FormulaLimits {
        max_support_bytes: usize::try_from(required - u128::from(short)).unwrap(),
        max_work: 0,
        ..limits
    };
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    let mut counters = Counters::resume(Accounting::default(), profile.work());
    let error = profile
        .phase(GroundingPhase::SupportCompletion, None, || {
            append.contains_pattern(
                pattern,
                values.as_slice(),
                workspace,
                GroundingWork::new(&checked, &mut counters, location()),
            )
        })
        .unwrap_err();
    let work = observer.0.into_inner().unwrap();
    (required, work, error)
}

#[test]
fn stopped_lookup_records_admitted_header() {
    let (required, work, error) = refused_lookup(false);
    assert!(matches!(
        error,
        FormulaFailure::Limit {
            resource: FormulaResource::Work,
            observed: 1,
            limit: 0,
            ..
        }
    ));
    assert_eq!(
        work.support_peak_bytes,
        Some(u64::try_from(required).unwrap())
    );
}

#[test]
fn rejected_lookup_does_not_record_proposed_storage() {
    let (required, work, error) = refused_lookup(true);
    assert!(matches!(error, FormulaFailure::Limit {
        resource: FormulaResource::SupportBytes, observed, limit, ..
    } if observed == required && limit == required - 1));
    assert_eq!(work.support_peak_bytes, Some(0));
}
