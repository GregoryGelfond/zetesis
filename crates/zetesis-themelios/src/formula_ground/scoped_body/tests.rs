use super::*;
use crate::formula_support::{
    Publication, SourceSelection,
    testing::{Fixture, binding},
};
use crate::{ExpansionLimits, FormulaLimits, FormulaResource, expansion::Budget};
use themelios_base::{
    source::SourceId,
    span::{ByteOffset, Location, Span},
};
use themelios_program::program::DefaultNegation;
use zetesis_core::{Atom, AtomPattern, Predicate, Term, Value, ValueNodeRef};

fn location() -> Location {
    Location {
        source: SourceId::new(0),
        span: Span::empty(ByteOffset::new(0)),
    }
}

#[test]
fn scoped_condition_keeps_the_shared_atom_payload() {
    let mut fixture = Fixture::default();
    let text = Value::String("scoped canonical text".repeat(64));
    let atom = Atom::new(Predicate::new("p", 1).unwrap(), vec![text.clone()]).unwrap();
    let pattern = fixture.pattern(
        &AtomPattern::new(atom.predicate().clone(), vec![Term::Constant(text)]).unwrap(),
        location(),
    );
    let (pending, selected) = fixture.with(location(), |support, computation, counters| {
        let limits = FormulaLimits::default();
        let source = computation
            .atom_ref((&atom).into(), &limits, counters, location())
            .unwrap();
        let mut selected =
            SourceSelection::new(computation, &limits, counters, location()).unwrap();
        selected
            .insert(
                &source,
                (FormulaResource::Atoms, 1),
                computation,
                &limits,
                counters,
                location(),
            )
            .unwrap();
        let literals = [LiteralIr::Atom(DefaultNegation::Not, pattern)];
        let binding = binding(&[], computation, counters, location());
        let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
        let mut context = Context {
            computation,
            limits: &limits,
            budget: &mut budget,
            counters,
            location: location(),
        };
        let validated = validate(&literals, &binding, support, &mut context).unwrap();
        (validated.condition(&mut context).unwrap(), selected)
    });
    let limits = FormulaLimits::default();
    let (mut completed, mut counters) = fixture.finish(location());
    let mut publication = Publication::new(&mut completed, &counters, location()).unwrap();
    let atoms = publication
        .atoms(selected, &limits, &mut counters, location())
        .unwrap();
    let condition = pending
        .publish(&mut publication, &limits, &mut counters, location())
        .unwrap();
    let ValueNodeRef::String(expected) = atoms
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
    let retained = condition
        .nodes()
        .iter()
        .find_map(|node| match node {
            ConditionNode::Atom(atom) => Some(atom),
            _ => None,
        })
        .unwrap();
    let ValueNodeRef::String(actual) = retained.values().at(0).unwrap().descriptor() else {
        panic!("text argument");
    };
    assert!(std::ptr::eq(expected, actual));
    assert!(
        condition
            .nodes()
            .iter()
            .any(|node| matches!(node, ConditionNode::Not(_)))
    );
}
