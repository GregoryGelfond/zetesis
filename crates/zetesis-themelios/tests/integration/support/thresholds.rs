//! The least resource threshold at which an attempt succeeds.

use zetesis_themelios::{
    AdmissionOptions, ExpansionFailure, ExpansionLimits, ExpansionResource, FormulaFailure,
    FormulaLimits, prepare_formula,
};

/// The least threshold at which `attempt` succeeds, found by doubling and then
/// bisecting; `attempt` must succeed at every threshold above its least.
pub fn first_success(mut attempt: impl FnMut(u64) -> bool) -> u64 {
    let mut high = 1;
    while !attempt(high) {
        high *= 2;
        assert!(high <= 1_048_576, "bounded resource threshold probe");
    }
    let mut low = 0;
    while low + 1 < high {
        let middle = low + (high - low) / 2;
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
pub fn minimum_preparation_bytes(source: &str) -> usize {
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
    let (mut lower, mut upper) = (0, ExpansionLimits::default().max_scalar_bytes);
    assert!(prepare(upper).is_ok());
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        if prepare(middle).is_ok() {
            upper = middle;
        } else {
            lower = middle + 1;
        }
    }
    assert!(prepare(lower).is_ok());
    assert!(matches!(
        prepare(lower - 1),
        Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::ScalarBytes,
            ..
        }))
    ));
    lower
}
