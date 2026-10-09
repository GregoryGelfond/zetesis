use super::*;
use crate::{AdmissionLimits, FormulaParts, Node, Theory, TightPlanLimits};

#[test]
fn checks_retain_their_scratch_allocations() {
    let theory = Theory::new(
        1,
        FormulaParts::new(vec![Node::atom(0)], vec![]).unwrap(),
        vec![0],
        AdmissionLimits::default(),
    )
    .unwrap();
    let cancellation = Cancellation::default();
    let plan = TightPlan::compile(&theory, TightPlanLimits::default(), &cancellation).unwrap();
    let present = Interpretation::new(&theory, [0]).unwrap();
    let absent = Interpretation::new(&theory, []).unwrap();
    let mut workspace = TightWorkspace::default();
    workspace
        .check(&plan, &present, TightCheckLimits::default(), &cancellation)
        .result
        .unwrap();
    let values = workspace.values.as_ptr();
    let supported = workspace.supported.as_ptr();
    let retained = workspace.retained_bytes();
    assert!(!workspace.values.is_empty());
    assert!(!workspace.supported.is_empty());
    for candidate in [&absent, &present, &absent, &present] {
        workspace
            .check(&plan, candidate, TightCheckLimits::default(), &cancellation)
            .result
            .unwrap();
        assert_eq!(workspace.values.as_ptr(), values);
        assert_eq!(workspace.supported.as_ptr(), supported);
        assert_eq!(workspace.retained_bytes(), retained);
    }
}
