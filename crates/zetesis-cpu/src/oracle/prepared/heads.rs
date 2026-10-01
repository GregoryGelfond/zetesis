//! Dense coordinates of ground source heads in one immutable preparation.
//!
//! A coordinate is a property of the entire ordered ground tuple and its
//! exact layout, never candidate truth. The owning `PreparedQueries` keeps
//! both source and layouts alive. Each candidate still visits the source,
//! checks its guards and marks the derived position in its own workspace.

use std::mem::size_of;

use zetesis_core::{Program, TemplateTerm};

use super::{Layouts, Stop, Work, reserve};

#[derive(Clone, Copy)]
struct Coordinate {
    slot: usize,
    position: usize,
}

/// One optional coordinate per template. Nonground or tree heads keep the
/// ordinary key-to-position operation. Retained storage is linear in source
/// templates; ranking work is the sum of the ground arguments' bound lookups.
#[derive(Default)]
pub(in crate::oracle) struct Heads(Vec<Option<Coordinate>>);

impl Heads {
    pub(super) fn prepare(
        program: &Program,
        layouts: &Layouts,
        base: u128,
        work: &mut Work<'_>,
    ) -> Result<Self, Stop> {
        let mut coordinates = Vec::new();
        let mut live = base;
        reserve(&mut coordinates, program.templates().len(), &mut live, work)?;
        for template in program.templates() {
            work.tick()?;
            let coordinate = if let Some(head) = template.head() {
                work.charge(head.terms().len())?;
                match layouts.slot(head.predicate()) {
                    Some(slot)
                        if head
                            .terms()
                            .iter()
                            .all(|term| matches!(term, TemplateTerm::Constant(_))) =>
                    {
                        let layout = layouts.get(head.predicate()).ok_or(Stop::InvalidProgram)?;
                        let values = head.terms().iter().filter_map(|term| match term {
                            TemplateTerm::Constant(value) => Some(value),
                            TemplateTerm::Variable(_) => None,
                        });
                        layout
                            .position_of(values, work)?
                            .map(|position| Coordinate { slot, position })
                    }
                    _ => None,
                }
            } else {
                None
            };
            coordinates.push(coordinate);
        }
        Ok(Self(coordinates))
    }

    pub(super) fn bytes(&self) -> u128 {
        self.0.capacity() as u128 * size_of::<Option<Coordinate>>() as u128
    }

    /// A prepared position belongs only to the template's own dense slot.
    /// Missing coordinates retain ordinary key evaluation, including a
    /// ground tuple absent from an empty inferred layout.
    pub(in crate::oracle) fn position(
        &self,
        template: usize,
        slot: usize,
    ) -> Result<Option<usize>, Stop> {
        self.0
            .get(template)
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
}
