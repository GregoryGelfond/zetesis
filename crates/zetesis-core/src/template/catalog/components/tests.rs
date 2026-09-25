//! Independent components retain occurrences, not another payload authority.

use super::*;
use crate::atom_interner::{AtomInterner, Limits};
use crate::catalog::{Error, ReadError};
use crate::{Atom, Predicate, Sign, Value};
use std::convert::Infallible;

const METADATA: usize = 1024 * 1024;
fn limits() -> Limits {
    Limits {
        max_atoms: 16,
        max_bytes: 4 * 1024 * 1024,
    }
}
const PERMIT: fn() -> Result<(), Infallible> = || Ok(());
fn owner() -> AtomInterner {
    let mut owner = AtomInterner::new();
    let atom = Atom::new(
        Predicate::new("p", 1).unwrap(),
        vec![Value::String("shared".into())],
    )
    .unwrap();
    owner
        .entry_atom_with(&atom, limits(), PERMIT)
        .unwrap()
        .insert_with(limits(), PERMIT)
        .unwrap();
    owner
}

#[test]
fn independent_occurrences_preserve_order_and_borrow_payload() {
    let owner = owner();
    let read = owner.read();
    let atom = owner.get(0).unwrap();
    let value = atom.values().at(0).unwrap();
    let constant = TemplateTerm::Constant(value);
    let mut components = TemplateComponents::new(read, METADATA).unwrap();
    let terms = [constant, TemplateTerm::Variable(7), constant];
    assert_eq!(
        components
            .append_terms_with(read, terms, METADATA, PERMIT)
            .unwrap(),
        0..3
    );
    let arguments = [constant];
    let pattern = PatternRef::from_parts(atom.predicate(), &arguments).unwrap();
    assert_eq!(
        components
            .append_pattern_with(read, pattern, METADATA, PERMIT)
            .unwrap(),
        0
    );
    let filter = FilterRef::from_parts(false, terms[1], constant);
    assert_eq!(
        components
            .append_filter_with(read, filter, METADATA, PERMIT)
            .unwrap(),
        0
    );
    let bound = components.bind_with(read, PERMIT).unwrap();
    assert_eq!(bound.terms().iter().collect::<Vec<_>>(), terms);
    assert_eq!(bound.pattern(0), Some(pattern));
    assert_eq!(bound.filter(0), Some(filter));
    let TemplateTerm::Constant(actual) = bound.term(2).unwrap() else {
        panic!("constant")
    };
    let ValueNodeRef::String(actual) = actual.descriptor() else {
        panic!("string")
    };
    let ValueNodeRef::String(expected) = value.descriptor() else {
        panic!("string")
    };
    assert_eq!(actual.as_ptr(), expected.as_ptr());
}

#[test]
fn empty_components_still_require_their_vocabulary() {
    let owner = owner();
    let foreign = self::owner();
    let components = TemplateComponents::new(owner.read(), METADATA).unwrap();
    assert!(matches!(
        components.bind_with(foreign.read(), PERMIT),
        Err(TemplateCatalogFailure::Read(ReadError::ForeignCatalog))
    ));
}

#[test]
fn constructor_only_text_requires_a_covering_prefix() {
    let mut owner = AtomInterner::new();
    let old = owner.publish_selection_with(&[], limits(), PERMIT).unwrap();
    let declaration = owner
        .split()
        .1
        .declare_constructor_with(
            ValueNodeRef::Function {
                name: "later_name",
                sign: Sign::Negative,
                arity: 3,
            },
            limits(),
            PERMIT,
        )
        .unwrap();
    let mut components = TemplateComponents::new(owner.read(), METADATA).unwrap();
    components
        .append_constructor_with(owner.read(), &declaration, METADATA, PERMIT)
        .unwrap();
    assert!(matches!(
        components.bind_with(old.read(), PERMIT),
        Err(TemplateCatalogFailure::Read(ReadError::OutsidePrefix))
    ));
    let current = owner.publish_selection_with(&[], limits(), PERMIT).unwrap();
    let bound = components.bind_with(current.read(), PERMIT).unwrap();
    assert_eq!(
        bound.constructor(0),
        Some(ValueNodeRef::Function {
            name: "later_name",
            sign: Sign::Negative,
            arity: 3
        })
    );
    assert_eq!(current.storage().atoms, 0);
    assert_eq!(current.storage().terms, 0);
}

