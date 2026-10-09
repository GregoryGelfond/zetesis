//! Whole-root grammar and paired dependency rows over the original node IDs.

use std::mem::size_of;

use super::{Budget, StratifiedError, StratifiedPlan, evaluate, order};
use crate::{Interpretation, NodeView, Theory};
use zetesis_cpu::Stop;

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Unsupported,
    False,
    True,
    Atom,
    Negative(usize),
    And(usize),
}

impl Kind {
    fn normal(self) -> bool {
        !matches!(self, Self::Unsupported)
    }
}

/// Positive incidences carry truth; negative incidences only order components.
#[derive(Clone, Copy)]
pub(super) struct Edge {
    pub target: usize,
    pub negative: bool,
}

pub(super) struct Graph {
    pub offsets: Vec<usize>,
    pub edges: Vec<Edge>,
    pub reverse_offsets: Vec<usize>,
    pub sources: Vec<usize>,
}

impl Graph {
    pub fn vertices(&self) -> usize {
        self.offsets.len() - 1
    }

    pub fn outgoing(&self, vertex: usize) -> &[Edge] {
        &self.edges[self.offsets[vertex]..self.offsets[vertex + 1]]
    }

    pub fn incoming(&self, vertex: usize) -> &[usize] {
        &self.sources[self.reverse_offsets[vertex]..self.reverse_offsets[vertex + 1]]
    }

    pub fn release(self, budget: &mut Budget<'_>) -> Result<(), StratifiedError> {
        budget.release(self.offsets)?;
        budget.release(self.edges)?;
        budget.release(self.reverse_offsets)?;
        budget.release(self.sources)
    }
}

pub(super) fn build(
    theory: &Theory,
    budget: &mut Budget<'_>,
) -> Result<(Interpretation, Option<usize>), StratifiedError> {
    budget.cancellation.poll()?;
    // Reserve all concurrently held vector headers, including temporary graph
    // cursors and component DFS storage. Capacity is admitted separately.
    let headers = size_of::<StratifiedPlan>()
        + size_of::<Graph>()
        + size_of::<order::Schedule>()
        + size_of::<Vec<Kind>>()
        + size_of::<Vec<usize>>() * 4
        + size_of::<Vec<order::Frame>>()
        + size_of::<Vec<bool>>()
        + size_of::<Vec<evaluate::State>>();
    budget.observe_bytes(headers as u128)?;
    let kinds = classify(theory, budget)?;
    let graph = graph(theory, &kinds, budget)?;
    let schedule = order::compile(&graph, theory.atom_count(), budget)?;
    let words = evaluate::consequences(theory, &kinds, &graph, &schedule, budget)?;
    graph.release(budget)?;
    schedule.release(budget)?;
    budget.release(kinds)?;
    let consequences = Interpretation {
        theory: theory.clone(),
        words,
    };
    let failed_constraint = evaluate::validate(&consequences, budget)?;
    Ok((consequences, failed_constraint))
}

fn classify(theory: &Theory, budget: &mut Budget<'_>) -> Result<Vec<Kind>, StratifiedError> {
    let mut kinds: Vec<Kind> = budget.reserve(theory.nodes().len())?;
    let view = theory.view();
    for index in 0..view.len() {
        budget.tick()?;
        let kind = match view.node(index).map_err(|_| Stop::InvalidProgram)? {
            NodeView::Atom(_) => Kind::Atom,
            NodeView::False => Kind::False,
            NodeView::And(operands) => {
                let mut normal = true;
                for &child in operands {
                    budget.tick()?;
                    normal &= kinds[child].normal();
                }
                if normal {
                    Kind::And(operands.len())
                } else {
                    Kind::Unsupported
                }
            }
            NodeView::Implies(left, right) => {
                budget.tick()?;
                let consequent = view.node(right).map_err(|_| Stop::InvalidProgram)?;
                budget.tick()?;
                match (
                    view.node(left).map_err(|_| Stop::InvalidProgram)?,
                    consequent,
                ) {
                    (NodeView::False, NodeView::False) => Kind::True,
                    (NodeView::Atom(atom), NodeView::False) => Kind::Negative(atom),
                    _ => Kind::Unsupported,
                }
            }
            NodeView::Or(_) => Kind::Unsupported,
        };
        kinds.push(kind);
    }
    Ok(kinds)
}

