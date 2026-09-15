//! A finite arithmetic match has exactly one invertible unknown occurrence.
//! Other operands must already be bound; division, powers and bit operations
//! remain consumers because their inverse is not a single finite candidate.

use super::{BTreeSet, Compiler, Operand, Template, UnaryOp};
use themelios_program::term::BinaryOp;

impl Compiler<'_> {
    pub(super) fn inverse_slot(
        &self,
        term: &Template,
        available: &BTreeSet<usize>,
    ) -> Option<usize> {
        match term {
            Template::Variable(slot) if !self.safe.contains(slot) && !available.contains(slot) => {
                Some(*slot)
            }
            Template::Unary(UnaryOp::Negate, inner) => self.inverse_slot(inner, available),
            Template::Binary(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul, left, right) => {
                if self.ready_with(right, available) {
                    self.inverse_slot(left, available)
                } else if self.ready_with(left, available) {
                    self.inverse_slot(right, available)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub(super) fn inverse_operands(&self, operands: &mut [Operand], available: &BTreeSet<usize>) {
        for operand in operands {
            match operand {
                Operand::Expression(expression) => {
                    if let Some(slot) = self.inverse_slot(expression, available) {
                        let expression = std::mem::replace(expression, Template::Variable(slot));
                        *operand = Operand::Inverse { slot, expression };
                    }
                }
                Operand::Function(_, _, children) | Operand::Tuple(children) => {
                    self.inverse_operands(children, available);
                }
                _ => {}
            }
        }
    }
}
