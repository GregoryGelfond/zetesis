//! One bounded bridge between upstream closed symbols and core flat values.

use themelios_program::symbol::{Name, Sign, Symbol};
use zetesis_core::{Value, ValueError, ValueLimits, ValueNode, ValueNodeRef, ValueResource};

pub(crate) fn from_symbol(symbol: &Symbol) -> Result<Value, ValueError> {
    traverse(symbol, ValueLimits::default(), Construction::default())
}

/// Check the logical value without constructing nodes or rendering output.
/// Actual node capacity and construction/render scratch remain the construction
/// sink's responsibility. Both sinks use this same typed Symbol walk.
pub(crate) fn validate_symbol(symbol: &Symbol) -> Result<(), ValueError> {
    traverse(symbol, ValueLimits::default(), Validation::default())
}

pub(crate) trait Sink {
    type Output;
    type Error: From<ValueError>;
    fn node(&mut self, node: ValueNodeRef<'_>) -> Result<(), Self::Error>;
    fn finish(self, bytes: u128, limits: ValueLimits) -> Result<Self::Output, Self::Error>;
    fn before(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn continues(&self) -> bool {
        true
    }
    fn start<'a>(&mut self, symbol: &'a Symbol) -> Result<Vec<(&'a Symbol, usize)>, Self::Error> {
        Ok(vec![(symbol, 1)])
    }
    fn reserve(
        &mut self,
        pending: &mut Vec<(&Symbol, usize)>,
        additional: usize,
    ) -> Result<(), Self::Error> {
        pending
            .try_reserve(additional)
            .map_err(|_| ValueError::Allocation.into())
    }
}

#[derive(Default)]
struct Construction {
    nodes: Vec<ValueNode>,
}

impl Sink for Construction {
    type Output = Value;
    type Error = ValueError;

    fn node(&mut self, node: ValueNodeRef<'_>) -> Result<(), ValueError> {
        self.nodes
            .try_reserve_exact(1)
            .map_err(|_| ValueError::Allocation)?;
        self.nodes.push(node.into_owned());
        Ok(())
    }

    fn finish(self, _bytes: u128, limits: ValueLimits) -> Result<Value, ValueError> {
        // Keep actual capacity, validation frames, rendering and final ownership
        // checks at the existing independent core constructor.
        Value::from_nodes(self.nodes, limits)
    }
}

#[derive(Default)]
struct Validation {
    spelling_bytes: u128,
}

impl Sink for Validation {
    type Output = ();
    type Error = ValueError;

    fn node(&mut self, node: ValueNodeRef<'_>) -> Result<(), ValueError> {
        self.spelling_bytes += node.rendered_bytes();
        Ok(())
    }

    fn finish(self, bytes: u128, limits: ValueLimits) -> Result<(), ValueError> {
        check(
            ValueResource::Bytes,
            bytes + self.spelling_bytes,
            limits.max_bytes,
        )
    }
}

fn check(resource: ValueResource, observed: u128, limit: usize) -> Result<(), ValueError> {
    if observed > limit as u128 {
        Err(ValueError::Limit {
            resource,
            observed,
            limit,
        })
    } else {
        Ok(())
    }
}

/// `pending` contains the untouched subtrees in reverse visit order; `nodes`
/// counts the completed preorder prefix. Each typed Symbol is visited once,
/// independently of the sink. Work is O(nodes + text), with O(frontier) borrowed
/// traversal storage in addition to the chosen sink's owned output.
pub(crate) fn traverse<S: Sink>(
    symbol: &Symbol,
    limits: ValueLimits,
    mut sink: S,
) -> Result<S::Output, S::Error> {
    let mut pending = sink.start(symbol)?;
    let mut nodes = 0_u128;
    let mut bytes = 0_u128;
    while !pending.is_empty() && sink.continues() {
        sink.before()?;
        let (symbol, depth) = pending.pop().expect("checked nonempty frontier");
        for (resource, observed, limit) in [
            (ValueResource::Nodes, nodes + 1, limits.max_nodes),
            (ValueResource::Depth, depth as u128, limits.max_depth),
        ] {
            check(resource, observed, limit)?;
        }
        let node = view(symbol);
        bytes += std::mem::size_of::<ValueNode>() as u128 + node.text_bytes() as u128;
        check(ValueResource::Bytes, bytes, limits.max_bytes)?;
        let children = symbol.arguments();
        check(
            ValueResource::Nodes,
            nodes + pending.len() as u128 + children.len() as u128 + 1,
            limits.max_nodes,
        )?;
        sink.reserve(&mut pending, children.len())?;
        pending.extend(children.iter().rev().map(|child| (child, depth + 1)));
        sink.node(node)?;
        nodes += 1;
    }
    sink.finish(bytes, limits)
}

pub(crate) fn view(symbol: &Symbol) -> ValueNodeRef<'_> {
    match symbol {
        Symbol::Infimum => ValueNodeRef::Infimum,
        Symbol::Supremum => ValueNodeRef::Supremum,
        Symbol::Number(number) => ValueNodeRef::Number(*number),
        Symbol::String(text) => ValueNodeRef::String(text),
        Symbol::Function {
            name,
            arguments,
            sign: Sign::Positive,
        } if arguments.is_empty() => ValueNodeRef::Symbol(name.as_str()),
        Symbol::Function {
            name,
            arguments,
            sign,
        } => ValueNodeRef::Function {
            name: name.as_str(),
            arity: arguments.len(),
            sign: crate::coherence::core_sign(*sign),
        },
        Symbol::Tuple(arguments) => ValueNodeRef::Tuple {
            arity: arguments.len(),
        },
    }
}

