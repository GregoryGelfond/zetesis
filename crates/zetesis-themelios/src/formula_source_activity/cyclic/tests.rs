use super::*;
use crate::expansion::Budget;
use crate::formula_support::Counters;
use crate::{ExpansionLimits, FormulaLimits};
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use zetesis_core::{Atom, Predicate};

fn atom(name: &str) -> Atom {
    Atom::new(Predicate::new(name, 0).unwrap(), Vec::new()).unwrap()
}

fn attempt(old: &mut SourceEligibility, next: &SourceEligibility) -> Result<bool, FormulaFailure> {
    let limits = FormulaLimits::default();
    let mut budget = Budget::new(ExpansionLimits::default(), 0);
    let mut counters = Counters::default();
    let mut context = Context {
        limits: &limits,
        budget: &mut budget,
        counters: &mut counters,
        location: Location {
            source: SourceId::new(0),
            span: Span::empty(ByteOffset::new(0)),
        },
    };
    old.refine_round(
        next,
        &BTreeSet::from([signature(atom("q").predicate())]),
        &mut context,
    )
}

#[test]
fn activity_outside_completed_support_is_refused() {
    let mut old = SourceEligibility {
        atoms: [(atom("q"), Activity::Optional)].into(),
    };
    let next = SourceEligibility {
        atoms: [(atom("r"), Activity::Required)].into(),
    };
    assert!(matches!(
        attempt(&mut old, &next),
        Err(FormulaFailure::SourceActivity { .. })
    ));
    assert!(old.atom_activity(&atom("q")) == Activity::Optional);
    assert_eq!(old.atoms.len(), 1);
}

#[test]
fn a_round_cannot_retract_established_information() {
    let mut old = SourceEligibility {
        atoms: [(atom("q"), Activity::Required)].into(),
    };
    assert!(matches!(
        attempt(&mut old, &SourceEligibility::default()),
        Err(FormulaFailure::SourceActivity { .. })
    ));
    assert!(old.atom_activity(&atom("q")) == Activity::Required);
}
