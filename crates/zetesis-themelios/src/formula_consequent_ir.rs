//! Finite consequent alternatives share value evaluation and structural matching.
//!
//! The fold consumes a finite source tree. Child plans are moved in postorder;
//! every moved operation is charged, so deeply nested source cannot hide the
//! potentially quadratic plan-copy work. Intervals become private range slots.

use themelios_program::program::{Arguments, Atom, DefaultNegation, Relation};
use themelios_program::term::{Term, TermParts, UnaryOp, Variable};
use zetesis_core::{AtomPattern, Predicate, Term as CoreTerm};

use crate::diagnostic::unsupported;
use crate::formula::ceiling;
use crate::formula_conditional_ir::{Alternative, ConsequentOperand};
use crate::formula_ir::{Compiler, Expression, LiteralIr, Operation, Variables};
use crate::{AdmissionFailure, ExpansionResource, FormulaFailure, FormulaResource, ProfileFeature};

impl Compiler<'_> {
    pub(super) fn consequent_alternative(
        &mut self,
        atom: &Atom,
        negation: DefaultNegation,
        condition: &Variables,
    ) -> Result<Alternative, FormulaFailure> {
        let Arguments::Single(arguments) = &atom.arguments else {
            unreachable!("conditional pools were selected")
        };
        let work = arguments.iter().flat_map(Term::subterms).count() as u128;
        self.budget.charge(
            ExpansionResource::TermWork,
            work * 2
                + condition.named.len() as u128
                + condition.safe.len() as u128
                + condition.argument_inputs.len() as u128,
            self.location,
        )?;
        // Each alternative owns its scope and binding plan. Reserve selected
        // map/set payload and three instruction-vector equivalents (source,
        // pending, scheduled) before cloning or allocating them. Tree allocator
        // overhead remains outside logical byte accounting.
        let scope_bytes = condition
            .named
            .keys()
            .map(|name| name.len() as u128 + std::mem::size_of::<(String, usize)>() as u128)
            .sum::<u128>()
            + (condition.safe.len() + condition.argument_inputs.len()) as u128
                * std::mem::size_of::<usize>() as u128;
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            scope_bytes
                + std::mem::size_of::<Alternative>() as u128
                + (work + 1) * 3 * std::mem::size_of::<LiteralIr>() as u128,
            self.location,
        )?;
        let mut local = condition.clone();
        let mut bindings = Vec::new();
        let missing = arguments
            .iter()
            .flat_map(Term::subterms)
            .any(|term| match term {
                Term::Variable(Variable::Anonymous) => true,
                Term::Variable(Variable::Named(name)) => !local.named.contains_key(name.as_str()),
                _ => false,
            });
        let operand = if let Some(LiteralIr::ProjectedAtom(_, projection)) =
            self.projected_atom(atom, negation, &mut local, &mut bindings)?
        {
            ConsequentOperand::Projection(projection)
        } else if missing && negation == DefaultNegation::None {
            ConsequentOperand::Atom(self.consequent_witness(
                atom,
                arguments,
                &mut local,
                &mut bindings,
            )?)
        } else {
            ConsequentOperand::Atom(self.consequent_atom(
                atom,
                arguments,
                &mut local,
                &mut bindings,
            )?)
        };
        self.bindings(&mut bindings, &mut local)?;
        self.variable_limit(&local)?;
        local.safety(self.location)?;
        Ok(Alternative {
            operand,
            bindings,
            variables: local.count,
        })
    }

    fn consequent_atom(
        &mut self,
        atom: &Atom,
        arguments: &[Term],
        local: &mut Variables,
        bindings: &mut Vec<LiteralIr>,
    ) -> Result<AtomPattern, FormulaFailure> {
        ceiling(
            FormulaResource::Arity,
            arguments.len() as u128,
            self.options.core_limits.max_predicate_arity as u128,
            self.location,
        )?;
        let mut terms = Vec::new();
        for term in arguments {
            if matches!(term, Term::Symbolic(_) | Term::Variable(_)) {
                let value = self.objective_term(term, local)?;
                if let CoreTerm::Constant(value) = &value {
                    self.value(value)?;
                }
                terms.push(value);
            } else {
                let value = self.consequent_expression(term, local, bindings)?;
                let target = self.consequent_slot(local)?;
                bindings.push(LiteralIr::Compare(
                    Expression {
                        nodes: vec![Operation::Variable(target)],
                    },
                    Relation::Eq,
                    value,
                ));
                terms.push(CoreTerm::Variable(target));
            }
        }
        let predicate = Predicate::with_sign(
            atom.name.as_str(),
            arguments.len(),
            crate::coherence::core_sign(atom.sign),
        )
        .map_err(|error| AdmissionFailure::Construction {
            error,
            location: self.location,
        })?;
        AtomPattern::new(predicate, terms).map_err(|error| {
            AdmissionFailure::Construction {
                error,
                location: self.location,
            }
            .into()
        })
    }

    /// Witnesses select relational rows, never invert arithmetic or enumerate a
    /// global value universe. Extracted names stay private to this alternative;
    /// evaluated positions consume those names after matching. The whole captured
    /// source atom remains the emitted logical consequent.
    fn consequent_witness(
        &mut self,
        atom: &Atom,
        arguments: &[Term],
        local: &mut Variables,
        bindings: &mut Vec<LiteralIr>,
    ) -> Result<AtomPattern, FormulaFailure> {
        if let Some(pattern) = self.positive_witness(atom, local, bindings)? {
            let atom = self.consequent_capture(&pattern.atom)?;
            bindings.push(LiteralIr::PatternAtom(pattern));
            Ok(atom)
        } else {
            if arguments
                .iter()
                .any(|term| !matches!(term, Term::Variable(_) | Term::Symbolic(_)))
            {
                return Err(unsupported(ProfileFeature::Term, self.location).into());
            }
            let atom = self.atom(atom, local, true)?;
            bindings.push(LiteralIr::Atom(DefaultNegation::None, atom.clone()));
            Ok(atom)
        }
    }

    /// Retain the complete capture independently of its matching instruction.
    /// Capture slots represent matched arguments; closed constants retain their
    /// complete values. Charge term/payload visits, predicate text, term cells
    /// and selected value payload before cloning. Structured payload is charged
    /// conservatively even when its immutable allocation is shared by the clone,
    /// following the existing value-copy budget. Allocator metadata is excluded.
    pub(super) fn consequent_capture(
        &mut self,
        pattern: &AtomPattern,
    ) -> Result<AtomPattern, FormulaFailure> {
        self.budget.charge(
            ExpansionResource::TermWork,
            pattern.terms().len() as u128
                + pattern
                    .terms()
                    .iter()
                    .map(|term| match term {
                        CoreTerm::Constant(zetesis_core::Value::Structured(value)) => {
                            value.nodes().len() as u128
                        }
                        _ => 0,
                    })
                    .sum::<u128>(),
            self.location,
        )?;
        let bytes = pattern.predicate().name().len() as u128
            + std::mem::size_of_val(pattern.terms()) as u128
            + pattern
                .terms()
                .iter()
                .map(|term| match term {
                    CoreTerm::Constant(value) => crate::formula_ir::value_bytes(value),
                    CoreTerm::Variable(_) => 0,
                })
                .sum::<u128>();
        self.budget
            .charge(ExpansionResource::ScalarBytes, bytes, self.location)?;
        Ok(pattern.clone())
    }

    pub(super) fn consequent_slot(
        &self,
        variables: &mut Variables,
    ) -> Result<usize, FormulaFailure> {
        ceiling(
            FormulaResource::Variables,
            variables.count as u128 + 1,
            self.options.core_limits.max_variables_per_template as u128,
            self.location,
        )?;
        Ok(variables.slot(&Variable::Anonymous))
    }

    pub(super) fn consequent_expression(
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
            self.consequent_join(children, operation)
        })
    }

    fn consequent_join(
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
