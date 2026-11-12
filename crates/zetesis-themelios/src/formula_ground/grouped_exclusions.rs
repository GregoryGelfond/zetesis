//! Complete typed binding fibers, without constructing their pair product.
//!
//! A checked source partition supplies the full product of the same positive
//! lane relation with itself, except equal lane values. Witnesses reduce with
//! Any within a lane; each distinct lane remains a separate unit occurrence.
//! The shared condition stays outside the count. No fact is inferred from
//! possible support. See the `GroupedWitnessExclusion` correspondence.

mod plan;
#[cfg(test)]
mod testing;
#[cfg(test)]
mod tests;

/// Isolate the existing pair collector in its own exact representation tests.
#[cfg(test)]
pub(super) fn without_grouping<T>(action: impl FnOnce() -> T) -> T {
    testing::scoped(false, action).0
}

use zetesis_ferraris::{NodeView as Node, Threshold};

use super::{Builder, FALSUM, cache::CoordinateMap};
use crate::formula_binding::Binding;
use crate::formula_ir::RuleIr;
use crate::formula_support::{Buffer, ComponentPlans, ComponentScope, Context, Ir, Support};
use crate::{FormulaFailure, ProgramSite};

/// Only a declined source grammar returns false. A resource or owner failure
/// remains a typed failure; an attempted grouped traversal is never retried as
/// an ordinary pair traversal.
pub(super) fn rule<'source>(
    builder: &mut Builder<'_, '_, 'source>,
    rule: &RuleIr,
    support: &Support<'source>,
) -> Result<bool, FormulaFailure> {
    #[cfg(test)]
    if !testing::enabled() {
        return Ok(false);
    }
    let Some(plan) = plan::Plan::new(builder, rule)? else {
        return Ok(false);
    };
    let fixed = Binding::new(
        builder.computation,
        builder.limits,
        builder.counters,
        rule.location,
    )?;
    let mut prepared = ComponentPlans::new(
        1,
        rule.variables,
        &fixed,
        support,
        Context::new(
            &*builder.computation,
            builder.limits,
            builder.counters,
            rule.location,
        ),
    )?;
    let mut groups = Groups::new(builder, rule.location)?;
    #[cfg(test)]
    testing::record(testing::Point::Accepted);
    collect(
        &plan,
        &fixed,
        &mut prepared,
        Pass::Identities(&mut groups),
        builder,
        rule,
        support,
    )?;
    if groups.has_pairs(builder, rule.location)? {
        collect(
            &plan,
            &fixed,
            &mut prepared,
            Pass::Witnesses(&mut groups),
            builder,
            rule,
            support,
        )?;
        groups.emit(builder, rule)?;
    }
    Ok(true)
}

