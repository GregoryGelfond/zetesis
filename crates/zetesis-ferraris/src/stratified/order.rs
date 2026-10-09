//! Iterative dependency components; negative edges require strict precedence.

use super::{Budget, StratifiedError, compile::Graph};
use zetesis_cpu::Stop;

/// One explicit DFS frame; child progress is retained before descending.
#[derive(Clone, Copy)]
pub(super) struct Frame {
    vertex: usize,
    next: usize,
}

/// Vertices grouped by a topological ordering of strongly connected components.
pub(super) struct Schedule {
    pub vertices: Vec<usize>,
    pub offsets: Vec<usize>,
    pub component: Vec<usize>,
}

impl Schedule {
    pub fn release(self, budget: &mut Budget<'_>) -> Result<(), StratifiedError> {
        budget.release(self.vertices)?;
        budget.release(self.offsets)?;
        budget.release(self.component)
    }
}

pub(super) fn compile(
    graph: &Graph,
    atoms: usize,
    budget: &mut Budget<'_>,
) -> Result<Schedule, StratifiedError> {
    let finished = finishing_order(graph, budget)?;
    let schedule = components(graph, &finished, budget)?;
    budget.release(finished)?;
    for source in 0..graph.vertices() {
        budget.tick()?;
        for edge in graph.outgoing(source) {
            budget.tick()?;
            let earlier = schedule.component[source];
            let later = schedule.component[edge.target];
            if earlier > later {
                return Err(Stop::InvalidProgram.into());
            }
            if edge.negative && earlier == later {
                return Err(StratifiedError::NegativeCycle {
                    atom: source,
                    body: edge.target - atoms,
                });
            }
        }
    }
    Ok(schedule)
}

fn finishing_order(graph: &Graph, budget: &mut Budget<'_>) -> Result<Vec<usize>, StratifiedError> {
    let mut seen = budget.filled(graph.vertices(), false)?;
    let mut stack: Vec<Frame> = budget.reserve(graph.vertices())?;
    let mut finished = budget.reserve(graph.vertices())?;
    for start in 0..graph.vertices() {
        budget.tick()?;
        if seen[start] {
            continue;
        }
        seen[start] = true;
        stack.push(Frame {
            vertex: start,
            next: graph.offsets[start],
        });
        while let Some(frame) = stack.last().copied() {
            budget.tick()?;
            if frame.next == graph.offsets[frame.vertex + 1] {
                stack.pop();
                finished.push(frame.vertex);
                continue;
            }
            let child = graph.edges[frame.next].target;
            // The frame exists and advances before the child's frame is pushed.
            stack.last_mut().ok_or(Stop::InvalidProgram)?.next += 1;
            if !seen[child] {
                seen[child] = true;
                stack.push(Frame {
                    vertex: child,
                    next: graph.offsets[child],
                });
            }
        }
    }
    budget.release(seen)?;
    budget.release(stack)?;
    Ok(finished)
}

fn components(
    graph: &Graph,
    finished: &[usize],
    budget: &mut Budget<'_>,
) -> Result<Schedule, StratifiedError> {
    let mut assigned = budget.filled(graph.vertices(), false)?;
    // A component coordinate is read only after its vertex has been assigned.
    let mut component = budget.filled(graph.vertices(), 0_usize)?;
    let mut vertices = budget.reserve(graph.vertices())?;
    let capacity = graph
        .vertices()
        .checked_add(1)
        .ok_or(StratifiedError::Overflow)?;
    let mut offsets = budget.reserve(capacity)?;
    offsets.push(0);
    for &start in finished.iter().rev() {
        budget.tick()?;
        if assigned[start] {
            continue;
        }
        let id = offsets.len() - 1;
        let mut next = vertices.len();
        assigned[start] = true;
        component[start] = id;
        vertices.push(start);
        while let Some(&vertex) = vertices.get(next) {
            budget.tick()?;
            next += 1;
            for &source in graph.incoming(vertex) {
                budget.tick()?;
                if !assigned[source] {
                    assigned[source] = true;
                    component[source] = id;
                    vertices.push(source);
                }
            }
        }
        offsets.push(vertices.len());
    }
    budget.release(assigned)?;
    Ok(Schedule {
        vertices,
        offsets,
        component,
    })
}
