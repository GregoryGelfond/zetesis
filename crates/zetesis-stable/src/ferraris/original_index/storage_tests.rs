//! Additional native index storage is admitted before an owner publishes it.

use super::{OriginalIndex, hooks, prepare};
use crate::{Cancellation, Incomplete};
use zetesis_ferraris::{
    AdmissionLimits, ExtremumCutRule, ExtremumRow, FormulaNodes, FormulaParts, Narrower, Node,
    Theory,
};

fn theory(native: bool) -> Theory {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new(vec![Node::atom(0), Node::atom(1)], vec![]).unwrap());
    let roots = if native {
        let mut transaction = nodes.transaction();
        let roots = transaction
            .push_extremum_cut_family_range(
                ExtremumRow {
                    conditions: &[0, 1],
                    ranks: &[1, 2],
                    rank_count: 4,
                },
                &[ExtremumCutRule::Prefix(2), ExtremumCutRule::Prefix(3)],
                10,
                10,
            )
            .unwrap()
            .collect();
        transaction.commit();
        roots
    } else {
        vec![0]
    };
    Theory::new(2, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap()
}

#[test]
fn zero_native_allowance_preserves_boolean_admission() {
    let theory = theory(false);
    let index = prepare(&theory, 0, &Cancellation::default()).unwrap();
    assert_eq!(index.extremum_index_bytes(), 0);
}

#[test]
fn native_identity_preflight_refuses_before_build() {
    let theory = theory(true);
    let mut owner = OriginalIndex::new(&theory, 0);
    let mut charged = 0;
    let result = owner.ensure(&Cancellation::default(), |work| {
        charged += work;
        Ok(())
    });
    assert!(
        matches!(result,Err(Incomplete::NativeNarrowingStorage {required,limit:0}) if required>0)
    );
    assert!(charged > 0);
    assert!(owner.get().is_none());
    assert_eq!(hooks::builds(&theory), 0);
}

#[test]
fn actual_native_capacity_is_the_publication_boundary() {
    let theory = theory(true);
    let bytes = u64::try_from(Narrower::new(&theory).extremum_index_bytes()).unwrap();
    for limit in [bytes - 1, bytes] {
        let mut owner = OriginalIndex::new(&theory, limit);
        let result = owner.ensure(&Cancellation::default(), |_| Ok(()));
        if limit == bytes {
            assert_eq!(
                result.unwrap().narrower().extremum_index_bytes(),
                u128::from(bytes)
            );
            assert!(owner.get().is_some());
        } else {
            assert!(
                matches!(result,Err(Incomplete::NativeNarrowingStorage {required,limit}) if required==u128::from(bytes)&&limit==u128::from(bytes-1))
            );
            assert!(owner.get().is_none());
        }
    }
}

#[test]
fn retained_index_is_rechecked_before_reuse() {
    let theory = theory(true);
    let mut owner = OriginalIndex::new(&theory, u64::MAX);
    let bytes = owner
        .ensure(&Cancellation::default(), |_| Ok(()))
        .unwrap()
        .narrower()
        .extremum_index_bytes();
    owner.native_byte_limit = u64::try_from(bytes - 1).unwrap();
    assert!(
        matches!(owner.ensure(&Cancellation::default(),|_|panic!("no reconstruction charge")),Err(Incomplete::NativeNarrowingStorage {required,..}) if required==bytes)
    );
    assert_eq!(hooks::builds(&theory), 1);
    assert!(
        owner.get().is_some(),
        "refusal does not discard the retained owner"
    );
}

#[test]
fn native_reservation_failure_keeps_its_typed_cause() {
    let theory = theory(true);
    let mut owner = OriginalIndex::new(&theory, u64::MAX);
    hooks::fail_next_build();
    assert!(matches!(
        owner.ensure(&Cancellation::default(), |_| Ok(())),
        Err(Incomplete::Allocation)
    ));
    assert!(owner.get().is_none());
}

#[test]
fn cancelled_index_preparation_publishes_nothing() {
    let theory = theory(true);
    let mut owner = OriginalIndex::new(&theory, u64::MAX);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert!(matches!(
        owner.ensure(&cancellation, |_| panic!("cancelled before logical charge")),
        Err(Incomplete::Cancelled)
    ));
    assert!(owner.get().is_none());
    assert_eq!(hooks::builds(&theory), 0);
}

#[test]
fn objective_bound_publication_uses_the_same_allowance() {
    let theory = theory(true);
    let result = super::super::conditions::Bound::prepare(&theory, 1, 0, &Cancellation::default());
    assert!(matches!(
        result,
        Err(Incomplete::NativeNarrowingStorage { limit: 0, .. })
    ));
    assert_eq!(hooks::builds(&theory), 0);
}
