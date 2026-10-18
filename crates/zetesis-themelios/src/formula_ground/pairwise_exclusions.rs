//! Replace complete pairwise exclusions from one fully traversed constraint.
//!
//! Source grouping only delimits a search. Distinct unordered pairs of exact
//! interned atom nodes certify every edge of a complete component; neither a
//! shared origin nor connectedness establishes an at-most-one condition.
//!
//! Pair nodes are deferred, not appended and later abandoned. Every joined row
//! still names and validates its original atoms and scalar obligations. Failed
//! rows retain the caller's ordinary source validation. Only positive atomic
//! bodies are collected; other source profiles keep their original route.
//!
//! Replacing all pair constraints by `threshold(atoms, 1, 2) -> false` preserves
//! original truth. A negative constraint's frozen truth, for arbitrary M and J,
//! equals its original truth at M: a false body freezes to false, and a violated
//! implication freezes to false. Thus the replacement preserves every reduct in
//! an arbitrary surrounding theory. It introduces no producer or atom.

mod plan;
#[cfg(test)]
mod tests;

use themelios_program::program::DefaultNegation;
use zetesis_ferraris::{NodeView as Node, Threshold};

use super::{Builder, MatchedNodes, VERUM};
use crate::FormulaFailure;
use crate::formula_binding::Binding;
use crate::formula_ir::{HeadIr, LiteralIr, RuleIr};
use crate::formula_support::{Buffer, witnesses::WitnessRow};

pub(super) struct Pending {
    pairs: Buffer<[usize; 2]>,
    mode: Mode,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Materialized,
    CapturedPairs,
    DeclinedCapture,
}

impl Pending {
    pub(super) fn for_rule(
        builder: &mut Builder<'_, '_, '_>,
        rule: &RuleIr,
    ) -> Result<Option<Self>, FormulaFailure> {
        #[cfg(test)]
        if !tests::enabled() {
            return Ok(None);
        }
        if !matches!(rule.head, HeadIr::Normal(None)) {
            return Ok(None);
        }
        let mut atoms = 0;
        for literal in &rule.body {
            builder.work(rule.location)?;
            match literal {
                LiteralIr::Atom(DefaultNegation::None, _) | LiteralIr::PatternAtom(_) => {
                    atoms += 1;
                }
                LiteralIr::Compare(_)
                | LiteralIr::ArgumentCheck(_)
                | LiteralIr::TupleCompare(..)
                | LiteralIr::Guard(_)
                | LiteralIr::HeadGuard(_)
                | LiteralIr::Bind { .. }
                | LiteralIr::Range { .. } => {}
                LiteralIr::Atom(..)
                | LiteralIr::ProjectedAtom(..)
                | LiteralIr::Conditional(_)
                | LiteralIr::Aggregate(_) => return Ok(None),
            }
        }
        if atoms < 2 {
            return Ok(None);
        }
        Ok(Some(Self {
            pairs: Buffer::new(
                builder.computation,
                builder.limits,
                builder.counters,
                rule.location,
            )?,
            mode: Mode::Materialized,
        }))
    }

    /// A captured template may move into the core only as a whole. Its mode
    /// records whether every passing row is a pair; no speculative DAG is made.
    pub(super) fn for_capture(
        builder: &mut Builder<'_, '_, '_>,
        rule: &RuleIr,
    ) -> Result<Option<Self>, FormulaFailure> {
        let mut pending = Self::for_rule(builder, rule)?;
        if let Some(pending) = &mut pending {
            pending.mode = Mode::CapturedPairs;
        }
        Ok(pending)
    }

    pub(super) fn row(
        &mut self,
        builder: &mut Builder<'_, '_, '_>,
        rule: &RuleIr,
        assignment: &Binding<'_>,
        mut matched: Option<(&mut MatchedNodes, &WitnessRow<'_, '_>)>,
    ) -> Result<(), FormulaFailure> {
        builder.work(rule.location)?;
        let assignment = rule.body_binding(assignment);
        let mut body = Body::One(VERUM);
        for (index, literal) in rule.body.iter().enumerate() {
            let named = match &mut matched {
                Some((nodes, row)) => nodes.literal(index, row, builder, rule.location)?,
                None => None,
            };
            let pattern = match literal {
                LiteralIr::Atom(DefaultNegation::None, pattern) => *pattern,
                LiteralIr::PatternAtom(pattern) => pattern.atom,
                _ => continue,
            };
            let atom = match named {
                Some(node) => node,
                None => builder.atom(pattern, &assignment, rule.location)?,
            };
            if self.mode == Mode::DeclinedCapture {
                continue;
            }
            let atom = builder.fixed_body_atom(atom, rule.location)?;
            body = body.append(atom, self.mode == Mode::Materialized, builder, rule)?;
        }
        if self.mode == Mode::DeclinedCapture {
            return Ok(());
        }
        match body {
            Body::Pair(left, right) => self.pairs.push(
                [left, right],
                builder.computation,
                builder.limits,
                builder.counters,
                rule.location,
            ),
            Body::One(body) | Body::Wider(Some(body)) if self.mode == Mode::Materialized => {
                let constraint = builder.neg(body, rule.location)?;
                builder.root(constraint, rule)
            }
            Body::One(_) | Body::Wider(_) => {
                self.mode = Mode::DeclinedCapture;
                self.pairs.clear();
                Ok(())
            }
        }
    }

