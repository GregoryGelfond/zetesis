//! Dense coordinates of ground source heads in one immutable preparation.
//!
//! A coordinate is a property of the entire ordered ground tuple and its
//! exact layout, never candidate truth. The owning `PreparedQueries` keeps
//! both source and layouts alive. Each candidate still visits the source,
//! checks its guards and marks the derived position in its own workspace.

use std::mem::size_of;

use zetesis_core::{DomainPositions, PatternRef, Program, RuleRef, TemplateTerm};

use super::{Layouts, Stop, Work, reserve};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Coordinate {
    slot: usize,
    position: usize,
}

/// One optional coordinate per template and per original table-fact occurrence. Nonground or tree heads keep the
/// ordinary key-to-position operation. Retained storage is linear in source
/// templates; ranking work is the sum of the ground arguments' bound lookups.
#[derive(Default)]
pub(in crate::oracle) struct Heads {
    templates: Vec<Option<Coordinate>>,
    facts: Vec<Option<Coordinate>>,
}

impl Heads {
    /// Rank each ground head on its predicate's dense layout, by identity when
    /// preparation holds the domain position table. `base` is the live
    /// storage already admitted, the table's included while it is read.
    pub(super) fn prepare(
        program: &Program,
        layouts: &Layouts,
        positions: Option<&DomainPositions<'_>>,
        base: u128,
        work: &mut Work<'_>,
    ) -> Result<Self, Stop> {
        let mut result = Self::default();
        let mut live = base;
        reserve(
            &mut result.templates,
            program.templates().len(),
            &mut live,
            work,
        )?;
        reserve(&mut result.facts, program.facts().len(), &mut live, work)?;
        for rule in program.rules() {
            work.tick()?;
            match rule {
                RuleRef::Template { template, .. } => {
                    result
                        .templates
                        .push(coordinate(template.head(), layouts, positions, work)?);
                }
                RuleRef::Fact(fact) => {
                    result
                        .facts
                        .push(coordinate(Some(fact.pattern()), layouts, positions, work)?);
                }
            }
        }
        Ok(result)
    }

    pub(super) fn bytes(&self) -> u128 {
        (self.templates.capacity() + self.facts.capacity()) as u128
            * size_of::<Option<Coordinate>>() as u128
    }

    /// Each template's coordinate as its slot and position, for comparison.
    #[cfg(test)]
    pub(super) fn coordinates(&self) -> Vec<Option<(usize, usize)>> {
        self.templates
            .iter()
            .map(|coordinate| coordinate.map(|Coordinate { slot, position }| (slot, position)))
            .collect()
    }

    /// A prepared position belongs only to the template's own dense slot.
    /// Missing coordinates retain ordinary key evaluation, including a
    /// ground tuple absent from an empty inferred layout.
    pub(in crate::oracle) fn position(
        &self,
        template: usize,
        slot: usize,
    ) -> Result<Option<usize>, Stop> {
        position(&self.templates, template, slot)
    }

    pub(in crate::oracle) fn fact_position(
        &self,
        fact: usize,
        slot: usize,
    ) -> Result<Option<usize>, Stop> {
        position(&self.facts, fact, slot)
    }
}

fn position(
    coordinates: &[Option<Coordinate>],
    index: usize,
    slot: usize,
) -> Result<Option<usize>, Stop> {
    coordinates
        .get(index)
        .copied()
        .flatten()
        .map(|coordinate| {
            if coordinate.slot == slot {
                Ok(coordinate.position)
            } else {
                Err(Stop::InvalidProgram)
            }
        })
        .transpose()
}

fn coordinate(
    head: Option<PatternRef<'_>>,
    layouts: &Layouts,
    positions: Option<&DomainPositions<'_>>,
    work: &mut Work<'_>,
) -> Result<Option<Coordinate>, Stop> {
    let Some(head) = head else {
        return Ok(None);
    };
    work.charge(head.terms().len())?;
    let Some(slot) = layouts.slot(head.predicate()) else {
        return Ok(None);
    };
    if !head
        .terms()
        .iter()
        .all(|term| matches!(term, TemplateTerm::Constant(_)))
    {
        return Ok(None);
    }
    let layout = layouts.get(head.predicate()).ok_or(Stop::InvalidProgram)?;
    let values = head.terms().iter().filter_map(|term| match term {
        TemplateTerm::Constant(value) => Some(value),
        TemplateTerm::Variable(_) => None,
    });
    let position = match positions {
        Some(positions) => layout.position_by_identity(values, positions, work)?,
        None => layout.position_of(values, work)?,
    };
    Ok(position.map(|position| Coordinate { slot, position }))
}
