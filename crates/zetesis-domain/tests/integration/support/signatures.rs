//! Positive signatures by name and arity.

use themelios_program::symbol::{Name, Sign, Signature};

/// The positive signature `name/arity`.
pub fn signature(name: &str, arity: u32) -> Signature {
    Signature {
        sign: Sign::Positive,
        name: Name::new(name).unwrap(),
        arity,
    }
}