fn graph(
    theory: &Theory,
    kinds: &[Kind],
    budget: &mut Budget<'_>,
) -> Result<Graph, StratifiedError> {
    let vertices = theory
        .atom_count()
        .checked_add(kinds.len())
        .ok_or(StratifiedError::Overflow)?;
    let length = vertices.checked_add(1).ok_or(StratifiedError::Overflow)?;
    let mut offsets = budget.filled(length, 0_usize)?;
    let mut reverse_offsets = budget.filled(length, 0_usize)?;
    edges(theory, kinds, budget, |source, edge, budget| {
        budget.dependency()?;
        offsets[source + 1] = offsets[source + 1]
            .checked_add(1)
            .ok_or(StratifiedError::Overflow)?;
        reverse_offsets[edge.target + 1] = reverse_offsets[edge.target + 1]
            .checked_add(1)
            .ok_or(StratifiedError::Overflow)?;
        Ok(())
    })?;
    for vertex in 0..vertices {
        budget.tick()?;
        offsets[vertex + 1] = offsets[vertex + 1]
            .checked_add(offsets[vertex])
            .ok_or(StratifiedError::Overflow)?;
        reverse_offsets[vertex + 1] = reverse_offsets[vertex + 1]
            .checked_add(reverse_offsets[vertex])
            .ok_or(StratifiedError::Overflow)?;
    }
    let mut forward = budget.filled(
        budget.statistics.dependencies,
        Edge {
            target: 0,
            negative: false,
        },
    )?;
    let mut sources = budget.filled(budget.statistics.dependencies, 0_usize)?;
    let mut next_forward = budget.reserve(vertices)?;
    let mut next_reverse = budget.reserve(vertices)?;
    for vertex in 0..vertices {
        budget.tick()?;
        next_forward.push(offsets[vertex]);
        next_reverse.push(reverse_offsets[vertex]);
    }
    edges(theory, kinds, budget, |source, edge, _| {
        forward[next_forward[source]] = edge;
        sources[next_reverse[edge.target]] = source;
        next_forward[source] += 1;
        next_reverse[edge.target] += 1;
        Ok(())
    })?;
    budget.release(next_forward)?;
    budget.release(next_reverse)?;
    Ok(Graph {
        offsets,
        edges: forward,
        reverse_offsets,
        sources,
    })
}

/// The same immutable incidence stream is counted and scattered. Every root
/// must pass, even if an earlier root or body is already false. Constraint-only
/// formulas supply no rule edge and therefore no support.
fn edges(
    theory: &Theory,
    kinds: &[Kind],
    budget: &mut Budget<'_>,
    mut visit: impl FnMut(usize, Edge, &mut Budget<'_>) -> Result<(), StratifiedError>,
) -> Result<(), StratifiedError> {
    let atoms = theory.atom_count();
    let view = theory.view();
    for (index, &kind) in kinds.iter().enumerate() {
        budget.tick()?;
        let target = atoms + index;
        match (kind, view.node(index).map_err(|_| Stop::InvalidProgram)?) {
            (Kind::Atom, NodeView::Atom(atom)) => {
                budget.tick()?;
                visit(
                    atom,
                    Edge {
                        target,
                        negative: false,
                    },
                    budget,
                )?;
            }
            (Kind::Negative(atom), _) => {
                budget.tick()?;
                visit(
                    atom,
                    Edge {
                        target,
                        negative: true,
                    },
                    budget,
                )?;
            }
            (Kind::And(_), NodeView::And(operands)) => {
                for &child in operands {
                    budget.tick()?;
                    visit(
                        atoms + child,
                        Edge {
                            target,
                            negative: false,
                        },
                        budget,
                    )?;
                }
            }
            _ => {}
        }
    }
    for &root in theory.roots() {
        budget.tick()?;
        if let Some((body, head)) = producer(theory, root, kinds, budget)? {
            budget.tick()?;
            visit(
                atoms + body,
                Edge {
                    target: head,
                    negative: false,
                },
                budget,
            )?;
        }
    }
    Ok(())
}

fn producer(
    theory: &Theory,
    root: usize,
    kinds: &[Kind],
    budget: &mut Budget<'_>,
) -> Result<Option<(usize, usize)>, StratifiedError> {
    let view = theory.view();
    match view.node(root).map_err(|_| Stop::InvalidProgram)? {
        NodeView::Atom(_) | NodeView::False => Ok(None),
        NodeView::Implies(body, head) => {
            budget.tick()?;
            match view.node(head).map_err(|_| Stop::InvalidProgram)? {
                NodeView::False => Ok(None),
                NodeView::Atom(atom) => {
                    if !kinds[body].normal() {
                        return Err(StratifiedError::UnsupportedBody { root, body });
                    }
                    Ok(Some((body, atom)))
                }
                _ => Err(StratifiedError::UnsupportedRoot { root }),
            }
        }
        NodeView::And(_) | NodeView::Or(_) => Err(StratifiedError::UnsupportedRoot { root }),
    }
}
