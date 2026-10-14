//! Total scalar arithmetic over full positive columns of one immutable snapshot.
//!
//! Each selected column contains every value of its variable in a complete
//! binding. Its numeric hull may include extra values: proving every arithmetic
//! intermediate defined on that hull therefore covers the binding. Correlations
//! and scalar guards provide no premises. Unknown bounds decline this optional
//! certificate; ordinary evaluation remains responsible for diagnostics.

use themelios_program::term::{BinaryOp, UnaryOp};
use zetesis_core::ValueNodeRef;

use super::{
    Computation, Context, Domain, Expression, FormulaFailure, LiteralIr, Operation, Postings,
    RuleIr, Support, domain, tick,
};
use crate::formula_support::Buffer;
use crate::scalar_arithmetic::Range;

/// The shared scalar grammar for source-column bounds. Recognition establishes
/// neither numeric inputs nor totality; checked ranges certify each intermediate.
pub(super) fn supports(operation: &Operation) -> bool {
    matches!(operation, Operation::Constant(_) | Operation::Variable(_)) || arithmetic(operation)
}

pub(super) fn arithmetic(operation: &Operation) -> bool {
    matches!(
        operation,
        Operation::Unary(UnaryOp::Negate, _)
            | Operation::Binary(
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod,
                _,
                _
            )
    )
}

/// Temporary numeric evidence for one normal body and immutable source snapshot.
///
/// A successful hull covers the same complete column throughout this attempt.
/// Only those hulls are retained; unknown domains still decline the certificate.
/// The buffers are created only for multi-input arithmetic, share the ordinary
/// support-storage leases, and are released with the attempt. No evidence crosses
/// a rule, support snapshot, candidate, or later totality attempt.
///
/// Each used variable's distinct numeric values are scanned at most once. Space
/// is O(rule variables + longest visited expression), independently of the number
/// of comparisons; every arithmetic intermediate is still checked separately.
pub(super) struct Preparation<'input, 'source> {
    rule: &'input RuleIr,
    support: &'input Support<'source>,
    workspace: Option<Workspace>,
}

struct Workspace {
    hulls: Buffer<Option<Range>>,
    values: Buffer<Range>,
}

impl<'input, 'source> Preparation<'input, 'source> {
    pub(super) fn new(rule: &'input RuleIr, support: &'input Support<'source>) -> Self {
        Self {
            rule,
            support,
            workspace: None,
        }
    }

    /// Seed one authenticated, closed range producer in the same workspace as
    /// positive-column hulls. This establishes bounds only, never a populated
    /// expression projection or completed-family evidence.
    pub(super) fn seed_range(
        &mut self,
        slot: usize,
        bounds: (i32, i32),
        context: &mut Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        tick(context)?;
        if self.workspace.is_none() {
            self.workspace = Some(Workspace::new(self.rule.body_variables, context)?);
        }
        let mut range = Range::point(bounds.0);
        range.include(bounds.1);
        self.workspace
            .as_mut()
            .expect("workspace initialized above")
            .hulls
            .slice_mut()[slot] = Some(range);
        Ok(())
    }

