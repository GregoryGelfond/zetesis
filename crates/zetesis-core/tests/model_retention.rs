//! Canonical retention follows actual catalog owners and transactional admission.

use zetesis_core::retention::{ModelRetention, RetainedPayload, RetentionError};
use zetesis_core::{Atom, AtomCatalog, Model, Predicate, Value};

fn catalog() -> AtomCatalog {
    AtomCatalog::new(vec![
        Atom::new(Predicate::new("a", 0).unwrap(), vec![]).unwrap(),
        Atom::new(
            Predicate::new("hidden", 1).unwrap(),
            vec![Value::String("secret".into())],
        )
        .unwrap(),
    ])
}

// u64 catalog length8; nullary a18; hidden("secret")38. The selected record
// contributes its own length8 and position8, even though hidden is not selected.
const CATALOG_BYTES: usize = 8 + 18 + 38;
const ONE_SELECTION: usize = 8 + 8;

#[test]
fn shared_catalog_is_charged_once() {
    let owner = catalog();
    let first = Model::from_positions(&owner, [0]).unwrap();
    let second = Model::from_positions(&owner, [1]).unwrap();
    let mut ledger = ModelRetention::default();
    ledger.admit(&first, 21, usize::MAX).unwrap().commit();
    let required = CATALOG_BYTES + 2 * ONE_SELECTION + 21 + 1;
    ledger.admit(&second, 1, required).unwrap().commit();
    assert_eq!(
        ledger.payload(),
        RetainedPayload {
            catalogs: 1,
            catalog_bytes: CATALOG_BYTES,
            selection_bytes: 2 * ONE_SELECTION,
            associated_bytes: 22,
            bytes: required,
        }
    );
}

#[test]
fn equal_catalog_contents_do_not_merge_owners() {
    let first = Model::from_positions(&catalog(), [0]).unwrap();
    let second = Model::from_positions(&catalog(), [0]).unwrap();
    assert_eq!(first, second);
    assert!(!first.catalog().same_owner(second.catalog()));
    let mut ledger = ModelRetention::default();
    let required = 2 * (CATALOG_BYTES + ONE_SELECTION);
    ledger.admit(&first, 0, required).unwrap().commit();
    assert_eq!(
        ledger.admit(&second, 0, required - 1).unwrap_err(),
        RetentionError::Bytes {
            required,
            limit: required - 1
        },
    );
    ledger.admit(&second, 0, required).unwrap().commit();
    assert_eq!(ledger.payload().catalogs, 2);
    assert_eq!(ledger.payload().catalog_bytes, 2 * CATALOG_BYTES);
    assert_eq!(ledger.payload().bytes, required);
}

#[test]
fn repeated_model_entries_keep_separate_selection_charges() {
    let model = Model::from_positions(&catalog(), [0]).unwrap();
    let clone = model.clone();
    assert_eq!(model.positions().as_ptr(), clone.positions().as_ptr());
    let mut ledger = ModelRetention::default();
    ledger.admit(&model, 0, usize::MAX).unwrap().commit();
    ledger.admit(&clone, 0, usize::MAX).unwrap().commit();
    assert_eq!(ledger.payload().bytes, CATALOG_BYTES + 2 * ONE_SELECTION);
}

#[test]
fn mixed_owner_order_preserves_complete_accounting() {
    let models: Vec<_> = (0..257)
        .map(|_| Model::from_positions(&catalog(), [0]).unwrap())
        .collect();
    let mut ledger = ModelRetention::default();
    // A permutation visits every distinct allocation, then revisits those same
    // owners in reverse. Equal atom contents cannot identify a catalog here.
    for index in 0..models.len() {
        ledger
            .admit(&models[(index * 17) % models.len()], 0, usize::MAX)
            .unwrap()
            .commit();
    }
    for model in models.iter().rev() {
        ledger.admit(model, 0, usize::MAX).unwrap().commit();
    }
    assert_eq!(ledger.payload().catalogs, models.len());
    assert_eq!(ledger.payload().catalog_bytes, models.len() * CATALOG_BYTES);
    assert_eq!(
        ledger.payload().selection_bytes,
        2 * models.len() * ONE_SELECTION
    );
}

