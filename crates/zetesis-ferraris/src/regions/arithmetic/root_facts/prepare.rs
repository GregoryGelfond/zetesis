//! Selected positive premises transposed into the shared activation kernel.
//!
//! Atom vertices precede one vertex per selected root. Premise occurrences
//! point to that rule; its one outgoing edge points to the original head.
//! Thus a productive atom activation identifies its original witness without
//! storing source identities on generic activation edges.

use crate::{
    Theory,
    positive::activation::{self, Edge, Effects, Point, State},
};
use zetesis_cpu::Stop;

use super::super::{
    budget::{Budget, BudgetError},
    storage::Storage,
};
use super::{Step, reader};

struct Rule {
    root: usize,
    head: usize,
    first: usize,
    count: usize,
}

struct Selected {
    rules: Vec<Rule>,
    premises: Vec<usize>,
}

struct Graph {
    offsets: Vec<usize>,
    edges: Vec<Edge>,
    states: Vec<State>,
    queue: Vec<usize>,
    words: Vec<u64>,
}

pub(super) fn propose(
    theory: &Theory,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_>,
) -> Result<Vec<Step>, BudgetError> {
    let selected = select(theory, storage, budget)?;
    let mut steps = storage.reserve(0, budget)?;
    budget.charge(1)?;
    if !selected.rules.is_empty() {
        let mut graph = transpose(theory.atom_count(), &selected, storage, budget)?;
        {
            let mut effects = Recording {
                atoms: theory.atom_count(),
                rules: &selected.rules,
                steps: &mut steps,
                storage,
                budget,
            };
            for (index, rule) in selected.rules.iter().enumerate() {
                effects.budget.charge(2)?;
                if rule.count == 0 {
                    effects.budget.charge(1)?;
                    activation::activate(
                        theory.atom_count() + index,
                        None,
                        &mut graph.states,
                        &mut graph.queue,
                        &mut effects,
                    )?;
                }
            }
            activation::drain(
                theory.atom_count(),
                &graph.offsets,
                &graph.edges,
                &mut graph.states,
                &mut graph.queue,
                &mut graph.words,
                &mut effects,
            )?;
        }
        graph.release(storage)?;
    }
    storage.release(selected.rules)?;
    storage.release(selected.premises)?;
    budget.charge(1)?;
    Ok(steps)
}

fn select(
    theory: &Theory,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_>,
) -> Result<Selected, BudgetError> {
    let mut rules = storage.reserve(0, budget)?;
    let mut premises = storage.reserve(0, budget)?;
    for root in 0..theory.roots().len() {
        budget.charge(1)?;
        let Some(rule) = reader::rule(theory, root, budget)? else {
            continue;
        };
        budget.charge(1)?;
        let first = premises.len();
        let accepted = reader::premises(
            theory,
            rule.body,
            storage,
            budget,
            |atom, storage, budget| {
                storage.push(&mut premises, atom, 1, budget)?;
                budget.charge(1)?;
                Ok(true)
            },
        )?;
        budget.charge(1)?;
        if accepted {
            budget.charge(5)?; // subtraction and retained rule fields
            let count = premises.len() - first;
            storage.push(
                &mut rules,
                Rule {
                    root,
                    head: rule.head,
                    first,
                    count,
                },
                4,
                budget,
            )?;
        } else {
            budget.charge(1)?; // discard this unsupported source's scalar suffix
            premises.truncate(first);
        }
    }
    budget.charge(1)?;
    Ok(Selected { rules, premises })
}

