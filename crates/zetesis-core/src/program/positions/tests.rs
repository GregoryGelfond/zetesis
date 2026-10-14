use super::{DomainPositions, fits};
use crate::{
    AdmissionLimits, AtomPattern, Predicate, Program, Sign, Template, Term, Value, ValueLimits,
    ValueNode,
};

fn function(name: &str, arity: usize) -> ValueNode {
    ValueNode::Function {
        name: name.into(),
        sign: Sign::Positive,
        arity,
    }
}

/// Facts `p(3)`, `p(a)` and `p(f(g(7)))`: `g(7)` and `7` are catalog terms
/// but not domain values, since only the whole argument is one.
fn facts() -> Program {
    let nested = Value::from_nodes(
        vec![function("f", 1), function("g", 1), ValueNode::Number(7)],
        ValueLimits::default(),
    )
    .unwrap();
    let statements = [Value::Number(3), Value::Symbol("a".into()), nested].map(|value| {
        let atom =
            AtomPattern::new(Predicate::new("p", 1).unwrap(), vec![Term::Constant(value)]).unwrap();
        Template::new(Some(atom), vec![], vec![], vec![], vec![])
    });
    Program::new(
        statements.into(),
        crate::FactTables::default(),
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn table(program: &Program) -> DomainPositions<'_> {
    DomainPositions::build(program, |slots| Ok::<_, ()>(Vec::with_capacity(slots)))
        .unwrap()
        .expect("a small domain gets a table")
}

#[test]
fn every_domain_value_finds_its_own_position() {
    let program = facts();
    let positions = table(&program);
    for index in 0..program.domain().len() {
        let value = program.domain().at(index).unwrap();
        assert_eq!(positions.position(value), Some(index));
    }
}

#[test]
fn only_domain_values_have_slots() {
    let program = facts();
    let positions = table(&program);
    let domain = &program.0.domain;
    assert!(
        positions.slots.len() > domain.len(),
        "the catalog has subterms"
    );
    let mut named = 0;
    for (term, slot) in positions.slots.iter().enumerate() {
        if let Some(slot) = slot {
            assert_eq!(domain[slot.get() as usize - 1].position(), term);
            named += 1;
        }
    }
    assert_eq!(named, domain.len());
}

#[test]
fn another_programs_value_has_no_position() {
    let program = facts();
    let other = facts();
    let positions = table(&program);
    let value = other.domain().at(0).unwrap();
    assert_eq!(positions.position(value), None);
}

#[test]
fn a_domain_of_u32_max_values_gets_no_table() {
    assert!(fits(u32::MAX as usize - 1));
    assert!(!fits(u32::MAX as usize));
}

#[test]
fn the_reservation_refusal_is_the_callers() {
    let program = facts();
    let built = DomainPositions::build(&program, |_| Err::<Vec<_>, _>("refused"));
    assert_eq!(built.err(), Some("refused"));
}

#[test]
fn the_table_reserves_one_slot_per_catalog_term() {
    let program = facts();
    let mut requested = 0;
    let positions = DomainPositions::build(&program, |slots| {
        requested = slots;
        Ok::<_, ()>(Vec::with_capacity(slots))
    })
    .unwrap()
    .unwrap();
    assert_eq!(requested, program.0.catalog.read().term_count());
    assert_eq!(positions.slots.len(), requested);
}
