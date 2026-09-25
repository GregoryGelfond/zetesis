//! Validated flat closed values. No operation recurses through logical depth.

use std::{cmp::Ordering, fmt};

use crate::{Sign, Value};

mod view;
pub(crate) mod spelling;
pub use spelling::ValueWriteError;
pub use view::ValueNodeRef;

/// One preorder node of a closed value. Child nodes immediately follow a head.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueNode {
    /// Least ASP value.
    Infimum,
    /// Checked-width integer leaf.
    Number(i32),
    /// Decoded string leaf.
    String(String),
    /// Positive named constant leaf.
    Symbol(String),
    /// Named function with exactly `arity` following subtrees.
    Function {
        /// Exact constructor name; source spelling is the adapter's contract.
        name: String,
        /// Constructor sign, independent of the outer predicate sign.
        sign: Sign,
        /// Number of ordered immediate children.
        arity: usize,
    },
    /// Anonymous constructor, including `()` and `(x,)`.
    Tuple {
        /// Number of ordered immediate children.
        arity: usize,
    },
    /// Greatest ASP value.
    Supremum,
}

impl PartialOrd for ValueNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ValueNode {
    fn cmp(&self, other: &Self) -> Ordering {
        crate::term_order::storage(self.view(), other.view())
    }
}

impl std::hash::Hash for ValueNode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        crate::term_hash::descriptor(self.view(), state);
    }
}

/// Construction ceilings for one owned value and bounded construction scratch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValueLimits {
    /// Maximum number of preorder nodes.
    pub max_nodes: usize,
    /// Maximum root-inclusive depth.
    pub max_depth: usize,
    /// Maximum retained node capacity, text and canonical spelling bytes, plus
    /// reserved validation/render frame bytes. This is accounting, not allocator RSS.
    pub max_bytes: usize,
}
impl Default for ValueLimits {
    fn default() -> Self {
        Self {
            max_nodes: 262_144,
            max_depth: 128,
            max_bytes: 16_777_216,
        }
    }
}

/// A construction resource, measured before retaining the value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueResource {
    /// Preorder nodes.
    Nodes,
    /// Root-inclusive logical depth.
    Depth,
    /// Retained payload and construction frame reservations.
    Bytes,
}

/// Closed-value construction failed without returning a partial value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueError {
    /// The supplied sequence is not exactly one finite tree, or a function name is empty.
    Shape,
    /// An inclusive construction ceiling was exceeded.
    Limit {
        /// The exhausted resource.
        resource: ValueResource,
        /// Requested amount.
        observed: u128,
        /// Inclusive allowance.
        limit: usize,
    },
    /// Temporary validation storage could not be reserved.
    Allocation,
}
impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shape => f.write_str("closed value is not one valid constructor tree"),
            Self::Limit {
                resource,
                observed,
                limit,
            } => write!(f, "closed value {resource:?} {observed} exceeds {limit}"),
            Self::Allocation => {
                f.write_str("closed value validation storage could not be reserved")
            }
        }
    }
}
impl std::error::Error for ValueError {}

/// Private validated preorder storage; integer positions are never semantic identity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct StructuralValue {
    nodes: std::sync::Arc<Vec<ValueNode>>,
    depth: usize,
    rendered: std::sync::Arc<String>,
    // Depth and spelling are deterministic derivatives of the canonical nodes.
    // Derived traits never compare allocation capacities or pointer addresses.
}
impl std::hash::Hash for StructuralValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        crate::term_hash::nodes(self.nodes(), state);
    }
}
impl StructuralValue {
    /// Exact canonical preorder nodes. No caller can mutate the validated shape.
    #[must_use]
    pub fn nodes(&self) -> &[ValueNode] {
        &self.nodes
    }

    /// Root-inclusive logical depth.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Retained node-buffer capacity, text lengths and spelling capacity; not RSS.
    #[must_use]
    pub fn payload_bytes(&self) -> usize {
        self.nodes.capacity() * std::mem::size_of::<ValueNode>()
            + self.nodes.iter().map(ValueNode::text_bytes).sum::<usize>()
            + self.rendered.capacity()
    }

    /// Exact bytes of the cached ASP spelling, without allocating.
    #[must_use]
    pub fn rendered_bytes(&self) -> usize {
        self.rendered.len()
    }

