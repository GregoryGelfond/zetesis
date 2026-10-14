//! The comparison literal: two operand expressions, their relation, and the
//! literal's integer comparison form or the reason it has none.
//!
//! The source program's comparison (`themelios_program::program::Comparison`)
//! is the written one: a first term and one or more relation steps, so
//! `1 < X < 5` is one written literal. A [`Comparison`] is one compiled step
//! of that concept: one relation between two operand expressions, made for
//! each step the compiler lowers into a comparison literal and for each
//! generated or captured equality.

use themelios_program::program::Relation;

use super::Expression;
use super::integer::{self, IntegerComparison, WithoutForm};
use crate::expansion::{Budget, ExpansionResource};
use crate::formula_support::{Counters, components};
use crate::{FormulaFailure, FormulaLimits, ProgramSite};

/// One compiled comparison step with its integer comparison form.
///
/// Its fields are private: a `Comparison` is made only by [`compile`], which
/// compiles the form from the operands it stores and reads every constant
/// itself, and copied only by [`Comparison::copy`]. Nothing reaches an
/// operand mutably afterwards, so the form agrees with its operands in
/// structure and in value for the literal's whole lifetime, the prepared
/// program's.
pub(crate) struct Comparison {
    left: Expression,
    relation: Relation,
    right: Expression,
    form: Result<IntegerComparison, WithoutForm>,
}

impl Comparison {
    pub(crate) fn left(&self) -> &Expression {
        &self.left
    }

    pub(crate) fn relation(&self) -> Relation {
        self.relation
    }

    pub(crate) fn right(&self) -> &Expression {
        &self.right
    }

    /// Both operands and the relation, as the general evaluator takes them.
    pub(crate) fn operands(&self) -> ([&Expression; 2], Relation) {
        ([&self.left, &self.right], self.relation)
    }

    /// Its integer comparison form, or why it has none.
    pub(crate) fn form(&self) -> Result<&IntegerComparison, WithoutForm> {
        self.form.as_ref().map_err(|reason| *reason)
    }

    /// Its operands and relation, ending the literal: the form is dropped
    /// with it, so no form outlives the operands it was compiled from.
    pub(crate) fn into_parts(self) -> (Expression, Relation, Expression) {
        (self.left, self.relation, self.right)
    }

    /// A copy whose operands are copied node for node by [`Expression::copy`],
    /// charging `work` before each node, with this literal's form. The form's
    /// bytes are charged as `ScalarBytes` before it is allocated.
    ///
    /// # Errors
    /// `work`'s refusal, or the expansion budget's.
    pub(crate) fn copy(
        &self,
        mut work: impl FnMut() -> Result<(), FormulaFailure>,
        budget: &mut Budget,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let left = self.left.copy(&mut work, budget, location)?;
        let right = self.right.copy(&mut work, budget, location)?;
        let form = match &self.form {
            Ok(form) => {
                budget.charge(
                    ExpansionResource::ScalarBytes,
                    IntegerComparison::bytes(form.steps()) as u128,
                    location,
                )?;
                Ok(form.copy())
            }
            Err(reason) => Err(*reason),
        };
        Ok(Self {
            left,
            relation: self.relation,
            right,
            form,
        })
    }
}

/// The comparison `left relation right`, compiled with its integer comparison
/// form: the only way a [`Comparison`] is made.
///
/// Both operands must admit the form's shape, the left operand's judged
/// first. A literal they admit is charged one `TermWork` unit per step of
/// both operands before its constants are read; each constant is then read
/// through `source` ([`components::Admission::scalar_number`]), which
/// charges formula work as any component read does, left operand before
/// right, until the first one that is not a number; and the form's bytes are
/// charged as `ScalarBytes` before it is allocated. A literal the shape rule
/// excludes is read and charged nothing.
///
/// # Errors
/// A refusal of those charges, of a constant read, or of cancellation.
pub(super) fn compile(
    (left, relation, right): (Expression, Relation, Expression),
    source: &components::Admission<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    budget: &mut Budget,
    location: ProgramSite,
) -> Result<Comparison, FormulaFailure> {
    let form = form(
        [&left.nodes, &right.nodes],
        source,
        limits,
        counters,
        budget,
        location,
    )?;
    Ok(Comparison {
        left,
        relation,
        right,
        form,
    })
}

fn form(
    [left, right]: [&[super::Operation]; 2],
    source: &components::Admission<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    budget: &mut Budget,
    location: ProgramSite,
) -> Result<Result<IntegerComparison, WithoutForm>, FormulaFailure> {
    if let Err(reason) = integer::shape(left).and_then(|()| integer::shape(right)) {
        return Ok(Err(reason));
    }
    let steps = left.len() + right.len();
    budget.charge(ExpansionResource::TermWork, steps as u128, location)?;
    let mut read = |scalar| source.scalar_number(scalar, limits, counters, location);
    let Some(left_numbers) = integer::numbers(left, &mut read)? else {
        return Ok(Err(WithoutForm::NonnumericConstant));
    };
    let Some(right_numbers) = integer::numbers(right, &mut read)? else {
        return Ok(Err(WithoutForm::NonnumericConstant));
    };
    budget.charge(
        ExpansionResource::ScalarBytes,
        IntegerComparison::bytes(steps) as u128,
        location,
    )?;
    Ok(Ok(IntegerComparison::new(
        [left, right],
        [&left_numbers, &right_numbers],
    )))
}
