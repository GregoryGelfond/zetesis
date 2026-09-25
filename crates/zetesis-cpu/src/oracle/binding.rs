//! Borrowed canonical or ingress bindings for the scalar prefix join.
//!
//! A binding owns no value. Shared typed comparisons charge each visited logical
//! navigation, descriptor and text-prefix operation before it proceeds.

use zetesis_core::{PatternRef, TemplateRef, TemplateTerm, catalog::TermRef};

use super::{Gates, Work, relations::Row};
use crate::Stop;

pub(super) fn bind<'source>(
    pattern: PatternRef<'_>,
    row: Row<'source>,
    assignment: &mut [Option<TermRef<'source>>],
    undo: &mut Vec<usize>,
    work: &mut Work<'_>,
) -> Result<bool, Stop> {
    let mut values = row.values();
    for term in pattern.terms() {
        work.tick()?;
        let value = values.next().ok_or(Stop::InvalidProgram)?;
        match term {
            TemplateTerm::Constant(expected) => {
                if !expected.compare_ref_with(value, || work.tick())?.is_eq() {
                    return Ok(false);
                }
            }
            TemplateTerm::Variable(variable) => {
                if let Some(expected) = assignment[variable] {
                    if !expected.compare_ref_with(value, || work.tick())?.is_eq() {
                        return Ok(false);
                    }
                } else {
                    assignment[variable] = Some(value);
                    undo.push(variable);
                }
            }
        }
    }
    Ok(true)
}

pub(super) fn clear(assignment: &mut [Option<TermRef<'_>>], undo: &mut Vec<usize>) {
    for variable in undo.drain(..) {
        assignment[variable] = None;
    }
}

pub(super) fn resolve<'a>(
    term: TemplateTerm<'a>,
    assignment: &[Option<TermRef<'a>>],
) -> Option<TermRef<'a>> {
    match term {
        TemplateTerm::Constant(value) => Some(value),
        TemplateTerm::Variable(variable) => assignment[variable],
    }
}

pub(super) fn guards(
    template: TemplateRef<'_>,
    assignment: &[Option<TermRef<'_>>],
    gates: Gates<'_>,
    work: &mut Work<'_>,
) -> Result<bool, Stop> {
    for filter in template.filters() {
        work.tick()?;
        let (left, right) = filter.terms();
        if let (Some(left), Some(right)) = (resolve(left, assignment), resolve(right, assignment)) {
            let equal = left.compare_ref_with(right, || work.tick())?.is_eq();
            if filter.is_equality() != equal {
                return Ok(false);
            }
        }
    }
    if matches!(gates, Gates::Unjudged) {
        return Ok(true);
    }
    for (patterns, required) in [(template.gate_true(), true), (template.gate_false(), false)] {
        for pattern in patterns {
            work.tick()?;
            work.charge(pattern.terms().len())?;
            // Absent slots defer the gate; a complete key borrows this binding.
            if let Ok(key) = pattern.key(assignment)
                && !gates.holds(&key, required, work)?
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
