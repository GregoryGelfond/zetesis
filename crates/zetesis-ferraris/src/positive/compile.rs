//! Complete original-root classification and checked forward incidence.

use std::mem::size_of;

use zetesis_cpu::Stop;

use super::{Budget, PositiveError, PositivePlan, propagate, validation};
use crate::{Interpretation, NodeView, Theory};

#[derive(Clone, Copy)]
enum Kind {
    Unsupported,
    False,
    True,
    Atom,
    And(usize),
    Or,
}

impl Kind {
    fn monotone(self) -> bool {
        !matches!(self, Self::Unsupported)
    }
    fn waiting(self) -> State {
        match self {
            Self::Atom | Self::Or => State::Awaiting(1),
            Self::And(arity) => State::Awaiting(arity),
            Self::Unsupported | Self::False | Self::True => State::Inactive,
        }
    }
}

/// A vertex awaits its remaining incidences or has been queued exactly once.
/// Inactive truth constants are activated explicitly during seeding; false and
/// unsupported nodes remain inactive.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    Inactive,
    Awaiting(usize),
    True,
}

pub(super) struct Graph {
    pub offsets: Vec<usize>,
    pub targets: Vec<usize>,
    pub states: Vec<State>,
    pub queue: Vec<usize>,
}

pub(super) fn build(
    theory: &Theory,
    budget: &mut Budget<'_>,
) -> Result<(Interpretation, Option<usize>), PositiveError> {
    budget.cancellation.poll()?;
    let headers = size_of::<PositivePlan>()
        + size_of::<Graph>()
        + size_of::<Vec<Kind>>()
        + size_of::<Vec<usize>>();
    budget.observe_bytes(headers as u128)?;
    let vertices = theory
        .atom_count()
        .checked_add(theory.nodes().len())
        .ok_or(PositiveError::Overflow)?;
    let kinds = classify(theory, budget)?;
    let mut offsets = initialized(
        vertices.checked_add(1).ok_or(PositiveError::Overflow)?,
        0_usize,
        budget,
    )?;
    edges(theory, &kinds, budget, |source, _, budget| {
        budget.dependency()?;
        offsets[source + 1] = offsets[source + 1]
            .checked_add(1)
            .ok_or(PositiveError::Overflow)?;
        Ok(())
    })?;
    for index in 0..vertices {
        budget.tick()?;
        offsets[index + 1] = offsets[index + 1]
            .checked_add(offsets[index])
            .ok_or(PositiveError::Overflow)?;
    }
    let mut targets = initialized(budget.statistics.dependencies, 0_usize, budget)?;
    let mut cursors = budget.reserve(vertices)?;
    for &offset in &offsets[..vertices] {
        budget.tick()?;
        cursors.push(offset);
    }
    edges(theory, &kinds, budget, |source, target, _| {
        targets[cursors[source]] = target;
        cursors[source] += 1;
        Ok(())
    })?;
    let mut states = budget.reserve(vertices)?;
    for _ in 0..theory.atom_count() {
        budget.tick()?;
        states.push(State::Awaiting(1));
    }
    for kind in &kinds {
        budget.tick()?;
        states.push(kind.waiting());
    }
    budget.release(kinds)?;
    budget.release(cursors)?;
    let queue = budget.reserve(vertices)?;
    let mut words = initialized(theory.atom_count().div_ceil(64), 0_u64, budget)?;
    let mut graph = Graph {
        offsets,
        targets,
        states,
        queue,
    };
    propagate::complete(theory, &mut graph, &mut words, budget)?;
    drop(graph);
    let least = Interpretation {
        theory: theory.clone(),
        words,
    };
    let failed = validation::complete(&least, budget)?;
    Ok((least, failed))
}

fn initialized<T: Copy>(
    count: usize,
    value: T,
    budget: &mut Budget<'_>,
) -> Result<Vec<T>, PositiveError> {
    let mut values = budget.reserve(count)?;
    for _ in 0..count {
        budget.tick()?;
        values.push(value);
    }
    Ok(values)
}

