use std::convert::Infallible;

use super::*;
use crate::{Condition, ConditionNode};
use zetesis_core::{Atom, AtomCatalog, FilterRef, Predicate, Value};

fn template(priority: i32, condition: Condition) -> ObjectiveTemplate {
    ObjectiveTemplate::new(
        Term::Constant(Value::Number(7)),
        priority,
        vec![Term::Constant(Value::Number(7))],
        vec![],
        vec![],
    )
    .with_condition(condition)
}
fn catalog() -> AtomCatalog {
    AtomCatalog::new(vec![
        Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(7)]).unwrap(),
    ])
    .unwrap()
}
fn condition(atoms: AtomCatalog) -> Condition {
    Condition::from_catalog_with(atoms, vec![ConditionNode::Atom(0)], usize::MAX, || {
        Ok::<_, Infallible>(())
    })
    .unwrap()
}

#[test]
fn canonical_rows_preserve_original_priority_occurrences() {
    let program = ObjectiveProgram::new(
        [2, 7, 2, -1]
            .map(|priority| template(priority, Condition::default()))
            .into(),
        AdmissionLimits::default(),
    )
    .unwrap();
    assert_eq!(program.priorities(), [7, 2, -1]);
    assert_eq!(
        program
            .templates()
            .iter()
            .map(ObjectiveTemplateRef::priority)
            .collect::<Vec<_>>(),
        [2, 7, 2, -1]
    );
    assert_eq!(
        program
            .templates()
            .iter()
            .rev()
            .map(ObjectiveTemplateRef::priority)
            .collect::<Vec<_>>(),
        [-1, 2, 7, 2]
    );
    for row in program.templates() {
        assert_eq!(
            row.weight(),
            TemplateTerm::Constant((&Value::Number(7)).into())
        );
        assert_eq!(row.tuple().len(), 1);
        assert_eq!(row.tuple().at(0), Some(row.weight()));
    }
}

#[test]
fn template_equality_is_independent_of_its_owner() {
    let owned = template(2, Condition::default());
    let first = ObjectiveProgram::new(vec![owned.clone()], AdmissionLimits::default()).unwrap();
    let second = ObjectiveProgram::from_catalog(
        row_catalog(&owned),
        vec![ObjectiveElement::new(0, 2)],
        AdmissionLimits::default(),
    )
    .unwrap();
    assert_eq!(first.templates().at(0), second.templates().at(0));
    let different = ObjectiveProgram::new(
        vec![template(3, Condition::default())],
        AdmissionLimits::default(),
    )
    .unwrap();
    assert_ne!(first.templates().at(0), different.templates().at(0));
}

#[test]
fn shared_condition_nodes_are_charged_once() {
    let atoms = catalog();
    let shared = condition(atoms.clone());
    let node_bytes = shared.node_storage_bytes();
    let shared_program = ObjectiveProgram::new(
        vec![template(0, shared.clone()), template(0, shared)],
        AdmissionLimits::default(),
    )
    .unwrap();
    let separate_program = ObjectiveProgram::new(
        vec![
            template(0, condition(atoms.clone())),
            template(0, condition(atoms)),
        ],
        AdmissionLimits::default(),
    )
    .unwrap();
    assert_eq!(
        separate_program.storage_bytes() - shared_program.storage_bytes(),
        node_bytes
    );
}

#[test]
fn final_objective_storage_obeys_the_combined_ceiling() {
    let original = template(
        1,
        Condition::new(vec![ConditionNode::Atom(
            Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(7)]).unwrap(),
        )]),
    );
    let program =
        ObjectiveProgram::new(vec![original.clone()], AdmissionLimits::default()).unwrap();
    let limit = usize::try_from(program.storage_bytes()).unwrap() - 1;
    let error = ObjectiveProgram::new(
        vec![original],
        AdmissionLimits {
            max_bytes: limit,
            ..AdmissionLimits::default()
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        AdmissionError::Storage {
            error: zetesis_core::catalog::Error::Storage { .. },
            ..
        } | AdmissionError::ConditionStorage {
            error: crate::ConditionError::Storage(zetesis_core::catalog::Error::Storage { .. }),
            ..
        }
    ));
}

#[test]
fn safety_scratch_uses_the_explicit_storage_allowance() {
    let predicate = Predicate::new("p", 1).unwrap();
    let positive = [AtomPattern::new(predicate, vec![Term::Variable(0)]).unwrap()];
    let result = ObjectiveTemplate::validate_fields(
        &Term::Variable(0),
        &[],
        &positive,
        &[],
        AdmissionLimits {
            max_bytes: 0,
            ..AdmissionLimits::default()
        },
        3,
    );
    assert!(matches!(
        result,
        Err(AdmissionError::Storage {
            template: Some(3),
            error: zetesis_core::catalog::Error::Storage { limit: 0, .. }
        })
    ));
}

