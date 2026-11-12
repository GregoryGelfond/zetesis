//! A bounded positive source grammar certifies the full unequal-lane product.

use themelios_program::program::{DefaultNegation, Relation};
use zetesis_core::{PatternRef, TemplateTerm};

use super::{Builder, owner};
use crate::formula_binding::Binding;
use crate::formula_ir::{HeadIr, LiteralIr, Operation, RuleIr};
use crate::formula_support::{Buffer, StorageLease, components};
use crate::{FormulaFailure, ProgramSite};

pub(super) struct Plan {
    lane: usize,
    literals: Buffer<LiteralIr>,
    used: Buffer<usize>,
    keys: Buffer<components::Term>,
    common: Buffer<usize>,
    left: Buffer<usize>,
    _lease: StorageLease,
}

impl Plan {
    pub(super) fn new(
        builder: &mut Builder<'_, '_, '_>,
        rule: &RuleIr,
    ) -> Result<Option<Self>, FormulaFailure> {
        let Some(lanes) = lanes(builder, rule)? else {
            return Ok(None);
        };
        let Some(mut partition) = Partition::new(builder, rule, lanes)? else {
            return Ok(None);
        };
        for &used in partition.used.slice() {
            builder.work(rule.location)?;
            if !used {
                return Ok(None);
            }
        }
        if partition.left.len() == 0
            || partition.left.len() != partition.right.len()
            || !partition.symmetric(builder, rule, lanes)?
        {
            return Ok(None);
        }
        let literals = partition.selected_literals(builder, rule)?;
        let mut used = Buffer::new(
            builder.computation,
            builder.limits,
            builder.counters,
            rule.location,
        )?;
        let mut keys = Buffer::new(
            builder.computation,
            builder.limits,
            builder.counters,
            rule.location,
        )?;
        for (slot, &read) in partition.used.iter().enumerate() {
            builder.work(rule.location)?;
            if read && slot != lanes[1] {
                used.push(
                    slot,
                    builder.computation,
                    builder.limits,
                    builder.counters,
                    rule.location,
                )?;
                if slot != lanes[0] {
                    keys.push(
                        components::Term::Variable(slot),
                        builder.computation,
                        builder.limits,
                        builder.counters,
                        rule.location,
                    )?;
                }
            }
        }
        let mut lease = builder.computation.lease();
        let header = size_of::<Self>()
            - size_of::<Buffer<LiteralIr>>()
            - 3 * size_of::<Buffer<usize>>()
            - size_of::<Buffer<components::Term>>();
        lease.observe(header, rule.location)?;
        builder.computation.storage_observed(
            &lease,
            0,
            header,
            builder.limits,
            builder.counters,
            rule.location,
        )?;
        Ok(Some(Self {
            lane: lanes[0],
            literals,
            used,
            keys,
            common: partition.common,
            left: partition.left,
            _lease: lease,
        }))
    }

    pub(super) fn literals(&self) -> &[LiteralIr] {
        self.literals.slice()
    }
    pub(super) fn used(&self) -> &[usize] {
        self.used.slice()
    }
    pub(super) fn left(&self) -> &[usize] {
        self.left.slice()
    }
    pub(super) fn common(&self) -> &[usize] {
        self.common.slice()
    }

    /// Both tuples are authenticated by the same current Binding and the
    /// Builder's one existing term coordinate owner, including the empty key.
    pub(super) fn identity(
        &self,
        builder: &mut Builder<'_, '_, '_>,
        binding: &Binding<'_>,
        location: ProgramSite,
    ) -> Result<[usize; 2], FormulaFailure> {
        let key = builder.head_tuple(self.keys.slice(), binding, location)?;
        let lane =
            builder.head_tuple(&[components::Term::Variable(self.lane)], binding, location)?;
        Ok([key, lane])
    }
}

fn lanes(
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
) -> Result<Option<[usize; 2]>, FormulaFailure> {
    if !matches!(rule.head, HeadIr::Normal(None))
        || rule.variables != rule.body_variables
        || rule.bindings.is_some()
    {
        return Ok(None);
    }
    let mut lanes = None;
    for literal in &rule.body {
        builder.work(rule.location)?;
        match literal {
            LiteralIr::Atom(DefaultNegation::None, _) => {}
            LiteralIr::Compare(comparison) => {
                let ([Operation::Variable(left)], [Operation::Variable(right)]) = (
                    comparison.left().nodes.as_slice(),
                    comparison.right().nodes.as_slice(),
                ) else {
                    return Ok(None);
                };
                if comparison.relation() != Relation::Neq || left == right || lanes.is_some() {
                    return Ok(None);
                }
                if *left >= rule.variables || *right >= rule.variables {
                    return Err(owner(rule.location));
                }
                lanes = Some([*left, *right]);
            }
            _ => return Ok(None),
        }
    }
    Ok(lanes)
}

struct Partition {
    used: Buffer<bool>,
    common: Buffer<usize>,
    left: Buffer<usize>,
    right: Buffer<usize>,
}

