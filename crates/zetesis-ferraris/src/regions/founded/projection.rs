//! Ordered incidences for one completed founded body projection.
//!
//! Parent destinations are included, nonabsorbed Boolean nodes. Head rows flatten
//! original producer/head rows, retaining only exact cyclic heads. One additional
//! row, after all original node rows, contains selected facts' cyclic heads. Both
//! projections preserve occurrence order and multiplicity and consult no region
//! mask. Omitted heads were already ignored by atom activation; noncyclic atoms
//! remain conceptual seeds. Original producers remain available to every consumer.
//!
//! Incidence indexes and immutable initialization roles publish together under
//! Scratch's exact owner. Indexes use shared adjacency construction, charging
//! original reads, offsets and temporary cursors. A later construction failure
//! drops every completed local view. Current region masks are applied at runtime.

use super::super::adjacency::Construction;
use super::{
    Adjacency, Initial, Narrower, NodeView, Producers, Scratch, State, Theory, Work, bit,
    initialization,
};
use zetesis_cpu::Stop;

#[derive(Debug)]
pub(super) struct Projection {
    pub(super) parents: Adjacency,
    pub(super) heads: Adjacency,
    pub(super) initial: Vec<Initial>,
}

impl Projection {
    pub(super) fn prepare(
        theory: &Theory,
        index: &Narrower,
        producers: &Producers,
        scratch: &Scratch,
        work: &mut Work<'_>,
    ) -> Result<Self, Stop> {
        let parents = prepare_parents(theory, index, scratch, work)?;
        let heads = prepare_heads(theory, producers, scratch, work)?;
        let initial = initialization::prepare(
            theory,
            index,
            &scratch.body_nodes,
            &scratch.cyclic_atoms,
            work,
        )?;
        Ok(Self {
            parents,
            heads,
            initial,
        })
    }

    /// Actual retained capacities; the enclosing Scratch counts every vector header.
    pub(super) fn payload_bytes(&self) -> u128 {
        self.parents.retained_bytes() - std::mem::size_of::<Adjacency>() as u128
            + self.heads.retained_bytes()
            - std::mem::size_of::<Adjacency>() as u128
            + self.initial.capacity() as u128 * std::mem::size_of::<Initial>() as u128
    }

    pub(super) fn fact_heads(&self) -> &[usize] {
        &self.heads[self.heads.len() - 1]
    }
}

fn prepare_parents(
    theory: &Theory,
    index: &Narrower,
    scratch: &Scratch,
    work: &mut Work<'_>,
) -> Result<Adjacency, Stop> {
    let edges = scratch.body_nodes.iter().flat_map(|&node| {
        // Charge an inspected source even when it has no parent occurrences.
        std::iter::once(Ok(None)).chain(index.parents[node].iter().map(move |&parent| {
            if matches!(scratch.states[parent], State::Outside) || index.absorbed[parent] {
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

fn prepare_heads(
    theory: &Theory,
    producers: &Producers,
    scratch: &Scratch,
    work: &mut Work<'_>,
) -> Result<Adjacency, Stop> {
    let fact_row = theory.nodes().len();
    let rows = fact_row.checked_add(1).ok_or(Stop::Allocation)?;
    let bodies = scratch.body_nodes.iter().flat_map(|&node| {
        std::iter::once(Ok(None)).chain(producers.by_body[node].iter().flat_map(move |&producer| {
            head_events(
                node,
                &producers.rules[producer].heads,
                &scratch.cyclic_atoms,
            )
        }))
    });
    let facts = scratch.fact_rows.iter().flat_map(|&producer| {
        head_events(
            fact_row,
            &producers.rules[producer].heads,
            &scratch.cyclic_atoms,
        )
    });
    Adjacency::try_build_in(rows, bodies.chain(facts), &mut Preparation { work })
}

/// The initial event charges the producer read, including empty head rows.
/// Every head occurrence, including an omitted one, has its own charged event.
fn head_events(
    row: usize,
    heads: &[usize],
    cyclic_atoms: &[u64],
) -> impl Iterator<Item = Result<Option<(usize, usize)>, Stop>> + Clone {
    std::iter::once(Ok(None)).chain(
        heads
            .iter()
            .map(move |&head| Ok(bit(cyclic_atoms, head).then_some((row, head)))),
    )
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
