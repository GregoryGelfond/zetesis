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
