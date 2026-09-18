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
//! ranks; every other predicate keeps its tree.

use std::collections::BTreeSet;

use zetesis_core::{Predicate, Program, Term, Value};

use super::{Limits, Statistics, Work};
use crate::{Control, Stop};

/// An upper bound on one argument's values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Bound {
    /// Only these values occur, in canonical order and without repetition.
    Finite(Vec<Value>),
    /// No finite bound: the argument is unbounded or too wide to keep.
    Unknown,
}

/// Ceilings on one inference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoundLimits {
    /// Charged head-term visits and value insertions.
    pub max_work: u64,
    /// The widest finite bound kept; a wider argument is unknown.
    pub max_values: usize,
}

impl Default for BoundLimits {
    fn default() -> Self {
        Self {
            max_work: 100_000_000,
            max_values: 1 << 20,
        }
    }
}

/// The bounds of every argument of every predicate of one program.
#[derive(Clone, Debug)]
pub struct ArgumentBounds {
    predicates: Vec<Predicate>,
    bounds: Vec<Vec<Bound>>,
    work: u64,
}

/// One argument's growing bound during the fixed point.
#[derive(Clone, Debug)]
enum Growing {
    Finite(BTreeSet<Value>),
    Unknown,
}

impl ArgumentBounds {
    /// Infer the bounds of the program's predicates.
    ///
    /// # Errors
    /// Returns the stop when the inference exceeds its work ceiling or
    /// control stops it.
    pub fn infer(program: &Program, limits: BoundLimits, control: &Control) -> Result<Self, Stop> {
        let mut work = Work {
            control,
            limits: Limits {
                max_work: limits.max_work,
                ..Limits::default()
            },
            statistics: Statistics::default(),
            mask_words: 0,
            pruned_prefixes: 0,
            mask_bytes: 0,
        };
        infer_with(program, limits.max_values, &mut work)
    }

    /// The bound of one argument; unknown for a predicate or an argument the
    /// program does not have.
    #[must_use]
    pub fn bound(&self, predicate: &Predicate, argument: usize) -> &Bound {
        const UNKNOWN: Bound = Bound::Unknown;
        self.bounds(predicate)
            .and_then(|bounds| bounds.get(argument))
            .unwrap_or(&UNKNOWN)
    }

    /// The bounds of every argument of a predicate, in argument order.
    #[must_use]
    pub fn bounds(&self, predicate: &Predicate) -> Option<&[Bound]> {
        self.predicates
            .binary_search(predicate)
            .ok()
            .map(|slot| self.bounds[slot].as_slice())
    }

    /// The work the inference charged.
    #[must_use]
    pub fn work(&self) -> u64 {
        self.work
    }
}

/// Infer the bounds, charging the given work; the width ceiling is
/// `max_values`.
pub(super) fn infer_with(
    program: &Program,
    max_values: usize,
    work: &mut Work<'_>,
) -> Result<ArgumentBounds, Stop> {
    let before = work.statistics.work;
    let predicates: Vec<Predicate> = program.predicates().to_vec();
    let mut growing: Vec<Vec<Growing>> = predicates
        .iter()
        .map(|predicate| {
            (0..predicate.arity())
                .map(|_| Growing::Finite(BTreeSet::new()))
                .collect()
        })
        .collect();
    let slot = |predicate: &Predicate| {
        predicates
            .binary_search(predicate)
            .map_err(|_| Stop::InvalidProgram)
    };
    // Each pass adds values or turns an argument unknown, never the
    // reverse, and the values come from the program's constants, so the
    // passes end when one changes nothing.
    loop {
        let mut changed = false;
        for template in program.templates() {
            let Some(head) = template.head() else {
                continue;
            };
            let target = slot(head.predicate())?;
            for (index, term) in head.terms().iter().enumerate() {
                work.tick()?;
                let contribution = match term {
                    Term::Constant(value) => Growing::Finite(BTreeSet::from([value.clone()])),
                    Term::Variable(variable) => {
                        variable_bound(template, *variable, &growing, &slot, work)?
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
        predicates,
        bounds,
        work: work.statistics.work - before,
    })
}

/// The values a head variable can take: the intersection of the bounds of
/// the positive body positions binding it, unknown positions binding nothing.
/// A variable no positive position binds is unknown: the bound stays an
/// upper domain whatever else binds it.
fn variable_bound(
    template: &zetesis_core::Template,
    variable: usize,
    growing: &[Vec<Growing>],
    slot: &impl Fn(&Predicate) -> Result<usize, Stop>,
    work: &mut Work<'_>,
) -> Result<Growing, Stop> {
    let mut bound: Option<BTreeSet<Value>> = None;
    for pattern in template.positive() {
        let source = slot(pattern.predicate())?;
        for (index, term) in pattern.terms().iter().enumerate() {
            if *term != Term::Variable(variable) {
                continue;
            }
            work.tick()?;
            match &growing[source][index] {
                Growing::Unknown => {}
                Growing::Finite(values) => {
                    work.charge(values.len())?;
                    bound = Some(match bound {
                        None => values.clone(),
                        Some(known) => known.intersection(values).cloned().collect(),
                    });
                }
            }
        }
    }
    Ok(bound.map_or(Growing::Unknown, Growing::Finite))
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