/// A typed refusal to construct a complete upstream symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BridgeError {
    /// Output or traversal capacity could not be represented or reserved.
    Allocation,
    /// A core name or structural shape cannot be represented by themelios.
    InvalidName,
    /// Named construction capacities exceeded their inclusive allowance.
    Storage {
        /// Capacity required by the next operation, including retained output.
        required: u128,
        /// Inclusive construction allowance.
        limit: u128,
    },
}

/// A symbol export refusal or the caller's original control failure.
#[derive(Debug)]
pub enum BridgeFailure<E> {
    /// Symbol construction could not complete within its representation or storage.
    Bridge(BridgeError),
    /// The caller refused the next operation; its typed cause is unchanged.
    Stopped(E),
}
impl<E> From<BridgeError> for BridgeFailure<E> {
    fn from(error: BridgeError) -> Self {
        Self::Bridge(error)
    }
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Allocation => f.write_str("symbol construction storage could not be reserved"),
            Self::InvalidName => f.write_str("logical value cannot be represented as a symbol"),
            Self::Storage { required, limit } => {
                write!(
                    f,
                    "symbol construction requires {required} bytes, allowance is {limit}"
                )
            }
        }
    }
}

impl std::error::Error for BridgeError {}

impl<E: std::fmt::Display> std::fmt::Display for BridgeFailure<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bridge(error) => error.fmt(f),
            Self::Stopped(error) => error.fmt(f),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for BridgeFailure<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Bridge(error) => Some(error),
            Self::Stopped(error) => Some(error),
        }
    }
}

