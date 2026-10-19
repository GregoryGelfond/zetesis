//! Ordered parent incidences for one completed founded body projection.
//!
//! Every retained edge comes from the original Narrower row. Its destination is
//! an included, nonabsorbed Boolean node; no candidate mask is consulted. Both
//! passes preserve original occurrence order and multiplicity. Omitted opaque
//! destinations are seeded True or Inactive and cannot consume parent events.
//! Only selected body sources are scanned. The shared adjacency builder still
//! initializes every original-node offset and charges empty rows and cursors.
//! The returned rows become authoritative only with Scratch's exact owner.

use super::super::adjacency::Construction;
use super::{Adjacency, Narrower, NodeView, State, Theory, Work};
use zetesis_cpu::Stop;

pub(super) fn prepare(
    theory: &Theory,
    index: &Narrower,
    body_nodes: &[usize],
    states: &[State],
    work: &mut Work<'_>,
) -> Result<Adjacency, Stop> {
    let edges = body_nodes.iter().flat_map(|&node| {
        // Charge an inspected source even when it has no parent occurrences.
        std::iter::once(Ok(None)).chain(index.parents[node].iter().map(move |&parent| {
            if matches!(states[parent], State::Outside) || index.absorbed[parent] {
                return Ok(None);
            }
            let boolean = matches!(
                theory
                    .view()
                    .node(parent)
                    .map_err(|_| Stop::InvalidProgram)?,
                NodeView::And(_) | NodeView::Or(_)
            );
            Ok(boolean.then_some((node, parent)))
        }))
    });
    Adjacency::try_build_in(theory.nodes().len(), edges, &mut Preparation { work })
}

/// Narrowing has a work quota and fallible scratch allocation, but no separate
/// memory ceiling. Retained capacities are reported by Scratch; construction's
/// additional row cursors are dropped before publication, including on failure.
struct Preparation<'w, 'q> {
    work: &'w mut Work<'q>,
}

impl Construction for Preparation<'_, '_> {
    fn tick(&mut self) -> Result<(), Stop> {
        self.work.tick()
    }

    fn zeros(&mut self, count: usize) -> Result<Vec<usize>, Stop> {
        let mut values = self.reserve(count)?;
        for _ in 0..count {
            self.work.tick()?;
            values.push(0);
        }
        Ok(values)
    }

    fn reserve(&mut self, count: usize) -> Result<Vec<usize>, Stop> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| Stop::Allocation)?;
        Ok(values)
    }

    fn release(&mut self, values: Vec<usize>) -> Result<(), Stop> {
        drop(values);
        Ok(())
    }
}