enum Pass<'a> {
    Identities(&'a mut Groups),
    Witnesses(&'a mut Groups),
}

/// Both traversals borrow the same immutable plan and completed Support, with
/// fresh current bindings. No frame or relation row survives a cursor advance.
fn collect<'plan, 'source: 'plan>(
    plan: &'plan plan::Plan,
    fixed: &Binding<'_>,
    prepared: &mut ComponentPlans<'plan, 'source>,
    mut pass: Pass<'_>,
    builder: &mut Builder<'_, '_, 'source>,
    rule: &RuleIr,
    support: &'plan Support<'source>,
) -> Result<(), FormulaFailure> {
    let mut join = prepared.join(
        0,
        ComponentScope {
            literals: plan.literals(),
            used: plan.used(),
        },
        fixed,
        support,
        builder.budget,
        Context::new(
            &*builder.computation,
            builder.limits,
            builder.counters,
            rule.location,
        ),
    )?;
    while let Some(row) = join.next_matched_row(
        builder.budget,
        Context::new(
            builder.computation,
            builder.limits,
            builder.counters,
            rule.location,
        ),
    )? {
        // The certificate admits positive atoms only. Keep the ordinary
        // validation boundary explicit if a rejected completion is returned.
        if !row.passes {
            builder.validate_body(plan.literals(), &row.values, support, rule.location)?;
            continue;
        }
        let key = plan.identity(builder, &row.values, rule.location)?;
        match &mut pass {
            Pass::Witnesses(groups) => {
                groups.witness(key, plan, &row.values, builder, rule, support)?;
                #[cfg(test)]
                testing::record(testing::Point::SecondRow);
            }
            Pass::Identities(groups) => {
                groups.identity(key, builder, rule.location)?;
                #[cfg(test)]
                testing::record(testing::Point::FirstRow);
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Fiber {
    count: usize,
    guard: Option<usize>,
}

/// Both maps account their complete headers and entry capacities. Coordinates
/// belong to the Builder's existing term table; no payload vocabulary is copied.
struct Groups {
    lanes: CoordinateMap<[usize; 2], Option<usize>>,
    fibers: CoordinateMap<usize, Fiber>,
}

impl Groups {
    fn new(
        builder: &mut Builder<'_, '_, '_>,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        Ok(Self {
            lanes: CoordinateMap::new(
                builder.computation,
                builder.limits,
                builder.counters,
                location,
            )?,
            fibers: CoordinateMap::new(
                builder.computation,
                builder.limits,
                builder.counters,
                location,
            )?,
        })
    }

    fn identity(
        &mut self,
        key: [usize; 2],
        builder: &mut Builder<'_, '_, '_>,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        if self
            .lanes
            .find(
                &key,
                builder.computation,
                builder.limits,
                builder.counters,
                location,
            )?
            .is_some()
        {
            return Ok(());
        }
        let previous = self.fibers.find(
            &key[0],
            builder.computation,
            builder.limits,
            builder.counters,
            location,
        )?;
        builder.work(location)?;
        let count = previous
            .map_or(0, |fiber| fiber.count)
            .checked_add(1)
            .ok_or_else(|| overflow(location))?;
        self.lanes.insert(
            key,
            None,
            None,
            Context::new(
                &*builder.computation,
                builder.limits,
                builder.counters,
                location,
            ),
        )?;
        self.fibers.insert(
            key[0],
            Fiber { count, guard: None },
            None,
            Context::new(
                &*builder.computation,
                builder.limits,
                builder.counters,
                location,
            ),
        )?;
        Ok(())
    }

    fn has_pairs(
        &self,
        builder: &mut Builder<'_, '_, '_>,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        for (_, fiber) in self.fibers.iter() {
            builder.work(location)?;
            if fiber.count >= 2 {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn witness<'source>(
        &mut self,
        key: [usize; 2],
        plan: &plan::Plan,
        binding: &Binding<'_>,
        builder: &mut Builder<'_, '_, 'source>,
        rule: &RuleIr,
        support: &Support<'source>,
    ) -> Result<(), FormulaFailure> {
        let location = rule.location;
        let mut fiber = self
            .fibers
            .find(
                &key[0],
                builder.computation,
                builder.limits,
                builder.counters,
                location,
            )?
            .ok_or_else(|| owner(location))?;
        builder.work(location)?;
        if fiber.count < 2 {
            return Ok(());
        }
        let previous = self
            .lanes
            .find(
                &key,
                builder.computation,
                builder.limits,
                builder.counters,
                location,
            )?
            .ok_or_else(|| owner(location))?;
        // Every named lane has a distinct partner in this exact shared key.
        // Singleton fibers never name atoms or acquire first-origin metadata.
        let witness = builder.body_ir(
            plan.left()
                .iter()
                .map(|&index| Ir::Temporary(&rule.body[index])),
            binding,
            location,
            support,
        )?;
        self.merge_witness(key, previous, witness, builder, location)?;
        if fiber.guard.is_none() {
            let guard = builder.body_ir(
                plan.common()
                    .iter()
                    .map(|&index| Ir::Temporary(&rule.body[index])),
                binding,
                location,
                support,
            )?;
            fiber.guard = Some(guard);
            self.fibers.insert(
                key[0],
                fiber,
                None,
                Context::new(
                    &*builder.computation,
                    builder.limits,
                    builder.counters,
                    location,
                ),
            )?;
        }
        Ok(())
    }

    /// Only a condition is combined here. The distinct binding identity was
    /// admitted in pass one; additional witnesses never add another count unit.
    fn merge_witness(
        &mut self,
        key: [usize; 2],
        previous: Option<usize>,
        witness: usize,
        builder: &mut Builder<'_, '_, '_>,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let condition = builder.or(previous.unwrap_or(FALSUM), witness, location)?;
        self.lanes.insert(
            key,
            Some(condition),
            None,
            Context::new(
                &*builder.computation,
                builder.limits,
                builder.counters,
                location,
            ),
        )?;
        Ok(())
    }

    fn emit(self, builder: &mut Builder<'_, '_, '_>, rule: &RuleIr) -> Result<(), FormulaFailure> {
        let location = rule.location;
        let mut conditions = Buffer::new(
            builder.computation,
            builder.limits,
            builder.counters,
            location,
        )?;
        let mut weights = Buffer::new(
            builder.computation,
            builder.limits,
            builder.counters,
            location,
        )?;
        let mut at = 0;
        for &(key, fiber) in self.fibers.iter() {
            builder.work(location)?;
            conditions.clear();
            while let Some(&(lane, condition)) = self.lanes.slice().get(at) {
                builder.work(location)?;
                if lane[0] != key {
                    break;
                }
                at += 1;
                if fiber.count >= 2 {
                    conditions.push(
                        condition.ok_or_else(|| owner(location))?,
                        builder.computation,
                        builder.limits,
                        builder.counters,
                        location,
                    )?;
                }
            }
            if fiber.count < 2 {
                continue;
            }
            builder.work(location)?;
            if conditions.len() != fiber.count {
                return Err(owner(location));
            }
            weights.resize(
                conditions.len(),
                1,
                builder.computation,
                builder.limits,
                builder.counters,
                location,
            )?;
            let threshold = builder.node(
                Node::Threshold(Threshold {
                    conditions: conditions.slice(),
                    weights: weights.slice(),
                    bound: 2,
                }),
                location,
            )?;
            let guarded = builder.and(
                fiber.guard.ok_or_else(|| owner(location))?,
                threshold,
                location,
            )?;
            let constraint = builder.neg(guarded, location)?;
            builder.root(constraint, rule)?;
            #[cfg(test)]
            testing::record(testing::Point::Group);
        }
        Ok(())
    }
}

fn owner(location: ProgramSite) -> FormulaFailure {
    FormulaFailure::SupportRelation {
        error: zetesis_core::relation::Failure::Owner,
        location,
    }
}

fn overflow(location: ProgramSite) -> FormulaFailure {
    FormulaFailure::SupportRelation {
        error: zetesis_core::relation::Failure::Overflow,
        location,
    }
}
