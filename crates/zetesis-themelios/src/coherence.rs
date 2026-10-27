//! Signed predicate identity at the source-admission boundary.

use zetesis_core::Sign;

pub(crate) fn core_sign(sign: themelios_program::symbol::Sign) -> Sign {
    match sign {
        themelios_program::symbol::Sign::Positive => Sign::Positive,
        themelios_program::symbol::Sign::Negative => Sign::Negative,
    }
}

pub(crate) fn source_sign(sign: Sign) -> themelios_program::symbol::Sign {
    match sign {
        Sign::Positive => themelios_program::symbol::Sign::Positive,
        Sign::Negative => themelios_program::symbol::Sign::Negative,
    }
}
