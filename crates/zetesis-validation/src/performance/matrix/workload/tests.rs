//! Native workload byte diagnostics and success-only admission.

use super::{Budget, Error, WorkloadLimits};

fn budget() -> Budget {
    Budget {
        limits: WorkloadLimits {
            metadata_bytes: 3,
            source_bytes: 3,
            total_source_bytes: 5,
            ..WorkloadLimits::default()
        },
        metadata: 0,
        sources: 0,
    }
}

#[test]
fn metadata_refusal_preserves_the_accepted_text_prefix() {
    let mut budget = budget();
    assert_eq!(budget.text("abc").unwrap(), "abc");
    budget.metadata(0).unwrap();
    assert!(matches!(
        budget.text("d"),
        Err(Error::Configuration(
            "workload metadata exceeds its byte ceiling"
        ))
    ));
    assert_eq!(budget.metadata, 3);
    assert_eq!(budget.sources, 0);
}

#[test]
fn metadata_overflow_precedes_the_authored_ceiling() {
    let mut budget = budget();
    budget.metadata = usize::MAX;
    assert!(matches!(
        budget.metadata(1),
        Err(Error::Configuration("workload byte count overflow"))
    ));
    assert_eq!(budget.metadata, usize::MAX);
}

#[test]
fn source_admission_keeps_individual_and_total_ceilings() {
    let mut budget = budget();
    budget.source(3).unwrap();
    budget.source(2).unwrap();
    budget.source(0).unwrap();
    assert!(matches!(
        budget.source(1),
        Err(Error::Configuration(
            "workload source exceeds its byte ceiling"
        ))
    ));
    assert_eq!(budget.sources, 5);
    let mut budget = self::budget();
    assert!(matches!(
        budget.source(4),
        Err(Error::Configuration(
            "workload source exceeds its byte ceiling"
        ))
    ));
    assert_eq!(budget.sources, 0);
}

#[test]
fn source_sum_overflow_precedes_even_a_per_file_refusal() {
    let mut budget = budget();
    budget.sources = usize::MAX;
    budget.limits.source_bytes = 0;
    assert!(matches!(
        budget.source(1),
        Err(Error::Configuration("workload byte count overflow"))
    ));
    assert_eq!(budget.sources, usize::MAX);
}
