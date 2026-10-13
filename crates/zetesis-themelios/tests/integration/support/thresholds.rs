//! The least resource threshold at which an attempt succeeds.

use zetesis_themelios::{
    AdmissionOptions, ExpansionFailure, ExpansionLimits, ExpansionResource, FormulaFailure,
    FormulaLimits, prepare_formula,
};

/// The least threshold at which `attempt` succeeds, found by doubling and then
/// bisecting. `attempt` must fail at zero, since the threshold is a resource
/// the operation charges, and succeed at every threshold from its least on;
/// every threshold the search tries is passed to `attempt`, which may check
/// how a failure refuses.
pub(crate) fn first_success(mut attempt: impl FnMut(u64) -> bool) -> u64 {
    assert!(!attempt(0), "the operation must charge this resource");
    let mut high = 1;
    while !attempt(high) {
        high *= 2;
        assert!(high <= 1 << 24, "bounded resource threshold probe");
    }
    let mut low = 0;
    while low + 1 < high {
        let middle = low.midpoint(high);
        if attempt(middle) {
            high = middle;
        } else {
            low = middle;
        }
    }
    high
}

/// The least `max_scalar_bytes` under which `source` prepares; one byte
/// less refuses it for its scalar bytes.
pub(crate) fn minimum_preparation_bytes(source: &str) -> usize {
    let prepare = |cap| {
        prepare_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits {
                max_scalar_bytes: cap,
                ..ExpansionLimits::default()
            },
            FormulaLimits::default(),
        )
    };
    let least = first_success(|cap| match prepare(usize::try_from(cap).unwrap()) {
        Ok(_) => true,
        Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::ScalarBytes,
            ..
        })) => false,
        Err(other) => panic!("unexpected refusal: {other}"),
    });
    usize::try_from(least).unwrap()
}
