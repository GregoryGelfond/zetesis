//! Typed scalar validation and conversion shared by formula admission.

use themelios_program::symbol::{Sign, Symbol};
use zetesis_core::Value;

use crate::program_admission::CompilationFailure;
use crate::{AdmissionFailure, ProfileFeature, ProgramSite};

fn unsupported(feature: ProfileFeature) -> CompilationFailure {
    CompilationFailure::Profile(feature)
}

pub(crate) fn scalar(
    source: &Symbol,
    location: impl Into<ProgramSite>,
) -> Result<Value, AdmissionFailure> {
    checked_scalar(source).map_err(|error| error.at(location))
}

fn checked_scalar(source: &Symbol) -> Result<Value, CompilationFailure> {
    Ok(match classify(source)? {
        Scalar::Number(number) => Value::Number(number),
        Scalar::String(text) => Value::String(text.into()),
        Scalar::Symbol(name) => Value::Symbol(name.into()),
        Scalar::Structural(symbol) => {
            crate::structural_value::from_symbol(symbol).map_err(value_failure)?
        }
        Scalar::Infimum => Value::Infimum,
        Scalar::Supremum => Value::Supremum,
    })
}

/// Validate a source scalar without manufacturing an owned core value. NUL and
/// typed structure checks are shared with conversion. Logical node/depth/text
/// bounds remain active; actual construction capacity is checked only when a
/// value is constructed. No authored arithmetic evaluation is skipped.
pub(crate) fn validate_scalar(
    source: &Symbol,
    location: impl Into<ProgramSite>,
) -> Result<(), AdmissionFailure> {
    let location = location.into();
    if let Scalar::Structural(symbol) = classify(source).map_err(|error| error.at(location))? {
        crate::structural_value::validate_symbol(symbol)
            .map_err(|error| value_failure(error).at(location))?;
    }
    Ok(())
}

enum Scalar<'a> {
    Number(i32),
    String(&'a str),
    Symbol(&'a str),
    Structural(&'a Symbol),
    Infimum,
    Supremum,
}

fn classify(source: &Symbol) -> Result<Scalar<'_>, CompilationFailure> {
    match source {
        Symbol::Number(number) => Ok(Scalar::Number(*number)),
        Symbol::String(text) => {
            // clingo's string symbol cannot preserve an embedded NUL; refuse
            // this otherwise parseable value instead of silently truncating it.
            if text.contains('\0') {
                return Err(unsupported(ProfileFeature::NulString));
            }
            Ok(Scalar::String(text))
        }
        Symbol::Function {
            arguments,
            sign: Sign::Positive,
            name,
        } if arguments.is_empty() => Ok(Scalar::Symbol(name.as_str())),
        Symbol::Function { .. } | Symbol::Tuple(_) => {
            for child in source.subsymbols() {
                if matches!(child, Symbol::String(text) if text.contains('\0')) {
                    return Err(unsupported(ProfileFeature::NulString));
                }
            }
            Ok(Scalar::Structural(source))
        }
        Symbol::Infimum => Ok(Scalar::Infimum),
        Symbol::Supremum => Ok(Scalar::Supremum),
    }
}

fn value_failure(error: zetesis_core::ValueError) -> CompilationFailure {
    CompilationFailure::Construction(zetesis_core::ConstructionError::Value(error))
}
