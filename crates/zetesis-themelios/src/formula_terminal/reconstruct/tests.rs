use super::*;
use crate::{AdmissionOptions, ExpansionLimits, FormulaLimits, FormulaMaterialization};

fn terminal() -> TerminalFormula {
    let FormulaMaterialization::Terminal(owner) = crate::prepare_formula(
        "{seed(1..3)}. left(X):-seed(X). right(X):-seed(X).".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_adaptive()
    .unwrap() else {
        panic!("terminal profile required")
    };
    assert_eq!(owner.deferred_templates(), 2);
    owner
}

fn selection(owner: &TerminalFormula) -> Model {
    Model::from_positions(
        owner.base_atom_catalog(),
        0..owner.base_atom_catalog().atoms().len(),
    )
    .unwrap()
}

#[test]
fn failed_preparation_publishes_no_partial_plan() {
    let mut owner = terminal();
    // Make the second retained rule malformed after the first has prepared.
    // The public partition cannot produce this state; the preparation boundary
    // still rejects it without keeping a partially authenticated plan.
    std::sync::Arc::get_mut(&mut owner.0)
        .unwrap()
        .extension
        .deferred[1]
        .head = crate::formula_ir::HeadIr::Normal(None);
    let model = selection(&owner);
    let mut cursor = owner.reconstruction().unwrap();
    assert!(
        cursor
            .reconstruct(&model, &Cancellation::default())
            .is_err()
    );
    assert!(cursor.plan.is_none());
    assert_eq!(cursor.statistics().completed, 0);
    assert!(matches!(
        cursor.reconstruct(&model, &Cancellation::default()),
        Err(ReconstructionError::Failed)
    ));
}

#[test]
fn a_reused_plan_is_charged_before_opening_a_writer() {
    let owner = terminal();
    let model = selection(&owner);
    let mut cursor = owner.reconstruction().unwrap();
    cursor
        .reconstruct(&model, &Cancellation::default())
        .unwrap();
    let mut plan = cursor.plan.take();
    let plan_bytes = plan.as_ref().unwrap().retained_bytes();
    assert!(plan_bytes > 0);
    let prepared = &owner.0.extension;
    let before_plan = prepared.metadata_bytes
        + prepared.closed.metadata_bytes()
        + size_of::<ClosedCatalog>() as u128
        + model.selection_bytes()
        + size_of::<TerminalReconstruction<'_>>() as u128;
    // A fresh writer does not yet exist. Even this initial admission must
    // include the session's already retained plan alongside its other owners.
    let limits = FormulaLimits {
        max_support_bytes: usize::try_from(before_plan).unwrap(),
        ..prepared.limits
    };
    let mut accounting = prepared.baseline.start();
    let failure = accounting
        .with_cancellation(&Cancellation::default(), |counters| {
            extend(&owner, &mut plan, &model, counters, &limits)
        })
        .unwrap_err();
    assert!(matches!(
        failure,
        ReconstructionError::Source(cause)
            if matches!(*cause, FormulaFailure::Limit {
                resource: crate::FormulaResource::SupportBytes,
                observed,
                ..
            } if observed == before_plan + plan_bytes)
    ));
}
