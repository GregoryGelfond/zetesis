//! Integer comparison forms: comparison literals a join decides with machine
//! integers.
//!
//! At a comparison literal's first decision, a join compiles it into a form
//! when both operands are integer arithmetic of at most [`MAX_NODES`] nodes
//! whose arguments name earlier nodes: each numeric constant becomes an `i32`
//! immediate, resolved once through the static components (which charge their
//! own lookup), and each variable keeps its slot. A longer operand, a
//! constructor, a nonnumeric constant or an argument naming a later node leaves
//! the literal without a form, and the literal's state records which.
//!
//! For a row whose operand variables are all bound to numbers, and whose
//! checked arithmetic succeeds, the comparison's value is the relation applied
//! to the order of the two machine integers, which is the ASP term order
//! restricted to numbers. No term is constructed and the projection memo is
//! neither read nor written. The evaluations and nodes are recorded as entered,
//! and the work performed, one unit per node and per variable read, is
//! admitted in one charge after evaluation and before the verdict is used.
//!
//! A row with a variable that cannot be read or is not a number, or with a
//! failing operation, declines: after the literal's first decision it charges
//! nothing and changes no state but the decline count, and the join's general
//! path decides it with its own charges, diagnostics, failure retention and
//! zero-divisor classification. A literal without a form never reaches a row.
//! Whether a literal has a form depends only on the literal, so a row's route is
//! a function of the literal and the row's values: every join that decides the
//! literal takes the same route on the same values.
//!
//! Compiling is charged and fallible like the join's other preparation: the
//! state vector's initialization, each constant's lookup and each reservation
//! charge their work, and a refused charge or reservation refuses the join's
//! decision rather than falling back. Storage is one state per body literal, one
//! step arena and one record per form, reserved through the join's lease and
//! counted in its storage bytes: with `L` literals, `F` forms and
//! `N ≤ 2·MAX_NODES·F` steps, `8·(L + N + F)` bytes of capacity.

use std::mem::size_of;

use themelios_program::program::Relation;
use themelios_program::term::{BinaryOp, UnaryOp};
use zetesis_core::ValueNodeRef;
use zetesis_core::catalog::CatalogRead;

use super::storage::{StorageLease, reserve, reserve_exact};
use super::{Computation, Context, GroundingWork, relation_holds};
use crate::FormulaFailure;
use crate::ProgramSite;
use crate::formula_binding::Binding;
use crate::formula_ir::{Expression, Operation};
use crate::grounding_observer::Event;

/// Operands with more nodes keep the general path: each operand evaluates in a
/// fixed array of this many integers, so no row allocates.
pub(super) const MAX_NODES: usize = 16;

/// What a join knows of one body literal's integer comparison form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LiteralState {
    /// Not decided yet in this join.
    Uncompiled,
    /// The literal has no form, for this reason; the general path decides it.
    WithoutForm(WithoutForm),
    /// The literal's form, by its index.
    Form(u32),
}

/// Why a comparison literal has no integer comparison form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WithoutForm {
    /// An operand has more than [`MAX_NODES`] nodes, or the join's literal or
    /// form count exceeds the state's index width.
    Bound,
    /// An operand constructs a function or tuple term.
    Constructor,
    /// An operand has a constant that is not a number.
    NonnumericConstant,
    /// An operation's argument names itself or a later node.
    ArgumentPosition,
}

/// One compiled node. Argument positions name earlier nodes of the same operand.
#[derive(Clone, Copy, Debug)]
enum Step {
    Number(i32),
    Variable(u32),
    Unary(UnaryOp, u8),
    Binary(BinaryOp, u8, u8),
    Absolute(u8),
}

/// One compiled literal: its left operand's steps start at `start`, its right
/// operand's follow them.
#[derive(Clone, Copy, Debug)]
struct Form {
    start: u32,
    left: u8,
    right: u8,
    relation: Relation,
}

impl Form {
    fn nodes(self) -> usize {
        usize::from(self.left) + usize::from(self.right)
    }
}

/// A join's integer comparison forms, empty until its first comparison
/// decision.
#[derive(Debug, Default)]
pub(super) struct IntegerComparisons {
    states: Vec<LiteralState>,
    steps: Vec<Step>,
    forms: Vec<Form>,
}

