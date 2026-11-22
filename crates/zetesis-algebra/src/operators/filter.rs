//! Residual Filter on already-formed bindings.

use super::arithmetic::{EnumerationStats, evaluate_arithmetic, evaluate_comparison};
use super::relation::Bindings;
use super::template::Template;

/// Keep the rows of `bindings` that satisfy the template's residual tests.
///
/// Arithmetic and comparison kernels run over columnar slices with a simple
/// per-row loop and a compacting pass; failed rows are dropped and are not
/// retained for Project. Each test charges one unit; each dropped or kept
/// row also charges one unit for the candidate visit.
///
/// # Errors
/// Returns the first callback refusal. On refusal the input is not returned;
/// already-written mask entries are discarded with it.
pub fn filter<E>(
    template: &Template,
    mut bindings: Bindings,
    account: &mut impl FnMut(u64) -> Result<(), E>,
    stats: &mut EnumerationStats,
) -> Result<Bindings, E> {
    let rows = bindings.len();
    if rows == 0 {
        return Ok(bindings);
    }
    let mut mask = vec![1u8; rows];
    let mut row = vec![0i32; bindings.width()];
    for (index, flag) in mask.iter_mut().enumerate() {
        stats.candidates = stats.candidates.saturating_add(1);
        account(1)?;
        bindings.row_into(index, &mut row);
        if !row_ok(template, &row, account, stats)? {
            *flag = 0;
        }
    }
    bindings.retain_mask(&mask);
    stats.retained = stats
        .retained
        .saturating_add(u64::try_from(bindings.len()).unwrap_or(u64::MAX));
    Ok(bindings)
}

fn row_ok<E>(
    template: &Template,
    row: &[i32],
    account: &mut impl FnMut(u64) -> Result<(), E>,
    stats: &mut EnumerationStats,
) -> Result<bool, E> {
    for &op in template.arithmetic() {
        stats.expression_evaluations = stats.expression_evaluations.saturating_add(1);
        account(1)?;
        match evaluate_arithmetic(op, row) {
            Some(true) => {}
            Some(false) | None => return Ok(false),
        }
    }
    for &comparison in template.comparisons() {
        stats.integer_comparisons = stats.integer_comparisons.saturating_add(1);
        account(1)?;
        if !evaluate_comparison(comparison, row) {
            return Ok(false);
        }
    }
    Ok(true)
}