impl Partition {
    /// Keep the admitted common/left occurrences in original source order.
    fn selected_literals(
        &self,
        builder: &mut Builder<'_, '_, '_>,
        rule: &RuleIr,
    ) -> Result<Buffer<LiteralIr>, FormulaFailure> {
        let mut literals = Buffer::new(
            builder.computation,
            builder.limits,
            builder.counters,
            rule.location,
        )?;
        // Merge ascending original positions. Join planning can order probes;
        // the source slice itself keeps every admitted original occurrence.
        let (mut common, mut left) = (0, 0);
        for (index, literal) in rule.body.iter().enumerate() {
            builder.work(rule.location)?;
            let selected = if self.common.slice().get(common) == Some(&index) {
                common += 1;
                true
            } else if self.left.slice().get(left) == Some(&index) {
                left += 1;
                true
            } else {
                false
            };
            if selected {
                let LiteralIr::Atom(DefaultNegation::None, pattern) = literal else {
                    return Err(owner(rule.location));
                };
                literals.push(
                    LiteralIr::Atom(DefaultNegation::None, *pattern),
                    builder.computation,
                    builder.limits,
                    builder.counters,
                    rule.location,
                )?;
            }
        }
        Ok(literals)
    }

    fn new(
        builder: &mut Builder<'_, '_, '_>,
        rule: &RuleIr,
        lanes: [usize; 2],
    ) -> Result<Option<Self>, FormulaFailure> {
        let mut result = Self {
            used: Buffer::new(
                builder.computation,
                builder.limits,
                builder.counters,
                rule.location,
            )?,
            common: Buffer::new(
                builder.computation,
                builder.limits,
                builder.counters,
                rule.location,
            )?,
            left: Buffer::new(
                builder.computation,
                builder.limits,
                builder.counters,
                rule.location,
            )?,
            right: Buffer::new(
                builder.computation,
                builder.limits,
                builder.counters,
                rule.location,
            )?,
        };
        result.used.resize(
            rule.variables,
            false,
            builder.computation,
            builder.limits,
            builder.counters,
            rule.location,
        )?;
        for (index, literal) in rule.body.iter().enumerate() {
            builder.work(rule.location)?;
            let LiteralIr::Atom(DefaultNegation::None, pattern) = literal else {
                continue;
            };
            let pattern = builder.computation.static_pattern(
                *pattern,
                builder.limits,
                builder.counters,
                rule.location,
            )?;
            let mut present = [false; 2];
            for term in pattern.terms() {
                builder.work(rule.location)?;
                if let TemplateTerm::Variable(slot) = term {
                    let read = result
                        .used
                        .slice_mut()
                        .get_mut(slot)
                        .ok_or_else(|| owner(rule.location))?;
                    *read = true;
                    present[0] |= slot == lanes[0];
                    present[1] |= slot == lanes[1];
                }
            }
            let selected = match present {
                [true, true] => return Ok(None),
                [true, false] => &mut result.left,
                [false, true] => &mut result.right,
                [false, false] => &mut result.common,
            };
            selected.push(
                index,
                builder.computation,
                builder.limits,
                builder.counters,
                rule.location,
            )?;
        }
        Ok(Some(result))
    }

    fn symmetric(
        &mut self,
        builder: &mut Builder<'_, '_, '_>,
        rule: &RuleIr,
        lanes: [usize; 2],
    ) -> Result<bool, FormulaFailure> {
        let mut matched = Buffer::new(
            builder.computation,
            builder.limits,
            builder.counters,
            rule.location,
        )?;
        matched.resize(
            self.right.len(),
            false,
            builder.computation,
            builder.limits,
            builder.counters,
            rule.location,
        )?;
        for &left in self.left.slice() {
            let left = pattern(builder, rule, left)?;
            let mut found = None;
            for (at, &right) in self.right.iter().enumerate() {
                builder.work(rule.location)?;
                if !matched.slice()[at]
                    && swapped(
                        left,
                        pattern(builder, rule, right)?,
                        lanes,
                        builder,
                        rule.location,
                    )?
                {
                    found = Some(at);
                    break;
                }
            }
            let Some(at) = found else {
                return Ok(false);
            };
            builder.work(rule.location)?;
            matched.slice_mut()[at] = true;
        }
        Ok(true)
    }
}

fn pattern<'source>(
    builder: &mut Builder<'_, '_, 'source>,
    rule: &RuleIr,
    index: usize,
) -> Result<PatternRef<'source>, FormulaFailure> {
    let LiteralIr::Atom(DefaultNegation::None, pattern) = &rule.body[index] else {
        return Err(owner(rule.location));
    };
    builder
        .computation
        .static_pattern(*pattern, builder.limits, builder.counters, rule.location)
}

fn swapped(
    left: PatternRef<'_>,
    right: PatternRef<'_>,
    lanes: [usize; 2],
    builder: &mut Builder<'_, '_, '_>,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    if !left
        .predicate()
        .equals_ref_with(right.predicate(), || builder.work(location))?
        || left.terms().len() != right.terms().len()
    {
        return Ok(false);
    }
    for (left, right) in left.terms().into_iter().zip(right.terms()) {
        builder.work(location)?;
        let equal = match (left, right) {
            (TemplateTerm::Variable(left), TemplateTerm::Variable(right)) => {
                let left = if left == lanes[0] {
                    lanes[1]
                } else if left == lanes[1] {
                    lanes[0]
                } else {
                    left
                };
                left == right
            }
            (TemplateTerm::Constant(left), TemplateTerm::Constant(right)) => {
                left.equals_ref_with(right, || builder.work(location))?
            }
            _ => false,
        };
        if !equal {
            return Ok(false);
        }
    }
    Ok(true)
}