impl IntegerComparisons {
    /// The heap capacity this value retains, counted in the join's storage.
    pub(super) fn capacity_bytes(&self) -> usize {
        self.states.capacity() * size_of::<LiteralState>()
            + self.steps.capacity() * size_of::<Step>()
            + self.forms.capacity() * size_of::<Form>()
    }

    /// Decide body literal `index` of a join with `literals` body literals on
    /// `binding`, when its form applies: `Some` with the comparison's value,
    /// or `None` when the general path must decide it. `base` is the join's
    /// storage bytes other than this value's capacity.
    ///
    /// # Errors
    /// A refusal while preparing the literal's form (a charge, a constant's
    /// lookup or a reservation), or a work or cancellation refusal at a
    /// decided row's one charge.
    pub(super) fn decide(
        &mut self,
        lease: &mut StorageLease,
        base: usize,
        (index, literals): (usize, usize),
        comparison: ([&Expression; 2], Relation),
        binding: &Binding<'_>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<bool>, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        if self.states.is_empty() {
            let other = base + self.capacity_bytes();
            reserve_exact(
                &mut self.states,
                literals,
                lease,
                other,
                Context::new(computation, limits, counters, location),
            )?;
            counters.charge_work(literals as u128, limits, location)?;
            self.states.resize(literals, LiteralState::Uncompiled);
        }
        let state = match self.states[index] {
            LiteralState::Uncompiled => {
                let state = self.compile(
                    lease,
                    base,
                    comparison,
                    Context::new(computation, limits, counters, location),
                )?;
                counters.record(match state {
                    LiteralState::Form(_) => Event::IntegerComparisonForm,
                    _ => Event::ComparisonWithoutIntegerForm,
                });
                self.states[index] = state;
                state
            }
            state => state,
        };
        let LiteralState::Form(form) = state else {
            return Ok(None);
        };
        let form = self.forms[form as usize];
        let Some((left, right, reads)) = self.evaluate(form, binding, computation.read(), location)
        else {
            counters.record(Event::IntegerComparisonDeclined);
            return Ok(None);
        };
        // Recorded as entered, like the general evaluator's, before the charge
        // that may refuse them.
        counters.record(Event::ExpressionEvaluation);
        counters.record(Event::ExpressionEvaluation);
        for _ in 0..form.nodes() {
            counters.record(Event::ExpressionNode);
        }
        counters.charge_work((form.nodes() + reads) as u128, limits, location)?;
        counters.record(Event::IntegerComparison);
        Ok(Some(relation_holds(form.relation, left.cmp(&right))))
    }

    /// Compile a literal's operands into a form, or name why it has none.
    fn compile(
        &mut self,
        lease: &mut StorageLease,
        base: usize,
        ([left, right], relation): ([&Expression; 2], Relation),
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<LiteralState, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        if let Some(reason) = shape(left).or_else(|| shape(right)) {
            return Ok(LiteralState::WithoutForm(reason));
        }
        let (Ok(start), Ok(index)) = (
            u32::try_from(self.steps.len()),
            u32::try_from(self.forms.len()),
        ) else {
            return Ok(LiteralState::WithoutForm(WithoutForm::Bound));
        };
        let other = base + self.capacity_bytes() - self.steps.capacity() * size_of::<Step>();
        reserve(
            &mut self.steps,
            left.nodes.len() + right.nodes.len(),
            lease,
            other,
            Context::new(computation, limits, counters, location),
        )?;
        let other = base + self.capacity_bytes() - self.forms.capacity() * size_of::<Form>();
        reserve(
            &mut self.forms,
            1,
            lease,
            other,
            Context::new(computation, limits, counters, location),
        )?;
        for node in left.nodes.iter().chain(&right.nodes) {
            let step = match node {
                Operation::Constant(scalar) => {
                    let value = computation.static_scalar(*scalar, limits, counters, location)?;
                    let ValueNodeRef::Number(number) = value.descriptor() else {
                        self.steps.truncate(start as usize);
                        return Ok(LiteralState::WithoutForm(WithoutForm::NonnumericConstant));
                    };
                    Step::Number(number)
                }
                Operation::Variable(slot) => {
                    Step::Variable(u32::try_from(*slot).expect("shape() bounded the slot"))
                }
                Operation::Unary(operator, argument) => Step::Unary(*operator, position(*argument)),
                Operation::Binary(operator, left, right) => {
                    Step::Binary(*operator, position(*left), position(*right))
                }
                Operation::Absolute(argument) => Step::Absolute(position(*argument)),
                Operation::Constructor(_) => unreachable!("shape() excluded constructors"),
            };
            self.steps.push(step);
        }
        self.forms.push(Form {
            start,
            left: u8::try_from(left.nodes.len()).expect("shape() bounded the operand"),
            right: u8::try_from(right.nodes.len()).expect("shape() bounded the operand"),
            relation,
        });
        Ok(LiteralState::Form(index))
    }

    /// Both operands' values and the variable reads made, or `None` when a
    /// variable cannot be read or is not a number, or an operation fails.
    fn evaluate(
        &self,
        form: Form,
        binding: &Binding<'_>,
        read: CatalogRead<'_>,
        location: ProgramSite,
    ) -> Option<(i32, i32, usize)> {
        let start = form.start as usize;
        let middle = start + usize::from(form.left);
        let end = middle + usize::from(form.right);
        let mut reads = 0;
        let left = operand(
            &self.steps[start..middle],
            binding,
            read,
            location,
            &mut reads,
        )?;
        let right = operand(
            &self.steps[middle..end],
            binding,
            read,
            location,
            &mut reads,
        )?;
        Some((left, right, reads))
    }
}