fn classify(theory: &Theory, budget: &mut Budget<'_>) -> Result<Vec<Kind>, PositiveError> {
    let mut kinds: Vec<Kind> = budget.reserve(theory.nodes().len())?;
    let view = theory.view();
    for index in 0..view.len() {
        budget.tick()?;
        kinds.push(match view.node(index).map_err(|_| Stop::InvalidProgram)? {
            NodeView::Atom(_) => Kind::Atom,
            NodeView::False => Kind::False,
            NodeView::And(operands) => {
                if monotone_operands(operands, &kinds, budget)? {
                    Kind::And(operands.len())
                } else {
                    Kind::Unsupported
                }
            }
            NodeView::Or(operands) => {
                if monotone_operands(operands, &kinds, budget)? {
                    Kind::Or
                } else {
                    Kind::Unsupported
                }
            }
            NodeView::Implies(a, b) => {
                budget.tick()?;
                let left = view.node(a).map_err(|_| Stop::InvalidProgram)?;
                budget.tick()?;
                let right = view.node(b).map_err(|_| Stop::InvalidProgram)?;
                if left == NodeView::False && right == NodeView::False {
                    Kind::True
                } else {
                    Kind::Unsupported
                }
            }
        });
    }
    Ok(kinds)
}

fn monotone_operands(
    operands: &[usize],
    kinds: &[Kind],
    budget: &mut Budget<'_>,
) -> Result<bool, PositiveError> {
    let mut monotone = true;
    for &child in operands {
        budget.tick()?;
        monotone &= kinds[child].monotone();
    }
    Ok(monotone)
}

/// The same finite edge stream is counted then written into reserved CSR slots.
/// Original roots are checked on both passes against the same immutable theory.
/// Unsupported unreachable nodes do not supply edges; every accepted body has
/// a complete monotone classification. No formula or atom occurrence is merged.
fn edges(
    theory: &Theory,
    kinds: &[Kind],
    budget: &mut Budget<'_>,
    mut visit: impl FnMut(usize, usize, &mut Budget<'_>) -> Result<(), PositiveError>,
) -> Result<(), PositiveError> {
    let atoms = theory.atom_count();
    for (index, &kind) in kinds.iter().enumerate() {
        budget.tick()?;
        match (
            kind,
            theory
                .view()
                .node(index)
                .map_err(|_| Stop::InvalidProgram)?,
        ) {
            (Kind::Atom, NodeView::Atom(atom)) => {
                budget.tick()?;
                visit(atom, atoms + index, budget)?;
            }
            (Kind::And(_), NodeView::And(operands)) | (Kind::Or, NodeView::Or(operands)) => {
                for &child in operands {
                    budget.tick()?;
                    visit(atoms + child, atoms + index, budget)?;
                }
            }
            _ => {}
        }
    }
    for &root in theory.roots() {
        budget.tick()?;
        if let Some((body, head)) = producer(theory, root, kinds)? {
            budget.tick()?;
            visit(atoms + body, head, budget)?;
        }
    }
    Ok(())
}

fn producer(
    theory: &Theory,
    root: usize,
    kinds: &[Kind],
) -> Result<Option<(usize, usize)>, PositiveError> {
    match theory.view().node(root).map_err(|_| Stop::InvalidProgram)? {
        NodeView::Atom(_) | NodeView::False => Ok(None),
        NodeView::Implies(_, head)
            if theory.view().node(head).map_err(|_| Stop::InvalidProgram)? == NodeView::False =>
        {
            Ok(None)
        }
        NodeView::Implies(body, head) => {
            let NodeView::Atom(atom) =
                theory.view().node(head).map_err(|_| Stop::InvalidProgram)?
            else {
                return Err(PositiveError::UnsupportedRoot { root });
            };
            if !kinds[body].monotone() {
                return Err(PositiveError::UnsupportedBody { root, body });
            }
            Ok(Some((body, atom)))
        }
        _ => Err(PositiveError::UnsupportedRoot { root }),
    }
}
