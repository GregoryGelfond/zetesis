//! Exact completion and qualification boundaries, independent of device access.

use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, Theory};
use zetesis_wgpu::{FormulaBatchStats, FormulaVerdict};

use crate::FormulaBenchmarkError;
use crate::formula_fixtures::reserve;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Membership {
    NotModel,
    Stable,
    NonMinimal,
}

pub(super) fn native(
    theory: &Theory,
    candidate: &Interpretation,
    work: u64,
) -> Result<Membership, FormulaBenchmarkError> {
    native_with_control(theory, candidate, work, &Control::default())
}

pub(super) fn native_with_control(
    theory: &Theory,
    candidate: &Interpretation,
    work: u64,
    control: &Control,
) -> Result<Membership, FormulaBenchmarkError> {
    let limits = zetesis_sat::Limits {
        search: zetesis_sat::SearchLimits {
            max_work: work,
            ..Default::default()
        },
        max_verification_work: work,
        ..Default::default()
    };
    match zetesis_sat::check(theory, candidate, limits, control) {
        zetesis_sat::Check::Stable => Ok(Membership::Stable),
        zetesis_sat::Check::NotModel => Ok(Membership::NotModel),
        zetesis_sat::Check::NonMinimal(_) | zetesis_sat::Check::Unsupported { .. } => {
            Ok(Membership::NonMinimal)
        }
        zetesis_sat::Check::Inconclusive(error) => Err(FormulaBenchmarkError::Incomplete(error)),
    }
}

pub(super) fn complete_residuals(
    theory: &Theory,
    candidates: &[Interpretation],
    verdicts: impl ExactSizeIterator<Item = FormulaVerdict>,
    expected: &[Membership],
    work: u64,
) -> Result<(Vec<Membership>, usize), FormulaBenchmarkError> {
    if candidates.len() != verdicts.len() || candidates.len() != expected.len() {
        return Err(FormulaBenchmarkError::Parity);
    }
    let mut completed = reserve(candidates.len())?;
    let mut residuals = 0;
    for ((candidate, verdict), reference) in candidates.iter().zip(verdicts).zip(expected) {
        completed.push(match verdict {
            FormulaVerdict::NotModel => Membership::NotModel,
            FormulaVerdict::NoProperSubset => Membership::Stable,
            FormulaVerdict::Residual(_) => {
                if *reference == Membership::NotModel {
                    return Err(FormulaBenchmarkError::Parity);
                }
                residuals += 1;
                native(theory, candidate, work)?
            }
        });
    }
    Ok((completed, residuals))
}

pub(super) fn verify(
    actual: &[Membership],
    expected: &[Membership],
) -> Result<(), FormulaBenchmarkError> {
    if actual != expected {
        return Err(FormulaBenchmarkError::Parity);
    }
    Ok(())
}

pub(super) fn verify_residency(
    stats: Option<&FormulaBatchStats>,
    iteration: usize,
) -> Result<(), FormulaBenchmarkError> {
    let stats = stats.ok_or(FormulaBenchmarkError::Residency)?;
    if iteration > 0 && (stats.theory_uploaded || stats.transport_allocated) {
        return Err(FormulaBenchmarkError::Residency);
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/support/formula_completion.rs"]
mod tests;
