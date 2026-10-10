//! Checked evaluation evidence for a keyed constraint before it is rewritten.
//!
//! Removing a positive witness or a false comparison can change which source
//! operations are reached. A rewrite is therefore admitted only when every
//! term of the original constraint is defined over an independent upper bound
//! on its relational bindings. Bounds come from fact-only predicates and the
//! fact-only conditions of keyed value positions, never from comparisons whose
//! exclusion behavior the rewrite changes. Unknown types or arithmetic bounds
//! leave the constraint written.
//!
//! Each arithmetic node must fit i32 before its enclosing expression is read.
//! This is an interval proof, not evaluation at endpoints without an invariant:
//! addition, subtraction, multiplication and division take their extrema at
//! interval endpoints; remainder uses the divisor's magnitude; a fixed power
//! additionally checks zero when an even power crosses it. Variable exponents
//! are left unproved. Bitwise operations have the complete i32 interval.
//!
//! Traversal is iterative. Its two scratch vectors hold at most one cell per
//! term node, reserved fallibly, and each visited node spends key work. Variable
//! bounds retain one interval per named variable, bounded by admitted source
//! syntax and key work. A stopped or incomplete proof supplies no permission.

use std::collections::BTreeMap;

use crate::scalar_arithmetic::Range;
use themelios_program::program::{Arguments, Body, BodyElement, LiteralInner};
use themelios_program::symbol::{Signature, Symbol, VarName};
use themelios_program::term::{Term, Variable};
use zetesis_domain::{FactIndex, KeyWork, KeyedRelation, Stop, atom_signature};

#[derive(Clone, Copy)]
pub(super) enum Failure {
    Work(Stop),
    Allocation,
}

impl From<Stop> for Failure {
    fn from(stop: Stop) -> Self {
        Self::Work(stop)
    }
}

/// Every value of the original constraint's terms is defined on all its
/// relational substitutions. Numeric bounds are independent of comparisons.
pub(super) struct Proof<'a> {
    variables: BTreeMap<&'a VarName, Range>,
}

impl<'a> Proof<'a> {
    pub(super) fn of(
        body: &'a Body,
        keys: &BTreeMap<&Signature, &KeyedRelation<'_>>,
        facts: &FactIndex<'_>,
        work: &mut KeyWork,
    ) -> Result<Option<Self>, Failure> {
        let mut proof = Self {
            variables: BTreeMap::new(),
        };
        // The caller admits only positive body literals before requesting
        // this certificate; a default-negated fact column would not bound a row.
        for element in body.elements() {
            work.step()?;
            let BodyElement::Literal(literal) = element.get() else {
                return Ok(None);
            };
            let LiteralInner::Atom(atom) = &literal.inner else {
                continue;
            };
            let atom = atom.get();
            let Arguments::Single(terms) = &atom.arguments else {
                return Ok(None);
            };
            let Some(signature) = atom_signature(atom, terms.len()) else {
                return Ok(None);
            };
            for (position, term) in terms.iter().enumerate() {
                work.step()?;
                let Term::Variable(Variable::Named(variable)) = term else {
                    continue;
                };
                let bound = match keys.get(&signature) {
                    Some(key) if position == key.value_position() => keyed_range(key, facts, work)?,
                    _ => fact_range(facts, &signature, position, work)?,
                };
                if let Some(bound) = bound {
                    // Either conjunct is already a necessary bound; retain
                    // the first rather than relying on an empty intersection.
                    proof.variables.entry(variable).or_insert(bound);
                }
            }
        }
        for element in body.elements() {
            work.step()?;
            let BodyElement::Literal(literal) = element.get() else {
                return Ok(None);
            };
            match &literal.inner {
                LiteralInner::Atom(atom) => {
                    for term in atom.get().argument_terms() {
                        if proof.value(term, work)?.is_none() {
                            return Ok(None);
                        }
                    }
                }
                LiteralInner::Comparison(comparison) => {
                    let comparison = comparison.get();
                    if proof.value(comparison.first(), work)?.is_none() {
                        return Ok(None);
                    }
                    for (_, term) in comparison.steps() {
                        if proof.value(term, work)?.is_none() {
                            return Ok(None);
                        }
                    }
                }
                LiteralInner::True | LiteralInner::False => return Ok(None),
            }
        }
        Ok(Some(proof))
    }

