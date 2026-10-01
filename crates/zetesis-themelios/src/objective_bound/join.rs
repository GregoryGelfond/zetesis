use std::cmp::Ordering;
use std::collections::BTreeMap;

use zetesis_core::catalog::{AtomRef, Atoms, TermRef};
use zetesis_core::{AtomIndex, AtomIndexError, AtomRows, PatternRef, TemplateTerm, ValueNodeRef};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{AggregateElement, Node, Theory};
use zetesis_objective::{Condition, ConditionNode, ObjectiveProgram, ObjectiveTemplateRef};

use super::{
    ObjectiveBoundError, ObjectiveBoundErrorKind as Kind, ObjectiveBoundResource as Resource,
    ObjectiveBoundStatistics, ObjectivePlan, ObjectivePlanLimits, Work,
};

struct Key<'a> {
    priority: i32,
    weight: i32,
    tuple: Vec<TermRef<'a>>,
    condition: usize,
}
struct Compiler<'a> {
    work: Work<'a>,
    index: AtomIndex<'a>,
    nodes: Vec<Node>,
    keys: Vec<Key<'a>>,
    truth: usize,
}

pub(super) fn compile(
    original: &Theory,
    atoms: Atoms<'_>,
    objectives: &ObjectiveProgram,
    limits: ObjectivePlanLimits,
    cancellation: &Cancellation,
) -> Result<ObjectivePlan, ObjectiveBoundError> {
    let mut work = Work {
        cancellation,
        limits,
        template: None,
        statistics: ObjectiveBoundStatistics::default(),
    };
    work.tick()?;
    if atoms.len() != original.atom_count() {
        return Err(work.error(Kind::AtomCatalog));
    }
    if atoms.len() > limits.max_atoms {
        return Err(work.limit(Resource::Atoms));
    }
    let mut nodes = Vec::new();
    // The existing atom ceiling admits both O(n) index orders and reusable
    // merge scratch. Only integer row IDs are allocated; payload remains here.
    let index =
        AtomIndex::from_catalog_with(atoms, || work.tick()).map_err(|error| match error {
            AtomIndexError::Stopped(error) => error,
            AtomIndexError::Allocation => work.error(Kind::Allocation),
            AtomIndexError::Duplicate { .. } => work.error(Kind::AtomCatalog),
        })?;
    for index in 0..atoms.len() {
        work.node(&mut nodes, Node::Atom(index))?;
    }
    let falsum = work.node(&mut nodes, Node::False)?;
    let truth = work.node(&mut nodes, Node::Implies(falsum, falsum))?;
    let mut compiler = Compiler {
        work,
        index,
        nodes,
        keys: Vec::new(),
        truth,
    };
    for (index, template) in objectives.templates().iter().enumerate() {
        compiler.work.template = Some(index);
        compiler.join(template)?;
    }
    compiler.work.template = None;
    let mut levels = BTreeMap::new();
    for priority in objectives.priorities() {
        compiler.work.tick()?;
        levels.insert(*priority, Vec::new());
    }
    for key in compiler.keys {
        compiler.work.tick()?;
        let elements = levels.entry(key.priority).or_default();
        elements
            .try_reserve(1)
            .map_err(|_| compiler.work.error(Kind::Allocation))?;
        elements.push(AggregateElement {
            weight: key.weight,
            condition: key.condition,
        });
    }
    Ok(ObjectivePlan {
        original: original.clone(),
        objectives: objectives.clone(),
        nodes: compiler.nodes,
        levels,
        statistics: compiler.work.statistics,
    })
}

struct Frame<'index, 'source> {
    rows: AtomRows<'index, 'source>,
    trail_start: usize,
}