    /// Read the same numeric hull used by expression totality. A generated
    /// range has already seeded its slot; other inputs require a full column.
    pub(super) fn positive_domain(
        &mut self,
        slot: usize,
        context: &mut Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure> {
        if self.workspace.is_none() {
            self.workspace = Some(Workspace::new(self.rule.body_variables, context)?);
        }
        let workspace = self
            .workspace
            .as_mut()
            .expect("workspace initialized above");
        tick(context)?;
        if workspace.hulls.slice()[slot].is_none() {
            workspace.hulls.slice_mut()[slot] =
                numeric_domain(slot, &self.rule.body, self.support, context)?;
        }
        Ok(workspace.hulls.slice()[slot]
            .and_then(Range::positive_bounds)
            .is_some())
    }

    pub(super) fn domain(
        &self,
        slot: usize,
        context: &mut Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<Option<Domain<'source>>, FormulaFailure> {
        domain(slot, &self.rule.body, self.support, context)
    }

    pub(super) fn total(
        &mut self,
        expression: &Expression,
        context: &mut Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure> {
        if expression.nodes.iter().any(|node| !supports(node)) {
            return Ok(false);
        }
        if self.workspace.is_none() {
            self.workspace = Some(Workspace::new(self.rule.body_variables, context)?);
        }
        self.workspace
            .as_mut()
            .expect("numeric workspace initialized above")
            .total(expression, &self.rule.body, self.support, context)
    }
}

impl Workspace {
    fn new(
        variables: usize,
        context: &mut Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let mut hulls = Buffer::new(
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        hulls.resize(
            variables,
            None,
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        let values = Buffer::new(
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        Ok(Self { hulls, values })
    }

    fn total(
        &mut self,
        expression: &Expression,
        literals: &[LiteralIr],
        support: &Support<'_>,
        context: &mut Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure> {
        self.values.clear();
        self.values.reserve(
            expression.nodes.len(),
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        for operation in &expression.nodes {
            tick(context)?;
            let bound = match operation {
                Operation::Constant(scalar) => match context
                    .computation
                    .static_scalar(
                        *scalar,
                        context.work.limits,
                        context.work.counters,
                        context.work.location,
                    )?
                    .descriptor()
                {
                    ValueNodeRef::Number(value) => Some(Range::point(value)),
                    _ => None,
                },
                Operation::Variable(slot) => {
                    if self.hulls.slice()[*slot].is_none() {
                        self.hulls.slice_mut()[*slot] =
                            numeric_domain(*slot, literals, support, context)?;
                    }
                    self.hulls.slice()[*slot]
                }
                Operation::Unary(UnaryOp::Negate, argument) => {
                    self.values.slice()[*argument].unary(UnaryOp::Negate)
                }
                Operation::Binary(operator, left, right) => {
                    self.values.slice()[*left].binary(*operator, self.values.slice()[*right])
                }
                // The syntax gate excludes constructors and other unary operations.
                _ => None,
            };
            let Some(bound) = bound else {
                return Ok(false);
            };
            self.values.push(
                bound,
                context.computation,
                context.work.limits,
                context.work.counters,
                context.work.location,
            )?;
        }
        Ok(!expression.nodes.is_empty())
    }
}

fn numeric_domain(
    slot: usize,
    literals: &[LiteralIr],
    support: &Support<'_>,
    context: &mut Context<'_, &mut Computation<'_, '_>>,
) -> Result<Option<Range>, FormulaFailure> {
    let Some(domain) = domain(slot, literals, support, context)? else {
        return Ok(None);
    };
    column_range(domain.rows, domain.column, context)
}

/// A target-column hull, distinct from the smallest covering variable domain.
/// The caller retains the exact immutable row owner, never a bare variable ID.
pub(in crate::formula_support) fn positive_bounds<C>(
    rows: &crate::formula_support::relations::RelationRows<'_>,
    column: usize,
    context: &mut Context<'_, C>,
) -> Result<Option<(i32, i32)>, FormulaFailure> {
    Ok(column_range(rows, column, context)?.and_then(Range::positive_bounds))
}

fn column_range<C>(
    rows: &crate::formula_support::relations::RelationRows<'_>,
    column: usize,
    context: &mut Context<'_, C>,
) -> Result<Option<Range>, FormulaFailure> {
    let Postings::Indexed(postings) = rows.postings(column) else {
        return Ok(None);
    };
    let mut bound: Option<Range> = None;
    for posting in postings.values() {
        tick(context)?;
        let position = *posting.first().expect("a distinct input has a source row");
        let value = rows
            .row(position)
            .expect("retained source row")
            .value(column)
            .expect("checked source column");
        let ValueNodeRef::Number(value) = value.descriptor() else {
            return Ok(None);
        };
        match &mut bound {
            Some(bound) => bound.include(value),
            None => bound = Some(Range::point(value)),
        }
    }
    Ok(bound)
}
