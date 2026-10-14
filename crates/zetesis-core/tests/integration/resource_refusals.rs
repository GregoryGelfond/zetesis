//! Public classification belongs to the owner of each resource failure.
use zetesis_core::{ModelError, ValueError, ValueResource, catalog::Error};

#[test]
fn catalog_classifies_every_capacity_and_identity_cause() {
    for refusal in [
        Error::Allocation,
        Error::Storage {
            required: 1,
            limit: 0,
        },
        Error::IdExhausted,
        Error::Overflow,
        Error::Value(ValueError::Allocation),
        Error::Value(ValueError::Limit {
            resource: ValueResource::Nodes,
            observed: 1,
            limit: 0,
        }),
    ] {
        assert!(refusal.is_resource_refusal(), "{refusal:?}");
    }
    for shape in [
        Error::Shape,
        Error::FrozenVocabulary,
        Error::VocabularyHasAtoms,
        Error::UnindexedVocabulary,
        Error::CatalogHasBase,
        Error::Value(ValueError::Shape),
    ] {
        assert!(!shape.is_resource_refusal(), "{shape:?}");
    }
}

#[test]
fn model_classification_keeps_nested_catalog_meaning() {
    for refusal in [
        ModelError::Allocation,
        ModelError::Bytes {
            required: 1,
            limit: 0,
        },
        ModelError::Catalog(Error::Allocation),
        ModelError::Catalog(Error::IdExhausted),
    ] {
        assert!(refusal.is_resource_refusal(), "{refusal:?}");
    }
    for shape in [
        ModelError::Order { position: 1 },
        ModelError::Position {
            position: 0,
            atoms: 0,
        },
        ModelError::Catalog(Error::Shape),
        ModelError::Catalog(Error::FrozenVocabulary),
    ] {
        assert!(!shape.is_resource_refusal(), "{shape:?}");
    }
}
