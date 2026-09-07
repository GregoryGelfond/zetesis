use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use zetesis_core::{Atom, AtomPattern, Filter, Predicate, Term, Value};
use zetesis_cpu::Control;
use zetesis_ferraris::{AggregateElement, Node, Theory};
use zetesis_objective::{ObjectiveProgram, ObjectiveTemplate};

use super::{
    ObjectiveBoundError, ObjectiveBoundErrorKind as Kind, ObjectiveBoundResource as Resource,
    ObjectiveBoundStatistics, ObjectivePlan, ObjectivePlanLimits, Work,
};

struct Key<'a> {
    priority: i32,
    weight: i32,
    tuple: Vec<&'a Value>,
    condition: usize,
}
struct Compiler<'a> {
    work: Work<'a>,
    atoms: &'a [Atom],
    relations: BTreeMap<&'a Predicate, Vec<usize>>,
    nodes: Vec<Node>,
    keys: Vec<Key<'a>>,
    truth: usize,
}

pub(super) fn compile(
    original: &Theory,
    atoms: &[Atom],
    objectives: &ObjectiveProgram,
    limits: ObjectivePlanLimits,
    control: &Control,
) -> Result<ObjectivePlan, ObjectiveBoundError> {
    let mut work = Work {
        control,
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
    let mut relations: BTreeMap<&Predicate, Vec<usize>> = BTreeMap::new();
    let mut unique = BTreeSet::new();
    for (index, atom) in atoms.iter().enumerate() {
        work.tick()?;
        catalog_work(&mut work, atom, atoms.len())?;
        if !unique.insert(atom) {
            return Err(work.error(Kind::AtomCatalog));
        }
        let row = relations.entry(atom.predicate()).or_default();
        row.try_reserve(1)
            .map_err(|_| work.error(Kind::Allocation))?;
        row.push(index);
        work.node(&mut nodes, Node::Atom(index))?;
    }
    let falsum = work.node(&mut nodes, Node::False)?;
    let truth = work.node(&mut nodes, Node::Implies(falsum, falsum))?;
    let mut compiler = Compiler {
        work,
        atoms,
        relations,
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
        nodes: compiler.nodes,
        levels,
        statistics: compiler.work.statistics,
    })
}

struct Frame<'a> {
    rows: &'a [usize],
    position: usize,
    trail_start: usize,
}

impl<'a> Compiler<'a> {
    fn join(&mut self, template: &'a ObjectiveTemplate) -> Result<(), ObjectiveBoundError> {
        if template.positive().len() > self.work.limits.max_body_atoms {
            return Err(self.work.limit(Resource::BodyAtoms));
        }
        if template.tuple().len() > self.work.limits.max_tuple_width {
            return Err(self.work.limit(Resource::TupleWidth));
        }
        for pattern in template.positive() {
            let depth = 2 * (usize::BITS - self.relations.len().leading_zeros()) + 1;
            let amount = u64::try_from(pattern.predicate().name().len())
                .ok()
                .and_then(|value| value.checked_mul(u64::from(depth)))
                .ok_or_else(|| self.work.error(Kind::Overflow))?;
            self.work.charge(amount)?;
        }
        let count = self.variables(template)?;
        let mut binding = self.work.reserve(count)?;
        binding.resize(count, None);
        let mut trail = self.work.reserve(count)?;
        if template.positive().is_empty() {
            return self.active(template, &binding, &[]);
        }
        let mut frames = self.work.reserve(template.positive().len())?;
        let mut chosen = self.work.reserve(template.positive().len())?;
        chosen.resize(template.positive().len(), 0);
        let relations = &self.relations;
        frames.push(Frame {
            rows: relations
                .get(template.positive()[0].predicate())
                .map_or(&[], Vec::as_slice),
            position: 0,
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
            let Some(&atom_index) = frame.rows.get(frame.position) else {
                frames.pop();
                continue;
            };
            frame.position += 1;
            if !matches(
                &mut self.work,
                &template.positive()[depth],
                &self.atoms[atom_index],
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
                    rows: relations
                        .get(template.positive()[depth + 1].predicate())
                        .map_or(&[], Vec::as_slice),
                    position: 0,
                    trail_start: trail.len(),
                });
            }
        }
        Ok(())
    }
    fn active(
        &mut self,
        template: &'a ObjectiveTemplate,
        binding: &[Option<&'a Value>],
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
    fn variables(&mut self, template: &ObjectiveTemplate) -> Result<usize, ObjectiveBoundError> {
        let mut count = 0;
        for term in template.positive().iter().flat_map(AtomPattern::terms) {
            self.work.tick()?;
            if let Term::Variable(variable) = term {
                count = count.max(
                    variable
                        .checked_add(1)
                        .ok_or_else(|| self.work.error(Kind::Overflow))?,
                );
            }
        }
        if count > self.work.limits.max_variables {
            return Err(self.work.limit(Resource::Variables));
        }
        Ok(count)
    }
}

fn matches<'a>(
    work: &mut Work<'_>,
    pattern: &AtomPattern,
    atom: &'a Atom,
    binding: &mut [Option<&'a Value>],
    trail: &mut Vec<usize>,
) -> Result<bool, ObjectiveBoundError> {
    for (term, value) in pattern.terms().iter().zip(atom.values()) {
        work.tick()?;
        match term {
            Term::Constant(constant) if compare(work, constant, value)? != Ordering::Equal => {
                return Ok(false);
            }
            Term::Variable(variable) => {
                if let Some(previous) = binding[*variable] {
                    if compare(work, previous, value)? != Ordering::Equal {
                        return Ok(false);
                    }
                } else {
                    binding[*variable] = Some(value);
                    trail.push(*variable);
                }
            }
            Term::Constant(_) => {}
        }
    }
    Ok(true)
}

fn resolve<'a>(
    work: &mut Work<'_>,
    term: &'a Term,
    binding: &[Option<&'a Value>],
) -> Result<&'a Value, ObjectiveBoundError> {
    work.tick()?;
    match term {
        Term::Constant(value) => Ok(value),
        Term::Variable(variable) => binding
            .get(*variable)
            .copied()
            .flatten()
            .ok_or_else(|| work.error(Kind::UnboundVariable)),
    }
}
fn compare(
    work: &mut Work<'_>,
    left: &Value,
    right: &Value,
) -> Result<Ordering, ObjectiveBoundError> {
    work.tick()?;
    if let (Value::String(a), Value::String(b)) | (Value::Symbol(a), Value::Symbol(b)) =
        (left, right)
    {
        for (a, b) in a.bytes().zip(b.bytes()) {
            work.tick()?;
            let order = a.cmp(&b);
            if order != Ordering::Equal {
                return Ok(order);
            }
        }
        return Ok(a.len().cmp(&b.len()));
    }
    for value in [left, right] {
        if let Value::Structured(value) = value {
            for _ in 0..value.payload_bytes() {
                work.tick()?;
            }
        }
    }
    Ok(left.cmp(right))
}