/// A constructor frame needs only its head, never a copied term description.
/// `None` is the anonymous tuple constructor, not an absent logical value.
struct Frame<'a> {
    name: Option<&'a str>,
    arguments: Vec<Symbol>,
    arity: usize,
    sign: Sign,
}
// At most N-1 unfinished parents and N output Symbol cells coexist. This
// layout relation is part of the advertised two-cells-per-node preflight.
const _: () = assert!(size_of::<Frame<'static>>() <= size_of::<Symbol>());

/// Allocation capacity retained by the conversion, including a root Symbol
/// cell. Completed child vectors remain live inside their parents, so their
/// capacities stay charged after a frame is popped. Temporary name-validation
/// text is checked separately while it overlaps with these owners.
struct ExportStorage {
    retained: u128,
    limit: u128,
}
impl ExportStorage {
    fn new(limit: u128) -> Result<Self, BridgeError> {
        let storage = Self {
            retained: size_of::<Symbol>() as u128,
            limit,
        };
        storage.check(0)?;
        Ok(storage)
    }
    fn check(&self, additional: u128) -> Result<(), BridgeError> {
        let required = self.retained + additional;
        if required > self.limit {
            Err(BridgeError::Storage {
                required,
                limit: self.limit,
            })
        } else {
            Ok(())
        }
    }
    fn retain(&mut self, actual: u128) -> Result<(), BridgeError> {
        self.check(actual)?;
        self.retained += actual;
        Ok(())
    }
    fn reserve<T, E>(
        &mut self,
        count: usize,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Vec<T>, BridgeFailure<E>> {
        self.check(count as u128 * size_of::<T>() as u128)?;
        before().map_err(BridgeFailure::Stopped)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| BridgeError::Allocation)?;
        // An allocator may provide more capacity than requested. Report that
        // actual envelope as storage refusal, never as allocation failure.
        self.retain(values.capacity() as u128 * size_of::<T>() as u128)?;
        Ok(values)
    }
    fn text<E>(
        &mut self,
        value: &str,
        name: bool,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<String, BridgeFailure<E>> {
        let validation = if name { value.len() as u128 } else { 0 };
        self.check(value.len() as u128 + validation)?;
        before().map_err(BridgeFailure::Stopped)?;
        let mut result = String::new();
        result
            .try_reserve_exact(value.len())
            .map_err(|_| BridgeError::Allocation)?;
        self.check(result.capacity() as u128 + validation)?;
        // Admit both copying and the name validator's independent text scan.
        for _ in 0..value.len() {
            before().map_err(BridgeFailure::Stopped)?;
        }
        if name {
            for _ in 0..value.len() {
                before().map_err(BridgeFailure::Stopped)?;
            }
        }
        self.retain(result.capacity() as u128)?;
        result.push_str(value);
        Ok(result)
    }
}

/// Reconstruct the upstream output directly from borrowed logical nodes.
/// The owned description already supplies an exact node/text preflight; its
/// payload is not included among the conversion's retained allocations.
pub(crate) fn to_symbol(value: &zetesis_core::StructuralValue) -> Result<Symbol, BridgeError> {
    let count = value.nodes().len();
    let text = value
        .nodes()
        .iter()
        .map(|node| node.view().text_bytes() as u128)
        .sum::<u128>();
    let limit = 2 * count as u128 * size_of::<Symbol>() as u128 + 2 * text;
    let mut storage = ExportStorage::new(limit)?;
    let mut nodes = value.nodes().iter();
    let result = from_nodes(
        count,
        value.depth(),
        &mut storage,
        |before| {
            before()?;
            Ok(nodes.next().map(ValueNode::view))
        },
        || Ok::<_, std::convert::Infallible>(()),
    );
    match result {
        Ok(symbol) => Ok(symbol),
        Err(BridgeFailure::Bridge(error)) => Err(error),
        Err(BridgeFailure::Stopped(never)) => match never {},
    }
}

/// Export a canonical term without an owned Value intermediate. The caller
/// chooses the construction allowance; the usual observation preflight is two
/// Symbol cells per expanded node plus twice its UTF-8 text. Actual frame,
/// child-vector and text capacities are checked before constructing output.
/// The callback admits canonical navigation, allocation, copying and assembly;
/// a stop drops every private partial Symbol and publishes no prefix.
pub(crate) fn term_symbol_with<E>(
    value: zetesis_core::catalog::TermRef<'_>,
    max_bytes: u128,
    mut before: impl FnMut() -> Result<(), E>,
) -> Result<Symbol, BridgeFailure<E>> {
    before().map_err(BridgeFailure::Stopped)?;
    let mut storage = ExportStorage::new(max_bytes)?;
    let mut nodes = value.nodes();
    from_nodes(
        value.expanded_nodes(),
        value
            .depth_with(&mut before)
            .map_err(BridgeFailure::Stopped)?,
        &mut storage,
        |before| nodes.next_with(before),
        before,
    )
}

/// Read one virtual predicate root followed by each borrowed argument's
/// preorder. The argument cursor only advances after the current term is
/// exhausted. The existing constructor stack therefore covers the whole atom
/// and accounts its output and scratch under one unchanged allowance.
pub(crate) fn atom_symbol_with<E>(
    atom: zetesis_core::catalog::AtomRef<'_>,
    max_bytes: u128,
    mut before: impl FnMut() -> Result<(), E>,
) -> Result<Symbol, BridgeFailure<E>> {
    before().map_err(BridgeFailure::Stopped)?;
    let mut storage = ExportStorage::new(max_bytes)?;
    let predicate = atom.predicate();
    before().map_err(BridgeFailure::Stopped)?;
    let name = predicate.name();
    before().map_err(BridgeFailure::Stopped)?;
    let sign = predicate.sign();
    before().map_err(BridgeFailure::Stopped)?;
    let mut arguments = atom.arguments().iter();
    let arity = arguments.len();
    let mut sizes = arguments.clone();
    let mut node_count = 1_usize;
    let mut depth = 1_usize;
    loop {
        before().map_err(BridgeFailure::Stopped)?;
        let Some(value) = sizes.next() else {
            break;
        };
        node_count = node_count
            .checked_add(value.expanded_nodes())
            .ok_or(BridgeError::Allocation)?;
        depth = depth.max(
            value
                .depth_with(&mut before)
                .map_err(BridgeFailure::Stopped)?
                .checked_add(1)
                .ok_or(BridgeError::Allocation)?,
        );
    }
    let mut root = Some(ValueNodeRef::Function { name, arity, sign });
    let mut nodes: Option<zetesis_core::catalog::TermNodes<'_>> = None;
    from_nodes(
        node_count,
        depth,
        &mut storage,
        |before| {
            if let Some(root) = root.take() {
                before()?;
                return Ok(Some(root));
            }
            loop {
                if let Some(current) = &mut nodes
                    && let Some(node) = current.next_with(&mut *before)?
                {
                    return Ok(Some(node));
                }
                before()?;
                let Some(value) = arguments.next() else {
                    return Ok(None);
                };
                nodes = Some(value.nodes());
            }
        },
        before,
    )
}

fn symbol_node<E>(
    node: ValueNodeRef<'_>,
    arguments: Vec<Symbol>,
    storage: &mut ExportStorage,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<Symbol, BridgeFailure<E>> {
    before().map_err(BridgeFailure::Stopped)?;
    Ok(match node {
        ValueNodeRef::Infimum => Symbol::Infimum,
        ValueNodeRef::Supremum => Symbol::Supremum,
        ValueNodeRef::Number(value) => Symbol::Number(value),
        ValueNodeRef::String(value) => Symbol::String(storage.text(value, false, before)?),
        ValueNodeRef::Symbol(name) => Symbol::Function {
            name: Name::new(storage.text(name, true, before)?)
                .map_err(|_| BridgeError::InvalidName)?,
            sign: Sign::Positive,
            arguments,
        },
        ValueNodeRef::Function { name, sign, .. } => Symbol::Function {
            name: Name::new(storage.text(name, true, before)?)
                .map_err(|_| BridgeError::InvalidName)?,
            sign: match sign {
                zetesis_core::Sign::Positive => Sign::Positive,
                zetesis_core::Sign::Negative => Sign::Negative,
            },
            arguments,
        },
        ValueNodeRef::Tuple { .. } => Symbol::Tuple(arguments),
    })
}
impl Frame<'_> {
    fn finish<E>(
        self,
        storage: &mut ExportStorage,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Symbol, BridgeFailure<E>> {
        before().map_err(BridgeFailure::Stopped)?;
        Ok(if let Some(name) = self.name {
            Symbol::Function {
                name: Name::new(storage.text(name, true, before)?)
                    .map_err(|_| BridgeError::InvalidName)?,
                sign: self.sign,
                arguments: self.arguments,
            }
        } else {
            Symbol::Tuple(self.arguments)
        })
    }
}

fn from_nodes<'a, E>(
    node_count: usize,
    depth: usize,
    storage: &mut ExportStorage,
    mut next: impl FnMut(&mut dyn FnMut() -> Result<(), E>) -> Result<Option<ValueNodeRef<'a>>, E>,
    mut before: impl FnMut() -> Result<(), E>,
) -> Result<Symbol, BridgeFailure<E>> {
    if node_count == 0 || depth == 0 || depth > node_count {
        return Err(BridgeError::InvalidName.into());
    }
    // Ancestors of the next node occupy at most depth-1 frames, bounded by
    // N-1. Reserve once, without geometric growth or old/new overlap, while
    // avoiding an N-frame reservation for a shallow, wide constructor.
    let frame_count = depth - 1;
    let mut frames: Vec<Frame<'a>> = storage.reserve(frame_count, &mut before)?;
    while let Some(node) = next(&mut before).map_err(BridgeFailure::Stopped)? {
        let head = match node {
            ValueNodeRef::Function { name, arity, sign } if arity != 0 => Some((
                Some(name),
                arity,
                match sign {
                    zetesis_core::Sign::Positive => Sign::Positive,
                    zetesis_core::Sign::Negative => Sign::Negative,
                },
            )),
            ValueNodeRef::Tuple { arity } if arity != 0 => Some((None, arity, Sign::Positive)),
            _ => None,
        };
        if let Some((name, arity, sign)) = head {
            if frames.len() >= frame_count {
                return Err(BridgeError::InvalidName.into());
            }
            let arguments = storage.reserve(arity, &mut before)?;
            frames.push(Frame {
                name,
                arguments,
                arity,
                sign,
            });
            continue;
        }
        let mut symbol = symbol_node(node, Vec::new(), storage, &mut before)?;
        loop {
            before().map_err(BridgeFailure::Stopped)?;
            let Some(parent) = frames.last_mut() else {
                return Ok(symbol);
            };
            parent.arguments.push(symbol);
            if parent.arguments.len() < parent.arity {
                break;
            }
            let parent = frames.pop().expect("completed parent frame");
            symbol = parent.finish(storage, &mut before)?;
        }
    }
    Err(BridgeError::InvalidName.into())
}

/// Conservative node/text payload before cloning an upstream structural symbol.
pub(crate) fn symbol_bytes(symbol: &Symbol) -> u128 {
    if matches!(symbol, Symbol::Tuple(_))
        || matches!(symbol, Symbol::Function { arguments, sign, .. } if !arguments.is_empty() || *sign == Sign::Negative)
    {
        symbol
            .subsymbols()
            .map(|node| std::mem::size_of::<ValueNode>() as u128 + view(node).text_bytes() as u128)
            .sum()
    } else {
        view(symbol).text_bytes() as u128
    }
}

#[cfg(test)]
mod tests;
