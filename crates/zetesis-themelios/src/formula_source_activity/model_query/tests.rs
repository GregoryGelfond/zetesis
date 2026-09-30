use super::*;
use crate::formula_support::components::Term;
use crate::formula_support::testing::{Fixture, binding};
use crate::test_support::location;
use crate::{ExpansionLimits, expansion::Budget};
use zetesis_core::{Atom, Predicate, Value, ValueNodeRef};

fn query(fixture: &mut Fixture) -> (PendingCondition, SourceSelection) {
    let limits = FormulaLimits::default();
    let text = Value::String("one canonical condition payload".repeat(64));
    let atom = Atom::new(Predicate::new("p", 1).unwrap(), vec![text.clone()]).unwrap();
    let pattern = fixture.admit(location(), |source, counters| {
        let scalar = source
            .scalar((&text).into(), &limits, counters, location())
            .unwrap();
        let predicate = source
            .predicate(atom.predicate().into(), &limits, counters, location())
            .unwrap();
        source
            .pattern(
                predicate,
                &[Term::Constant(scalar)],
                &limits,
                counters,
                location(),
            )
            .unwrap()
    });
    fixture.with(location(), |_, computation, counters| {
        // An unrelated earlier discovery must not shift a query's local atom 0.
        computation
            .atom_ref(
                (&Atom::new(Predicate::new("unrelated", 0).unwrap(), vec![]).unwrap()).into(),
                &limits,
                counters,
                location(),
            )
            .unwrap();
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
        let literals = [
            LiteralIr::Atom(DefaultNegation::None, pattern),
            LiteralIr::Atom(DefaultNegation::NotNot, pattern),
        ];
        let binding = binding(&[], computation, counters, location());
        let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
        let mut context = Context {
            computation,
            limits: &limits,
            budget: &mut budget,
            counters,
            location: location(),
        };
        (
            condition(&literals, &binding, &mut context).unwrap(),
            selected,
        )
    })
}

#[test]
fn final_condition_shares_the_source_payload() {
    let mut fixture = Fixture::default();
    let (pending, selected) = query(&mut fixture);
    let (mut completed, mut counters) = fixture.finish(location());
    let limits = FormulaLimits::default();
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
    let mut occurrences = 0;
    for node in condition.nodes() {
        if let ConditionNode::Atom(atom) = node {
            assert_eq!(atom.predicate().name(), "p");
            let ValueNodeRef::String(actual) = atom.values().at(0).unwrap().descriptor() else {
                panic!("text argument");
            };
            assert!(std::ptr::eq(expected, actual));
            occurrences += 1;
        }
    }
    assert_eq!(occurrences, 2);
    drop(publication);
    drop(completed);
    drop(atoms);
    let atom = condition
        .nodes()
        .iter()
        .find_map(|node| match node {
            ConditionNode::Atom(atom) => Some(atom),
            _ => None,
        })
        .unwrap();
    let ValueNodeRef::String(retained) = atom.values().at(0).unwrap().descriptor() else {
        panic!("retained text argument");
    };
    assert_eq!(retained, "one canonical condition payload".repeat(64));
}

#[test]
fn final_condition_rejects_a_foreign_source() {
    let mut fixture = Fixture::default();
    let (pending, _) = query(&mut fixture);
    let (_, mut counters) = fixture.finish(location());
    let (mut foreign, _) = Fixture::default().finish(location());
    let mut publication = Publication::new(&mut foreign, &counters, location()).unwrap();
    let result = pending.publish(
        &mut publication,
        &FormulaLimits::default(),
        &mut counters,
        location(),
    );
    assert!(matches!(
        result,
        Err(FormulaFailure::SupportRelation {
            error: zetesis_core::relation::Failure::Owner,
            ..
        })
    ));
}

#[test]
fn final_condition_admits_its_canonical_envelope() {
    let mut fixture = Fixture::default();
    let (pending, _) = query(&mut fixture);
    let node_buffer = pending.query.nodes.capacity() * size_of::<ConditionIndex>();
    let (mut completed, mut counters) = fixture.finish(location());
    let limits = FormulaLimits {
        objective: zetesis_objective::AdmissionLimits {
            max_condition_node_bytes: node_buffer,
            ..zetesis_objective::AdmissionLimits::default()
        },
        ..FormulaLimits::default()
    };
    let mut publication = Publication::new(&mut completed, &counters, location()).unwrap();
    let result = pending.publish(&mut publication, &limits, &mut counters, location());
    assert!(matches!(result, Err(FormulaFailure::ObjectiveCondition {
        error: ConditionError::Storage(CatalogError::Storage { required, limit }), ..
    }) if limit == node_buffer && required > node_buffer as u128));
}

#[test]
fn publication_rejects_an_unrelated_workspace() {
    let mut fixture = Fixture::default();
    let (pending, _) = query(&mut fixture);
    let (mut completed, _original_counters) = fixture.finish(location());
    let mut unrelated = Counters::default();
    let mut publication = Publication::new(&mut completed, &unrelated, location()).unwrap();
    let result = pending.publish(
        &mut publication,
        &FormulaLimits::default(),
        &mut unrelated,
        location(),
    );
    assert!(matches!(
        result,
        Err(FormulaFailure::TermAssignment {
            error: zetesis_core::catalog::AssignmentError::Read(
                zetesis_core::catalog::ReadError::ForeignCatalog
            ),
            ..
        })
    ));
}

#[test]
fn atom_publication_checks_the_selection_workspace() {
    let mut fixture = Fixture::default();
    let (_, selected) = query(&mut fixture);
    let (mut completed, _original_counters) = fixture.finish(location());
    let mut unrelated = Counters::default();
    let mut publication = Publication::new(&mut completed, &unrelated, location()).unwrap();
    let result = publication.atoms(
        selected,
        &FormulaLimits::default(),
        &mut unrelated,
        location(),
    );
    assert!(matches!(
        result,
        Err(FormulaFailure::TermAssignment {
            error: zetesis_core::catalog::AssignmentError::Read(
                zetesis_core::catalog::ReadError::ForeignCatalog
            ),
            ..
        })
    ));
}