#[test]
fn binding_requires_every_referenced_term() {
    let mut owner = owner();
    let old = owner.publish_selection_with(&[], limits(), PERMIT).unwrap();
    let key = owner
        .split()
        .1
        .import_term_with(
            (&Value::Number(42)).into(),
            crate::catalog::Limits::default(),
            limits(),
            PERMIT,
        )
        .unwrap();
    let mut components = TemplateComponents::new(owner.read(), METADATA).unwrap();
    let term = owner.read().term(&key).unwrap();
    components
        .append_terms_with(
            owner.read(),
            [TemplateTerm::Constant(term)],
            METADATA,
            PERMIT,
        )
        .unwrap();
    assert!(matches!(
        components.bind_with(old.read(), PERMIT),
        Err(TemplateCatalogFailure::Read(ReadError::OutsidePrefix))
    ));
}

#[test]
fn binding_requires_every_referenced_predicate() {
    let mut owner = owner();
    let old = owner.publish_selection_with(&[], limits(), PERMIT).unwrap();
    let key = owner
        .declare_predicate_with(&Predicate::new("later", 0).unwrap(), limits(), PERMIT)
        .unwrap();
    let mut components = TemplateComponents::new(owner.read(), METADATA).unwrap();
    let pattern = PatternRef::from_parts(owner.read().predicate(&key).unwrap(), &[]).unwrap();
    components
        .append_pattern_with(owner.read(), pattern, METADATA, PERMIT)
        .unwrap();
    assert!(matches!(
        components.bind_with(old.read(), PERMIT),
        Err(TemplateCatalogFailure::Read(ReadError::OutsidePrefix))
    ));
}

#[test]
fn append_refuses_ingress_constants_and_poisons_binding() {
    let owner = owner();
    let mut components = TemplateComponents::new(owner.read(), METADATA).unwrap();
    let input = Value::Number(42);
    assert_eq!(
        components.append_terms_with(
            owner.read(),
            [TemplateTerm::Constant((&input).into())],
            METADATA,
            PERMIT
        ),
        Err(TemplateCatalogFailure::Read(ReadError::Uninterned))
    );
    assert!(matches!(
        components.bind_with(owner.read(), PERMIT),
        Err(TemplateCatalogFailure::Incomplete)
    ));
}

fn append_all<E>(
    components: &mut TemplateComponents,
    owner: &AtomInterner,
    constructor: &DeclaredConstructor,
    mut before: impl FnMut() -> Result<(), E>,
) -> Result<(), TemplateCatalogFailure<E>> {
    let atom = owner.get(0).unwrap();
    let predicate = owner.read().declare_existing(atom.predicate()).unwrap();
    components.append_predicate_with(owner.read(), &predicate, METADATA, &mut before)?;
    let constant = TemplateTerm::Constant(atom.values().at(0).unwrap());
    components.append_terms_with(
        owner.read(),
        [constant, TemplateTerm::Variable(3), constant],
        METADATA,
        &mut before,
    )?;
    let arguments = [constant];
    let pattern = PatternRef::from_parts(atom.predicate(), &arguments).unwrap();
    components.append_pattern_with(owner.read(), pattern, METADATA, &mut before)?;
    components.append_filter_with(
        owner.read(),
        FilterRef::from_parts(true, constant, TemplateTerm::Variable(3)),
        METADATA,
        &mut before,
    )?;
    components.append_constructor_with(owner.read(), constructor, METADATA, before)?;
    Ok(())
}

