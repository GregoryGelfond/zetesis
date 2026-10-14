//! Necessary costs from complete source choices and checked forward implications.
//!
//! A selected member must imply one allocated objective key. Every key is
//! globally coalesced before this pass, and groups allocate disjoint key sets.
//! Prepay each active group's minimum and retain every eligible key's excess
//! over that minimum. The original cost covers this sum whenever an active
//! group has at least one eligible allocated key.
//! Cyclic or unsupported implication searches decline a proof; they never prove
//! absence. The resulting restriction is only for original candidates.

mod activations;
mod residuals;

#[cfg(test)]
mod tests;

use std::borrow::Cow;

use zetesis_cpu::Cancellation;
use zetesis_ferraris::{AggregateElement, FormulaNodes, FormulaParts, NodeView, Theory};
use zetesis_objective::Score;

use super::{
    ObjectiveBound, ObjectiveBoundError, ObjectiveBoundErrorKind as Kind, ObjectiveBoundLimits,
    ObjectiveBoundResource as Resource, ObjectiveBoundStatistics, ObjectivePlan,
    ObjectivePlanLimits, Work,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    priority: i32,
    index: usize,
}

#[derive(Debug)]
struct Group {
    priority: i32,
    activation: usize,
    weight: i32,
    keys: Vec<usize>,
}

#[derive(Debug)]
pub(super) struct Costs {
    /// Mandatory floors, grouped by priority; conditions address `activations`.
    pub groups: Vec<AggregateElement>,
    /// Changed priorities only. Eligibility still addresses the exact plan.
    levels: Vec<residuals::Level>,
    activations: FormulaParts,
}

pub(super) fn prepare(
    plan: &ObjectivePlan,
    required: &crate::RequiredChoices,
    limits: ObjectivePlanLimits,
    max_bytes: u128,
    cancellation: &Cancellation,
) -> Result<(Option<Costs>, ObjectiveBoundStatistics), ObjectiveBoundError> {
    let mut work = Work {
        limits,
        cancellation,
        template: None,
        statistics: ObjectiveBoundStatistics::default(),
    };
    work.tick()?;
    if required.capture_failure().is_some() {
        return Err(work.error(Kind::ChoiceCapture));
    }
    if !required.belongs_to(&plan.original) {
        return Err(work.error(Kind::ChoiceOwner));
    }
    if required.groups.len() > limits.max_keys {
        return Err(work.limit(Resource::Keys));
    }
    let mut required_member = false;
    for group in &required.groups {
        work.tick()?;
        required_member |= group.lower > 0 && !group.members.is_empty();
    }
    let mut eligible_level = false;
    for elements in plan.levels.values() {
        let mut nonnegative = true;
        let mut positive = false;
        for element in elements {
            work.tick()?;
            nonnegative &= element.weight >= 0;
            positive |= element.weight > 0;
        }
        eligible_level |= nonnegative && positive;
    }
    if !required_member || !eligible_level {
        return Ok((None, work.statistics));
    }
    let mut memory = Storage::new(max_bytes, required, plan.choices.as_ref(), &work)?;
    let mut bridges = Bridges::new(plan, required, &mut memory, &mut work)?;
    let mut levels = memory.reserve(plan.levels.len(), &work)?;
    for (&priority, elements) in &plan.levels {
        work.tick()?;
        let mut nonnegative = true;
        for element in elements {
            work.tick()?;
            nonnegative &= element.weight >= 0;
        }
        if !nonnegative {
            continue;
        }
        let mut allocated = memory.reserve(elements.len(), &work)?;
        allocated.resize(elements.len(), false);
        levels.push(Level {
            priority,
            elements,
            allocated,
        });
    }
    let mut groups = memory.reserve(required.groups.len(), &work)?;
    for source in &required.groups {
        work.tick()?;
        if source.lower == 0 {
            continue;
        }
        for level in &mut levels {
            if let Some(group) = bridges.cost(source, level, &mut memory, &mut work)? {
                memory.grow(&mut groups, 1, &work)?;
                groups.push(group);
            }
        }
    }
    work.tick()?;
    let costs = if groups.is_empty() {
        None
    } else {
        let activations =
            activations::prepare(&plan.original, &mut groups, &mut memory, &mut work)?;
        Some(residuals::prepare(
            plan,
            &groups,
            activations,
            &mut memory,
            &mut work,
        )?)
    };
    Ok((costs, work.statistics))
}

struct Level<'a> {
    priority: i32,
    elements: &'a [AggregateElement],
    allocated: Vec<bool>,
}

