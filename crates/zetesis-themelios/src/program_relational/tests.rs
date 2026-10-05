//! Controlled preparation publishes no partial relational receipt.

use std::cell::Cell;

use themelios_program::{
    Name, Term,
    program::{Atom, Program, Rule},
};
use zetesis_cpu::{Cancellation, Stop};

use crate::expansion::Budget;
use crate::{ExpansionFailure, ExpansionLimits, FormulaPurpose, ProgramSite, extended};

#[test]
fn cancellation_discards_partial_relational_work() {
    let program = Program::of(["a", "b"].map(|name| {
        Rule::fact(Atom::new(
            Name::new(name).unwrap(),
            [Term::Interval {
                lower: Box::new(1.into()),
                upper: Box::new(3.into()),
            }],
        ))
    }));
    let cancellation = Cancellation::default();
    let mut budget =
        Budget::new(ExpansionLimits::default(), 100).with_cancellation(Some(cancellation.clone()));
    let visited = Cell::new(0);
    let interrupted_site = Cell::new(None);
    let error = extended::compile_relational(
        &program,
        zetesis_core::AdmissionLimits::default(),
        &mut budget,
        ProgramSite::program(),
        FormulaPurpose::Ordinary,
        |carrier, site| {
            visited.set(visited.get() + 1);
            if visited.get() == 2 {
                interrupted_site.set(Some(site));
                cancellation.cancel();
            }
            extended::program_sites(carrier, site)
        },
        |site| *site,
    )
    .unwrap_err();
    assert!(matches!(error, ExpansionFailure::Interrupted {
        reason: Stop::Cancelled, location,
    } if Some(location) == interrupted_site.get()));
    assert_eq!(visited.get(), 2);
    // The first finite fact completed; the interruption prevents publication of
    // those templates rather than merely stopping before any work was attempted.
    assert_eq!(budget.usage().templates, 3);
    assert_eq!(
        interrupted_site
            .get()
            .unwrap()
            .statement_id()
            .unwrap()
            .index(),
        1
    );
}
