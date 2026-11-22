//! Existential projection of retained bindings onto a head.

use super::relation::{Atoms, Bindings};
use super::template::{HeadTerm, Template, TemplateError};

/// Project retained bindings onto the template head, coalescing duplicates.
///
/// Charges follow [`Atoms::from_unsorted`]: an ordered input coalesces in one
/// adjacent pass; an inversion retains its discovery work before ordinary
/// sorting and uniquing. Head extraction charges one unit per retained row.
///
/// # Errors
/// Returns the first callback refusal, with no partial set. A missing or
/// empty head is `Ok(Err(TemplateError::MissingHead))` and is not charged.
///
/// # Panics
/// Panics if the extracted head columns fail their layout check. The
/// columns are built with equal length and nonempty width, so this is an
/// internal invariant.
pub fn project<E>(
    template: &Template,
    bindings: &Bindings,
    account: &mut impl FnMut(u64) -> Result<(), E>,
) -> Result<Result<Atoms, TemplateError>, E> {
    if template.head().is_empty() {
        return Ok(Err(TemplateError::MissingHead));
    }
    let width = template.head().len();
    let mut columns = vec![Vec::with_capacity(bindings.len()); width];
    let mut buffer = vec![0i32; width];
    for row in bindings.row_range() {
        account(1)?;
        for (index, term) in template.head().iter().enumerate() {
            buffer[index] = match *term {
                HeadTerm::Slot(slot) => bindings.column(slot)[row],
                HeadTerm::Const(value) => value,
            };
        }
        for (column, value) in columns.iter_mut().zip(&buffer) {
            column.push(*value);
        }
    }
    Ok(Ok(
        Atoms::from_unsorted(&columns, account)?.expect("projected columns are well formed")
    ))
}
