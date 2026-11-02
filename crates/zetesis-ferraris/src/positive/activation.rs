//! One monotone activation queue shared by caller-prepared incidence graphs.
//!
//! Each vertex is queued only on its first activation. Each outgoing incidence
//! is then read once, including repeated incidences. Source classification,
//! initial seeds, capacity admission and semantic validation belong to callers.
//! This is the existing positive propagation schedule, not another source
//! evaluator or an answer-set certificate.

#[cfg(test)]
mod tests;

/// Inactive constants are seeded explicitly; false/unsupported vertices stay
/// inactive. Awaiting counters count occurrences, not distinct source vertices.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    Inactive,
    Awaiting(usize),
    Weighted(i128),
    True,
}

#[derive(Clone, Copy)]
pub(crate) struct Edge {
    pub(crate) target: usize,
    pub(crate) weight: u64,
}

/// The old positive caller's debit points, alongside finer operation chunks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Point {
    Operation,
    Dequeue,
    Incidence,
    Activation,
}

/// Caller effects at an operation's admission boundary.
///
/// `before` precedes the complete stated chunk. `PositivePlan` debits only its
/// three legacy points. AR debits all word-operation chunks on its `Awaiting`-only
/// graph. `Weighted` arithmetic retains its native i128 operations and legacy
/// positive contract; it has no new word-charge contract in this factoring.
/// A caller requiring word accounting must not construct `Weighted` states.
///
/// `activated` follows the state/queue mutation and must admit any observation's
/// own writes before performing them. Failure returns immediately; no caller may
/// publish that unfinished attempt as a checked semantic result.
pub(crate) trait Effects {
    type Error;

    fn before(&mut self, point: Point, work: u64) -> Result<(), Self::Error>;

    /// `None` names an explicit seed; an incidence supplies its source vertex.
    fn activated(&mut self, vertex: usize, source: Option<usize>) -> Result<(), Self::Error>;
}

/// First activation only. The caller reserves at least `states.len()` queue
/// cells before seeding and keeps every `True` state represented exactly once in
/// the queue. Thus this push never grows the queue. Vertex IDs are validated by
/// the caller's graph construction; no unchecked memory access is used.
pub(crate) fn activate<E: Effects>(
    vertex: usize,
    source: Option<usize>,
    states: &mut [State],
    queue: &mut Vec<usize>,
    effects: &mut E,
) -> Result<(), E::Error> {
    effects.before(Point::Operation, 1)?; // inspect the already-true state
    if states[vertex] != State::True {
        effects.before(Point::Activation, 3)?; // state tag, queued vertex, queue length
        states[vertex] = State::True;
        queue.push(vertex);
        effects.activated(vertex, source)?;
    }
    Ok(())
}

/// Drain the caller's pre-reserved queue in source-before-target order.
///
/// The CSR offsets span every state, edges target valid states, `Awaiting`
/// counters are positive until activation, and words cover all atom vertices.
/// These are construction invariants, not extra run-time source assumptions.
/// The queue retains its processed prefix, preserving first-activation order.
/// Every actual chunk is admitted before its data operations; finite-loop and
/// address-control bookkeeping is excluded from the logical work unit.
pub(crate) fn drain<E: Effects>(
    atoms: usize,
    offsets: &[usize],
    edges: &[Edge],
    states: &mut [State],
    queue: &mut Vec<usize>,
    words: &mut [u64],
    effects: &mut E,
) -> Result<(), E::Error> {
    let mut next = 0;
    while next < queue.len() {
        effects.before(Point::Dequeue, 1)?; // read the queued vertex
        let vertex = queue[next];
        next += 1;
        effects.before(Point::Operation, 1)?;
        if vertex < atoms {
            effects.before(Point::Operation, 4)?; // word, bit, shift and OR
            words[vertex / 64] |= 1 << (vertex % 64);
        }
        effects.before(Point::Operation, 2)?; // the outgoing span's endpoints
        let first = offsets[vertex];
        let end = offsets[vertex + 1];
        let mut edge = first;
        while edge < end {
            effects.before(Point::Incidence, 3)?; // edge's two fields and target copy
            let incidence = edges[edge];
            edge += 1;
            let target = incidence.target;
            effects.before(Point::Operation, 1)?; // inspect the target state
            match states[target] {
                State::Awaiting(remaining) => {
                    effects.before(Point::Operation, 1)?;
                    if remaining == 1 {
                        activate(target, Some(vertex), states, queue, effects)?;
                    } else {
                        effects.before(Point::Operation, 3)?; // subtract, state tag and count
                        states[target] = State::Awaiting(remaining - 1);
                    }
                }
                State::Weighted(remaining) => {
                    // This branch is the unchanged legacy positive arithmetic.
                    // AR's selected-rule graph never constructs Weighted states.
                    let weight = i128::from(incidence.weight);
                    if weight >= remaining {
                        activate(target, Some(vertex), states, queue, effects)?;
                    } else {
                        states[target] = State::Weighted(remaining - weight);
                    }
                }
                State::Inactive | State::True => {}
            }
        }
    }
    Ok(())
}
