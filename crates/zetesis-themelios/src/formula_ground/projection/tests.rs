use super::*;
use crate::formula_support::{self, testing};
use themelios_base::{
    source::SourceId,
    span::{ByteOffset, Span},
};
use zetesis_core::{Predicate, ValueNodeRef};

fn location() -> Location {
    Location {
        source: SourceId::new(0),
        span: Span::empty(ByteOffset::new(0)),
    }
}

#[test]
fn projection_uses_the_completed_support_payload() {
    let crate::formula::Preparation {
        program: prepared,
        catalog,
        accounting,
        mut budget,
        ..
    } = testing::prepare("p(\"same canonical text\"). #project p/1.");
    let limits = FormulaLimits::default();
    let mut counters = Counters::resume(accounting, crate::grounding_observer::Work::default());
    let mut completed = formula_support::build(
        catalog,
        &prepared,
        None,
        &limits,
        &mut budget,
        &mut counters,
        location(),
    )
    .unwrap();
    let (pending, original) = {
        let (support, mut append) = completed.split(&limits, &mut counters, location()).unwrap();
        let queries = support
            .queries(crate::JoinStrategy::Indexed, &limits, &counters, location())
            .unwrap();
        let predicate = Predicate::new("p", 1).unwrap();
        let original = queries
            .support()
            .rows(&predicate)
            .next()
            .unwrap()
            .atom()
            .values()
            .at(0)
            .unwrap();
        let ValueNodeRef::String(original) = original.descriptor() else {
            panic!("text argument");
        };
        let original = original.as_ptr();
        let mut computation = Computation::new(&mut append, queries.support());
        (
            prepare(
                &prepared,
                &queries,
                &mut computation,
                &limits,
                &mut budget,
                &mut counters,
                location(),
            )
            .unwrap(),
            original,
        )
    };
    let mut publication = Publication::new(&mut completed, &counters, location()).unwrap();
    let projection = pending
        .publish(&mut publication, &limits, &mut counters, location())
        .unwrap();
    drop(publication);
    drop(completed);
    assert!(projection.is_explicit());
    assert_eq!(projection.atoms().len(), 1);
    let ValueNodeRef::String(retained) = projection
        .atoms()
        .at(0)
        .unwrap()
        .values()
        .at(0)
        .unwrap()
        .descriptor()
    else {
        panic!("text argument");
    };
    assert_eq!(retained, "same canonical text");
    assert_eq!(retained.as_ptr(), original);
}

#[test]
fn projection_publishes_semantic_atom_order() {
    let crate::formula::Preparation {
        program: prepared,
        catalog,
        accounting,
        mut budget,
        ..
    } = testing::prepare("p(3). p(1). p(2). #project p/1.");
    let limits = FormulaLimits::default();
    let mut counters = Counters::resume(accounting, crate::grounding_observer::Work::default());
    let mut completed = formula_support::build(
        catalog,
        &prepared,
        None,
        &limits,
        &mut budget,
        &mut counters,
        location(),
    )
    .unwrap();
    let pending = {
        let (support, mut append) = completed.split(&limits, &mut counters, location()).unwrap();
        let queries = support
            .queries(crate::JoinStrategy::Indexed, &limits, &counters, location())
            .unwrap();
        let mut computation = Computation::new(&mut append, queries.support());
        prepare(
            &prepared,
            &queries,
            &mut computation,
            &limits,
            &mut budget,
            &mut counters,
            location(),
        )
        .unwrap()
    };
    let mut publication = Publication::new(&mut completed, &counters, location()).unwrap();
    let projection = pending
        .publish(&mut publication, &limits, &mut counters, location())
        .unwrap();
    let values: Vec<_> = projection
        .atoms()
        .iter()
        .map(|atom| atom.values().at(0).unwrap().descriptor())
        .collect();
    assert_eq!(
        values,
        [
            ValueNodeRef::Number(1),
            ValueNodeRef::Number(2),
            ValueNodeRef::Number(3)
        ]
    );
}
