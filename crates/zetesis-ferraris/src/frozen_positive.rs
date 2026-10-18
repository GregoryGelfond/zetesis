//! Least consequences of a checked positive fragment of one frozen reduct.
//!
//! Original satisfaction is authenticated by `FormulaEvaluation`. Candidate-false
//! nodes are falsum; a true default negation is truth in the reduct. Every live
//! producer needs a positive And/Or body and one distinct live atomic head.
//! Satisfied constraints and rules with false bodies impose nothing on subsets.
//! These checks cover every root. Least closure starts empty; equality with the
//! candidate decides stability by `ConstrainedPositive.stable_iff_frozen_least`.
//! Complete-root recognition and the CSR implementation remain correspondence
//! obligations; that theorem does not verify this Rust implementation.
//!
//! The source incidence graph is reused across candidates. Each query resets
//! masks, classifies frozen nodes, and queues each vertex at most once. Unsupported
//! roots decline before incidence storage is allocated or prepared. This is
//! a candidate-specific certificate, not a classification of the original theory.

use std::mem::size_of;

use crate::{
    EvaluationError, EvaluationLimits, FormulaEvaluation, Interpretation, NodeView, Theory,
};
use zetesis_cpu::{Cancellation, Meter, Stop};

#[cfg(test)]
mod tests;

/// Completed decision, or absence of this optional frozen-positive certificate.
#[derive(Debug)]
pub enum FrozenPositiveOutcome {
    /// Some active root or body is outside the checked fragment.
    Declined,
    /// The authenticated original evaluation failed a root.
    NotModel,
    /// Complete least consequences equal the original candidate.
    Stable,
    /// Complete least consequences form a proper subset of the candidate.
    /// Consumers may independently validate this original-identity witness.
    NonMinimal(Interpretation),
}

/// An attempt's result, charged prefix and retained capacity, including failure.
#[derive(Debug)]
pub struct FrozenPositiveAttempt {
    /// A completed result, grammar decline or explicit failure.
    pub result: Result<FrozenPositiveOutcome, EvaluationError>,
    /// Work preparing incidences, a subset of `work`.
    pub preparation_work: u64,
    /// Node, root, incidence, initialization and queue operations spent.
    pub work: u64,
    /// Actual workspace header and vector capacities after this attempt.
    pub retained_bytes: u128,
    /// Maximum observed workspace plus returned-witness capacity in this call.
    /// The witness copy is admitted while the workspace remains live.
    pub peak_bytes: u128,
}

