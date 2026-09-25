//! Typed wildcard keys. Absence of an argument value is pattern metadata, never
//! a fabricated ASP scalar. Ground subpatterns normalize to canonical terms.

use super::{AtomRef, Binding, Error, ErrorKind, Interpreter, Metric, Resource, Work};
use crate::observation::{AtomKeyTemplate, KeyTemplate};
use std::{cmp::Ordering, ops::Range};
use zetesis_core::{
    ValueNodeRef,
    catalog::{PredicateRef, TermAssignment, TermKey, TermRead},
};

#[derive(Clone, Copy)]
pub(super) struct Key(usize);
#[derive(Clone)]
enum Node {
    Any,
    Ground(usize),
    Construct(crate::metadata::Constructor, Range<usize>),
}
struct Atom<'a> {
    predicate: PredicateRef<'a>,
    children: Range<usize>,
    metric: Metric,
}
/// Payload-free key topology; ground leaves share one derived-term witness.
pub(super) struct Keys<'a> {
    nodes: Vec<Node>,
    edges: Vec<usize>,
    atoms: Vec<Atom<'a>>,
    terms: TermAssignment,
    peak: u128,
}
impl Keys<'_> {
    pub fn new(read: TermRead<'_>) -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            atoms: Vec::new(),
            terms: read.assignment(),
            peak: 0,
        }
    }
    pub fn bytes(&self) -> u128 {
        (size_of::<Self>() - size_of::<TermAssignment>()) as u128
            + self.nodes.capacity() as u128 * size_of::<Node>() as u128
            + self.edges.capacity() as u128 * size_of::<usize>() as u128
            + self.atoms.capacity() as u128 * size_of::<Atom<'_>>() as u128
            + self.terms.retained_bytes() as u128
    }
    pub fn peak(&self) -> u128 {
        self.peak
    }
    fn reserve(
        &mut self,
        nodes: usize,
        edges: usize,
        atoms: usize,
        arena: u128,
        work: &mut Work<'_>,
    ) -> Result<(), Error> {
        let mut current = self.bytes();
        reserve(
            &mut self.nodes,
            nodes,
            &mut current,
            &mut self.peak,
            arena,
            work,
        )?;
        reserve(
            &mut self.edges,
            edges,
            &mut current,
            &mut self.peak,
            arena,
            work,
        )?;
        reserve(
            &mut self.atoms,
            atoms,
            &mut current,
            &mut self.peak,
            arena,
            work,
        )
    }
}
fn reserve<T>(
    values: &mut Vec<T>,
    extra: usize,
    current: &mut u128,
    peak: &mut u128,
    arena: u128,
    work: &mut Work<'_>,
) -> Result<(), Error> {
    let count = values
        .len()
        .checked_add(extra)
        .ok_or_else(|| work.error(ErrorKind::Allocation))?;
    if count <= values.capacity() {
        return Ok(());
    }
    let required = *current + count as u128 * size_of::<T>() as u128;
    work.check(
        Resource::TermStorageBytes,
        arena + required,
        work.limits.max_term_storage_bytes as u128,
    )?;
    work.step(values.len() as u128 + 1)?;
    let mut next = work.reserve(count)?;
    let actual = *current + next.capacity() as u128 * size_of::<T>() as u128;
    *peak = (*peak).max(arena + actual);
    work.check(
        Resource::TermStorageBytes,
        arena + actual,
        work.limits.max_term_storage_bytes as u128,
    )?;
    *current = actual - values.capacity() as u128 * size_of::<T>() as u128;
    next.append(values);
    *values = next;
    Ok(())
}
fn node(value: Node, ctx: &mut Interpreter<'_, '_, '_>) -> Result<usize, Error> {
    ctx.keys
        .reserve(1, 0, 0, ctx.terms.storage_bytes(), ctx.work)?;
    let index = ctx.keys.nodes.len();
    ctx.keys.nodes.push(value);
    ctx.observe();
    Ok(index)
}
fn ground(key: &TermKey, ctx: &mut Interpreter<'_, '_, '_>) -> Result<usize, Error> {
    let slot = ctx.keys.terms.len();
    let external =
        ctx.terms.storage_bytes() + ctx.keys.bytes() - ctx.keys.terms.retained_bytes() as u128;
    let remaining = (ctx.work.limits.max_term_storage_bytes as u128)
        .checked_sub(external)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| {
            ctx.work.error(ErrorKind::Limit {
                resource: Resource::TermStorageBytes,
                observed: external,
                limit: ctx.work.limits.max_term_storage_bytes as u128,
            })
        })?;
    let before = ctx.keys.terms.retained_bytes();
    let resized = ctx
        .keys
        .terms
        .resize_with(slot + 1, remaining, || ctx.work.step(1));
    if ctx.keys.terms.retained_bytes() > before {
        let overlap = ctx.terms.storage_bytes()
            + ctx.keys.bytes()
            + (before - size_of::<TermAssignment>()) as u128;
        ctx.keys.peak = ctx.keys.peak.max(overlap);
    }
    ctx.observe();
    resized.map_err(|error| match error {
        zetesis_core::catalog::AssignmentFailure::Assignment(
            zetesis_core::catalog::AssignmentError::Storage(
                zetesis_core::catalog::Error::Storage { required, .. },
            ),
        ) => ctx.work.error(ErrorKind::Limit {
            resource: Resource::TermStorageBytes,
            observed: external + required,
            limit: ctx.work.limits.max_term_storage_bytes as u128,
        }),
        error => super::binding::failure(error, ctx.work),
    })?;
    ctx.keys
        .terms
        .set_with(slot, key, || ctx.work.step(1))
        .map_err(|error| super::binding::failure(error, ctx.work))?;
    node(Node::Ground(slot), ctx)
}
fn edges(children: &[usize], ctx: &mut Interpreter<'_, '_, '_>) -> Result<Range<usize>, Error> {
    ctx.keys
        .reserve(0, children.len(), 0, ctx.terms.storage_bytes(), ctx.work)?;
    ctx.work.step(children.len() as u128)?;
    let start = ctx.keys.edges.len();
    ctx.keys.edges.extend_from_slice(children);
    ctx.observe();
    Ok(start..ctx.keys.edges.len())
}
fn shape(
    constructor: crate::metadata::Constructor,
    children: &[usize],
    ctx: &mut Interpreter<'_, '_, '_>,
) -> Result<usize, Error> {
    let mut frame = ctx.frame(children.len())?;
    let mut slots = ctx.work.reserve(children.len())?;
    let mut complete = true;
    for (index, &child) in children.iter().enumerate() {
        if let Node::Ground(slot) = ctx.keys.nodes[child] {
            let key = ctx
                .keys
                .terms
                .key(slot)
                .expect("key slot extent")
                .expect("ground slot");
            ctx.set(&mut frame, index, &key)?;
            slots.push(index);
        } else {
            complete = false;
            break;
        }
    }
    if complete {
        let declaration = ctx
            .metadata
            .constructor_declaration(constructor)
            .expect("compiled shape");
        let key = ctx.build(&declaration, frame.as_slice(), &slots)?;
        ground(&key, ctx)
    } else {
        let children = edges(children, ctx)?;
        node(Node::Construct(constructor, children), ctx)
    }
}
fn measure_node(
    index: usize,
    depth: usize,
    metric: &mut Metric,
    ctx: &mut Interpreter<'_, '_, '_>,
) -> Result<(), Error> {
    ctx.work.step(1)?;
    ctx.work.check(
        Resource::Depth,
        depth as u128,
        ctx.work.limits.max_symbol_depth as u128,
    )?;
    match ctx.keys.nodes[index].clone() {
        Node::Any => ctx.work.node(metric),
        Node::Ground(slot) => {
            let key = ctx
                .keys
                .terms
                .key(slot)
                .expect("ground key")
                .expect("present key");
            ctx.measure_key(&key, depth, metric)
        }
        Node::Construct(shape, children) => {
            ctx.work.node(metric)?;
            ctx.work.payload(
                ctx.metadata.constructor(shape).expect("shape").text_bytes(),
                metric,
            )?;
            for position in children {
                measure_node(ctx.keys.edges[position], depth + 1, metric, ctx)?;
            }
            Ok(())
        }
    }
}
fn atom_key<'input>(
    predicate: PredicateRef<'input>,
    children: &[usize],
    ctx: &mut Interpreter<'input, '_, '_>,
) -> Result<(Key, Metric), Error> {
    let mut metric = Metric {
        nodes: 1,
        bytes: predicate.name().len(),
    };
    ctx.work
        .check(Resource::Nodes, 1, ctx.work.limits.max_symbol_nodes as u128)?;
    ctx.work
        .check(Resource::Depth, 1, ctx.work.limits.max_symbol_depth as u128)?;
    ctx.work.check(
        Resource::Bytes,
        metric.bytes as u128,
        ctx.work.limits.max_symbol_bytes as u128,
    )?;
    for &child in children {
        measure_node(child, 2, &mut metric, ctx)?;
    }
    ctx.work.construction_check(metric)?;
    let children = edges(children, ctx)?;
    ctx.keys
        .reserve(0, 0, 1, ctx.terms.storage_bytes(), ctx.work)?;
    let key = Key(ctx.keys.atoms.len());
    ctx.keys.atoms.push(Atom {
        predicate,
        children,
        metric,
    });
    ctx.observe();
    Ok((key, metric))
}
pub(super) fn capture<'input>(
    atom: AtomRef<'input>,
    ctx: &mut Interpreter<'input, '_, '_>,
) -> Result<(Key, Metric), Error> {
    let mut children = ctx.work.reserve(atom.values().len())?;
    for value in atom.values() {
        let key = ctx.input(value)?;
        children.push(ground(&key, ctx)?);
    }
    atom_key(atom.predicate(), &children, ctx)
}
pub(super) struct Alternatives {
    pub values: Vec<(Key, Metric)>,
    pub bytes: u128,
}
fn product<'input, T>(
    choices: &[Vec<usize>],
    ctx: &mut Interpreter<'input, '_, '_>,
    mut action: impl FnMut(&[usize], &mut Interpreter<'input, '_, '_>) -> Result<T, Error>,
) -> Result<Vec<T>, Error> {
    let mut output = Vec::new();
    if choices.iter().any(Vec::is_empty) {
        return Ok(output);
    }
    let mut indices = ctx.work.reserve(choices.len())?;
    indices.resize(choices.len(), 0);
    let mut selected = ctx.work.reserve(choices.len())?;
    loop {
        ctx.work.step(1 + choices.len() as u128)?;
        selected.clear();
        selected.extend(
            choices
                .iter()
                .zip(&indices)
                .map(|(values, &index)| values[index]),
        );
        output
            .try_reserve(1)
            .map_err(|_| ctx.work.error(ErrorKind::Allocation))?;
        output.push(action(&selected, ctx)?);
        let mut position = indices.len();
        loop {
            if position == 0 {
                return Ok(output);
            }
            position -= 1;
            indices[position] += 1;
            if indices[position] < choices[position].len() {
                break;
            }
            indices[position] = 0;
        }
    }
}
fn alternatives<'input>(
    template: &KeyTemplate,
    binding: &Binding<'input>,
    ctx: &mut Interpreter<'input, '_, '_>,
) -> Result<Vec<usize>, Error> {
    ctx.work.step(1)?;
    match template {
        KeyTemplate::Any => {
            let value = node(Node::Any, ctx)?;
            let mut out = ctx.work.reserve(1)?;
            out.push(value);
            Ok(out)
        }
        KeyTemplate::Value(value) => {
            let mut output = Vec::new();
            super::values::each(value, binding, ctx, |key, _, ctx| {
                output
                    .try_reserve(1)
                    .map_err(|_| ctx.work.error(ErrorKind::Allocation))?;
                output.push(ground(&key, ctx)?);
                Ok(())
            })?;
            Ok(output)
        }
        KeyTemplate::Pool(items) => {
            let mut output = Vec::new();
            for item in items {
                let mut values = alternatives(item, binding, ctx)?;
                ctx.work.step(values.len() as u128)?;
                output
                    .try_reserve(values.len())
                    .map_err(|_| ctx.work.error(ErrorKind::Allocation))?;
                output.append(&mut values);
            }
            Ok(output)
        }
        KeyTemplate::Construct(constructor, items) => {
            let mut children = ctx.work.reserve(items.len())?;
            for item in items {
                children.push(alternatives(item, binding, ctx)?);
            }
            product(&children, ctx, |children, ctx| {
                shape(*constructor, children, ctx)
            })
        }
    }
}
pub(super) fn collect<'input>(
    template: &AtomKeyTemplate,
    binding: &Binding<'input>,
    ctx: &mut Interpreter<'input, '_, '_>,
) -> Result<Alternatives, Error> {
    let mut choices = ctx.work.reserve(template.arguments.len())?;
    for argument in &template.arguments {
        choices.push(alternatives(argument, binding, ctx)?);
    }
    let predicate = ctx
        .metadata
        .predicate(template.predicate)
        .expect("compiled predicate");
    let mut bytes = 0;
    let result = product(&choices, ctx, |children, ctx| {
        let (key, metric) = atom_key(predicate, children, ctx)?;
        ctx.work.check(
            Resource::LocalBytes,
            ctx.work.local_bytes + metric.payload(),
            ctx.work.limits.max_local_bytes as u128,
        )?;
        ctx.work.local_bytes += metric.payload();
        bytes += metric.payload();
        Ok((key, metric))
    });
    match result {
        Ok(values) => Ok(Alternatives { values, bytes }),
        Err(error) => {
            ctx.work.local_bytes -= bytes;
            Err(error)
        }
    }
}
pub(super) fn predicate<'input>(
    key: Key,
    ctx: &Interpreter<'input, '_, '_>,
) -> PredicateRef<'input> {
    ctx.keys.atoms[key.0].predicate
}
pub(super) fn metric(key: Key, ctx: &Interpreter<'_, '_, '_>) -> Metric {
    ctx.keys.atoms[key.0].metric
}
fn matches(
    index: usize,
    value: super::TermRef<'_>,
    ctx: &mut Interpreter<'_, '_, '_>,
) -> Result<bool, Error> {
    ctx.work.step(1)?;
    match ctx.keys.nodes[index].clone() {
        Node::Any => Ok(true),
        Node::Ground(slot) => {
            let expected = ctx
                .keys
                .terms
                .as_slice()
                .term_with(ctx.terms.read(), slot, || ctx.work.step(1))
                .map_err(|error| super::binding::failure(error, ctx.work))?
                .expect("ground key");
            expected
                .compare_ref_with(value, || ctx.work.step(1))
                .map(Ordering::is_eq)
        }
        Node::Construct(shape, children) => {
            let expected = ctx.metadata.constructor(shape).expect("compiled shape");
            let actual = value.descriptor();
            ctx.work
                .step((expected.text_bytes() + actual.text_bytes()) as u128 + 1)?;
            if !same_shape(expected, actual) {
                return Ok(false);
            }
            for (index, position) in children.enumerate() {
                if !matches(
                    ctx.keys.edges[position],
                    value.child(index).expect("checked shape"),
                    ctx,
                )? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }
}
fn same_shape(left: ValueNodeRef<'_>, right: ValueNodeRef<'_>) -> bool {
    match (left, right) {
        (ValueNodeRef::Tuple { arity: a }, ValueNodeRef::Tuple { arity: b }) => a == b,
        (
            ValueNodeRef::Function {
                name: a,
                sign: sa,
                arity: aa,
            },
            ValueNodeRef::Function {
                name: b,
                sign: sb,
                arity: ab,
            },
        ) => a == b && sa == sb && aa == ab,
        (
            ValueNodeRef::Function {
                name,
                sign: zetesis_core::Sign::Positive,
                arity: 0,
            },
            ValueNodeRef::Symbol(actual),
        ) => name == actual,
        _ => false,
    }
}
pub(super) fn atom(
    key: Key,
    atom: AtomRef<'_>,
    ctx: &mut Interpreter<'_, '_, '_>,
) -> Result<bool, Error> {
    let children = ctx.keys.atoms[key.0].children.clone();
    for (position, value) in children.zip(atom.values()) {
        if !matches(ctx.keys.edges[position], value, ctx)? {
            return Ok(false);
        }
    }
    Ok(true)
}
fn compare_nodes(
    left: usize,
    right: usize,
    ctx: &mut Interpreter<'_, '_, '_>,
) -> Result<Ordering, Error> {
    ctx.work.step(1)?;
    let left = ctx.keys.nodes[left].clone();
    let right = ctx.keys.nodes[right].clone();
    match (left, right) {
        (Node::Any, Node::Any) => Ok(Ordering::Equal),
        (Node::Any, _) => Ok(Ordering::Less),
        (_, Node::Any) => Ok(Ordering::Greater),
        (Node::Ground(a), Node::Ground(b)) => {
            let a = ctx.keys.terms.key(a).expect("slot").expect("ground");
            let b = ctx.keys.terms.key(b).expect("slot").expect("ground");
            ctx.compare(&a, &b)
        }
        (Node::Ground(_), _) => Ok(Ordering::Less),
        (_, Node::Ground(_)) => Ok(Ordering::Greater),
        (Node::Construct(a, ca), Node::Construct(b, cb)) => {
            let a = ctx.metadata.constructor(a).expect("shape");
            let b = ctx.metadata.constructor(b).expect("shape");
            ctx.work
                .step((a.text_bytes() + b.text_bytes()) as u128 + 1)?;
            let descriptor = shape_order(a, b);
            if !descriptor.is_eq() {
                return Ok(descriptor);
            }
            for (a, b) in ca.zip(cb) {
                let order = compare_nodes(ctx.keys.edges[a], ctx.keys.edges[b], ctx)?;
                if !order.is_eq() {
                    return Ok(order);
                }
            }
            Ok(Ordering::Equal)
        }
    }
}
fn shape_order(left: ValueNodeRef<'_>, right: ValueNodeRef<'_>) -> Ordering {
    let key = |shape| match shape {
        ValueNodeRef::Tuple { arity } => (0, arity, zetesis_core::Sign::Positive, ""),
        ValueNodeRef::Function { name, sign, arity } => (1, arity, sign, name),
        _ => unreachable!("compiled constructor shape"),
    };
    key(left).cmp(&key(right))
}
pub(super) fn compare(
    left: Key,
    right: Key,
    ctx: &mut Interpreter<'_, '_, '_>,
) -> Result<Ordering, Error> {
    let a = &ctx.keys.atoms[left.0];
    let b = &ctx.keys.atoms[right.0];
    let predicate = a
        .predicate
        .compare_ref_with(b.predicate, || ctx.work.step(1))?;
    if !predicate.is_eq() {
        return Ok(predicate);
    }
    for (a, b) in a.children.clone().zip(b.children.clone()) {
        let order = compare_nodes(ctx.keys.edges[a], ctx.keys.edges[b], ctx)?;
        if !order.is_eq() {
            return Ok(order);
        }
    }
    Ok(Ordering::Equal)
}