/// Why an operand admits no form, judged from its nodes alone: `None` when
/// every node is a variable whose slot fits a `u32`, a constant (whose kind
/// compilation reads) or a checked integer operation whose arguments name
/// earlier nodes.
fn shape(expression: &Expression) -> Option<WithoutForm> {
    if expression.nodes.is_empty() || expression.nodes.len() > MAX_NODES {
        return Some(WithoutForm::Bound);
    }
    expression
        .nodes
        .iter()
        .enumerate()
        .find_map(|(position, node)| match node {
            Operation::Constant(_) => None,
            Operation::Variable(slot) => {
                u32::try_from(*slot).is_err().then_some(WithoutForm::Bound)
            }
            Operation::Unary(_, argument) | Operation::Absolute(argument) => {
                (*argument >= position).then_some(WithoutForm::ArgumentPosition)
            }
            Operation::Binary(_, left, right) => {
                (*left >= position || *right >= position).then_some(WithoutForm::ArgumentPosition)
            }
            Operation::Constructor(_) => Some(WithoutForm::Constructor),
        })
}

/// An earlier node's position, which `shape` bounded below `MAX_NODES`.
fn position(argument: usize) -> u8 {
    u8::try_from(argument).expect("an admitted operand has at most MAX_NODES nodes")
}

/// One operand's value, counting its variable reads.
fn operand(
    steps: &[Step],
    binding: &Binding<'_>,
    read: CatalogRead<'_>,
    location: ProgramSite,
    reads: &mut usize,
) -> Option<i32> {
    let mut values = [0_i32; MAX_NODES];
    for (index, step) in steps.iter().enumerate() {
        values[index] = match *step {
            Step::Number(number) => number,
            Step::Variable(slot) => {
                *reads += 1;
                let term = binding
                    .read_with(slot as usize, read, location, || Ok(()))
                    .ok()?;
                let ValueNodeRef::Number(number) = term.descriptor() else {
                    return None;
                };
                number
            }
            Step::Unary(operator, argument) => {
                crate::scalar_arithmetic::unary(operator, values[usize::from(argument)]).ok()?
            }
            Step::Binary(operator, left, right) => crate::scalar_arithmetic::binary(
                operator,
                values[usize::from(left)],
                values[usize::from(right)],
            )
            .ok()?,
            Step::Absolute(argument) => {
                crate::scalar_arithmetic::absolute(values[usize::from(argument)]).ok()?
            }
        };
    }
    steps.len().checked_sub(1).map(|root| values[root])
}

#[cfg(test)]
mod tests;