#[derive(Clone, Copy, Debug)]
enum State {
    Inactive,
    Awaiting(usize),
    True,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Head {
    False,
    True,
    Atom(usize),
    Multiple,
    Unsupported,
}

#[derive(Clone, Copy, Debug)]
struct Classification {
    monotone: bool,
    head: Head,
}

/// Source-bound incidences and exclusive scratch for frozen-positive closure.
///
/// Storage is linear in atoms, nodes, roots and operand occurrences. Changing
/// theory identity rebuilds incidences; equal-looking theories never share local
/// identities. Queries reuse capacities, never earlier truth or consequences.
#[derive(Debug, Default)]
pub struct FrozenPositiveWorkspace {
    owner: Option<Theory>,
    offsets: Vec<usize>,
    edges: Vec<usize>,
    states: Vec<State>,
    classes: Vec<Classification>,
    heads: Vec<Option<usize>>,
    queue: Vec<usize>,
    words: Vec<u64>,
}

impl FrozenPositiveWorkspace {
    /// Header and actual vector capacities. Shared theory payload, allocator
    /// metadata, allocation overlap and the returned witness are excluded.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + (self.offsets.capacity() as u128
                + self.edges.capacity() as u128
                + self.queue.capacity() as u128)
                * size_of::<usize>() as u128
            + self.states.capacity() as u128 * size_of::<State>() as u128
            + self.classes.capacity() as u128 * size_of::<Classification>() as u128
            + self.heads.capacity() as u128 * size_of::<Option<usize>>() as u128
            + self.words.capacity() as u128 * size_of::<u64>() as u128
    }

    /// Conservative reservation for fresh preparation and one query. The
    /// coefficients cover two incidence passes, prefixes/cursors, initialization,
    /// classification, seeding, one queue event per vertex, one incidence visit,
    /// and final atom comparison/copy. Reuse and early declines spend less.
    #[must_use]
    pub fn work_bound(theory: &Theory) -> u128 {
        12 * theory.atom_count() as u128
            + 18 * theory.nodes().len() as u128
            + 10 * theory.parts().occurrences() as u128
            + 16 * theory.roots().len() as u128
            + 1
    }

    /// Decide the authenticated candidate when every frozen root is covered.
    ///
    /// Bodies admit atoms, falsum, And/Or and syntactic default negation.
    /// Candidate-false subformulas of any shape become falsum. Heads admit
    /// atoms and disjunctions with one distinct live atom; a true negated head
    /// is truth. Multiple live heads or other active shapes decline. Original
    /// nonmodels return before preparation. Each vertex activates at most once.
    ///
    /// Storage checks precede growth and inspect actual capacities afterward.
    /// A stopped query publishes no partial closure; cancellation and exhausted
    /// resources never become a grammar decline or a semantic verdict.
    pub fn check(
        &mut self,
        evaluated: &FormulaEvaluation<'_>,
        limits: EvaluationLimits,
        cancellation: &Cancellation,
    ) -> FrozenPositiveAttempt {
        let mut preparation_work = 0;
        let mut peak_bytes = self.retained_bytes();
        let metered = cancellation.metered(limits.max_work, |meter| {
            meter.poll()?;
            if !evaluated.is_model() {
                return Ok(FrozenPositiveOutcome::NotModel);
            }
            let theory = evaluated.theory();
            self.reserve_classification(theory, limits.max_bytes)?;
            self.classify(evaluated, meter)?;
            if !self.producers(evaluated, meter)? {
                return Ok(FrozenPositiveOutcome::Declined);
            }
            self.reserve(theory, limits.max_bytes)?;
            if self
                .owner
                .as_ref()
                .is_none_or(|owner| !owner.same_instance(theory))
            {
                self.owner = None;
                let before = meter.charged();
                let prepared = self.prepare(theory, meter);
                preparation_work = meter.charged() - before;
                prepared?;
                self.owner = Some(theory.clone());
            }
            self.complete(evaluated, limits.max_bytes, &mut peak_bytes, meter)
        });
        FrozenPositiveAttempt {
            result: metered.result,
            preparation_work,
            work: metered.charged,
            retained_bytes: self.retained_bytes(),
            peak_bytes: peak_bytes.max(self.retained_bytes()),
        }
    }

    fn reserve_classification(
        &mut self,
        theory: &Theory,
        max_bytes: usize,
    ) -> Result<(), EvaluationError> {
        let requested = self.retained_bytes()
            + theory.nodes().len().saturating_sub(self.classes.capacity()) as u128
                * size_of::<Classification>() as u128
            + theory.roots().len().saturating_sub(self.heads.capacity()) as u128
                * size_of::<Option<usize>>() as u128;
        storage(requested, max_bytes)?;
        reserve(&mut self.classes, theory.nodes().len())?;
        reserve(&mut self.heads, theory.roots().len())?;
        storage(self.retained_bytes(), max_bytes)
    }

    fn reserve(&mut self, theory: &Theory, max_bytes: usize) -> Result<(), EvaluationError> {
        let vertices = vertices(theory)?;
        let incidences = theory
            .nodes()
            .len()
            .checked_add(theory.parts().occurrences())
            .and_then(|count| count.checked_add(theory.roots().len()))
            .ok_or(Stop::Allocation)?;
        let offset_count = vertices.checked_add(1).ok_or(Stop::Allocation)?;
        let requested = size_of::<Self>() as u128
            + (offset_count.max(self.offsets.capacity()) as u128
                + incidences.max(self.edges.capacity()) as u128
                + vertices.max(self.queue.capacity()) as u128)
                * size_of::<usize>() as u128
            + vertices.max(self.states.capacity()) as u128 * size_of::<State>() as u128
            + theory.nodes().len().max(self.classes.capacity()) as u128
                * size_of::<Classification>() as u128
            + theory.roots().len().max(self.heads.capacity()) as u128
                * size_of::<Option<usize>>() as u128
            + theory.atom_count().div_ceil(64).max(self.words.capacity()) as u128
                * size_of::<u64>() as u128;
        storage(requested, max_bytes)?;
        reserve(&mut self.offsets, offset_count)?;
        reserve(&mut self.edges, incidences)?;
        reserve(&mut self.states, vertices)?;
        reserve(&mut self.classes, theory.nodes().len())?;
        reserve(&mut self.heads, theory.roots().len())?;
        reserve(&mut self.queue, vertices)?;
        reserve(&mut self.words, theory.atom_count().div_ceil(64))?;
        storage(self.retained_bytes(), max_bytes)
    }

    fn prepare(&mut self, theory: &Theory, meter: &mut Meter<'_>) -> Result<(), EvaluationError> {
        let vertices = vertices(theory)?;
        self.offsets.clear();
        for _ in 0..=vertices {
            tick(meter)?;
            self.offsets.push(0);
        }
        incidences(theory, meter, |source, _| self.offsets[source + 1] += 1)?;
        for vertex in 0..vertices {
            tick(meter)?;
            self.offsets[vertex + 1] += self.offsets[vertex];
        }
        self.edges.clear();
        for _ in 0..self.offsets[vertices] {
            tick(meter)?;
            self.edges.push(0);
        }
        self.queue.clear();
        for &offset in &self.offsets[..vertices] {
            tick(meter)?;
            self.queue.push(offset);
        }
        incidences(theory, meter, |source, target| {
            self.edges[self.queue[source]] = target;
            self.queue[source] += 1;
        })?;
        self.queue.clear();
        Ok(())
    }

    fn complete(
        &mut self,
        evaluated: &FormulaEvaluation<'_>,
        max_bytes: usize,
        peak_bytes: &mut u128,
        meter: &mut Meter<'_>,
    ) -> Result<FrozenPositiveOutcome, EvaluationError> {
        let theory = evaluated.theory();
        self.initialize(evaluated, meter)?;
        let atoms = theory.atom_count();
        let rules = atoms + theory.nodes().len();
        let mut next = 0;
        while let Some(&vertex) = self.queue.get(next) {
            tick(meter)?;
            next += 1;
            if vertex < atoms {
                self.words[vertex / 64] |= 1 << (vertex % 64);
            } else if vertex >= rules
                && let Some(head) = self.heads[vertex - rules]
            {
                self.activate(head, meter)?;
            }
            for edge in self.offsets[vertex]..self.offsets[vertex + 1] {
                tick(meter)?;
                let target = self.edges[edge];
                match self.states[target] {
                    State::Awaiting(1) => self.activate(target, meter)?,
                    State::Awaiting(remaining) => {
                        self.states[target] = State::Awaiting(remaining - 1);
                    }
                    State::Inactive | State::True => {}
                }
            }
        }
        let mut proper = false;
        for atom in 0..atoms {
            tick(meter)?;
            let derived = self.words[atom / 64] & (1 << (atom % 64)) != 0;
            let candidate = evaluated.interpretation().contains(atom);
            if derived && !candidate {
                return Err(Stop::InvalidProgram.into());
            }
            proper |= candidate && !derived;
        }
        if !proper {
            return Ok(FrozenPositiveOutcome::Stable);
        }
        let base = self.retained_bytes();
        storage(
            base + self.words.len() as u128 * size_of::<u64>() as u128,
            max_bytes,
        )?;
        let mut words = Vec::new();
        reserve(&mut words, self.words.len())?;
        let simultaneous = base + words.capacity() as u128 * size_of::<u64>() as u128;
        *peak_bytes = (*peak_bytes).max(simultaneous);
        storage(simultaneous, max_bytes)?;
        for &word in &self.words {
            tick(meter)?;
            words.push(word);
        }
        Ok(FrozenPositiveOutcome::NonMinimal(Interpretation {
            theory: theory.clone(),
            words,
        }))
    }

    fn initialize(
        &mut self,
        evaluated: &FormulaEvaluation<'_>,
        meter: &mut Meter<'_>,
    ) -> Result<(), EvaluationError> {
        let theory = evaluated.theory();
        let atoms = theory.atom_count();
        self.queue.clear();
        self.states.clear();
        for _ in 0..vertices(theory)? {
            tick(meter)?;
            self.states.push(State::Inactive);
        }
        self.words.clear();
        for _ in 0..theory.atom_count().div_ceil(64) {
            tick(meter)?;
            self.words.push(0);
        }
        for atom in 0..atoms {
            tick(meter)?;
            self.states[atom] = State::Awaiting(1);
        }
        for node in 0..theory.nodes().len() {
            tick(meter)?;
            if !evaluated.truth()[node] || !self.classes[node].monotone {
                continue;
            }
            match theory.view().node(node).map_err(|_| Stop::InvalidProgram)? {
                NodeView::Atom(_) | NodeView::Or(_) => {
                    self.states[atoms + node] = State::Awaiting(1);
                }
                NodeView::And(row) => {
                    self.states[atoms + node] = State::Awaiting(row.len());
                    if row.is_empty() {
                        self.activate(atoms + node, meter)?;
                    }
                }
                // Classification admits only true syntactic negation here.
                NodeView::Implies(_, _) => self.activate(atoms + node, meter)?,
                _ => {}
            }
        }
        let rules = atoms + theory.nodes().len();
        for (position, &root) in theory.roots().iter().enumerate() {
            tick(meter)?;
            if self.heads[position].is_some() {
                self.states[rules + position] = State::Awaiting(1);
                if !matches!(
                    theory.view().node(root).map_err(|_| Stop::InvalidProgram)?,
                    NodeView::Implies(_, _)
                ) {
                    self.activate(rules + position, meter)?;
                }
            }
        }
        Ok(())
    }

    fn classify(
        &mut self,
        evaluated: &FormulaEvaluation<'_>,
        meter: &mut Meter<'_>,
    ) -> Result<(), EvaluationError> {
        let theory = evaluated.theory();
        self.classes.clear();
        for node in 0..theory.nodes().len() {
            tick(meter)?;
            let mut class = Classification {
                monotone: true,
                head: Head::False,
            };
            if evaluated.truth()[node] {
                let kind = theory.view().node(node).map_err(|_| Stop::InvalidProgram)?;
                match kind {
                    NodeView::Atom(atom) => {
                        class.head = Head::Atom(atom);
                    }
                    NodeView::And(row) | NodeView::Or(row) => {
                        let conjunction = matches!(kind, NodeView::And(_));
                        class.head = if conjunction {
                            Head::Unsupported
                        } else {
                            Head::False
                        };
                        for &child in row {
                            tick(meter)?;
                            class.monotone &= self.classes[child].monotone;
                            if !conjunction {
                                class.head = combine(class.head, self.classes[child].head);
                            }
                        }
                    }
                    NodeView::Implies(_, consequent) => {
                        tick(meter)?;
                        if theory
                            .view()
                            .node(consequent)
                            .map_err(|_| Stop::InvalidProgram)?
                            .is_false()
                        {
                            class.head = Head::True;
                        } else {
                            class.monotone = false;
                            class.head = Head::Unsupported;
                        }
                    }
                    _ => {
                        class.monotone = false;
                        class.head = Head::Unsupported;
                    }
                }
            }
            self.classes.push(class);
        }
        Ok(())
    }

    fn producers(
        &mut self,
        evaluated: &FormulaEvaluation<'_>,
        meter: &mut Meter<'_>,
    ) -> Result<bool, EvaluationError> {
        let theory = evaluated.theory();
        self.heads.clear();
        for (position, &root) in theory.roots().iter().enumerate() {
            tick(meter)?;
            let (body, head) = match theory.view().node(root).map_err(|_| Stop::InvalidProgram)? {
                NodeView::Implies(body, head) => (Some(body), head),
                _ => (None, root),
            };
            self.heads.push(None);
            if body.is_some_and(|body| !evaluated.truth()[body])
                || theory
                    .view()
                    .node(head)
                    .map_err(|_| Stop::InvalidProgram)?
                    .is_false()
                || self.classes[head].head == Head::True
            {
                continue;
            }
            let Head::Atom(atom) = self.classes[head].head else {
                return Ok(false);
            };
            if body.is_some_and(|body| !self.classes[body].monotone) {
                return Ok(false);
            }
            self.heads[position] = Some(atom);
        }
        Ok(true)
    }

    fn activate(&mut self, vertex: usize, meter: &mut Meter<'_>) -> Result<(), EvaluationError> {
        if !matches!(self.states[vertex], State::True) {
            tick(meter)?;
            self.states[vertex] = State::True;
            self.queue.push(vertex);
        }
        Ok(())
    }
}

