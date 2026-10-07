use super::*;
use crate::{ExpansionLimits, StatementId};
use themelios_program::program::{Atom, Rule};
use themelios_program::symbol::Name;
use zetesis_cpu::{Cancellation, Stop};

fn fact(lower: i32, upper: i32) -> WithProvenance<Statement> {
    WithProvenance::constructed(
        Rule::fact(Atom::new(
            Name::new("p").unwrap(),
            [SourceTerm::Interval {
                lower: Box::new(lower.into()),
                upper: Box::new(upper.into()),
            }],
        ))
        .into(),
    )
}

#[test]
fn cancellation_preserves_an_accepted_expansion_prefix() {
    let cancellation = Cancellation::default();
    let mut budget =
        Budget::new(ExpansionLimits::default(), 100).with_cancellation(Some(cancellation.clone()));
    let first = ProgramSite::statement(StatementId::new(0), None);
    let second = ProgramSite::statement(StatementId::new(1), None);
    assert_eq!(
        facts(&fact(1, 3), &mut budget, first)
            .unwrap()
            .unwrap()
            .len(),
        3
    );
    let accepted = budget.usage();
    assert_eq!(accepted.templates, 3);
    cancellation.cancel();
    let error = facts(&fact(4, 6), &mut budget, second).unwrap_err();
    assert!(matches!(error, ExpansionFailure::Interrupted {
        reason: Stop::Cancelled, location,
    } if location == second));
    assert_eq!(budget.usage(), accepted);
}

fn family_budget(bytes: usize) -> Budget {
    Budget::new(
        ExpansionLimits {
            max_templates: usize::MAX,
            max_values: usize::MAX,
            max_term_work: usize::MAX,
            max_scalar_bytes: usize::MAX,
            max_family_bytes: bytes,
            ..ExpansionLimits::default()
        },
        usize::MAX,
    )
}

#[test]
fn interval_facts_refuse_before_materialization() {
    let location = ProgramSite::statement(StatementId::new(0), None);
    let error = facts(&fact(1, 1_000_000), &mut family_budget(1024), location).unwrap_err();
    assert!(matches!(error, ExpansionFailure::Limit {
        resource: ExpansionResource::FamilyBytes, limit: 1024, observed, ..
    } if observed > 1_000_000));
}

#[test]
fn fact_family_storage_boundary_is_inclusive() {
    let location = ProgramSite::statement(StatementId::new(0), None);
    let error = facts(&fact(1, 3), &mut family_budget(0), location).unwrap_err();
    let ExpansionFailure::Limit {
        resource: ExpansionResource::FamilyBytes,
        observed,
        ..
    } = error
    else {
        panic!("wrong pre-materialization refusal: {error}");
    };
    let exact = usize::try_from(observed).unwrap();
    assert!(
        matches!(facts(&fact(1, 3), &mut family_budget(exact - 1), location), Err(ExpansionFailure::Limit {
        resource: ExpansionResource::FamilyBytes, observed: amount, ..
    }) if amount == observed)
    );
    assert_eq!(
        facts(&fact(1, 3), &mut family_budget(exact), location)
            .unwrap()
            .unwrap()
            .len(),
        3
    );
}