fn transpose(
    atoms: usize,
    selected: &Selected,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_>,
) -> Result<Graph, BudgetError> {
    budget.charge(3)?;
    let vertices = atoms
        .checked_add(selected.rules.len())
        .ok_or(BudgetError::Stopped(Stop::Allocation))?;
    let offset_count = vertices
        .checked_add(1)
        .ok_or(BudgetError::Stopped(Stop::Allocation))?;
    let edge_count = selected
        .premises
        .len()
        .checked_add(selected.rules.len())
        .ok_or(BudgetError::Stopped(Stop::Allocation))?;
    let mut offsets = storage.filled(offset_count, 0_usize, 1, budget)?;
    incidences(atoms, selected, budget, |source, _, budget| {
        budget.charge(3)?;
        offsets[source + 1] = offsets[source + 1]
            .checked_add(1)
            .ok_or(BudgetError::Stopped(Stop::Allocation))?;
        Ok(())
    })?;
    for index in 0..vertices {
        budget.charge(4)?;
        offsets[index + 1] = offsets[index + 1]
            .checked_add(offsets[index])
            .ok_or(BudgetError::Stopped(Stop::Allocation))?;
    }
    let mut edges = storage.filled(
        edge_count,
        Edge {
            target: 0,
            weight: 1,
        },
        2,
        budget,
    )?;
    let mut cursors = storage.reserve(vertices, budget)?;
    for &offset in &offsets[..vertices] {
        budget.charge(1)?;
        storage.push(&mut cursors, offset, 1, budget)?;
    }
    incidences(atoms, selected, budget, |source, edge, budget| {
        budget.charge(4)?; // cursor, edge fields and increment (including its store)
        edges[cursors[source]] = edge;
        cursors[source] += 1;
        Ok(())
    })?;
    storage.release(cursors)?;
    let mut states = storage.filled(vertices, State::Awaiting(1), 2, budget)?;
    for (index, rule) in selected.rules.iter().enumerate() {
        budget.charge(1)?;
        states[atoms + index] = if rule.count == 0 {
            budget.charge(1)?; // state tag
            State::Inactive
        } else {
            budget.charge(2)?; // state tag and remaining count
            State::Awaiting(rule.count)
        };
    }
    let queue = storage.reserve(vertices, budget)?;
    let words = storage.filled(atoms.div_ceil(64), 0_u64, 1, budget)?;
    budget.charge(1)?;
    Ok(Graph {
        offsets,
        edges,
        states,
        queue,
        words,
    })
}

/// Each call visits exactly the retained occurrence edges and one head edge
/// per selected source. Both CSR passes use this one incidence definition.
fn incidences(
    atoms: usize,
    selected: &Selected,
    budget: &mut Budget<'_, '_>,
    mut visit: impl FnMut(usize, Edge, &mut Budget<'_, '_>) -> Result<(), BudgetError>,
) -> Result<(), BudgetError> {
    for (index, rule) in selected.rules.iter().enumerate() {
        budget.charge(4)?; // rule occurrence, rule vertex and span boundaries
        let target = atoms + index;
        for &atom in &selected.premises[rule.first..rule.first + rule.count] {
            budget.charge(3)?; // source coordinate and edge fields
            visit(atom, Edge { target, weight: 1 }, budget)?;
        }
        budget.charge(2)?;
        visit(
            target,
            Edge {
                target: rule.head,
                weight: 1,
            },
            budget,
        )?;
    }
    Ok(())
}

impl Graph {
    fn release(self, storage: &mut Storage) -> Result<(), BudgetError> {
        storage.release(self.offsets)?;
        storage.release(self.edges)?;
        storage.release(self.states)?;
        storage.release(self.queue)?;
        storage.release(self.words)
    }
}

struct Recording<'r, 's, 'b, 'a, 'q> {
    atoms: usize,
    rules: &'r [Rule],
    steps: &'s mut Vec<Step>,
    storage: &'b mut Storage,
    budget: &'b mut Budget<'a, 'q>,
}

impl Effects for Recording<'_, '_, '_, '_, '_> {
    type Error = BudgetError;

    fn before(&mut self, _point: Point, work: u64) -> Result<(), Self::Error> {
        self.budget.charge(work)
    }

    fn activated(&mut self, vertex: usize, source: Option<usize>) -> Result<(), Self::Error> {
        self.budget.charge(1)?;
        if vertex >= self.atoms {
            return Ok(());
        }
        self.budget.charge(3)?; // source tag, atom/rule offset and rule lookup
        let rule = source
            .and_then(|source| source.checked_sub(self.atoms))
            .and_then(|index| self.rules.get(index))
            .ok_or(BudgetError::Stopped(Stop::InvalidProgram))?;
        self.budget.charge(1)?;
        if rule.head != vertex {
            return Err(BudgetError::Stopped(Stop::InvalidProgram));
        }
        self.budget.charge(2)?;
        self.storage.push(
            self.steps,
            Step {
                root: rule.root,
                atom: vertex,
            },
            2,
            self.budget,
        )
    }
}
