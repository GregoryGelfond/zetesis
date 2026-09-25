//! Checked canonical reads; equality uses the core's shared iterative cursor.

use super::super::{Error, Work};
use zetesis_core::{Sign, ValueNodeRef, catalog::TermRef};

pub(in crate::observation::evaluate) fn descriptor<'a>(
    value: TermRef<'a>,
    work: &mut Work<'_>,
) -> Result<ValueNodeRef<'a>, Error> {
    work.step(1)?;
    Ok(value.descriptor())
}

pub(in crate::observation::evaluate) fn text_equal(
    left: &str,
    right: &str,
    work: &mut Work<'_>,
) -> Result<bool, Error> {
    work.step(1)?;
    if left.len() != right.len() {
        return Ok(false);
    }
    for (left, right) in left.as_bytes().iter().zip(right.as_bytes()) {
        work.step(1)?;
        if left != right {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(in crate::observation::evaluate) fn constructor(
    expected: ValueNodeRef<'_>,
    actual: ValueNodeRef<'_>,
    work: &mut Work<'_>,
) -> Result<bool, Error> {
    work.step(1)?;
    match (expected, actual) {
        (
            ValueNodeRef::Function { name, sign, arity },
            ValueNodeRef::Function {
                name: actual,
                sign: actual_sign,
                arity: actual_arity,
            },
        ) => Ok(sign == actual_sign && arity == actual_arity && text_equal(name, actual, work)?),
        (
            ValueNodeRef::Function {
                name,
                sign: Sign::Positive,
                arity: 0,
            },
            ValueNodeRef::Symbol(actual),
        ) => text_equal(name, actual, work),
        (ValueNodeRef::Tuple { arity }, ValueNodeRef::Tuple { arity: actual }) => {
            Ok(arity == actual)
        }
        _ => Ok(false),
    }
}

pub(in crate::observation::evaluate) fn equal(
    left: TermRef<'_>,
    right: TermRef<'_>,
    work: &mut Work<'_>,
) -> Result<bool, Error> {
    left.equals_ref_with(right, || work.step(1))
}
