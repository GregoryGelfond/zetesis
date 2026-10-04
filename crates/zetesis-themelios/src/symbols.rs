//! Bounded export of borrowed logical values to themelios symbols.
//!
//! These operations construct the canonical upstream [`Symbol`] directly from
//! [`TermRef`] and [`AtomRef`]. They do not render text, reparse it, or construct
//! an intermediate owned core value. The input remains borrowed and unchanged.
//! A caller exporting a complete interpretation visits every atom; source
//! display selection does not belong to this conversion.
//!
//! The allowance covers named output and traversal capacities, including
//! allocator overcapacity. It excludes allocator bookkeeping and the borrowed
//! input. A callback can charge cumulative work and poll cancellation before
//! navigation, reservation, text copying, and symbol assembly. Each failure
//! returns no partial symbol. Collecting the returned symbols into themelios's
//! `AnswerSet` uses the upstream `BTreeSet` allocation boundary; these operations
//! do not make that collection's allocations fallible.

use themelios_program::Symbol;
use zetesis_core::catalog::{AtomRef, TermRef};

pub use crate::structural_value::{BridgeError as Error, BridgeFailure as Failure};

/// Export a borrowed closed term under one construction allowance.
///
/// Work is linear in the expanded logical nodes and text, plus canonical
/// navigation and depth measurement. A borrowed ingress subtree requires an
/// allocation-free depth scan with quadratic worst-case work; every scan step
/// checks the callback. Complete ingress and canonical values use cached depth.
/// Retained output is linear in the expansion; traversal scratch is proportional
/// to its depth. Shared subterms become the tree required by the upstream symbol,
/// without an intermediate owned core value.
///
/// # Errors
/// Returns a typed storage, allocation, or name refusal, or the callback's
/// original failure. No partial symbol is returned.
pub fn term_with<E>(
    value: TermRef<'_>,
    max_bytes: u128,
    before: impl FnMut() -> Result<(), E>,
) -> Result<Symbol, Failure<E>> {
    crate::structural_value::term_symbol_with(value, max_bytes, before)
}

/// Export a complete signed atom under one construction allowance.
///
/// The allowance covers the predicate, all arguments, and traversal scratch
/// together; it is not reset for each argument. Work is linear in the expanded
/// logical nodes and text, plus canonical navigation. Retained output is linear
/// in that expansion; traversal scratch is proportional to the greatest depth.
///
/// ```
/// use zetesis_core::{Atom, Predicate, Sign, Value};
/// use zetesis_cpu::Cancellation;
/// use zetesis_themelios::{logical::Symbol, symbols};
///
/// let atom = Atom::new(
///     Predicate::with_sign("p", 1, Sign::Negative)?,
///     vec![Value::Number(i32::MIN)],
/// )?;
/// let cancellation = Cancellation::default();
/// let symbol = symbols::atom_with((&atom).into(), 4096, || cancellation.poll())?;
/// assert!(matches!(&symbol, Symbol::Function { arguments, sign, .. }
///     if arguments == &[Symbol::Number(i32::MIN)]
///         && *sign == zetesis_themelios::logical::Sign::Negative));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// Returns a typed storage, allocation, or name refusal, or the callback's
/// original failure. No partial symbol is returned.
pub fn atom_with<E>(
    atom: AtomRef<'_>,
    max_bytes: u128,
    before: impl FnMut() -> Result<(), E>,
) -> Result<Symbol, Failure<E>> {
    crate::structural_value::atom_symbol_with(atom, max_bytes, before)
}

#[cfg(test)]
mod tests;
