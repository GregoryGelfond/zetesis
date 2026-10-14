//! The atom occurrences a frame holds, at every depth.
//!
//! One enumeration serves two readers: demand, which keeps postings for the
//! columns a join can bind, and the producer registration, which records the
//! predicates a rich producer reads. It matches every form of the IR with no
//! wildcard arm, so a new form is a compile error here rather than a read
//! silently missing from either reader.

use super::components::{Pattern, Predicate};
use crate::FormulaFailure;
use crate::formula_conditional_ir::{Consequent, ConsequentOperand};
use crate::formula_ir::{AggregateKey, HeadIr, LiteralIr, Projection, RuleIr};

/// One atom occurrence, by its compiled coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Occurrence {
    /// An atom pattern of either sign: a body atom, a structural occurrence,
    /// an aggregate element's atom key, a conditional literal's atom, or a
    /// projection's witness atom.
    Atom(Pattern),
    /// The predicate an argument projection reads.
    Projected(Predicate),
}

/// A reader of occurrences. The walk charges one `work` unit per literal it
/// visits, at every depth, before reporting that literal's occurrences.
pub(super) trait Visitor {
    fn work(&mut self) -> Result<(), FormulaFailure>;
    fn occurrence(&mut self, occurrence: Occurrence) -> Result<(), FormulaFailure>;
}

/// The frames a traversal of `rule` reads: its body, then the condition of
/// each head element. The atoms its head derives are not among them.
pub(super) fn rule(rule: &RuleIr, visitor: &mut impl Visitor) -> Result<(), FormulaFailure> {
    frame(&rule.body, visitor)?;
    match &rule.head {
        HeadIr::Normal(_) | HeadIr::Disjunction(_) => {}
        HeadIr::Choice(group) => {
            for element in &group.elements {
                frame(&element.condition, visitor)?;
            }
        }
        HeadIr::ConditionalDisjunction { elements, .. } => {
            for element in elements {
                frame(&element.condition, visitor)?;
            }
        }
    }
    Ok(())
}

/// Every occurrence of `literals` and of every frame nested in them: aggregate
/// elements' conditions and atom keys, conditional literals' conditions,
/// consequents and bindings, and projections' predicates and witnesses.
pub(super) fn frame(
    literals: &[LiteralIr],
    visitor: &mut impl Visitor,
) -> Result<(), FormulaFailure> {
    let mut pending = vec![literals];
    while let Some(literals) = pending.pop() {
        for literal in literals {
            visitor.work()?;
            match literal {
                LiteralIr::Atom(_, atom) => visitor.occurrence(Occurrence::Atom(*atom))?,
                LiteralIr::PatternAtom(atom) => visitor.occurrence(Occurrence::Atom(atom.atom))?,
                LiteralIr::ProjectedAtom(_, value) => projection(value, visitor, &mut pending)?,
                LiteralIr::Conditional(value) => {
                    pending.push(&value.condition);
                    match &value.consequent {
                        Consequent::Atoms(_, alternatives) => {
                            for alternative in alternatives {
                                pending.push(&alternative.bindings);
                                match &alternative.operand {
                                    ConsequentOperand::Atom(atom) => {
                                        visitor.occurrence(Occurrence::Atom(*atom))?;
                                    }
                                    ConsequentOperand::Projection(value) => {
                                        projection(value, visitor, &mut pending)?;
                                    }
                                }
                            }
                        }
                        Consequent::Guards(alternatives) => {
                            for alternative in alternatives {
                                pending.push(&alternative.bindings);
                            }
                        }
                        Consequent::Guard(_) => {}
                    }
                }
                LiteralIr::Aggregate(value) => {
                    for element in &value.elements {
                        pending.push(&element.condition);
                        match &element.key {
                            AggregateKey::Atom(atom) => {
                                visitor.occurrence(Occurrence::Atom(*atom))?;
                            }
                            AggregateKey::Tuple(_) => {}
                        }
                    }
                }
                LiteralIr::Compare(..)
                | LiteralIr::ArgumentCheck { .. }
                | LiteralIr::TupleCompare(..)
                | LiteralIr::Guard(_)
                | LiteralIr::HeadGuard(_)
                | LiteralIr::Bind { .. }
                | LiteralIr::Range { .. } => {}
            }
        }
    }
    Ok(())
}

fn projection<'ir>(
    value: &'ir Projection,
    visitor: &mut impl Visitor,
    pending: &mut Vec<&'ir [LiteralIr]>,
) -> Result<(), FormulaFailure> {
    match value {
        Projection::Arguments { predicate, .. } => {
            visitor.occurrence(Occurrence::Projected(*predicate))?;
        }
        Projection::Witnesses { atom, bindings, .. } => {
            visitor.occurrence(Occurrence::Atom(*atom))?;
            pending.push(bindings);
        }
    }
    Ok(())
}