    /// Canonical identity encoding size, independent of allocator capacity.
    #[must_use]
    pub fn canonical_bytes(&self) -> usize {
        self.nodes.iter().map(ValueNode::canonical_bytes).sum()
    }
}

fn ceiling(resource: ValueResource, observed: u128, limit: usize) -> Result<(), ValueError> {
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

impl Value {
    /// Validate one flat closed tree. Positive nullary functions normalize to symbols.
    /// Scalars retain their existing public variants; tuples never collapse to a scalar.
    ///
    /// # Errors
    /// Refuses malformed trees, empty function names, resource excess or validation allocation.
    pub fn from_nodes(mut nodes: Vec<ValueNode>, limits: ValueLimits) -> Result<Self, ValueError> {
        ceiling(ValueResource::Nodes, nodes.len() as u128, limits.max_nodes)?;
        let mut bytes = nodes.capacity() as u128 * std::mem::size_of::<ValueNode>() as u128;
        ceiling(ValueResource::Bytes, bytes, limits.max_bytes)?;
        let frames = nodes.len().min(limits.max_depth);
        let mut scratch = frames as u128
            * (std::mem::size_of::<usize>() + std::mem::size_of::<(usize, bool, bool)>()) as u128;
        ceiling(ValueResource::Bytes, bytes + scratch, limits.max_bytes)?;
        let mut remaining = Vec::new();
        let mut maximum_depth = 0;
        remaining
            .try_reserve_exact(frames)
            .map_err(|_| ValueError::Allocation)?;
        scratch = remaining.capacity() as u128 * std::mem::size_of::<usize>() as u128
            + frames as u128 * std::mem::size_of::<(usize, bool, bool)>() as u128;
        ceiling(ValueResource::Bytes, bytes + scratch, limits.max_bytes)?;
        for (index, node) in nodes.iter_mut().enumerate() {
            while remaining.last() == Some(&0) {
                remaining.pop();
            }
            if let Some(parent) = remaining.last_mut() {
                *parent -= 1;
            } else if index != 0 {
                return Err(ValueError::Shape);
            }
            maximum_depth = maximum_depth.max(remaining.len() + 1);
            ceiling(
                ValueResource::Depth,
                remaining.len() as u128 + 1,
                limits.max_depth,
            )?;
            bytes += node.text_bytes() as u128;
            ceiling(ValueResource::Bytes, bytes + scratch, limits.max_bytes)?;
            if let ValueNode::Function { name, sign, arity } = node {
                if name.is_empty() {
                    return Err(ValueError::Shape);
                }
                if *arity == 0 && *sign == Sign::Positive {
                    *node = ValueNode::Symbol(std::mem::take(name));
                }
            }
            let arity = node.arity();
            if arity != 0 {
                remaining.push(arity);
            }
        }
        if nodes.is_empty() || remaining.iter().any(|left| *left != 0) {
            return Err(ValueError::Shape);
        }
        if nodes.len() == 1 {
            match nodes.pop().ok_or(ValueError::Shape)? {
                ValueNode::Infimum => return Ok(Self::Infimum),
                ValueNode::Supremum => return Ok(Self::Supremum),
                ValueNode::Number(n) => return Ok(Self::Number(n)),
                ValueNode::String(s) => return Ok(Self::String(s)),
                ValueNode::Symbol(s) => return Ok(Self::Symbol(s)),
                node => nodes.push(node),
            }
        }
        let spelling_bytes = nodes.iter().map(ValueNode::rendered_bytes).sum::<u128>();
        ceiling(
            ValueResource::Bytes,
            bytes + scratch + spelling_bytes,
            limits.max_bytes,
        )?;
        // The preceding ceiling proves representability because max_bytes is usize.
        let spelling_bytes = usize::try_from(spelling_bytes).map_err(|_| ValueError::Allocation)?;
        let mut rendered = String::new();
        rendered
            .try_reserve_exact(spelling_bytes)
            .map_err(|_| ValueError::Allocation)?;
        ceiling(
            ValueResource::Bytes,
            bytes + scratch + rendered.capacity() as u128,
            limits.max_bytes,
        )?;
        let mut render_frames = Vec::new();
        render_frames
            .try_reserve_exact(frames)
            .map_err(|_| ValueError::Allocation)?;
        scratch = remaining.capacity() as u128 * std::mem::size_of::<usize>() as u128
            + render_frames.capacity() as u128 * std::mem::size_of::<(usize, bool, bool)>() as u128;
        ceiling(
            ValueResource::Bytes,
            bytes + scratch + rendered.capacity() as u128,
            limits.max_bytes,
        )?;
        render_nodes(&nodes, &mut render_frames, &mut rendered)
            .map_err(|_| ValueError::Allocation)?;
        Ok(Self::Structured(StructuralValue {
            nodes: std::sync::Arc::new(nodes),
            depth: maximum_depth,
            rendered: std::sync::Arc::new(rendered),
        }))
    }

    /// Referenced value payload, excluding its outer `Value` slot. Structural
    /// clones share this payload; consumers may charge it conservatively per use.
    #[must_use]
    pub fn payload_bytes(&self) -> usize {
        match self {
            Self::String(text) | Self::Symbol(text) => text.len(),
            Self::Structured(value) => value.payload_bytes(),
            _ => 0,
        }
    }

    /// Named nested buffer capacity, excluding this inline value.
    ///
    /// Includes actual string/node-vector/spelling capacities and the structural
    /// vector/string owner headers. Shared buffers are counted per occurrence;
    /// Arc counters and allocator metadata are excluded. Visits structural node
    /// descriptors without comparing or copying their text. Returns `None` if
    /// the wide accounting sum cannot be represented.
    #[must_use]
    pub fn checked_payload_capacity_bytes(&self) -> Option<u128> {
        match self {
            Self::String(text) | Self::Symbol(text) => Some(text.capacity() as u128),
            Self::Structured(value) => {
                let fixed = (value.nodes.capacity() as u128)
                    .checked_mul(std::mem::size_of::<ValueNode>() as u128)?
                    .checked_add(value.rendered.capacity() as u128)?
                    .checked_add(std::mem::size_of::<Vec<ValueNode>>() as u128)?
                    .checked_add(std::mem::size_of::<String>() as u128)?;
                value.nodes.iter().try_fold(fixed, |bytes, node| {
                    let capacity = match node {
                        ValueNode::String(text)
                        | ValueNode::Symbol(text)
                        | ValueNode::Function { name: text, .. } => text.capacity() as u128,
                        _ => 0,
                    };
                    bytes.checked_add(capacity)
                })
            }
            _ => Some(0),
        }
    }

    /// Logical identity encoding bytes: tags, lengths, numbers and text.
    #[must_use]
    pub fn canonical_bytes(&self) -> usize {
        match self {
            Self::Number(_) => 5,
            Self::String(text) | Self::Symbol(text) => 9 + text.len(),
            Self::Structured(value) => value.canonical_bytes(),
            Self::Infimum | Self::Supremum => 1,
        }
    }
}

impl ValueNode {
    /// Borrow the complete node description in constant time, without copying
    /// text. The view remains valid only while this node is borrowed; it does
    /// not validate any surrounding tree.
    #[must_use]
    pub fn view(&self) -> ValueNodeRef<'_> {
        match self {
            Self::Infimum => ValueNodeRef::Infimum,
            Self::Number(number) => ValueNodeRef::Number(*number),
            Self::String(text) => ValueNodeRef::String(text),
            Self::Symbol(name) => ValueNodeRef::Symbol(name),
            Self::Function { name, sign, arity } => ValueNodeRef::Function {
                name,
                sign: *sign,
                arity: *arity,
            },
            Self::Tuple { arity } => ValueNodeRef::Tuple { arity: *arity },
            Self::Supremum => ValueNodeRef::Supremum,
        }
    }

    fn text_bytes(&self) -> usize {
        self.view().text_bytes()
    }
    fn rendered_bytes(&self) -> u128 {
        self.view().rendered_bytes()
    }
    fn canonical_bytes(&self) -> usize {
        usize::try_from(self.view().canonical_bytes()).expect("admitted node encoding length")
    }
    fn arity(&self) -> usize {
        match self {
            Self::Function { arity, .. } | Self::Tuple { arity } => *arity,
            _ => 0,
        }
    }
}

impl fmt::Display for StructuralValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.rendered)
    }
}

fn render_nodes(
    nodes: &[ValueNode],
    frames: &mut Vec<spelling::Frame>,
    output: &mut String,
) -> fmt::Result {
    for node in nodes {
        spelling::node(node.view(), frames, output, &mut |_| {
            Ok::<_, std::convert::Infallible>(())
        })
        .map_err(|_| fmt::Error)?;
    }
    Ok(())
}