fn active<'a>(
    work: &mut Work<'_>,
    nodes: &mut Vec<Node>,
    keys: &mut Vec<Key<'a>>,
    truth: usize,
    template: &'a ObjectiveTemplate,
    binding: &[Option<&'a Value>],
    chosen: &[usize],
) -> Result<(), ObjectiveBoundError> {
    work.tick()?;
    if work.statistics.bindings >= work.limits.max_bindings {
        return Err(work.limit(Resource::Bindings));
    }
    work.statistics.bindings += 1;
    for filter in template.filters() {
        let (left, right) = filter.terms();
        let left = resolve(work, left, binding)?;
        let right = resolve(work, right, binding)?;
        if (compare(work, left, right)? == Ordering::Equal) != matches!(filter, Filter::Eq(..)) {
            return Ok(());
        }
    }
    let Value::Number(weight) = resolve(work, template.weight(), binding)? else {
        return Ok(());
    };
    let weight = template
        .weight_polarity()
        .normalize(*weight)
        .ok_or_else(|| work.error(Kind::WeightNormalizationOverflow))?;
    let mut tuple = work.reserve(template.tuple().len())?;
    for term in template.tuple() {
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
        let order = compare(work, left, right)?;
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
        work.tick()?;
        let payload = match value {
            Value::Infimum | Value::Supremum => 0,
            Value::Number(_) => 4,
            Value::Structured(value) => value.canonical_bytes() - 1,
            Value::String(text) | Value::Symbol(text) => 8_usize
                .checked_add(text.len())
                .ok_or_else(|| work.error(Kind::Overflow))?,
        };
        bytes = bytes
            .checked_add(1)
            .and_then(|value| value.checked_add(payload))
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

fn catalog_work(work: &mut Work<'_>, atom: &Atom, count: usize) -> Result<(), ObjectiveBoundError> {
    // Conservative logical payload charge for ordered catalog/index lookups.
    // This is not an allocator or RSS estimate.
    let mut bytes = atom.predicate().name().len();
    for value in atom.values() {
        work.tick()?;
        let payload = match value {
            Value::String(text) | Value::Symbol(text) => text.len(),
            Value::Structured(value) => value.payload_bytes(),
            Value::Number(_) | Value::Infimum | Value::Supremum => 1,
        };
        bytes = bytes
            .checked_add(payload)
            .ok_or_else(|| work.error(Kind::Overflow))?;
    }
    let depth = 2 * (usize::BITS - count.leading_zeros()) + 1;
    let amount = u64::try_from(bytes)
        .ok()
        .and_then(|value| value.checked_mul(u64::from(depth)))
        .ok_or_else(|| work.error(Kind::Overflow))?;
    work.charge(amount)
}
