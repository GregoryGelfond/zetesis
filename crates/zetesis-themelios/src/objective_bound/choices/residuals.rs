//! Compile immutable allocated minima into the lower objective once.
//!
//! The proof's member/key lists are temporary. Retain only activation floors
//! and nonzero residual contributions; unchanged priorities borrow the exact
//! objective. A floor condition addresses the compact activation owner, whereas
//! residual eligibility keeps its original objective-plan coordinate.

use std::ops::Range;

use zetesis_ferraris::{AggregateElement, FormulaParts};

use super::{Costs, Group, Kind, ObjectiveBoundError, ObjectivePlan, Storage, Work};

#[derive(Debug)]
pub(super) struct Level {
    pub priority: i32,
    pub groups: Range<usize>,
    pub residuals: Vec<AggregateElement>,
}

pub(super) fn prepare(
    plan: &ObjectivePlan,
    allocations: &[Group],
    activations: FormulaParts,
    memory: &mut Storage,
    work: &mut Work<'_>,
) -> Result<Costs, ObjectiveBoundError> {
    let mut groups = memory.reserve(allocations.len(), work)?;
    let mut levels = memory.reserve(allocations.len().min(plan.levels.len()), work)?;
    for (&priority, original) in &plan.levels {
        work.tick()?;
        let start = groups.len();
        let mut matching = allocations
            .iter()
            .filter(|group| group.priority == priority)
            .peekable();
        // Charge the inspected allocation list before deciding whether this
        // priority needs any retained representation beyond the exact plan.
        work.charge(u64::try_from(allocations.len()).map_err(|_| work.error(Kind::Overflow))?)?;
        if matching.peek().is_none() {
            continue;
        }
        let mut prepaid = memory.reserve(original.len(), work)?;
        for _ in original {
            work.tick()?;
            prepaid.push(0);
        }
        for group in matching {
            work.tick()?;
            for &key in &group.keys {
                work.tick()?;
                prepaid[key] = group.weight;
            }
            groups.push(AggregateElement {
                weight: group.weight,
                condition: group.activation,
            });
        }
        // Reuse the temporary minima for residual weights, then reserve only
        // the nonzero contributions that the compiled lower objective needs.
        let mut count = 0;
        for (element, weight) in original.iter().zip(&mut prepaid) {
            work.tick()?;
            *weight = element
                .weight
                .checked_sub(*weight)
                .ok_or_else(|| work.error(Kind::Overflow))?;
            count += usize::from(*weight != 0);
        }
        let mut residuals = memory.reserve(count, work)?;
        for (element, &weight) in original.iter().zip(&prepaid) {
            work.tick()?;
            if weight != 0 {
                residuals.push(AggregateElement {
                    weight,
                    condition: element.condition,
                });
            }
        }
        levels.push(Level {
            priority,
            groups: start..groups.len(),
            residuals,
        });
        memory.retire(&prepaid);
    }
    Ok(Costs {
        groups,
        levels,
        activations,
    })
}
