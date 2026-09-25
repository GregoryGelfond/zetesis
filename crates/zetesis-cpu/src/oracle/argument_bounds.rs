//! Upper bounds on the values each argument of a predicate can take in any
//! closure of a program.
//!
//! Every atom of a closure is an instance of a template's head, so an
//! argument's values are bounded by what the heads can put there: a constant
//! contributes itself, and a variable ranges within the values of every
//! positive body position that binds it, the intersection of their bounds.
//! Gates and filters only restrict, so they are ignored and the bound stays an
//! upper bound. Closed under the templates to a least fixed point, the bounds
//! are finite because every value is a constant of the program; an argument
//! whose set would exceed the width ceiling is unknown instead, and unknown
//! absorbs: a variable bound only at unknown positions is unknown.
//!
//! This is the finite-set lattice of the domain analysis in `zetesis-domain`,
//! computed over the admitted templates after expansion, where the closure
//! runs and where a range such as `node(1..16)` is already its facts. A
//! bounded predicate whose product of widths fits a ceiling is read as a
//! dense relation, a bit array over the mixed-radix index of its arguments'
//! ranks; every other predicate keeps its tree. The inference serves
//! preparation alone and takes its width ceiling and its work from it.

use std::collections::BTreeSet;

use zetesis_core::{Program, TemplateRef, TemplateTerm, catalog::PredicateRef};

use super::Work;
use crate::Stop;

/// An upper bound on one argument's values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Bound {
    /// Only these values occur, in canonical order and without repetition.
    Finite(Vec<usize>),
    /// No finite bound: the argument is unbounded or too wide to keep.
    Unknown,
}

/// The bounds of every argument of every predicate of one program.
#[derive(Clone, Debug)]
pub(crate) struct ArgumentBounds {
    program: Program,
    bounds: Vec<Vec<Bound>>,
}

/// One argument's growing bound during the fixed point.
#[derive(Clone, Debug)]
enum Growing {
    Finite(BTreeSet<usize>),
    Unknown,
}

impl ArgumentBounds {
    /// The bounds of every argument of a predicate, in argument order.
    pub(crate) fn bounds<'a>(&self, predicate: impl Into<PredicateRef<'a>>) -> Option<&[Bound]> {
        self.program
            .predicates()
            .binary_search(predicate)
            .ok()
            .map(|table| self.bounds[table].as_slice())
    }
}

/// Infer the bounds of the program's predicates, charging the given work;
/// the width ceiling is `max_values`, an argument wider than it unknown.
///
/// # Errors
/// Returns the stop when the inference exceeds the work's ceiling or its
/// control stops it.
pub(super) fn infer(
    program: &Program,
    max_values: usize,
    work: &mut Work<'_>,
) -> Result<ArgumentBounds, Stop> {
    let mut growing: Vec<Vec<Growing>> = program
        .predicates()
        .iter()
        .map(|predicate| {
            (0..predicate.arity())
                .map(|_| Growing::Finite(BTreeSet::new()))
                .collect()
        })
        .collect();
    // Each pass adds values or turns an argument unknown, never the
    // reverse, and the values come from the program's constants, so the
    // passes end when one changes nothing.
    loop {
        let mut changed = false;
        for template in program.templates() {
            let Some(head) = template.head() else {
                continue;
            };
            let target = table(program, head.predicate(), work)?;
            for (index, term) in head.terms().iter().enumerate() {
                work.tick()?;
                let contribution = match term {
                    TemplateTerm::Constant(value) => Growing::Finite(BTreeSet::from([program
                        .domain()
                        .binary_search_with(value, || work.tick())?
                        .map_err(|_| Stop::InvalidProgram)?])),
                    TemplateTerm::Variable(variable) => {
                        variable_bound(program, template, variable, &growing, work)?
                    }
                };
                changed |= widen(&mut growing[target][index], contribution, max_values, work)?;
            }
        }
        if !changed {
            break;
        }
    }
    let bounds = growing
        .into_iter()
        .map(|arguments| {
            arguments
                .into_iter()
                .map(|argument| match argument {
                    Growing::Finite(values) => Bound::Finite(values.into_iter().collect()),
                    Growing::Unknown => Bound::Unknown,
                })
                .collect()
        })
        .collect();
    Ok(ArgumentBounds {
        program: program.clone(),
        bounds,
    })
}

/// The values a head variable can take: the intersection of the bounds of
/// the positive body positions binding it, unknown positions binding nothing.
/// A variable no positive position binds is unknown: the bound stays an
/// upper domain whatever else binds it.
fn variable_bound(
    program: &Program,
    template: TemplateRef<'_>,
    variable: usize,
    growing: &[Vec<Growing>],
    work: &mut Work<'_>,
) -> Result<Growing, Stop> {
    let mut bound: Option<BTreeSet<usize>> = None;
    for pattern in template.positive() {
        let source = table(program, pattern.predicate(), work)?;
        for (index, term) in pattern.terms().iter().enumerate() {
            if term != TemplateTerm::Variable(variable) {
                continue;
            }
            work.tick()?;
            match &growing[source][index] {
                Growing::Unknown => {}
                Growing::Finite(values) => {
                    work.charge(values.len())?;
                    bound = Some(match bound {
                        None => values.clone(),
                        Some(known) => known.intersection(values).copied().collect(),
                    });
                }
            }
        }
    }
    Ok(bound.map_or(Growing::Unknown, Growing::Finite))
}

fn table(
    program: &Program,
    predicate: PredicateRef<'_>,
    work: &mut Work<'_>,
) -> Result<usize, Stop> {
    program
        .predicates()
        .binary_search_with(predicate, || work.tick())?
        .map_err(|_| Stop::InvalidProgram)
}

/// Widen an argument's bound by a contribution; whether anything changed.
fn widen(
    argument: &mut Growing,
    contribution: Growing,
    max_values: usize,
    work: &mut Work<'_>,
) -> Result<bool, Stop> {
    match (&mut *argument, contribution) {
        (Growing::Unknown, _) => Ok(false),
        (Growing::Finite(_), Growing::Unknown) => {
            *argument = Growing::Unknown;
            Ok(true)
        }
        (Growing::Finite(values), Growing::Finite(added)) => {
            let before = values.len();
            work.charge(added.len())?;
            values.extend(added);
            if values.len() > max_values {
                *argument = Growing::Unknown;
                return Ok(true);
            }
            Ok(values.len() != before)
        }
    }
}

#[cfg(test)]
#[path = "argument_bounds_tests.rs"]
mod tests;