fn combine(left: Head, right: Head) -> Head {
    match (left, right) {
        (Head::True, _) | (_, Head::True) => Head::True,
        (Head::False, head) | (head, Head::False) => head,
        (Head::Atom(left), Head::Atom(right)) if left == right => Head::Atom(left),
        (Head::Unsupported, _) | (_, Head::Unsupported) => Head::Unsupported,
        _ => Head::Multiple,
    }
}

fn vertices(theory: &Theory) -> Result<usize, EvaluationError> {
    theory
        .atom_count()
        .checked_add(theory.nodes().len())
        .and_then(|count| count.checked_add(theory.roots().len()))
        .ok_or_else(|| Stop::Allocation.into())
}

fn incidences(
    theory: &Theory,
    meter: &mut Meter<'_>,
    mut visit: impl FnMut(usize, usize),
) -> Result<(), EvaluationError> {
    let atoms = theory.atom_count();
    for node in 0..theory.nodes().len() {
        tick(meter)?;
        match theory.view().node(node).map_err(|_| Stop::InvalidProgram)? {
            NodeView::Atom(atom) => {
                tick(meter)?;
                visit(atom, atoms + node);
            }
            NodeView::And(row) | NodeView::Or(row) => {
                for &child in row {
                    tick(meter)?;
                    visit(atoms + child, atoms + node);
                }
            }
            _ => {}
        }
    }
    for (position, &root) in theory.roots().iter().enumerate() {
        tick(meter)?;
        if let NodeView::Implies(body, _) =
            theory.view().node(root).map_err(|_| Stop::InvalidProgram)?
        {
            tick(meter)?;
            visit(atoms + body, atoms + theory.nodes().len() + position);
        }
    }
    Ok(())
}

fn reserve<T>(values: &mut Vec<T>, count: usize) -> Result<(), EvaluationError> {
    values
        .try_reserve_exact(count.saturating_sub(values.len()))
        .map_err(|_| Stop::Allocation.into())
}

fn storage(required: u128, limit: usize) -> Result<(), EvaluationError> {
    if required > limit as u128 {
        Err(EvaluationError::Storage { required, limit })
    } else {
        Ok(())
    }
}

fn tick(meter: &mut Meter<'_>) -> Result<(), EvaluationError> {
    meter.charge().map_err(|error| match error {
        zetesis_cpu::ChargeError::Stopped(stop) => EvaluationError::Stopped(stop),
        zetesis_cpu::ChargeError::WorkLimit => EvaluationError::Stopped(Stop::WorkLimit),
    })
}