impl<'a> Compiler<'a> {
    fn join(&mut self, template: ObjectiveTemplateRef<'a>) -> Result<(), ObjectiveBoundError> {
        if template.positive().len() > self.work.limits.max_body_atoms {
            return Err(self.work.limit(Resource::BodyAtoms));
        }
        if template.tuple().len() > self.work.limits.max_tuple_width {
            return Err(self.work.limit(Resource::TupleWidth));
        }
        let count = self.variables(template)?;
        let mut binding = self.work.reserve(count)?;
        binding.resize(count, None);
        let mut trail = self.work.reserve(count)?;
        let condition = self.condition(template.condition())?;
        if template.positive().is_empty() {
            return self.active(template, &binding, condition.as_slice());
        }
        let mut frames = self.work.reserve(template.positive().len())?;
        let mut chosen = self.work.reserve(template.positive().len())?;
        chosen.resize(template.positive().len(), 0);
        if let Some(condition) = condition {
            chosen
                .try_reserve(1)
                .map_err(|_| self.work.error(Kind::Allocation))?;
            chosen.push(condition);
        }
        let lookup = self.index.lookup();
        frames.push(Frame {
            rows: lookup.predicate_with(
                template
                    .positive()
                    .at(0)
                    .expect("nonempty positive body")
                    .predicate(),
                || self.work.tick(),
            )?,
            trail_start: 0,
        });
        while !frames.is_empty() {
            self.work.tick()?;
            let depth = frames.len() - 1;
            let frame = &mut frames[depth];
            while trail.len() > frame.trail_start {
                self.work.tick()?;
                let variable = trail
                    .pop()
                    .ok_or_else(|| self.work.error(Kind::UnboundVariable))?;
                binding[variable] = None;
            }
            let Some(row) = frame.rows.next() else {
                frames.pop();
                continue;
            };
            let atom_index = row.position();
            if !matches(
                &mut self.work,
                template.positive().at(depth).expect("join depth in body"),
                row.atom(),
                &mut binding,
                &mut trail,
            )? {
                continue;
            }
            chosen[depth] = atom_index;
            if depth + 1 == template.positive().len() {
                active(
                    &mut self.work,
                    &mut self.nodes,
                    &mut self.keys,
                    self.truth,
                    template,
                    &binding,
                    &chosen,
                )?;
            } else {
                frames.push(Frame {
                    rows: lookup.predicate_with(
                        template
                            .positive()
                            .at(depth + 1)
                            .expect("next join depth in body")
                            .predicate(),
                        || self.work.tick(),
                    )?,
                    trail_start: trail.len(),
                });
            }
        }
        Ok(())
    }
    fn active(
        &mut self,
        template: ObjectiveTemplateRef<'a>,
        binding: &[Option<TermRef<'a>>],
        chosen: &[usize],
    ) -> Result<(), ObjectiveBoundError> {
        active(
            &mut self.work,
            &mut self.nodes,
            &mut self.keys,
            self.truth,
            template,
            binding,
            chosen,
        )
    }
    fn variables(
        &mut self,
        template: ObjectiveTemplateRef<'_>,
    ) -> Result<usize, ObjectiveBoundError> {
        let mut count = 0;
        for index in 0..template.positive().len() {
            self.work.tick()?;
            let pattern = template.positive().at(index).expect("bounded body pattern");
            for column in 0..pattern.terms().len() {
                self.work.tick()?;
                if let TemplateTerm::Variable(variable) =
                    pattern.terms().at(column).expect("bounded pattern column")
                {
                    count = count.max(
                        variable
                            .checked_add(1)
                            .ok_or_else(|| self.work.error(Kind::Overflow))?,
                    );
                }
            }
        }
        if count > self.work.limits.max_variables {
            return Err(self.work.limit(Resource::Variables));
        }
        Ok(count)
    }

    fn condition(&mut self, condition: &Condition) -> Result<Option<usize>, ObjectiveBoundError> {
        let mut nodes: Vec<usize> = self.work.reserve(condition.nodes().len())?;
        for index in 0..condition.nodes().len() {
            self.work.tick()?;
            let operation = condition
                .nodes()
                .at(index)
                .expect("bounded condition operation");
            let node = match operation {
                ConditionNode::Boolean(true) => self.truth,
                ConditionNode::Boolean(false) => self.work.node(&mut self.nodes, Node::False)?,
                ConditionNode::Atom(atom) => self.condition_atom(atom)?,
                ConditionNode::Not(operand) => {
                    let falsum = self.work.node(&mut self.nodes, Node::False)?;
                    self.work
                        .node(&mut self.nodes, Node::Implies(nodes[operand], falsum))?
                }
                ConditionNode::And(left, right) => self
                    .work
                    .node(&mut self.nodes, Node::And(nodes[left], nodes[right]))?,
                ConditionNode::Or(left, right) => self
                    .work
                    .node(&mut self.nodes, Node::Or(nodes[left], nodes[right]))?,
            };
            nodes.push(node);
        }
        Ok(nodes.last().copied())
    }

    fn condition_atom(&mut self, query: AtomRef<'_>) -> Result<usize, ObjectiveBoundError> {
        if let Some(row) = self.index.lookup().get_with(query, || self.work.tick())? {
            return Ok(row.position());
        }
        // Atoms outside the caller's complete catalog are false in every
        // represented candidate. Querying one must not enlarge that catalog.
        self.work.node(&mut self.nodes, Node::False)
    }
}

fn matches<'a>(
    work: &mut Work<'_>,
    pattern: PatternRef<'_>,
    atom: AtomRef<'a>,
    binding: &mut [Option<TermRef<'a>>],
    trail: &mut Vec<usize>,
) -> Result<bool, ObjectiveBoundError> {
    for column in 0..pattern.terms().len() {
        work.tick()?;
        let term = pattern.terms().at(column).expect("bounded pattern column");
        let value = atom.values().at(column).expect("matched predicate arity");
        match term {
            TemplateTerm::Constant(constant)
                if !constant.equals_ref_with(value, || work.tick())? =>
            {
                return Ok(false);
            }
            TemplateTerm::Variable(variable) => {
                if let Some(previous) = binding[variable] {
                    if !previous.equals_ref_with(value, || work.tick())? {
                        return Ok(false);
                    }
                } else {
                    binding[variable] = Some(value);
                    trail.push(variable);
                }
            }
            TemplateTerm::Constant(_) => {}
        }
    }
    Ok(true)
}

fn resolve<'a>(
    work: &mut Work<'_>,
    term: TemplateTerm<'a>,
    binding: &[Option<TermRef<'a>>],
) -> Result<TermRef<'a>, ObjectiveBoundError> {
    work.tick()?;
    match term {
        TemplateTerm::Constant(value) => Ok(value),
        TemplateTerm::Variable(variable) => binding
            .get(variable)
            .copied()
            .flatten()
            .ok_or_else(|| work.error(Kind::UnboundVariable)),
    }
}
// Canonical typed identity order for contribution-key storage.
// This is not ASP term order; use the shared checked comparator, not Value::Ord
// behind a conservative payload-size estimate.
fn compare_identity(
    work: &mut Work<'_>,
    left: TermRef<'_>,
    right: TermRef<'_>,
) -> Result<Ordering, ObjectiveBoundError> {
    left.compare_ref_with(right, || work.tick())
}

