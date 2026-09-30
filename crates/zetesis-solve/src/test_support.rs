//! Helpers the crate's unit tests share across its modules.

use zetesis_core::Model;

/// The string argument of `model`'s first atom.
///
/// # Panics
/// Panics unless `model`'s first atom's first argument is a string.
pub(crate) fn payload(model: &Model) -> &str {
    let zetesis_core::ValueNodeRef::String(value) = model
        .atoms()
        .first()
        .unwrap()
        .values()
        .at(0)
        .unwrap()
        .descriptor()
    else {
        panic!("fixture contains one string argument");
    };
    value
}
