//! Shared finite interval lowering for scoped structural value consumers.
//!
//! The iterative fold moves child plans in postorder and charges every moved
//! operation. Each interval owns a private range slot; consumers retain their
//! outer-rule, local-element, or conditional-alternative composition boundary.

use crate::diagnostic::unsupported;
use crate::formula_ir::{Compiler, Expression, LiteralIr, Operation, Variables};
use crate::{ExpansionResource, FormulaFailure, ProfileFeature};
use themelios_program::term::{Term, TermParts, UnaryOp, Variable};

impl Compiler<'_> {
    pub(super) fn ranged_expression(
        &mut self,
        term: &Term,
        variables: &mut Variables,
        bindings: &mut Vec<LiteralIr>,
    ) -> Result<Expression, FormulaFailure> {
        if !term
            .subterms()
            .any(|node| matches!(node, Term::Interval { .. }))
        {
            return self.expression(term, variables);
        }
        self.value_plan_preflight(term)?;
        // Reserve the source fold and its leaf plans before cloning. Subsequent
        // concatenations reserve their own operation storage before allocation.
        let count = term.subterms().count() as u128;
        self.budget
            .charge(ExpansionResource::TermWork, count * 3, self.location)?;
        let text = term
            .subterms()
            .map(|node| match node {
                Term::Symbolic(symbol) => crate::structural_value::symbol_bytes(symbol),
                Term::Function { name, .. } => name.as_str().len() as u128,
                Term::Variable(Variable::Named(name)) => name.as_str().len() as u128,
                _ => 0,
            })
            .sum::<u128>();
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            text + count * (std::mem::size_of::<Term>() + std::mem::size_of::<Operation>()) as u128,
            self.location,
        )?;
        term.clone().try_fold(|parts| {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            let (children, operation) = match parts {
                TermParts::Variable(variable) => {
                    return self.expression(&Term::Variable(variable), variables);
                }
                TermParts::Symbolic(symbol) => {
                    return self.expression(&Term::Symbolic(symbol), variables);
                }
                TermParts::Interval { lower, upper } => {
                    let target = self.consequent_slot(variables)?;
                    self.budget.charge(
                        ExpansionResource::ScalarBytes,
                        (3 * std::mem::size_of::<LiteralIr>() + std::mem::size_of::<Operation>())
                            as u128,
                        self.location,
                    )?;
                    bindings.push(LiteralIr::Range {
                        target,
                        lower,
                        upper,
                        binder: false,
                    });
                    return Ok(Expression {
                        nodes: vec![Operation::Variable(target)],
                    });
                }
                TermParts::UnaryOperation {
                    operator,
                    mut argument,
                } => {
                    if operator == UnaryOp::Negate
                        && let Some(Operation::Constructor(constructor)) = argument.nodes.last_mut()
                        && constructor.name.is_some()
                    {
                        constructor.sign = match constructor.sign {
                            zetesis_core::Sign::Positive => zetesis_core::Sign::Negative,
                            zetesis_core::Sign::Negative => zetesis_core::Sign::Positive,
                        };
                        return Ok(argument);
                    }
                    (vec![argument], Pending::Unary(operator))
                }
                TermParts::BinaryOperation {
                    operator,
                    left,
                    right,
                } => (vec![left, right], Pending::Binary(operator)),
                TermParts::Absolute(argument) => (vec![argument], Pending::Absolute),
                TermParts::Function { name, arguments } => (
                    arguments,
                    Pending::Constructor(Some(name.as_str().to_owned())),
                ),
                TermParts::Tuple(arguments) => (arguments, Pending::Constructor(None)),
                _ => return Err(unsupported(ProfileFeature::Term, self.location).into()),
            };
            self.join_value_plans(children, operation)
        })
    }

    fn join_value_plans(
        &mut self,
        children: Vec<Expression>,
        operation: Pending,
    ) -> Result<Expression, FormulaFailure> {
        let count = children
            .iter()
            .map(|child| child.nodes.len() as u128)
            .sum::<u128>()
            + 1;
        self.budget
            .charge(ExpansionResource::TermWork, count, self.location)?;
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            count * std::mem::size_of::<Operation>() as u128
                + children.len() as u128 * std::mem::size_of::<usize>() as u128,
            self.location,
        )?;
        let mut nodes = Vec::new();
        let mut roots = Vec::new();
        for child in children {
            let offset = nodes.len();
            for mut node in child.nodes {
                match &mut node {
                    Operation::Unary(_, input) | Operation::Absolute(input) => *input += offset,
                    Operation::Binary(_, left, right) => {
                        *left += offset;
                        *right += offset;
                    }
                    Operation::Constructor(constructor) => {
                        for input in &mut constructor.arguments {
                            *input += offset;
                        }
                    }
                    Operation::Variable(_) | Operation::Constant(_) => {}
                }
                nodes.push(node);
            }
            roots.push(nodes.len() - 1);
        }
        nodes.push(match operation {
            Pending::Unary(operator) => Operation::Unary(operator, roots[0]),
            Pending::Binary(operator) => Operation::Binary(operator, roots[0], roots[1]),
            Pending::Absolute => Operation::Absolute(roots[0]),
            Pending::Constructor(name) => {
                Operation::Constructor(Box::new(crate::formula_value::Constructor {
                    name,
                    sign: zetesis_core::Sign::Positive,
                    arguments: roots,
                }))
            }
        });
        Ok(Expression { nodes })
    }
}

enum Pending {
    Unary(UnaryOp),
    Binary(themelios_program::term::BinaryOp),
    Absolute,
    Constructor(Option<String>),
}