fn active<'a>(
    work: &mut Work<'_>,
    nodes: &mut Vec<Node>,
    keys: &mut Vec<Key<'a>>,
    truth: usize,
    template: ObjectiveTemplateRef<'a>,
    binding: &[Option<TermRef<'a>>],
    chosen: &[usize],
) -> Result<(), ObjectiveBoundError> {
    work.tick()?;
    if work.statistics.bindings >= work.limits.max_bindings {
        return Err(work.limit(Resource::Bindings));
    }
    work.statistics.bindings += 1;
    for index in 0..template.filters().len() {
        work.tick()?;
        let filter = template.filters().at(index).expect("bounded filter index");
        let (left, right) = filter.terms();
        let left = resolve(work, left, binding)?;
        let right = resolve(work, right, binding)?;
        if left.equals_ref_with(right, || work.tick())? != filter.is_equality() {
            return Ok(());
        }
    }
    let weight = resolve(work, template.weight(), binding)?;
    work.tick()?;
    let ValueNodeRef::Number(weight) = weight.descriptor() else {
        return Ok(());
    };
    let weight = template
        .weight_polarity()
        .normalize(weight)
        .ok_or_else(|| work.error(Kind::WeightNormalizationOverflow))?;
    let mut tuple = work.reserve(template.tuple().len())?;
    for index in 0..template.tuple().len() {
        work.tick()?;
        let term = template.tuple().at(index).expect("bounded tuple field");
        tuple.push(resolve(work, term, binding)?);
    }
    let mut condition = truth;
    for atom in chosen {
        condition = if condition == truth {
            *atom
        } else {
            work.node(nodes, Node::And(condition, *atom))?
        };
    }
    contribute(
        work,
        nodes,
        keys,
        Key {
            priority: template.priority(),
            weight,
            tuple,
            condition,
        },
    )
}

fn key_order(
    work: &mut Work<'_>,
    left: &Key<'_>,
    right: &Key<'_>,
) -> Result<Ordering, ObjectiveBoundError> {
    work.tick()?;
    let prefix = left
        .priority
        .cmp(&right.priority)
        .then(left.weight.cmp(&right.weight));
    if prefix != Ordering::Equal {
        return Ok(prefix);
    }
    for (left, right) in left.tuple.iter().zip(&right.tuple) {
        let order = compare_identity(work, *left, *right)?;
        if order != Ordering::Equal {
            return Ok(order);
        }
    }
    Ok(left.tuple.len().cmp(&right.tuple.len()))
}
fn contribute<'a>(
    work: &mut Work<'_>,
    nodes: &mut Vec<Node>,
    keys: &mut Vec<Key<'a>>,
    key: Key<'a>,
) -> Result<(), ObjectiveBoundError> {
    let mut low = 0;
    let mut high = keys.len();
    while low < high {
        let middle = low + (high - low) / 2;
        match key_order(work, &keys[middle], &key)? {
            Ordering::Less => low = middle + 1,
            Ordering::Greater => high = middle,
            Ordering::Equal => {
                if keys[middle].condition != key.condition {
                    keys[middle].condition =
                        work.node(nodes, Node::Or(keys[middle].condition, key.condition))?;
                }
                return Ok(());
            }
        }
    }
    if keys.len() >= work.limits.max_keys {
        return Err(work.limit(Resource::Keys));
    }
    let mut bytes: usize = 16;
    for value in &key.tuple {
        let length = value.canonical_bytes_with(|| work.tick())?;
        bytes = bytes
            .checked_add(length)
            .ok_or_else(|| work.error(Kind::Overflow))?;
    }
    let bytes = work
        .statistics
        .key_bytes
        .checked_add(bytes)
        .ok_or_else(|| work.error(Kind::Overflow))?;
    if bytes > work.limits.max_key_bytes {
        return Err(work.limit(Resource::KeyBytes));
    }
    for _ in low..keys.len() {
        work.tick()?;
    }
    keys.try_reserve(1)
        .map_err(|_| work.error(Kind::Allocation))?;
    keys.insert(low, key);
    work.statistics.keys = keys.len();
    work.statistics.key_bytes = bytes;
    Ok(())
}