    pub(super) fn numeric(&self, term: &Term, work: &mut KeyWork) -> Result<bool, Failure> {
        Ok(matches!(self.value(term, work)?, Some(Value::Number(_))))
    }

    fn value(&self, term: &Term, work: &mut KeyWork) -> Result<Option<Value>, Failure> {
        let mut nodes = Vec::new();
        for node in term.subterms() {
            work.step()?;
            nodes.try_reserve(1).map_err(|_| Failure::Allocation)?;
            nodes.push(node);
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(nodes.len())
            .map_err(|_| Failure::Allocation)?;
        // Reversed preorder visits every child before its parent, leaving
        // the left child's value on top of the right child's value.
        for node in nodes.into_iter().rev() {
            work.step()?;
            let value = match node {
                Term::Variable(Variable::Named(variable)) => self
                    .variables
                    .get(variable)
                    .copied()
                    .map_or(Value::Any, Value::Number),
                Term::Symbolic(Symbol::Number(number)) => Value::Number(Range::point(*number)),
                Term::Variable(Variable::Anonymous) | Term::Symbolic(_) => Value::Any,
                Term::Function { arguments, .. } | Term::Tuple(arguments) => {
                    values.truncate(values.len() - arguments.len());
                    Value::Any
                }
                Term::UnaryOperation { operator, .. } => {
                    let Some(Value::Number(argument)) = values.pop() else {
                        return Ok(None);
                    };
                    let Some(bound) = argument.unary(*operator) else {
                        return Ok(None);
                    };
                    Value::Number(bound)
                }
                Term::BinaryOperation { operator, .. } => {
                    let (Some(Value::Number(left)), Some(Value::Number(right))) =
                        (values.pop(), values.pop())
                    else {
                        return Ok(None);
                    };
                    let Some(bound) = left.binary(*operator, right) else {
                        return Ok(None);
                    };
                    Value::Number(bound)
                }
                Term::Absolute(_) => {
                    let Some(Value::Number(argument)) = values.pop() else {
                        return Ok(None);
                    };
                    let Some(bound) = argument.absolute() else {
                        return Ok(None);
                    };
                    Value::Number(bound)
                }
                Term::Pool(_) | Term::Interval { .. } | Term::External { .. } => return Ok(None),
            };
            values.push(value);
        }
        Ok(values.pop())
    }
}

/// A value known to be defined; `Any` establishes no numeric type.
enum Value {
    Any,
    Number(Range),
}

fn fact_range(
    facts: &FactIndex<'_>,
    signature: &Signature,
    position: usize,
    work: &mut KeyWork,
) -> Result<Option<Range>, Failure> {
    let Some(values) = facts.values(signature, position, work)? else {
        return Ok(None);
    };
    let mut range: Option<Range> = None;
    for value in values {
        work.step()?;
        let Symbol::Number(value) = value else {
            return Ok(None);
        };
        match &mut range {
            Some(range) => range.include(*value),
            None => range = Some(Range::point(*value)),
        }
    }
    Ok(range)
}

fn keyed_range(
    key: &KeyedRelation<'_>,
    facts: &FactIndex<'_>,
    work: &mut KeyWork,
) -> Result<Option<Range>, Failure> {
    // KeyedRelation construction certifies an all-positive, flat condition.
    // Read that proved premise; negative conditions never produce this owner.
    for literal in key.condition().literals() {
        work.step()?;
        let LiteralInner::Atom(atom) = &literal.get().inner else {
            continue;
        };
        let atom = atom.get();
        let Arguments::Single(terms) = &atom.arguments else {
            continue;
        };
        let Some(signature) = atom_signature(atom, terms.len()) else {
            continue;
        };
        for (position, term) in terms.iter().enumerate() {
            work.step()?;
            if matches!(term, Term::Variable(Variable::Named(name)) if name == key.value_variable())
                && let Some(range) = fact_range(facts, &signature, position, work)?
            {
                return Ok(Some(range));
            }
        }
    }
    Ok(None)
}
