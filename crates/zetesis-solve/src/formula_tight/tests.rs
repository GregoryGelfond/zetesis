//! The device adapter's verdict mapping uses complete support evidence.

use zetesis_ferraris::{
    AdmissionLimits, Interpretation, Node, Theory, TightCheckLimits, TightPlan, TightPlanLimits,
};
use zetesis_sat::BatchVerdict;

#[test]
fn support_decisions_agree_with_independent_reducts() {
    // Include unproduced carrier atoms and original failures, not just stable
    // interpretations offered by the ordinary candidate proposer.
    for roots in [vec![], vec![0], vec![2], vec![0, 2]] {
        let theory = Theory::new(
            3,
            vec![Node::Atom(0), Node::Atom(1), Node::Implies(0, 1)],
            roots,
            AdmissionLimits::default(),
        )
        .unwrap();
        let cancellation = zetesis_cpu::Cancellation::default();
        let plan = TightPlan::compile(&theory, TightPlanLimits::default(), &cancellation).unwrap();
        for mask in 0..8 {
            let candidate =
                Interpretation::new(&theory, (0..3).filter(|atom| mask & (1 << atom) != 0))
                    .unwrap();
            let support = plan
                .check(&candidate, TightCheckLimits::default(), &cancellation)
                .unwrap();
            let exact = zetesis_ferraris::check(
                &theory,
                &candidate,
                zetesis_ferraris::Limits::default(),
                &cancellation,
            )
            .unwrap();
            match super::verdict(support.verdict) {
                BatchVerdict::NoProperSubset => assert!(exact.accepted()),
                BatchVerdict::Refuted => {
                    assert!(!exact.accepted());
                    assert!(!matches!(
                        exact.verdict(),
                        zetesis_ferraris::Verdict::NotModel { .. }
                    ));
                }
                BatchVerdict::NotModel => assert!(matches!(
                    exact.verdict(),
                    zetesis_ferraris::Verdict::NotModel { .. }
                )),
                BatchVerdict::Residual => panic!("complete tight evidence must decide membership"),
            }
        }
    }
}