fn constructor(owner: &mut AtomInterner) -> DeclaredConstructor {
    owner
        .split()
        .1
        .declare_constructor_with(
            ValueNodeRef::Function {
                name: "shared",
                sign: Sign::Negative,
                arity: 2,
            },
            limits(),
            PERMIT,
        )
        .unwrap()
}

#[test]
fn every_component_work_cutoff_poison_prevents_exposure() {
    let mut owner = owner();
    let constructor = constructor(&mut owner);
    let mut measured = TemplateComponents::new(owner.read(), METADATA).unwrap();
    let mut work = 0;
    append_all(&mut measured, &owner, &constructor, || {
        work += 1;
        Ok::<(), usize>(())
    })
    .unwrap();
    for cut in 0..work {
        let mut components = TemplateComponents::new(owner.read(), METADATA).unwrap();
        let mut visited = 0;
        let result = append_all(&mut components, &owner, &constructor, || {
            if visited == cut {
                return Err(cut);
            }
            visited += 1;
            Ok(())
        });
        assert_eq!(result, Err(TemplateCatalogFailure::Stopped(cut)));
        assert_eq!(visited, cut);
        assert!(components.storage_bytes() <= measured.storage_bytes());
        assert!(components.storage_peak_bytes() <= measured.storage_peak_bytes());
        assert!(matches!(
            components.bind_with(owner.read(), PERMIT),
            Err(TemplateCatalogFailure::Incomplete)
        ));
        assert!(matches!(
            components.append_terms_with(owner.read(), [], METADATA, PERMIT),
            Err(TemplateCatalogFailure::Incomplete)
        ));
    }
}

#[test]
fn every_component_callback_panic_poisons_binding() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let mut owner = owner();
    let constructor = constructor(&mut owner);
    let mut measured = TemplateComponents::new(owner.read(), METADATA).unwrap();
    let mut work = 0;
    append_all(&mut measured, &owner, &constructor, || {
        work += 1;
        PERMIT()
    })
    .unwrap();
    for cut in 0..work {
        let mut components = TemplateComponents::new(owner.read(), METADATA).unwrap();
        let mut visited = 0;
        assert!(
            catch_unwind(AssertUnwindSafe(|| append_all(
                &mut components,
                &owner,
                &constructor,
                || {
                    assert_ne!(visited, cut, "injected callback panic");
                    visited += 1;
                    PERMIT()
                }
            )))
            .is_err()
        );
        assert_eq!(visited, cut);
        assert!(matches!(
            components.bind_with(owner.read(), PERMIT),
            Err(TemplateCatalogFailure::Incomplete)
        ));
    }
}

#[test]
fn an_input_iterator_panic_poisons_partial_terms() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let owner = owner();
    let mut components = TemplateComponents::new(owner.read(), METADATA).unwrap();
    let mut advances = 0;
    let terms = std::iter::from_fn(|| {
        advances += 1;
        assert_eq!(advances, 1, "injected iterator panic");
        Some(TemplateTerm::Variable(0))
    });
    assert!(
        catch_unwind(AssertUnwindSafe(|| components.append_terms_with(
            owner.read(),
            terms,
            METADATA,
            PERMIT
        )))
        .is_err()
    );
    assert_eq!(components.data.terms.len(), 1);
    assert!(matches!(
        components.bind_with(owner.read(), PERMIT),
        Err(TemplateCatalogFailure::Incomplete)
    ));
}