struct Bridges<'a> {
    graph: Implications<'a>,
    proof: Proof,
    targets: Vec<Vec<Key>>,
}

impl<'a> Bridges<'a> {
    fn new(
        plan: &'a ObjectivePlan,
        required: &crate::RequiredChoices,
        memory: &mut Storage,
        work: &mut Work<'_>,
    ) -> Result<Self, ObjectiveBoundError> {
        let graph = Implications::new(plan, memory, work)?;
        let mut proof = Proof::new(&graph, memory, work)?;
        let targets = graph.targets(plan, required, &mut proof, memory, work)?;
        Ok(Self {
            graph,
            proof,
            targets,
        })
    }

    /// Every member must imply an unused objective key. Keys used by this group
    /// may coincide with each other, but never with a preceding group's keys.
    fn cost(
        &mut self,
        source: &crate::formula_count_plan::RequiredGroup,
        level: &mut Level<'_>,
        memory: &mut Storage,
        work: &mut Work<'_>,
    ) -> Result<Option<Group>, ObjectiveBoundError> {
        work.tick()?;
        let mut keys = memory.reserve(source.members.len(), work)?;
        let mut weight = i32::MAX;
        for &member in &source.members {
            work.tick()?;
            self.proof.assume(&self.graph, source.body, member, work)?;
            let mut chosen = None;
            for key in &self.targets[member] {
                work.tick()?;
                if key.priority != level.priority || level.allocated[key.index] {
                    continue;
                }
                let element = level.elements[key.index];
                if element.weight > 0
                    && self.proof.proves(
                        &self.graph,
                        self.graph.objective + element.condition,
                        work,
                    )?
                {
                    chosen = Some(key.index);
                    weight = weight.min(element.weight);
                    break;
                }
            }
            let Some(index) = chosen else {
                memory.retire(&keys);
                return Ok(None);
            };
            keys.push(index);
        }
        if keys.is_empty() {
            memory.retire(&keys);
            return Ok(None);
        }
        // Repeated members may imply the same coalesced key; count it once.
        let length = u64::try_from(keys.len()).map_err(|_| work.error(Kind::Overflow))?;
        work.charge(
            length
                .checked_mul(length)
                .ok_or_else(|| work.error(Kind::Overflow))?,
        )?;
        keys.sort_unstable();
        keys.dedup();
        for &index in &keys {
            work.tick()?;
            level.allocated[index] = true;
        }
        Ok(Some(Group {
            priority: level.priority,
            activation: source.body,
            weight,
            keys,
        }))
    }
}

/// Original normal-rule roots are sufficient forward implications. Other roots
/// remain in the original theory but supply no edge to this optional proof.
struct Implications<'a> {
    plan: &'a ObjectivePlan,
    bodies: Vec<Vec<usize>>,
    facts: Vec<bool>,
    objective: usize,
    atoms: usize,
    vertices: usize,
}

#[derive(Clone, Copy)]
enum Reading {
    Constant(bool),
    All,
    Any,
    Weighted(i128),
}

impl<'a> Implications<'a> {
    fn new(
        plan: &'a ObjectivePlan,
        memory: &mut Storage,
        work: &mut Work<'_>,
    ) -> Result<Self, ObjectiveBoundError> {
        let theory = &plan.original;
        if theory.atom_count() > work.limits.max_atoms {
            return Err(work.limit(Resource::Atoms));
        }
        if theory.nodes().len() > work.limits.max_nodes {
            return Err(work.limit(Resource::Nodes));
        }
        let mut bodies = memory.reserve(theory.atom_count(), work)?;
        let mut facts = memory.reserve(theory.atom_count(), work)?;
        for _ in 0..theory.atom_count() {
            work.tick()?;
            bodies.push(Vec::new());
            facts.push(false);
        }
        for &root in theory.roots() {
            work.tick()?;
            match theory
                .view()
                .node(root)
                .map_err(|error| work.error(Kind::Theory(error)))?
            {
                NodeView::Atom(atom) => facts[atom] = true,
                NodeView::Implies(body, head) => {
                    work.tick()?;
                    if let NodeView::Atom(atom) = theory
                        .view()
                        .node(head)
                        .map_err(|error| work.error(Kind::Theory(error)))?
                    {
                        memory.grow(&mut bodies[atom], 1, work)?;
                        bodies[atom].push(body);
                    }
                }
                _ => {}
            }
        }
        let objective = theory.nodes().len();
        let atoms = objective
            .checked_add(plan.nodes.view().len())
            .ok_or_else(|| work.error(Kind::Overflow))?;
        let vertices = atoms
            .checked_add(theory.atom_count())
            .ok_or_else(|| work.error(Kind::Overflow))?;
        Ok(Self {
            plan,
            bodies,
            facts,
            objective,
            atoms,
            vertices,
        })
    }

