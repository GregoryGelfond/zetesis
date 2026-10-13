//! The objective plan of an admitted formula.

use zetesis_cpu::Cancellation;
use zetesis_themelios::AdmittedFormula;
use zetesis_themelios::objective_bound::{ObjectivePlan, ObjectivePlanLimits};

/// The objective plan of `input` under the default plan limits, which it must
/// fit.
pub(crate) fn exact(input: &AdmittedFormula) -> ObjectivePlan {
    ObjectivePlan::new(
        input.theory(),
        input.atoms(),
        input.objectives(),
        ObjectivePlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap()
}
