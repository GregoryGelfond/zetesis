//! Select structural alternatives before compiling a pattern, without cloning source trees.
//!
//! Widths are checked before retaining any Cartesian product. Each selected tree
//! then pays ordinary source node/text charges before its owned nodes are built.
//! Arithmetic selection preserves its expression; the separate inverse planner
//! recognizes finite one-candidate captures after structural selection.

use super::{Compiler, Error, Operand, Pattern, Term, UnaryOp, evaluated};
use crate::observation::{ErrorKind, Predicate, Resource};
use themelios_program::program::Atom;
use themelios_program::symbol::Sign;

fn width(term: &Term) -> u128 {
    match term {
        Term::Pool(items) => items
            .iter()
            .fold(0u128, |sum, item| sum.saturating_add(width(item))),
        Term::Function { arguments, .. } | Term::Tuple(arguments) => product(arguments),
        Term::UnaryOperation {
            operator: UnaryOp::Negate,
            argument,
        } if matches!(argument.as_ref(), Term::Function { .. }) => width(argument),
        _ => 1,
    }
}
fn product(arguments: &[Term]) -> u128 {
    arguments.iter().fold(1u128, |product, argument| {
        product.saturating_mul(width(argument))
    })
}
impl Compiler<'_> {
    fn selected_operands(
        &mut self,
        arguments: &[Term],
        mut position: u128,
        depth: usize,
    ) -> Result<Vec<Operand>, Error> {
        self.arity(arguments.len())?;
        let mut terms = Vec::new();
        for argument in arguments.iter().rev() {
            let alternatives = width(argument);
            terms.push(self.selected_operand(argument, position % alternatives, depth)?);
            position /= alternatives;
        }
        terms.reverse();
        Ok(terms)
    }
    fn selected_operand(
        &mut self,
        term: &Term,
        mut position: u128,
        depth: usize,
    ) -> Result<Operand, Error> {
        self.node(depth)?;
        match term {
            Term::Pool(items) => {
                for item in items {
                    let alternatives = width(item);
                    if position < alternatives {
                        return self.selected_operand(item, position, depth + 1);
                    }
                    position -= alternatives;
                }
                unreachable!("selected position is inside the bounded structural pool")
            }
            Term::Function { name, arguments } => {
                self.text(name.as_str())?;
                Ok(Operand::Function(
                    Sign::Positive,
                    name.clone(),
                    self.selected_operands(arguments, position, depth + 1)?,
                ))
            }
            Term::Tuple(arguments) => Ok(Operand::Tuple(self.selected_operands(
                arguments,
                position,
                depth + 1,
            )?)),
            Term::UnaryOperation {
                operator: UnaryOp::Negate,
                argument,
            } if matches!(argument.as_ref(), Term::Function { .. }) => {
                let Operand::Function(_, name, arguments) =
                    self.selected_operand(argument, position, depth + 1)?
                else {
                    unreachable!()
                };
                Ok(Operand::Function(Sign::Negative, name, arguments))
            }
            _ => self.operand(term, true, depth),
        }
    }
    fn source_has_pool(&mut self, term: &Term, depth: usize) -> Result<bool, Error> {
        self.node(depth)?;
        let mut pooled = matches!(term, Term::Pool(_));
        match term {
            Term::Function { arguments, .. }
            | Term::External { arguments, .. }
            | Term::Tuple(arguments)
            | Term::Pool(arguments) => {
                for argument in arguments {
                    pooled |= self.source_has_pool(argument, depth + 1)?;
                }
            }
            Term::UnaryOperation { argument, .. } | Term::Absolute(argument) => {
                pooled |= self.source_has_pool(argument, depth + 1)?;
            }
            Term::BinaryOperation { left, right, .. }
            | Term::Interval {
                lower: left,
                upper: right,
            } => {
                pooled |= self.source_has_pool(left, depth + 1)?;
                pooled |= self.source_has_pool(right, depth + 1)?;
            }
            _ => {}
        }
        Ok(pooled)
    }
    pub(in super::super) fn pattern_variants(
        &mut self,
        atom: &Atom,
        arguments: &[Term],
    ) -> Result<Vec<Pattern>, Error> {
        let mut pooled = false;
        for argument in arguments {
            pooled |= self.source_has_pool(argument, 1)?;
        }
        let previous = self.pool_binding;
        self.pool_binding |= pooled;
        let result = self.variants(atom, arguments);
        self.pool_binding = previous;
        result
    }
    fn variants(&mut self, atom: &Atom, arguments: &[Term]) -> Result<Vec<Pattern>, Error> {
        let count = product(arguments);
        let remaining = (self.limits.max_nodes as usize).saturating_sub(self.nodes);
        if count > remaining as u128 {
            return Err(self.error(ErrorKind::Limit {
                resource: Resource::Nodes,
                observed: (self.nodes as u128).saturating_add(count),
                limit: u128::from(self.limits.max_nodes),
            }));
        }
        let mut patterns = Vec::new();
        for position in 0..count {
            self.node(1)?;
            self.text(atom.name.as_str())?;
            let mut terms = self.selected_operands(arguments, position, 1)?;
            self.prepare_inverses(&mut terms);
            let predicate = Predicate::with_sign(
                atom.name.as_str(),
                terms.len(),
                crate::coherence::core_sign(atom.sign),
            )
            .map_err(|_| self.error(ErrorKind::InvalidSymbol))?;
            patterns.push(Pattern {
                predicate,
                evaluated: terms.iter().any(evaluated),
                terms,
                key: None,
            });
        }
        Ok(patterns)
    }
}
