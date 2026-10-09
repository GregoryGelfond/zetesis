//! One truth publication per vertex, with negative reads after predecessor closure.

use std::mem::size_of;

use super::{
    Budget, StratifiedError, StratifiedPlan, StratifiedResource,
    compile::{Graph, Kind},
    order::Schedule,
};
use crate::{
    EvaluationError, EvaluationLimits, EvaluationWorkspace, Interpretation, NodeView, Theory,
};
use zetesis_cpu::Stop;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    Inactive,
    Awaiting(usize),
    True,
}

pub(super) fn consequences(
    theory: &Theory,
    kinds: &[Kind],
    graph: &Graph,
    schedule: &Schedule,
    budget: &mut Budget<'_>,
) -> Result<Vec<u64>, StratifiedError> {
    let mut states = budget.reserve(graph.vertices())?;
    for _ in 0..theory.atom_count() {
        budget.tick()?;
        states.push(State::Awaiting(1));
    }
    for &kind in kinds {
        budget.tick()?;
        states.push(match kind {
            Kind::Atom => State::Awaiting(1),
            Kind::And(arity) => State::Awaiting(arity),
            Kind::Unsupported | Kind::False | Kind::True | Kind::Negative(_) => State::Inactive,
        });
    }
    let mut words = budget.filled(theory.atom_count().div_ceil(64), 0_u64)?;
    let mut queue = budget.reserve(graph.vertices())?;
    for &root in theory.roots() {
        budget.tick()?;
        if let NodeView::Atom(atom) = theory.view().node(root).map_err(|_| Stop::InvalidProgram)? {
            publish(atom, theory.atom_count(), &mut states, &mut words, budget)?;
        }
    }
    for (component, range) in schedule.offsets.windows(2).enumerate() {
        queue.clear();
        for &vertex in &schedule.vertices[range[0]..range[1]] {
            budget.tick()?;
            if states[vertex] == State::True {
                queue.push(vertex);
            } else if vertex >= theory.atom_count() {
                let seed = match kinds[vertex - theory.atom_count()] {
                    Kind::True => true,
                    Kind::Negative(atom) => words[atom / 64] & (1 << (atom % 64)) == 0,
                    _ => false,
                };
                if seed {
                    publish(vertex, theory.atom_count(), &mut states, &mut words, budget)?;
                    queue.push(vertex);
                }
            }
        }
        propagate(
            theory.atom_count(),
            component,
            graph,
            schedule,
            &mut Progress {
                states: &mut states,
                words: &mut words,
                queue: &mut queue,
            },
            budget,
        )?;
        budget.statistics.components += 1;
    }
    budget.release(states)?;
    budget.release(queue)?;
    Ok(words)
}

struct Progress<'a> {
    states: &'a mut [State],
    words: &'a mut [u64],
    queue: &'a mut Vec<usize>,
}

fn propagate(
    atoms: usize,
    component: usize,
    graph: &Graph,
    schedule: &Schedule,
    progress: &mut Progress<'_>,
    budget: &mut Budget<'_>,
) -> Result<(), StratifiedError> {
    let mut next = 0;
    while let Some(&vertex) = progress.queue.get(next) {
        budget.tick()?;
        next += 1;
        for edge in graph.outgoing(vertex) {
            budget.tick()?;
            budget.statistics.propagated_dependencies += 1;
            if edge.negative {
                continue;
            }
            match progress.states[edge.target] {
                State::Awaiting(1) => {
                    publish(edge.target, atoms, progress.states, progress.words, budget)?;
                    // Earlier components may establish a later vertex's truth;
                    // its outgoing edges are visited only when that component starts.
                    if schedule.component[edge.target] == component {
                        progress.queue.push(edge.target);
                    }
                }
                State::Awaiting(remaining) => {
                    progress.states[edge.target] = State::Awaiting(remaining - 1);
                }
                State::Inactive | State::True => {}
            }
        }
    }
    Ok(())
}

fn publish(
    vertex: usize,
    atoms: usize,
    states: &mut [State],
    words: &mut [u64],
    budget: &mut Budget<'_>,
) -> Result<(), StratifiedError> {
    if states[vertex] != State::True {
        budget.tick()?;
        states[vertex] = State::True;
        if vertex < atoms {
            words[vertex / 64] |= 1 << (vertex % 64);
            budget.statistics.derived_atoms += 1;
        } else {
            budget.statistics.activated_nodes += 1;
        }
    }
    Ok(())
}

pub(super) fn validate(
    consequences: &Interpretation,
    budget: &mut Budget<'_>,
) -> Result<Option<usize>, StratifiedError> {
    budget.cancellation.poll()?;
    // All graph, scheduling and closure vectors have been released. Only the
    // final plan and its packed interpretation overlap exact original evaluation.
    let base = size_of::<StratifiedPlan>() as u128
        + consequences.words.capacity() as u128 * size_of::<u64>() as u128;
    let mut workspace = EvaluationWorkspace::default();
    budget.observe_bytes(base + workspace.retained_bytes())?;
    let available = usize::try_from(budget.limits.max_bytes as u128 - base)
        .map_err(|_| StratifiedError::Overflow)?;
    let attempt = workspace.evaluate(
        consequences,
        EvaluationLimits {
            max_work: budget.limits.max_work - budget.statistics.work,
            max_bytes: available,
        },
        budget.cancellation,
    );
    budget.statistics.work = budget
        .statistics
        .work
        .checked_add(attempt.work)
        .ok_or(StratifiedError::Overflow)?;
    let capacity = budget.observe_bytes(base + attempt.retained_bytes);
    let truth = attempt.result.map_err(|error| match error {
        EvaluationError::Stopped(Stop::WorkLimit) => StratifiedError::Limit {
            resource: StratifiedResource::Work,
            observed: u128::from(budget.statistics.work) + 1,
            limit: u128::from(budget.limits.max_work),
        },
        EvaluationError::Stopped(stop) => StratifiedError::Stopped(stop),
        EvaluationError::Storage { required, .. } => StratifiedError::Limit {
            resource: StratifiedResource::Bytes,
            observed: base + required,
            limit: budget.limits.max_bytes as u128,
        },
    })?;
    capacity?;
    let failed = truth.failed_root();
    if let Some(root) = failed {
        let view = consequences.theory().view();
        let constraint = match view.node(root).map_err(|_| Stop::InvalidProgram)? {
            NodeView::False => true,
            NodeView::Implies(_, head) => {
                view.node(head).map_err(|_| Stop::InvalidProgram)? == NodeView::False
            }
            NodeView::Atom(_) | NodeView::And(_) | NodeView::Or(_) => false,
        };
        if !constraint {
            return Err(StratifiedError::InvalidClosure { root });
        }
    }
    budget.cancellation.poll()?;
    Ok(failed)
}
