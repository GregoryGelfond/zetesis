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

use std::collections::BTreeMap;

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
pub(super) struct Group {
    priority: i32,
    activation: usize,
    weight: i32,
    keys: Vec<usize>,
}

#[derive(Debug)]
pub(super) struct Costs {
    pub groups: Vec<Group>,
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
        Some(Costs {
            groups,
            activations,
        })
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

    fn reading(&self, goal: usize) -> Reading {
        if goal >= self.atoms {
            return if self.facts[goal - self.atoms] {
                Reading::Constant(true)
            } else {
                Reading::Any
            };
        }
        match self.node(goal) {
            NodeView::Atom(_) | NodeView::Or(_) => Reading::Any,
            NodeView::And(_) => Reading::All,
            NodeView::False => Reading::Constant(false),
            NodeView::Implies(left, right) => {
                let base = if goal < self.objective {
                    0
                } else {
                    self.objective
                };
                Reading::Constant(
                    self.node(base + left) == NodeView::False
                        && self.node(base + right) == NodeView::False,
                )
            }
        }
    }

    fn child(&self, goal: usize, position: usize) -> Option<usize> {
        if goal >= self.atoms {
            return self.bodies[goal - self.atoms].get(position).copied();
        }
        let base = if goal < self.objective {
            0
        } else {
            self.objective
        };
        match self.node(goal) {
            NodeView::Atom(atom) => (position == 0).then_some(self.atoms + atom),
            NodeView::And(row) | NodeView::Or(row) => row.get(position).map(|node| base + node),
            NodeView::False | NodeView::Implies(..) => None,
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
                    if matches!(self.reading(goal), Reading::Constant(_)) {
                        continue;
                    }
                    let mut position = 0;
                    while let Some(child) = self.child(goal, position) {
                        work.tick()?;
                        if proof.states[child] == 0 {
                            proof.mark(child, 1);
                            pending.push(child);
                        }
                        position += 1;
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
    frames: Vec<(usize, usize)>,
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
        self.frames.push((activation, 0));
        while let Some((node, _)) = self.frames.pop() {
            work.tick()?;
            match graph.node(node) {
                NodeView::Atom(atom) => self.mark(graph.atoms + atom, 3),
                NodeView::And(row) => {
                    for &child in row {
                        work.tick()?;
                        if self.states[child] != 3 {
                            self.mark(child, 3);
                            self.frames.push((child, 0));
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
        self.frames.push((goal, 0));
        let mut result = None;
        while let Some(&(current, position)) = self.frames.last() {
            work.tick()?;
            let reading = graph.reading(current);
            let complete = match (reading, result.take()) {
                (Reading::Constant(value), _) => Some(value),
                (Reading::All, Some(false)) => Some(false),
                (Reading::Any, Some(true)) => Some(true),
                _ => None,
            };
            if let Some(value) = complete {
                self.frames.pop();
                self.mark(current, if value { 3 } else { 2 });
                result = Some(value);
                continue;
            }
            if let Some(child) = graph.child(current, position) {
                self.frames.last_mut().expect("active proof frame").1 += 1;
                if self.states[child] == 0 {
                    self.mark(child, 1);
                    self.frames.push((child, 0));
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
    let mut nodes = FormulaNodes::default();
    for index in 0..exact.view().len() {
        work.node(
            &mut nodes,
            exact
                .view()
                .node(index)
                .map_err(|error| work.error(Kind::Theory(error)))?,
        )?;
    }
    let falsum = work.node(&mut nodes, NodeView::False)?;
    let mut root = work.node(&mut nodes, NodeView::Implies(falsum, falsum))?;
    let copied = activations::append(&costs.activations, &mut nodes, work)?;
    let mut priorities = BTreeMap::new();
    for &priority in plan.levels.keys() {
        work.tick()?;
        priorities.insert(priority, 0_i64);
    }
    for (priority, cost) in incumbent.costs() {
        work.tick()?;
        priorities.insert(*priority, *cost);
    }
    for (priority, bound) in priorities {
        let original = plan.levels.get(&priority).map_or(&[][..], Vec::as_slice);
        let mut prepaid = work.reserve(original.len())?;
        prepaid.resize(original.len(), 0);
        let length = original
            .len()
            .checked_add(costs.groups.len())
            .ok_or_else(|| work.error(Kind::Overflow))?;
        let mut elements = work.reserve(length)?;
        for group in &costs.groups {
            work.tick()?;
            if group.priority != priority {
                continue;
            }
            for &key in &group.keys {
                work.tick()?;
                prepaid[key] = group.weight;
            }
            let activation = copied[group.activation];
            elements.push(AggregateElement {
                weight: group.weight,
                condition: activation,
            });
        }
        for (index, element) in original.iter().enumerate() {
            work.tick()?;
            let weight = element
                .weight
                .checked_sub(prepaid[index])
                .ok_or_else(|| work.error(Kind::Overflow))?;
            if weight != 0 {
                elements.push(AggregateElement {
                    weight,
                    condition: element.condition,
                });
            }
        }
        let (elements, bound) =
            super::bound::normalize::prepare(&elements, bound, falsum, &mut nodes, limits, work)?;
        let family = super::bound::family(&mut nodes, &elements, bound, limits, work)?;
        let suffix = work.node(&mut nodes, NodeView::And(&[family.roots()[1], root]))?;
        root = work.node(&mut nodes, NodeView::Or(&[family.roots()[0], suffix]))?;
    }
    let root = work.node(&mut nodes, NodeView::And(&[exact.roots()[0], root]))?;
    let admission = (nodes.view().len() as u128) * 2 + nodes.parts().occurrences() as u128 + 1;
    work.charge(u64::try_from(admission).map_err(|_| work.error(Kind::Overflow))?)?;
    Theory::new(
        plan.original.atom_count(),
        nodes.into_parts(),
        vec![root],
        zetesis_ferraris::AdmissionLimits {
            max_atoms: plan.original.atom_count(),
            max_nodes: limits.aggregate.max_nodes,
            max_roots: 1,
            max_operands: limits.aggregate.max_operands,
        },
    )
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
            bytes += previous.activations.node_capacity() as u128
                * size_of::<zetesis_ferraris::Node>() as u128;
            bytes += previous.activations.operand_capacity() as u128 * size_of::<usize>() as u128;
            for group in &previous.groups {
                bytes += Self::vector_bytes(&group.keys);
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