fn row_catalog(input: &ObjectiveTemplate) -> TemplateCatalog {
    let mut builder = zetesis_core::TemplateCatalogBuilder::new(usize::MAX).unwrap();
    builder
        .append(
            std::iter::once(TemplateTerm::from(input.weight()))
                .chain(input.tuple().iter().map(TemplateTerm::from)),
            input.positive().iter().map(PatternRef::from),
            input.filters().iter().map(FilterRef::from),
        )
        .unwrap();
    builder.finish().unwrap()
}

#[test]
fn catalog_admission_preserves_occurrence_priorities() {
    let input = template(0, Condition::default());
    let catalog = row_catalog(&input);
    let program = ObjectiveProgram::from_catalog(
        catalog,
        [2, 7, 2, -1]
            .map(|priority| ObjectiveElement::new(0, priority))
            .into(),
        AdmissionLimits::default(),
    )
    .unwrap();
    assert_eq!(program.priorities(), [7, 2, -1]);
    assert_eq!(
        program
            .templates()
            .iter()
            .map(ObjectiveTemplateRef::priority)
            .collect::<Vec<_>>(),
        [2, 7, 2, -1]
    );
}

#[test]
fn catalog_admission_uses_the_same_variable_policy() {
    let positive = |variable| {
        AtomPattern::new(
            Predicate::new("p", 1).unwrap(),
            vec![Term::Variable(variable)],
        )
        .unwrap()
    };
    for input in [
        ObjectiveTemplate::new(Term::Variable(0), 0, vec![], vec![], vec![]),
        ObjectiveTemplate::new(Term::Variable(1), 0, vec![], vec![positive(1)], vec![]),
        ObjectiveTemplate::new(
            Term::Variable(0),
            0,
            vec![],
            vec![positive(0)],
            vec![Filter::Eq(Term::Variable(0), Term::Variable(1))],
        ),
    ] {
        let catalog = row_catalog(&input);
        let expected = ObjectiveProgram::new(vec![input], AdmissionLimits::default()).unwrap_err();
        let actual = ObjectiveProgram::from_catalog(
            catalog,
            vec![ObjectiveElement::new(0, 0)],
            AdmissionLimits::default(),
        )
        .unwrap_err();
        assert_eq!(actual, expected);
        assert!(matches!(
            actual,
            AdmissionError::UnsafeVariable { .. } | AdmissionError::NonDenseVariable { .. }
        ));
    }
}

#[test]
fn catalog_rows_keep_the_original_text_allocation() {
    let input = ObjectiveTemplate::new(
        Term::Constant(Value::Number(1)),
        0,
        vec![Term::Constant(Value::String(
            "shared canonical text".repeat(128),
        ))],
        vec![],
        vec![],
    );
    let catalog = row_catalog(&input);
    let TemplateTerm::Constant(original) = catalog.at(0).unwrap().terms().at(1).unwrap() else {
        panic!("constant tuple");
    };
    let zetesis_core::ValueNodeRef::String(original) = original.descriptor() else {
        panic!("text tuple");
    };
    let program = ObjectiveProgram::from_catalog(
        catalog.clone(),
        vec![ObjectiveElement::new(0, 0)],
        AdmissionLimits::default(),
    )
    .unwrap();
    let TemplateTerm::Constant(retained) =
        program.templates().at(0).unwrap().tuple().at(0).unwrap()
    else {
        panic!("constant tuple");
    };
    let zetesis_core::ValueNodeRef::String(retained) = retained.descriptor() else {
        panic!("text tuple");
    };
    assert!(std::ptr::eq(original, retained));
}

#[test]
fn catalog_admission_requires_a_weight_field() {
    let mut builder = zetesis_core::TemplateCatalogBuilder::new(usize::MAX).unwrap();
    builder.append([], [], []).unwrap();
    let catalog = builder.finish().unwrap();
    assert_eq!(
        ObjectiveProgram::from_catalog(
            catalog,
            vec![ObjectiveElement::new(0, 0)],
            AdmissionLimits::default()
        )
        .unwrap_err(),
        AdmissionError::MissingWeight {
            template: 0,
            row: 0
        }
    );
}

#[test]
fn catalog_admission_checks_the_row_coordinate() {
    let catalog = row_catalog(&template(0, Condition::default()));
    assert_eq!(
        ObjectiveProgram::from_catalog(
            catalog,
            vec![ObjectiveElement::new(1, 0)],
            AdmissionLimits::default()
        )
        .unwrap_err(),
        AdmissionError::CatalogRow {
            template: 0,
            row: 1
        }
    );
}

