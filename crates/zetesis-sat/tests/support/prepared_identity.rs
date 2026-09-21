//! Only the exact immutable prepared owner authorizes topology reuse.

use super::*;
use crate::ReductPreparationLimits;
use zetesis_ferraris::{Node, Theory};

fn theory(root: usize) -> Theory {
    Theory::new(
        2,
        vec![Node::Atom(0), Node::Atom(1)],
        vec![root],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap()
}

fn prepare(theory: &Theory) -> PreparedReduct {
    PreparedReduct::prepare(
        theory,
        ReductPreparationLimits::default(),
        &Cancellation::default(),
    )
    .result
    .unwrap()
}

fn check(
    owner: &PreparedReduct,
    workspace: &mut ReductWorkspace,
) -> (Check, ReductQueryStatistics) {
    owner.check(
        &Interpretation::new(owner.theory(), [0, 1]).unwrap(),
        workspace,
        Limits::default(),
        &Cancellation::default(),
    )
}

#[test]
fn equal_shape_owners_require_their_own_countermodels() {
    let first = prepare(&theory(0));
    let second = prepare(&theory(1));
    assert_eq!(first.cnf_shape(), second.cnf_shape());
    let mut workspace = ReductWorkspace::default();
    for (owner, atom) in [(&first, 0), (&second, 1), (&first, 0)] {
        let (result, _) = check(owner, &mut workspace);
        let Check::NonMinimal(witness) = result else {
            panic!("expected proper subset: {result:?}")
        };
        assert_eq!(witness.atoms().collect::<Vec<_>>(), [atom]);
        assert!(witness.theory().same_instance(owner.theory()));
        assert!(workspace.owner.ptr_eq(&Arc::downgrade(&owner.0)));
    }
}

#[test]
fn only_same_owner_reuses_the_watch_index() {
    let theory = theory(0);
    let first = prepare(&theory);
    let second = prepare(&theory);
    let mut workspace = ReductWorkspace::default();
    let (_, cold) = check(&first, &mut workspace);
    let (_, warm) = check(&first.clone(), &mut workspace);
    assert!(warm.statistics.search.work < cold.statistics.search.work);
    let (_, switched) = check(&second, &mut workspace);
    assert_eq!(switched.statistics.search.work, cold.statistics.search.work);
    assert!(workspace.owner.ptr_eq(&Arc::downgrade(&second.0)));
    assert!(!workspace.owner.ptr_eq(&Arc::downgrade(&first.0)));
}

#[test]
fn workspace_identity_does_not_retain_prepared_payload() {
    let theory = theory(0);
    let owner = prepare(&theory);
    let identity = Arc::downgrade(&owner.0);
    let mut workspace = ReductWorkspace::default();
    let (_, cold) = check(&owner, &mut workspace);
    drop(owner);
    assert!(identity.upgrade().is_none());
    assert!(workspace.owner.ptr_eq(&identity));
    // The retained weak token prevents the old allocation's identity from being
    // recycled. A new preparation of the same theory must index independently.
    let replacement = prepare(&theory);
    assert!(!identity.ptr_eq(&Arc::downgrade(&replacement.0)));
    let (_, fresh) = check(&replacement, &mut workspace);
    assert_eq!(fresh.statistics.search.work, cold.statistics.search.work);
}

#[test]
fn lowered_capacity_refusal_preserves_owner_for_retry() {
    let owner = prepare(&theory(0));
    let mut workspace = ReductWorkspace::default();
    check(&owner, &mut workspace);
    let retained = workspace.retained_bytes();
    let limit = u64::try_from(retained).unwrap() - 1;
    let candidate = Interpretation::new(owner.theory(), [0, 1]).unwrap();
    let (result, receipt) = owner.check(
        &candidate,
        &mut workspace,
        Limits {
            max_reduct_bytes: limit,
            ..Default::default()
        },
        &Cancellation::default(),
    );
    assert!(
        matches!(result, Check::Inconclusive(Incomplete::ReductStorage { required, limit: observed }) if required == retained && observed == u128::from(limit))
    );
    assert_eq!(receipt.statistics.search.work, 0);
    assert_eq!(receipt.retained_bytes, retained);
    assert!(workspace.owner.ptr_eq(&Arc::downgrade(&owner.0)));
    let (retried, _) = check(&owner, &mut workspace);
    assert!(matches!(retried, Check::NonMinimal(_)));
}
