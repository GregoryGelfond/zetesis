use super::*;
use crate::expansion::Budget;
use crate::formula_support::{Computation, Counters, SupportCatalog};
use crate::{ExpansionLimits, FormulaLimits};
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use zetesis_core::{Atom, Predicate};

fn atom(name: &str) -> Atom {
    Atom::new(Predicate::new(name, 0).unwrap(), Vec::new()).unwrap()
}

fn context<T>(limits: &FormulaLimits, f: impl FnOnce(&mut Context<'_, '_, '_>) -> T) -> T {
    let mut budget = Budget::new(ExpansionLimits::default(), 0);
    let mut counters = Counters::default();
    let location = Location {
        source: SourceId::new(0),
        span: Span::empty(ByteOffset::new(0)),
    };
    let mut catalog = SupportCatalog::default();
    let (relations, mut append) = catalog.split(limits, &mut counters, location).unwrap();
    let support = Support::indexed(&relations, limits, &counters, location).unwrap();
    let mut computation = Computation::new(&mut append, &support);
    f(&mut Context {
        computation: &mut computation,
        limits,
        budget: &mut budget,
        counters: &mut counters,
        location,
    })
}

fn initial(activity: Activity, context: &mut Context<'_, '_, '_>) -> SourceEligibility {
    let mut owner = SourceEligibility::new(context).unwrap();
    owner.round = Some(Round::new(context));
    owner
        .retain_atom((&atom("q")).into(), activity, 0, context)
        .unwrap();
    owner.publish_predicate(context).unwrap();
    owner.round = Some(Round::new(context));
    owner
}

fn activity(owner: &SourceEligibility, context: &mut Context<'_, '_, '_>) -> Activity {
    let input = atom("q");
    let source = context
        .computation
        .atom_ref(
            (&input).into(),
            context.limits,
            context.counters,
            context.location,
        )
        .unwrap();
    owner.source_activity(&source, context).unwrap()
}

fn unresolved() -> BTreeSet<Signature> {
    BTreeSet::from([signature(atom("q").predicate())])
}

#[test]
fn activity_outside_completed_support_is_refused() {
    context(&FormulaLimits::default(), |context| {
        let mut owner = initial(Activity::Optional, context);
        owner
            .retain_atom((&atom("r")).into(), Activity::Required, 1, context)
            .unwrap();
        assert!(matches!(
            owner.refine_round(&unresolved(), context),
            Err(FormulaFailure::SourceActivity { .. })
        ));
        // Identity may remain after refusal, but it has not entered truth coverage.
        assert_eq!(owner.activity.len(), 1);
        assert!(activity(&owner, context) == Activity::Optional);
    });
}

#[test]
fn a_round_cannot_retract_established_information() {
    context(&FormulaLimits::default(), |context| {
        let mut owner = initial(Activity::Required, context);
        assert!(matches!(
            owner.refine_round(&unresolved(), context),
            Err(FormulaFailure::SourceActivity { .. })
        ));
        assert!(activity(&owner, context) == Activity::Required);
    });
}

#[test]
fn refinement_reuses_the_original_identity_authority() {
    context(&FormulaLimits::default(), |context| {
        let mut owner = initial(Activity::Optional, context);
        let input = atom("q");
        let source = context
            .computation
            .atom_ref(
                (&input).into(),
                context.limits,
                context.counters,
                context.location,
            )
            .unwrap();
        let before = context
            .computation
            .source_atom(&source, context.limits, context.counters, context.location)
            .unwrap()
            .predicate()
            .name()
            .as_ptr();
        owner
            .retain_atom((&atom("q")).into(), Activity::Required, 1, context)
            .unwrap();
        assert!(owner.refine_round(&unresolved(), context).unwrap());
        assert_eq!(owner.atoms.len(), 1);
        let after = context
            .computation
            .source_atom(&source, context.limits, context.counters, context.location)
            .unwrap()
            .predicate()
            .name()
            .as_ptr();
        assert_eq!(after, before);
        assert!(activity(&owner, context) == Activity::Required);
    });
}

#[test]
fn empty_round_refines_optional_identity_to_absent() {
    context(&FormulaLimits::default(), |context| {
        let mut owner = initial(Activity::Optional, context);
        assert!(owner.refine_round(&unresolved(), context).unwrap());
        assert!(activity(&owner, context) == Activity::Absent);
        assert_eq!(owner.atoms.len(), 1);
    });
}

#[test]
fn activity_storage_refusal_precedes_selection() {
    context(&FormulaLimits::default(), |context| {
        let tight = FormulaLimits {
            max_support_bytes: 0,
            ..*context.limits
        };
        let refused = Context {
            computation: context.computation,
            limits: &tight,
            budget: context.budget,
            counters: context.counters,
            location: context.location,
        };
        assert!(matches!(
            SourceEligibility::new(&refused),
            Err(FormulaFailure::Limit {
                resource: crate::FormulaResource::SupportBytes,
                ..
            })
        ));
    });
}

#[test]
fn every_stage_work_cutoff_preserves_work_refusal() {
    let run = |limit| {
        context(
            &FormulaLimits {
                max_work: limit,
                ..FormulaLimits::default()
            },
            |context| {
                let mut owner = SourceEligibility::new(context).unwrap();
                owner.round = Some(Round::new(context));
                let result = owner.retain_atom((&atom("q")).into(), Activity::Optional, 0, context);
                (result, context.counters.accounting.work)
            },
        )
    };
    let (result, count) = run(u64::MAX);
    result.unwrap();
    for limit in 0..count {
        assert!(matches!(
            run(limit).0,
            Err(FormulaFailure::Limit {
                resource: crate::FormulaResource::Work,
                ..
            })
        ));
    }
}