#[test]
fn receipts_count_actual_nested_capacities_once() {
    let mut owner = owner();
    let constructor = constructor(&mut owner);
    let mut components = TemplateComponents::new(owner.read(), METADATA).unwrap();
    append_all(&mut components, &owner, &constructor, PERMIT).unwrap();
    let expected = size_of::<TemplateComponents>()
        + components.data.terms.capacity() * size_of::<TermData>()
        + components.data.patterns.capacity() * size_of::<PatternData>()
        + components.data.filters.capacity() * size_of::<FilterData>()
        + components.constructors.capacity() * size_of::<ConstructorData>()
        + components.predicates.capacity() * size_of::<crate::catalog::storage::PredicateId>()
        + components
            .data
            .patterns
            .iter()
            .map(|pattern| pattern.terms.capacity() * size_of::<TermData>())
            .sum::<usize>();
    assert_eq!(components.storage_bytes(), expected as u128);
}

#[test]
fn optional_growth_falls_back_to_the_funded_capacity() {
    let owner = owner();
    let mut components = TemplateComponents::new(owner.read(), METADATA).unwrap();
    components
        .append_terms_with(
            owner.read(),
            [TemplateTerm::Variable(0); 4],
            METADATA,
            PERMIT,
        )
        .unwrap();
    let old_capacity = components.data.terms.capacity();
    let required = old_capacity + 1;
    let limit =
        usize::try_from(components.storage_bytes()).unwrap() + required * size_of::<TermData>();
    components.restart_storage_peak();
    let extra = required - components.data.terms.len();
    components
        .append_terms_with(
            owner.read(),
            std::iter::repeat_n(TemplateTerm::Variable(0), extra),
            limit,
            PERMIT,
        )
        .unwrap();
    assert_eq!(components.data.terms.capacity(), required);
    assert_eq!(components.storage_peak_bytes(), limit as u128);
}

#[test]
fn rejected_reservation_does_not_invent_an_actual_peak() {
    let owner = owner();
    let mut components = TemplateComponents::new(owner.read(), METADATA).unwrap();
    let bytes = components.storage_bytes();
    assert!(matches!(
        components.append_terms_with(
            owner.read(),
            [TemplateTerm::Variable(0)],
            usize::try_from(bytes).unwrap(),
            PERMIT
        ),
        Err(TemplateCatalogFailure::Storage(Error::Storage { .. }))
    ));
    assert_eq!(components.storage_bytes(), bytes);
    assert_eq!(components.storage_peak_bytes(), bytes);
    assert_eq!(components.data.terms.capacity(), 0);
}

#[test]
fn binding_work_does_not_scale_with_occurrence_count() {
    let owner = owner();
    let mut work = Vec::new();
    for count in [0, 1, 1000] {
        let mut components = TemplateComponents::new(owner.read(), METADATA).unwrap();
        components
            .append_terms_with(
                owner.read(),
                std::iter::repeat_n(TemplateTerm::Variable(0), count),
                METADATA,
                PERMIT,
            )
            .unwrap();
        let mut visited = 0;
        components
            .bind_with(owner.read(), || {
                visited += 1;
                PERMIT()
            })
            .unwrap();
        work.push(visited);
    }
    assert_eq!(work, [4, 4, 4]);
}

#[test]
fn recovered_constructor_handle_outlives_the_reader_borrow() {
    let mut owner = owner();
    let constructor = constructor(&mut owner);
    let mut components = TemplateComponents::new(owner.read(), METADATA).unwrap();
    components
        .append_constructor_with(owner.read(), &constructor, METADATA, PERMIT)
        .unwrap();
    let recovered = {
        let bound = components.bind_with(owner.read(), PERMIT).unwrap();
        bound.declared_constructor(0).unwrap()
    };
    // This mutation could not compile if the recovered declaration retained the
    // reader borrow. Its text ID remains valid after unrelated vocabulary growth.
    owner
        .split()
        .1
        .import_term_with(
            (&Value::Number(42)).into(),
            crate::catalog::Limits::default(),
            limits(),
            PERMIT,
        )
        .unwrap();
    let recovered = recovered.with_sign(Sign::Positive).unwrap();
    let position = components
        .append_constructor_with(owner.read(), &recovered, METADATA, PERMIT)
        .unwrap();
    let bound = components.bind_with(owner.read(), PERMIT).unwrap();
    assert_eq!(
        bound.constructor(position),
        Some(ValueNodeRef::Function {
            name: "shared",
            sign: Sign::Positive,
            arity: 2
        })
    );
}