    /// A completed capture either publishes all its certified groups or keeps
    /// the original stream. No pair fallback or partial strengthening is emitted.
    pub(super) fn finish_capture(
        mut self,
        builder: &mut Builder<'_, '_, '_>,
        rule: &RuleIr,
    ) -> Result<bool, FormulaFailure> {
        if self.mode != Mode::CapturedPairs || self.pairs.len() < 3 {
            return Ok(false);
        }
        let plan = plan::Plan::new(&mut self.pairs, builder, rule)?;
        for &[left, right] in self.pairs.slice() {
            let component = plan.component(left, builder, rule)?;
            if left == right || !plan.complete(component) {
                return Ok(false);
            }
        }
        emit_groups(&plan, builder, rule)?;
        Ok(true)
    }

    /// The caller reaches this boundary only after the complete original join.
    /// Private planning/refusal cannot publish a partially certified group.
    pub(super) fn finish(
        mut self,
        builder: &mut Builder<'_, '_, '_>,
        rule: &RuleIr,
    ) -> Result<(), FormulaFailure> {
        if self.pairs.len() < 3 {
            for &[left, right] in self.pairs.slice() {
                emit_pair(left, right, builder, rule)?;
            }
            return Ok(());
        }
        let plan = plan::Plan::new(&mut self.pairs, builder, rule)?;
        // Every declined occurrence is retained, including symmetric duplicates.
        for &[left, right] in self.pairs.slice() {
            let component = plan.component(left, builder, rule)?;
            if left == right || !plan.complete(component) {
                emit_pair(plan.node(left), plan.node(right), builder, rule)?;
            }
        }
        emit_groups(&plan, builder, rule)
    }
}

fn emit_groups(
    plan: &plan::Plan,
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
) -> Result<(), FormulaFailure> {
    let members = plan.members(builder, rule)?;
    let mut conditions = Buffer::new(
        builder.computation,
        builder.limits,
        builder.counters,
        rule.location,
    )?;
    let mut weights = Buffer::new(
        builder.computation,
        builder.limits,
        builder.counters,
        rule.location,
    )?;
    let mut start = 0;
    while start < members.len() {
        builder.work(rule.location)?;
        let component = members.slice()[start][0];
        let mut end = start + 1;
        while end < members.len() {
            builder.work(rule.location)?;
            if members.slice()[end][0] != component {
                break;
            }
            end += 1;
        }
        if plan.complete(component) {
            conditions.clear();
            for &[_, node] in &members.slice()[start..end] {
                conditions.push(
                    node,
                    builder.computation,
                    builder.limits,
                    builder.counters,
                    rule.location,
                )?;
            }
            weights.resize(
                conditions.len(),
                1,
                builder.computation,
                builder.limits,
                builder.counters,
                rule.location,
            )?;
            let threshold = builder.node(
                Node::Threshold(Threshold {
                    conditions: conditions.slice(),
                    weights: weights.slice(),
                    bound: 2,
                }),
                rule.location,
            )?;
            let constraint = builder.neg(threshold, rule.location)?;
            builder.root(constraint, rule)?;
        }
        start = end;
    }
    Ok(())
}

/// Delay exactly the first conjunction. Additional children resume the original
/// left fold; repeated singleton atoms keep the original `and(a, a) = a` rule.
enum Body {
    One(usize),
    Pair(usize, usize),
    Wider(Option<usize>),
}

impl Body {
    fn append(
        self,
        atom: usize,
        materialized: bool,
        builder: &mut Builder<'_, '_, '_>,
        rule: &RuleIr,
    ) -> Result<Self, FormulaFailure> {
        if atom == VERUM {
            return Ok(self);
        }
        Ok(match self {
            Self::One(VERUM) => Self::One(atom),
            Self::One(previous) if previous == atom => Self::One(atom),
            Self::One(previous) => Self::Pair(previous, atom),
            Self::Pair(left, right) if materialized => {
                let body = builder.and(left, right, rule.location)?;
                Self::Wider(Some(builder.and(body, atom, rule.location)?))
            }
            Self::Pair(..) | Self::Wider(None) => Self::Wider(None),
            Self::Wider(Some(body)) => Self::Wider(Some(builder.and(body, atom, rule.location)?)),
        })
    }
}

fn emit_pair(
    left: usize,
    right: usize,
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
) -> Result<(), FormulaFailure> {
    let body = builder.and(left, right, rule.location)?;
    let constraint = builder.neg(body, rule.location)?;
    builder.root(constraint, rule)
}