    fn node(&self, goal: usize) -> NodeView<'_> {
        if goal < self.objective {
            self.plan.original.view().node(goal).expect("original node")
        } else {
            self.plan
                .nodes
                .view()
                .node(goal - self.objective)
                .expect("objective node")
        }
    }

    fn reading(&self, goal: usize, work: &mut Work<'_>) -> Result<Reading, ObjectiveBoundError> {
        if goal >= self.atoms {
            return Ok(if self.facts[goal - self.atoms] {
                Reading::Constant(true)
            } else {
                Reading::Any
            });
        }
        Ok(match self.node(goal) {
            NodeView::Atom(_) | NodeView::Or(_) => Reading::Any,
            NodeView::And(_) => Reading::All,
            NodeView::False => Reading::Constant(false),
            NodeView::Threshold(threshold) => Reading::Weighted(threshold.bound),
            NodeView::ExtremumCut(cut) => {
                cut.endpoint_truth().map_or(Reading::Any, Reading::Constant)
            }
            NodeView::Implies(left, right) => {
                let base = if goal < self.objective {
                    0
                } else {
                    self.objective
                };
                Reading::Constant(
                    self.structural_falsum(base + left, work)?
                        && self.structural_falsum(base + right, work)?,
                )
            }
        })
    }

    /// Preserve the former literal-falsum implication proof for an empty native
    /// witness set. Failure to prove a child is never used as negative evidence.
    fn structural_falsum(
        &self,
        goal: usize,
        work: &mut Work<'_>,
    ) -> Result<bool, ObjectiveBoundError> {
        match self.node(goal) {
            NodeView::False => Ok(true),
            NodeView::ExtremumCut(cut) => {
                if let Some(value) = cut.endpoint_truth() {
                    return Ok(!value);
                }
                for (&child, &rank) in cut.row.conditions.iter().zip(cut.row.ranks) {
                    work.tick()?;
                    let base = if goal < self.objective {
                        0
                    } else {
                        self.objective
                    };
                    if cut.qualifies(rank) && self.node(base + child) != NodeView::False {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Advance a physical occurrence cursor. Skipped ordinal ranks are charged
    /// once, avoiding repeated prefix scans when witnesses are sparse.
    fn child(
        &self,
        goal: usize,
        position: &mut usize,
        work: &mut Work<'_>,
    ) -> Result<Option<usize>, ObjectiveBoundError> {
        if goal >= self.atoms {
            let child = self.bodies[goal - self.atoms].get(*position).copied();
            *position += usize::from(child.is_some());
            return Ok(child);
        }
        let base = if goal < self.objective {
            0
        } else {
            self.objective
        };
        let child = match self.node(goal) {
            NodeView::Atom(atom) => (*position == 0).then_some(self.atoms + atom),
            NodeView::And(row) | NodeView::Or(row) => row.get(*position).map(|node| base + node),
            NodeView::Threshold(threshold) => {
                threshold.conditions.get(*position).map(|node| base + node)
            }
            NodeView::ExtremumCut(cut) => {
                if cut.endpoint_truth().is_some() {
                    return Ok(None);
                }
                while let Some(&rank) = cut.row.ranks.get(*position) {
                    work.tick()?;
                    let child = cut.row.conditions[*position];
                    *position += 1;
                    if cut.qualifies(rank) {
                        return Ok(Some(base + child));
                    }
                }
                return Ok(None);
            }
            NodeView::False | NodeView::Implies(..) => None,
        };
        *position += usize::from(child.is_some());
        Ok(child)
    }

    fn weight(&self, goal: usize, position: usize) -> u64 {
        if let NodeView::Threshold(threshold) = self.node(goal) {
            threshold.weights[position]
        } else {
            0
        }
    }

    /// Reverse reachable objective keys avoid a member-by-all-keys proof scan.
    /// This is only an overapproximation; `Proof` validates every used bridge.
    fn targets(
        &self,
        plan: &ObjectivePlan,
        required: &crate::RequiredChoices,
        proof: &mut Proof,
        memory: &mut Storage,
        work: &mut Work<'_>,
    ) -> Result<Vec<Vec<Key>>, ObjectiveBoundError> {
        let mut members = memory.reserve(self.bodies.len(), work)?;
        let mut output = memory.reserve(self.bodies.len(), work)?;
        for _ in 0..self.bodies.len() {
            work.tick()?;
            members.push(false);
            output.push(Vec::new());
        }
        for group in &required.groups {
            for &member in &group.members {
                work.tick()?;
                members[member] = true;
            }
        }
        let mut pending = memory.reserve(self.vertices, work)?;
        for (&priority, elements) in &plan.levels {
            for (index, element) in elements.iter().enumerate() {
                work.tick()?;
                if element.weight <= 0 {
                    continue;
                }
                proof.clear(work)?;
                let start = self.objective + element.condition;
                proof.mark(start, 1);
                pending.push(start);
                while let Some(goal) = pending.pop() {
                    work.tick()?;
                    if goal >= self.atoms && members[goal - self.atoms] {
                        let targets = &mut output[goal - self.atoms];
                        memory.grow(targets, 1, work)?;
                        targets.push(Key { priority, index });
                    }
                    if matches!(self.reading(goal, work)?, Reading::Constant(_)) {
                        continue;
                    }
                    let mut position = 0;
                    while let Some(child) = self.child(goal, &mut position, work)? {
                        work.tick()?;
                        if proof.states[child] == 0 {
                            proof.mark(child, 1);
                            pending.push(child);
                        }
                    }
                }
            }
        }
        proof.clear(work)?;
        memory.retire(&pending);
        memory.retire(&members);
        Ok(output)
    }
}

/// The iterative proof visits each goal once under one group's assumptions.
/// An active cycle yields no proof. A failed proof is never a negative fact.
struct Proof {
    states: Vec<u8>,
    touched: Vec<usize>,
    frames: Vec<Frame>,
}

#[derive(Clone, Copy)]
struct Frame {
    goal: usize,
    position: usize,
    sum: i128,
}

impl Frame {
    fn new(goal: usize) -> Self {
        Self {
            goal,
            position: 0,
            sum: 0,
        }
    }
}

impl Proof {
    fn new(
        graph: &Implications<'_>,
        memory: &mut Storage,
        work: &mut Work<'_>,
    ) -> Result<Self, ObjectiveBoundError> {
        let mut states = memory.reserve(graph.vertices, work)?;
        for _ in 0..graph.vertices {
            work.tick()?;
            states.push(0);
        }
        Ok(Self {
            states,
            touched: memory.reserve(graph.vertices, work)?,
            frames: memory.reserve(graph.vertices, work)?,
        })
    }

    fn mark(&mut self, goal: usize, value: u8) {
        if self.states[goal] == 0 {
            self.touched.push(goal);
        }
        self.states[goal] = value;
    }

    fn clear(&mut self, work: &mut Work<'_>) -> Result<(), ObjectiveBoundError> {
        for goal in self.touched.drain(..) {
            work.tick()?;
            self.states[goal] = 0;
        }
        self.frames.clear();
        Ok(())
    }

    fn assume(
        &mut self,
        graph: &Implications<'_>,
        activation: usize,
        member: usize,
        work: &mut Work<'_>,
    ) -> Result<(), ObjectiveBoundError> {
        self.clear(work)?;
        self.mark(graph.atoms + member, 3);
        self.mark(activation, 3);
        self.frames.push(Frame::new(activation));
        while let Some(Frame { goal: node, .. }) = self.frames.pop() {
            work.tick()?;
            match graph.node(node) {
                NodeView::Atom(atom) => self.mark(graph.atoms + atom, 3),
                NodeView::And(row) => {
                    for &child in row {
                        work.tick()?;
                        if self.states[child] != 3 {
                            self.mark(child, 3);
                            self.frames.push(Frame::new(child));
                        }
                    }
                }
                NodeView::ExtremumCut(cut) => {
                    if let Some(child) = unique_witness(cut, work)?
                        && self.states[child] != 3
                    {
                        self.mark(child, 3);
                        self.frames.push(Frame::new(child));
                    }
                }
                NodeView::Threshold(threshold) => {
                    let mut total = 0_i128;
                    for &weight in threshold.weights {
                        work.tick()?;
                        total = total
                            .checked_add(i128::from(weight))
                            .ok_or_else(|| work.error(Kind::Overflow))?;
                    }
                    for (&child, &weight) in threshold.conditions.iter().zip(threshold.weights) {
                        work.tick()?;
                        // Omitting this contribution already makes the assumed
                        // true threshold impossible. Repetitions remain separate
                        // sufficient tests; no tuple multiplicity is discarded.
                        if weight != 0
                            && total - i128::from(weight) < threshold.bound
                            && self.states[child] != 3
                        {
                            self.mark(child, 3);
                            self.frames.push(Frame::new(child));
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn proves(
        &mut self,
        graph: &Implications<'_>,
        goal: usize,
        work: &mut Work<'_>,
    ) -> Result<bool, ObjectiveBoundError> {
        if self.states[goal] != 0 {
            return Ok(self.states[goal] == 3);
        }
        self.mark(goal, 1);
        self.frames.push(Frame::new(goal));
        let mut result = None;
        while let Some(&Frame {
            goal: current,
            position,
            ..
        }) = self.frames.last()
        {
            work.tick()?;
            let reading = graph.reading(current, work)?;
            let child_result = result.take();
            if matches!(reading, Reading::Weighted(_)) && child_result == Some(true) {
                let weight = i128::from(graph.weight(current, position - 1));
                let frame = self.frames.last_mut().expect("active proof frame");
                frame.sum = frame
                    .sum
                    .checked_add(weight)
                    .ok_or_else(|| work.error(Kind::Overflow))?;
            }
            let complete = match (reading, child_result) {
                (Reading::Constant(value), _) => Some(value),
                (Reading::All, Some(false)) => Some(false),
                (Reading::Any, Some(true)) => Some(true),
                (Reading::Weighted(bound), _)
                    if self.frames.last().expect("active proof frame").sum >= bound =>
                {
                    Some(true)
                }
                _ => None,
            };
            if let Some(value) = complete {
                self.frames.pop();
                self.mark(current, if value { 3 } else { 2 });
                result = Some(value);
                continue;
            }
            let frame = self.frames.last_mut().expect("active proof frame");
            if let Some(child) = graph.child(current, &mut frame.position, work)? {
                if self.states[child] == 0 {
                    self.mark(child, 1);
                    self.frames.push(Frame::new(child));
                } else {
                    result = Some(self.states[child] == 3);
                }
            } else {
                let value = matches!(reading, Reading::All);
                self.frames.pop();
                self.mark(current, if value { 3 } else { 2 });
                result = Some(value);
            }
        }
        Ok(result == Some(true))
    }
}

/// A true disjunction forces its only distinct child, including a composite
/// child or repeated occurrences. Endpoint truth has no witness obligation.
fn unique_witness(
    cut: zetesis_ferraris::ExtremumCut<'_>,
    work: &mut Work<'_>,
) -> Result<Option<usize>, ObjectiveBoundError> {
    if cut.endpoint_truth().is_some() {
        return Ok(None);
    }
    let mut witness = None;
    for (&child, &rank) in cut.row.conditions.iter().zip(cut.row.ranks) {
        work.tick()?;
        if cut.qualifies(rank) {
            if witness.is_some_and(|previous| previous != child) {
                return Ok(None);
            }
            witness = Some(child);
        }
    }
    Ok(witness)
}

pub(super) fn strengthen(
    plan: &ObjectivePlan,
    mut exact: ObjectiveBound,
    incumbent: &Score,
    limits: ObjectiveBoundLimits,
    cancellation: &Cancellation,
) -> ObjectiveBound {
    let Some(costs) = &plan.choices else {
        return exact;
    };
    let mut work = Work {
        limits: ObjectivePlanLimits {
            max_nodes: limits.aggregate.max_nodes,
            max_operands: limits.aggregate.max_operands,
            max_work: limits.max_work,
            ..ObjectivePlanLimits::default()
        },
        cancellation,
        template: None,
        statistics: exact.statistics,
    };
    match combined(plan, costs, &exact.theory, incumbent, limits, &mut work) {
        Ok(theory) => exact.theory = theory,
        Err(error) => exact.choice_failure = Some(error),
    }
    exact.statistics = work.statistics;
    exact
}

fn combined(
    plan: &ObjectivePlan,
    costs: &Costs,
    exact: &Theory,
    incumbent: &Score,
    limits: ObjectiveBoundLimits,
    work: &mut Work<'_>,
) -> Result<Theory, ObjectiveBoundError> {
    let parts = exact.parts();
    work.charge(u64::try_from(parts.copy_work()).map_err(|_| work.error(Kind::Overflow))?)?;
    let mut nodes = FormulaNodes::new(
        parts
            .try_clone()
            .map_err(|error| work.error(Kind::Theory(error)))?,
    );
    let copied = activations::append(&costs.activations, &mut nodes, work)?;
    // Both priority lists ascend, and every prepared level belongs to the exact
    // plan. Extra incumbent priorities leave this cursor in place.
    let mut prepared = costs.levels.iter().peekable();
    let root = super::bound::lexicographic(
        &mut nodes,
        plan.levels.keys().copied(),
        incumbent,
        limits,
        work,
        |priority, work| {
            let Some(level) = prepared.peek().filter(|level| level.priority == priority) else {
                return Ok(Cow::Borrowed(
                    plan.levels.get(&priority).map_or(&[][..], Vec::as_slice),
                ));
            };
            let count = level
                .groups
                .len()
                .checked_add(level.residuals.len())
                .ok_or_else(|| work.error(Kind::Overflow))?;
            let mut elements = work.reserve(count)?;
            for group in &costs.groups[level.groups.clone()] {
                work.tick()?;
                elements.push(AggregateElement {
                    weight: group.weight,
                    condition: copied[group.condition],
                });
            }
            for &residual in &level.residuals {
                work.tick()?;
                elements.push(residual);
            }
            prepared.next();
            Ok(Cow::Owned(elements))
        },
    )?;
    let root = work.node(&mut nodes, NodeView::And(&[exact.roots()[0], root]))?;
    let admission = nodes.prepare_admission(
        plan.original.atom_count(),
        vec![root],
        zetesis_ferraris::AdmissionLimits {
            max_atoms: plan.original.atom_count(),
            max_nodes: limits.aggregate.max_nodes,
            max_roots: 1,
            max_operands: limits.aggregate.max_operands,
        },
    );
    work.charge(u64::try_from(admission.work()).map_err(|_| work.error(Kind::Overflow))?)?;
    admission
        .admit()
        .map_err(|error| work.error(Kind::Theory(error)))
}

/// Named vector capacities in optional proof preparation. Source premises are
/// borrowed and charged once. Allocator overhead and unrelated owners are not
/// represented; this is an explicit preparation allowance, not a process limit.
struct Storage {
    bytes: u128,
    limit: u128,
}

impl Storage {
    fn new(
        limit: u128,
        required: &crate::RequiredChoices,
        previous: Option<&Costs>,
        work: &Work<'_>,
    ) -> Result<Self, ObjectiveBoundError> {
        let mut bytes = required.retained_bytes() + size_of::<Costs>() as u128;
        if let Some(previous) = previous {
            bytes += Self::vector_bytes(&previous.groups);
            bytes += previous.activations.retained_bytes();
            bytes += Self::vector_bytes(&previous.levels);
            for level in &previous.levels {
                bytes += Self::vector_bytes(&level.residuals);
            }
        }
        let result = Self { bytes, limit };
        result.check(0, work)?;
        Ok(result)
    }

    fn vector_bytes<T>(values: &Vec<T>) -> u128 {
        values.capacity() as u128 * size_of::<T>() as u128
    }

    fn check(&self, additional: u128, work: &Work<'_>) -> Result<(), ObjectiveBoundError> {
        if self
            .bytes
            .checked_add(additional)
            .is_none_or(|total| total > self.limit)
        {
            return Err(work.limit(Resource::ChoiceBytes));
        }
        Ok(())
    }

    fn reserve<T>(&mut self, count: usize, work: &Work<'_>) -> Result<Vec<T>, ObjectiveBoundError> {
        self.check(count as u128 * size_of::<T>() as u128, work)?;
        let values = work.reserve(count)?;
        let bytes = Self::vector_bytes(&values);
        self.check(bytes, work)?;
        self.bytes += bytes;
        Ok(values)
    }

    fn grow<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
        work: &Work<'_>,
    ) -> Result<(), ObjectiveBoundError> {
        let length = values
            .len()
            .checked_add(additional)
            .ok_or_else(|| work.error(Kind::Overflow))?;
        if length <= values.capacity() {
            return Ok(());
        }
        let before = Self::vector_bytes(values);
        self.check(
            (length - values.capacity()) as u128 * size_of::<T>() as u128,
            work,
        )?;
        values
            .try_reserve_exact(additional)
            .map_err(|_| work.error(Kind::Allocation))?;
        let delta = Self::vector_bytes(values) - before;
        self.check(delta, work)?;
        self.bytes += delta;
        Ok(())
    }

    fn retire<T>(&mut self, values: &Vec<T>) {
        self.bytes -= Self::vector_bytes(values);
    }
}