#[test]
fn constructor_recovery_refuses_an_absent_occurrence() {
    let owner = owner();
    let components = TemplateComponents::new(owner.read(), METADATA).unwrap();
    assert!(
        components
            .bind_with(owner.read(), PERMIT)
            .unwrap()
            .declared_constructor(0)
            .is_none()
    );
}

#[test]
fn standalone_predicates_preserve_signed_occurrences_without_atoms() {
    let mut owner = AtomInterner::new();
    let old = owner.publish_selection_with(&[], limits(), PERMIT).unwrap();
    let input = Predicate::with_sign("projection", 7, Sign::Negative).unwrap();
    let predicate = owner
        .declare_predicate_with(&input, limits(), PERMIT)
        .unwrap();
    let mut components = TemplateComponents::new(owner.read(), METADATA).unwrap();
    for expected in 0..2 {
        assert_eq!(
            components
                .append_predicate_with(owner.read(), &predicate, METADATA, PERMIT)
                .unwrap(),
            expected
        );
    }
    assert!(matches!(
        components.bind_with(old.read(), PERMIT),
        Err(TemplateCatalogFailure::Read(ReadError::OutsidePrefix))
    ));
    let current = owner.publish_selection_with(&[], limits(), PERMIT).unwrap();
    let view = components.bind_with(current.read(), PERMIT).unwrap();
    assert_eq!(view.predicate(0), view.predicate(1));
    let found = view.predicate(0).unwrap();
    assert_eq!(found.compare(&input), std::cmp::Ordering::Equal);
    assert!(std::ptr::eq(
        found.name(),
        current.read().predicate(&predicate).unwrap().name()
    ));
    assert!(view.predicate(2).is_none());
    assert_eq!(current.storage().atoms, 0);
    assert_eq!(current.storage().terms, 0);
}

#[test]
fn standalone_predicate_refusal_preserves_actual_receipt_and_poison() {
    let mut owner = AtomInterner::new();
    let declared = owner
        .declare_predicate_with(&Predicate::new("p", 0).unwrap(), limits(), PERMIT)
        .unwrap();
    let mut components = TemplateComponents::new(owner.read(), METADATA).unwrap();
    let before = components.storage_bytes();
    assert!(matches!(
        components.append_predicate_with(
            owner.read(),
            &declared,
            usize::try_from(before).unwrap(),
            PERMIT
        ),
        Err(TemplateCatalogFailure::Storage(Error::Storage { .. }))
    ));
    assert_eq!(components.storage_bytes(), before);
    assert_eq!(components.storage_peak_bytes(), before);
    assert!(components.predicates.is_empty());
    assert!(matches!(
        components.bind_with(owner.read(), PERMIT),
        Err(TemplateCatalogFailure::Incomplete)
    ));
}

#[test]
fn standalone_predicate_cannot_adopt_a_foreign_declaration() {
    let mut owner = AtomInterner::new();
    let mut foreign = AtomInterner::new();
    let input = Predicate::new("equal_spelling", 0).unwrap();
    owner
        .declare_predicate_with(&input, limits(), PERMIT)
        .unwrap();
    let foreign = foreign
        .declare_predicate_with(&input, limits(), PERMIT)
        .unwrap();
    let mut components = TemplateComponents::new(owner.read(), METADATA).unwrap();
    assert_eq!(
        components.append_predicate_with(owner.read(), &foreign, METADATA, PERMIT),
        Err(TemplateCatalogFailure::Read(ReadError::ForeignCatalog))
    );
    assert!(matches!(
        components.bind_with(owner.read(), PERMIT),
        Err(TemplateCatalogFailure::Incomplete)
    ));
}