#[test]
fn abandoned_admission_does_not_register_an_owner() {
    let first = Model::from_positions(&catalog(), [0]).unwrap();
    let second = Model::from_positions(&catalog(), [1]).unwrap();
    let mut ledger = ModelRetention::default();
    ledger.admit(&first, 0, usize::MAX).unwrap().commit();
    let before = ledger.payload();
    let admission = ledger.admit(&second, 0, usize::MAX).unwrap();
    assert_eq!(admission.payload().catalogs, 2);
    // The consumer can fail to reserve its own answer slot after this prepare.
    drop(admission);
    assert_eq!(ledger.payload(), before);
    let required = before.bytes + CATALOG_BYTES + ONE_SELECTION;
    assert_eq!(
        ledger.admit(&second, 0, required - 1).unwrap_err(),
        RetentionError::Bytes {
            required,
            limit: required - 1
        },
    );
}

#[test]
fn failed_replacement_preserves_the_prior_family() {
    let old = Model::from_positions(&catalog(), [0]).unwrap();
    let new = Model::from_positions(&catalog(), [1]).unwrap();
    let mut ledger = ModelRetention::default();
    ledger.admit(&old, 9, usize::MAX).unwrap().commit();
    let before = ledger.payload();
    let required = CATALOG_BYTES + ONE_SELECTION + 21;
    assert_eq!(
        ledger.replace(&new, 21, required - 1).unwrap_err(),
        RetentionError::Bytes {
            required,
            limit: required - 1
        },
    );
    assert_eq!(ledger.payload(), before);
    drop(ledger.replace(&new, 21, required).unwrap());
    assert_eq!(ledger.payload(), before);
    // A retained old owner must still be recognized after both refusals.
    ledger
        .admit(&old, 0, before.bytes + ONE_SELECTION)
        .unwrap()
        .commit();
    assert_eq!(ledger.payload().catalogs, 1);
}

#[test]
fn replacement_discards_the_old_owner_charge() {
    let old = Model::from_positions(&catalog(), [0]).unwrap();
    let new = Model::default();
    let mut ledger = ModelRetention::default();
    ledger.admit(&old, 21, usize::MAX).unwrap().commit();
    ledger.replace(&new, 1, 8 + 8 + 1).unwrap().commit();
    assert_eq!(ledger.payload().bytes, 17);
    assert_eq!(ledger.payload().catalog_bytes, 8);
    assert_eq!(ledger.payload().associated_bytes, 1);
    let required = 17 + CATALOG_BYTES + ONE_SELECTION;
    ledger.admit(&old, 0, required).unwrap().commit();
    assert_eq!(ledger.payload().catalogs, 2);
    assert_eq!(ledger.payload().bytes, required);
}

#[test]
fn associated_payload_overflow_preserves_the_prefix() {
    let model = Model::default();
    let mut ledger = ModelRetention::default();
    ledger.admit(&model, 1, usize::MAX).unwrap().commit();
    let before = ledger.payload();
    assert_eq!(
        ledger.admit(&model, usize::MAX, usize::MAX).unwrap_err(),
        RetentionError::Overflow
    );
    assert_eq!(ledger.payload(), before);
    assert_eq!(
        ledger.replace(&model, usize::MAX, usize::MAX).unwrap_err(),
        RetentionError::Overflow
    );
    assert_eq!(ledger.payload(), before);
}

#[test]
fn lower_ceiling_checks_the_whole_retained_payload() {
    let model = Model::default();
    let mut ledger = ModelRetention::default();
    ledger.admit(&model, 0, 16).unwrap().commit();
    assert_eq!(
        ledger.admit(&model, 0, 0).unwrap_err(),
        RetentionError::Bytes {
            required: 24,
            limit: 0
        },
    );
    assert_eq!(ledger.payload().bytes, 16);
}

#[test]
fn clear_starts_a_new_retained_family() {
    let model = Model::from_positions(&catalog(), [0]).unwrap();
    let mut ledger = ModelRetention::default();
    ledger.admit(&model, 21, usize::MAX).unwrap().commit();
    ledger.clear();
    assert_eq!(ledger.payload(), RetainedPayload::default());
    ledger
        .admit(&model, 0, CATALOG_BYTES + ONE_SELECTION)
        .unwrap()
        .commit();
    assert_eq!(ledger.payload().bytes, CATALOG_BYTES + ONE_SELECTION);
}