#[test]
fn catalog_admission_does_not_import_conditions() {
    let catalog = row_catalog(&template(0, Condition::default()));
    let condition = Condition::new(vec![ConditionNode::Atom(
        Atom::new(Predicate::new("q", 0).unwrap(), vec![]).unwrap(),
    )]);
    assert_eq!(
        ObjectiveProgram::from_catalog(
            catalog,
            vec![ObjectiveElement::new(0, 0).with_condition(condition)],
            AdmissionLimits::default()
        )
        .unwrap_err(),
        AdmissionError::ConditionNotCanonical { template: 0 }
    );
}

#[test]
fn catalog_admission_bounds_simultaneous_safety_scratch() {
    let catalog = row_catalog(&template(0, Condition::default()));
    let program = ObjectiveProgram::from_catalog(
        catalog.clone(),
        vec![ObjectiveElement::new(0, 0)],
        AdmissionLimits::default(),
    )
    .unwrap();
    let retained = usize::try_from(program.storage_bytes()).unwrap();
    // Even a variable-free row has two live validation vector headers. The
    // retained-only ceiling must refuse that admission overlap.
    let error = ObjectiveProgram::from_catalog(
        catalog,
        vec![ObjectiveElement::new(0, 0)],
        AdmissionLimits {
            max_bytes: retained,
            ..AdmissionLimits::default()
        },
    )
    .unwrap_err();
    assert!(matches!(error, AdmissionError::Storage { template: Some(0),
        error: zetesis_core::catalog::Error::Storage { required, limit: 0 } }
        if required == 2 * size_of::<Vec<bool>>() as u128));
}

#[test]
fn selected_conditions_count_the_shared_snapshot_once() {
    use zetesis_core::{
        TemplateCatalogSelection,
        atom_interner::{AtomInterner, Limits},
    };
    let mut owner = AtomInterner::default();
    let limits = Limits {
        max_atoms: 8,
        max_bytes: usize::MAX as u128,
    };
    let input = Atom::new(
        Predicate::new("p", 2).unwrap(),
        vec![
            Value::Number(1),
            Value::String("shared source vocabulary".repeat(64)),
        ],
    )
    .unwrap();
    owner
        .entry_atom_with(&input, limits, || Ok::<_, Infallible>(()))
        .unwrap()
        .insert_with(limits, || Ok::<_, Infallible>(()))
        .unwrap();
    let mut selected = TemplateCatalogSelection::new(owner.read(), usize::MAX).unwrap();
    let atom = owner.get(0).unwrap();
    selected
        .append_with(
            owner.read(),
            atom.values().iter().map(TemplateTerm::Constant),
            [],
            [],
            usize::MAX,
            || Ok::<_, Infallible>(()),
        )
        .unwrap();
    let source = owner
        .publish_selection_with(&[0], limits, || Ok::<_, Infallible>(()))
        .unwrap();
    let conditions = owner
        .publish_selection_with(&[0], limits, || Ok::<_, Infallible>(()))
        .unwrap();
    assert!(!source.same_owner(&conditions));
    assert!(source.shares_snapshot(&conditions));
    let condition_bytes = conditions.publication_bytes();
    let condition = condition(conditions);
    let condition_bytes = condition_bytes + condition.node_storage_bytes();
    let catalog = selected
        .finish_with(source, usize::MAX, || Ok::<_, Infallible>(()))
        .unwrap();
    let baseline = ObjectiveProgram::from_catalog(
        catalog.clone(),
        vec![ObjectiveElement::new(0, 0)],
        AdmissionLimits::default(),
    )
    .unwrap();
    let program = ObjectiveProgram::from_catalog(
        catalog,
        vec![ObjectiveElement::new(0, 0).with_condition(condition)],
        AdmissionLimits::default(),
    )
    .unwrap();
    assert_eq!(
        program.storage_bytes() - baseline.storage_bytes(),
        condition_bytes
    );
    let row = program.templates().at(0).unwrap();
    let TemplateTerm::Constant(tuple) = row.tuple().at(0).unwrap() else {
        panic!("constant tuple");
    };
    let ConditionNode::Atom(atom) = row.condition().nodes().at(0).unwrap() else {
        panic!("condition atom");
    };
    let zetesis_core::ValueNodeRef::String(left) = tuple.descriptor() else {
        panic!("text tuple");
    };
    let zetesis_core::ValueNodeRef::String(right) = atom.values().at(1).unwrap().descriptor()
    else {
        panic!("text argument");
    };
    assert!(std::ptr::eq(left, right));
}
