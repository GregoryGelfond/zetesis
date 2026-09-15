//! Finite term alternatives composed before their source scope is expanded.
//!
//! The upstream rewrite visits children first. Each child therefore has one
//! outer pool at most; this pass distributes those alternatives through one
//! constructor or operator. It never evaluates intervals or erases empty ranges:
//! their endpoint checks and variable safety belong to the binding consumer.

use themelios_base::span::Location;
use themelios_program::term::{Term, TermParts};
use themelios_program::transform::Visit;

use super::Footprint;
use crate::expansion::Budget;
use crate::{ExpansionFailure, ExpansionResource};

pub(crate) fn distribute(
    term: Term,
    budget: &mut Budget,
    location: Location,
) -> Result<Term, ExpansionFailure> {
    // Pool-free nodes retain the original normalizer and its charging order.
    let pooled = match &term {
        Term::Function { arguments, .. }
        | Term::Tuple(arguments)
        | Term::External { arguments, .. } => arguments.iter().any(is_pool),
        Term::UnaryOperation { argument, .. } | Term::Absolute(argument) => is_pool(argument),
        Term::BinaryOperation { left, right, .. }
        | Term::Interval {
            lower: left,
            upper: right,
        } => is_pool(left) || is_pool(right),
        _ => false,
    };
    if !pooled {
        return Ok(term);
    }
    match term.into_parts() {
        TermParts::Function { name, arguments } => product(
            &arguments,
            name.as_str().len(),
            budget,
            location,
            |arguments| Term::Function {
                name: name.clone(),
                arguments,
            },
        ),
        TermParts::Tuple(arguments) => product(&arguments, 0, budget, location, Term::Tuple),
        TermParts::External { name, arguments } => product(
            &arguments,
            name.as_str().len(),
            budget,
            location,
            |arguments| Term::External {
                name: name.clone(),
                arguments,
            },
        ),
        TermParts::UnaryOperation { operator, argument } => {
            product(&[argument], 0, budget, location, |mut arguments| {
                Term::UnaryOperation {
                    operator,
                    argument: Box::new(arguments.pop().expect("one unary argument")),
                }
            })
        }
        TermParts::Absolute(argument) => {
            product(&[argument], 0, budget, location, |mut arguments| {
                Term::Absolute(Box::new(arguments.pop().expect("one absolute argument")))
            })
        }
        TermParts::BinaryOperation {
            operator,
            left,
            right,
        } => product(&[left, right], 0, budget, location, |mut arguments| {
            let right = arguments.pop().expect("right binary argument");
            let left = arguments.pop().expect("left binary argument");
            Term::BinaryOperation {
                operator,
                left: Box::new(left),
                right: Box::new(right),
            }
        }),
        TermParts::Interval { lower, upper } => {
            product(&[lower, upper], 0, budget, location, |mut arguments| {
                let upper = arguments.pop().expect("upper interval endpoint");
                let lower = arguments.pop().expect("lower interval endpoint");
                Term::Interval {
                    lower: Box::new(lower),
                    upper: Box::new(upper),
                }
            })
        }
        _ => unreachable!("only compound nodes with pooled children are distributed"),
    }
}

fn is_pool(term: &Term) -> bool {
    matches!(term, Term::Pool(_))
}

fn alternatives(term: &Term) -> &[Term] {
    if let Term::Pool(items) = term {
        items
    } else {
        std::slice::from_ref(term)
    }
}

fn product(
    arguments: &[Term],
    name_bytes: usize,
    budget: &mut Budget,
    location: Location,
    build: impl Fn(Vec<Term>) -> Term,
) -> Result<Term, ExpansionFailure> {
    let count = arguments.iter().fold(1_u128, |count, argument| {
        count.saturating_mul(alternatives(argument).len() as u128)
    });
    let mut footprint = Footprint::default();
    for term in arguments.iter().flat_map(Term::subterms) {
        footprint.visit_term(term);
    }
    budget.charge(ExpansionResource::TermWork, footprint.nodes, location)?;
    budget.charge(ExpansionResource::Values, count, location)?;
    budget.charge(
        ExpansionResource::TermWork,
        count.saturating_mul(footprint.nodes.saturating_add(1)),
        location,
    )?;
    // Reserve selected term/text copies, the output root and cursor cells before
    // retaining the family. This is cumulative logical payload, not allocator RSS.
    budget.charge(
        ExpansionResource::ScalarBytes,
        count
            .saturating_mul(
                footprint
                    .bytes
                    .saturating_mul(2)
                    .saturating_add((std::mem::size_of::<Term>() + name_bytes) as u128),
            )
            .saturating_add(arguments.len() as u128 * std::mem::size_of::<usize>() as u128),
        location,
    )?;
    let count = usize::try_from(count).expect("finite value allowance fits usize");
    let mut output = Vec::with_capacity(count);
    let mut positions = vec![0; arguments.len()];
    for _ in 0..count {
        let selected = arguments
            .iter()
            .zip(&positions)
            .map(|(argument, &position)| alternatives(argument)[position].clone())
            .collect();
        output.push(build(selected));
        for (position, argument) in positions.iter_mut().zip(arguments).rev() {
            *position += 1;
            if *position < alternatives(argument).len() {
                break;
            }
            *position = 0;
        }
    }
    Ok(Term::pool(output).expect("source pool products are nonempty"))
}
