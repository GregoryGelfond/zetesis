//! Exact iterative comparison of canonical terms and ingress descriptions.
//!
//! The cursor retains one parent, not a depth-sized stack. Child prefix ends
//! make rank selection logarithmic in arity; repeated ancestor recovery can
//! still cost quadratic work on deep combs. No flattened term is retained.

use std::{cmp::Ordering, convert::Infallible};

use crate::{Atom, Value, ValueNodeRef};

use super::view::{AtomRef, TermRef};

#[derive(Clone, Copy)]
struct Parent<'a> {
    term: TermRef<'a>,
    child: usize,
}

/// Cursor state is constant in logical depth and expanded tree size.
#[derive(Clone)]
pub(super) struct Preorder<'a> {
    root: TermRef<'a>,
    position: usize,
    current: Option<TermRef<'a>>,
    parent: Option<Parent<'a>>,
}

impl<'a> Preorder<'a> {
    pub(super) const fn new(root: TermRef<'a>) -> Self {
        Self {
            root,
            position: 0,
            current: None,
            parent: None,
        }
    }

    pub(super) fn next_with<E>(
        &mut self,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Option<TermRef<'a>>, E> {
        before()?;
        if self.position == self.root.expanded_nodes() {
            return Ok(None);
        }
        let (next, parent) = if let Some(current) = self.current {
            before()?;
            if arity(current.descriptor()) != 0 {
                before()?;
                (
                    current.child(0).expect("admitted first child"),
                    Some(Parent {
                        term: current,
                        child: 0,
                    }),
                )
            } else if let Some(parent) = self.parent {
                before()?;
                let sibling = parent.child + 1;
                if sibling < arity(parent.term.descriptor()) {
                    before()?;
                    (
                        parent.term.child(sibling).expect("admitted sibling"),
                        Some(Parent {
                            term: parent.term,
                            child: sibling,
                        }),
                    )
                } else {
                    select(self.root, self.position, before)?
                }
            } else {
                select(self.root, self.position, before)?
            }
        } else {
            (self.root, None)
        };
        self.position += 1;
        self.current = Some(next);
        self.parent = parent;
        Ok(Some(next))
    }
}

impl<'a> Iterator for Preorder<'a> {
    type Item = TermRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        infallible(self.next_with(&mut || Ok::<_, Infallible>(())))
    }
}

fn select<'a, E>(
    mut current: TermRef<'a>,
    mut position: usize,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<(TermRef<'a>, Option<Parent<'a>>), E> {
    let mut parent = None;
    while position != 0 {
        position -= 1;
        before()?;
        let mut low = 0;
        let mut high = arity(current.descriptor());
        // Child ends partition the admitted parent's expanded descendants.
        while low < high {
            let middle = low + (high - low) / 2;
            before()?;
            if current.child_end(middle).expect("admitted child end") <= position {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        if low != 0 {
            before()?;
            position -= current.child_end(low - 1).expect("preceding child end");
        }
        parent = Some(Parent {
            term: current,
            child: low,
        });
        before()?;
        current = current.child(low).expect("rank selects an admitted child");
    }
    Ok((current, parent))
}

pub(super) fn subterm_with<'a, E>(
    root: TermRef<'a>,
    position: usize,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<TermRef<'a>, E> {
    select(root, position, before).map(|(term, _)| term)
}

pub(super) const fn arity(node: ValueNodeRef<'_>) -> usize {
    match node {
        ValueNodeRef::Function { arity, .. } | ValueNodeRef::Tuple { arity } => arity,
        _ => 0,
    }
}

pub(super) const fn structured(node: ValueNodeRef<'_>) -> bool {
    matches!(
        node,
        ValueNodeRef::Function { .. } | ValueNodeRef::Tuple { .. }
    )
}

fn infallible<T>(result: Result<T, Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(never) => match never {},
    }
}

enum Nodes<'a> {
    Canonical(Preorder<'a>),
    Ingress { term: TermRef<'a>, position: usize },
}

impl<'a> Nodes<'a> {
    fn canonical(term: TermRef<'a>) -> Self {
        if term.is_canonical() {
            Self::Canonical(Preorder::new(term))
        } else {
            Self::Ingress { term, position: 0 }
        }
    }

    fn ingress(value: &'a Value) -> Self {
        Self::canonical(TermRef::from(value))
    }

    /// Read the root once and leave the cursor ready for its descendants.
    /// Admitted terms are nonempty; the first node needs no rank navigation.
    fn take_root<E>(
        &mut self,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<ValueNodeRef<'a>, E> {
        before()?;
        match self {
            Self::Canonical(cursor) => {
                let descriptor = cursor.root.descriptor();
                cursor.position = 1;
                cursor.current = Some(cursor.root);
                Ok(descriptor)
            }
            Self::Ingress { term, position } => {
                let descriptor = term.descriptor();
                *position = 1;
                Ok(descriptor)
            }
        }
    }

    fn next_with<E>(
        &mut self,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Option<ValueNodeRef<'a>>, E> {
        match self {
            Self::Canonical(cursor) => {
                let Some(term) = cursor.next_with(before)? else {
                    return Ok(None);
                };
                before()?;
                Ok(Some(term.descriptor()))
            }
            Self::Ingress { term, position } => {
                before()?;
                let node = term.flat_node(*position);
                if node.is_some() {
                    *position += 1;
                }
                Ok(node)
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Order {
    Storage { metered: bool },
    Asp { metered: bool },
}

impl Order {
    fn compare<E>(
        self,
        left: ValueNodeRef<'_>,
        right: ValueNodeRef<'_>,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Ordering, E> {
        before()?;
        let metered = match self {
            Self::Storage { metered } | Self::Asp { metered } => metered,
        };
        let text = |left: &str, right: &str| {
            if metered {
                crate::identity::bytes(left.as_bytes(), right.as_bytes(), before)
            } else {
                Ok(left.cmp(right))
            }
        };
        match self {
            Self::Storage { .. } => crate::term_order::storage_with(left, right, text),
            Self::Asp { .. } => crate::term_order::asp_with(left, right, text),
        }
    }
}

fn nodes_with<E>(
    mut left: Nodes<'_>,
    mut right: Nodes<'_>,
    order: Order,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<Ordering, E> {
    let left_root = left.take_root(before)?;
    let right_root = right.take_root(before)?;
    if matches!(order, Order::Storage { .. }) {
        before()?;
        let comparison = crate::term_order::root_storage_rank(left_root)
            .cmp(&crate::term_order::root_storage_rank(right_root));
        if !comparison.is_eq() {
            return Ok(comparison);
        }
    }
    let comparison = order.compare(left_root, right_root, before)?;
    if !comparison.is_eq() || (arity(left_root) == 0 && arity(right_root) == 0) {
        return Ok(comparison);
    }
    loop {
        let left = left.next_with(before)?;
        let right = right.next_with(before)?;
        let (Some(left), Some(right)) = (left, right) else {
            before()?;
            return Ok(left.is_some().cmp(&right.is_some()));
        };
        let comparison = order.compare(left, right, before)?;
        if !comparison.is_eq() {
            return Ok(comparison);
        }
    }
}

pub(super) fn term(left: TermRef<'_>, right: TermRef<'_>) -> Ordering {
    if left.same_identity(right) {
        Ordering::Equal
    } else {
        infallible(nodes_with(
            Nodes::canonical(left),
            Nodes::canonical(right),
            Order::Storage { metered: false },
            &mut || Ok::<_, Infallible>(()),
        ))
    }
}

pub(super) fn term_with<E>(
    left: TermRef<'_>,
    right: TermRef<'_>,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<Ordering, E> {
    if same_term_with(left, right, before)? {
        return Ok(Ordering::Equal);
    }
    term_contents_with(left, right, before)
}

/// Compare contents after the caller has ruled out a canonical identity result.
/// This is also the cross-owner fallback for checked equality.
pub(super) fn term_contents_with<E>(
    left: TermRef<'_>,
    right: TermRef<'_>,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<Ordering, E> {
    nodes_with(
        Nodes::canonical(left),
        Nodes::canonical(right),
        Order::Storage { metered: true },
        before,
    )
}

pub(super) fn term_value(left: TermRef<'_>, right: &Value) -> Ordering {
    term(left, TermRef::from(right))
}

pub(super) fn term_value_with<E>(
    left: TermRef<'_>,
    right: &Value,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<Ordering, E> {
    nodes_with(
        Nodes::canonical(left),
        Nodes::ingress(right),
        Order::Storage { metered: true },
        before,
    )
}

pub(super) fn asp(left: TermRef<'_>, right: TermRef<'_>) -> Ordering {
    if left.same_identity(right) {
        Ordering::Equal
    } else {
        infallible(nodes_with(
            Nodes::canonical(left),
            Nodes::canonical(right),
            Order::Asp { metered: false },
            &mut || Ok::<_, Infallible>(()),
        ))
    }
}

pub(super) fn asp_with<E>(
    left: TermRef<'_>,
    right: TermRef<'_>,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<Ordering, E> {
    if same_term_with(left, right, before)? {
        return Ok(Ordering::Equal);
    }
    nodes_with(
        Nodes::canonical(left),
        Nodes::canonical(right),
        Order::Asp { metered: true },
        before,
    )
}

/// Scoped canonical identity proves equality in either order without reading
/// descriptors or text. Unequal IDs establish no ordering, even in one owner.
fn same_term_with<E>(
    left: TermRef<'_>,
    right: TermRef<'_>,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<bool, E> {
    if !left.is_canonical() || !right.is_canonical() {
        return Ok(false);
    }
    before()?;
    Ok(left.same_identity(right))
}

pub(super) fn atom(left: AtomRef<'_>, right: AtomRef<'_>) -> Ordering {
    if left.same_identity(right) {
        Ordering::Equal
    } else {
        let (left, right) = (left.read(), right.read());
        left.predicate()
            .cmp(&right.predicate())
            .then_with(|| left.arguments().cmp(right.arguments()))
    }
}

pub(super) fn atom_with<E>(
    left: AtomRef<'_>,
    right: AtomRef<'_>,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<Ordering, E> {
    if left.canonical().is_some() && right.canonical().is_some() {
        before()?;
        if left.same_identity(right) {
            return Ok(Ordering::Equal);
        }
    }
    atom_contents_with(left, right, before)
}

/// Compare contents after the caller has ruled out a canonical identity result.
/// Checked equality uses this foreign-authority fallback without probing again.
pub(super) fn atom_contents_with<E>(
    left: AtomRef<'_>,
    right: AtomRef<'_>,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<Ordering, E> {
    before()?;
    let left = left.read();
    let left_predicate = left.predicate();
    before()?;
    let right = right.read();
    let right_predicate = right.predicate();
    let predicate = left_predicate.compare_ref_with(right_predicate, &mut *before)?;
    if !predicate.is_eq() {
        return Ok(predicate);
    }
    // Each permit precedes the logical read it admits; column reads use the
    // two rows resolved above.
    before()?;
    let arity = left.arity();
    for column in 0..arity {
        before()?;
        let left = left.at_valid_column(column);
        before()?;
        let right = right.at_valid_column(column);
        let comparison = term_with(left, right, before)?;
        if !comparison.is_eq() {
            return Ok(comparison);
        }
    }
    Ok(Ordering::Equal)
}

pub(super) fn atom_value(left: AtomRef<'_>, right: &Atom) -> Ordering {
    let left = left.read();
    left.predicate().compare(right.predicate()).then_with(|| {
        left.arguments()
            .zip(right.values())
            .map(|(left, right)| term_value(left, right))
            .find(|order| !order.is_eq())
            .unwrap_or(Ordering::Equal)
    })
}

pub(super) fn atom_value_with<E>(
    left: AtomRef<'_>,
    right: &Atom,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<Ordering, E> {
    atom_with(left, AtomRef::from(right), before)
}

#[cfg(test)]
mod identity_tests;

#[cfg(test)]
mod root_tests;

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use crate::{Sign, Value, ValueLimits, ValueNode};

    use super::{Preorder, TermRef};
    use crate::catalog::{Limits, storage::Store};

    fn function(arity: usize) -> ValueNode {
        ValueNode::Function {
            name: "f".into(),
            sign: Sign::Positive,
            arity,
        }
    }

    fn admitted(nodes: Vec<ValueNode>) -> Value {
        Value::from_nodes(
            nodes,
            ValueLimits {
                max_nodes: 20_001,
                max_depth: 20_001,
                max_bytes: 8_000_000,
            },
        )
        .unwrap()
    }

    #[test]
    fn canonical_preorder_matches_flat_admission() {
        let mut fixtures = Vec::new();
        let mut wide = vec![function(256)];
        for number in 0..256 {
            wide.extend([function(1), ValueNode::Number(number)]);
        }
        fixtures.push(wide);
        let mut left_comb = Vec::new();
        for _ in 0..64 {
            left_comb.extend([function(2), ValueNode::Number(0)]);
        }
        left_comb.push(ValueNode::Number(1));
        fixtures.push(left_comb);
        let mut right_comb = vec![function(2); 64];
        right_comb.push(ValueNode::Number(1));
        right_comb.extend(std::iter::repeat_n(ValueNode::Number(0), 64));
        fixtures.push(right_comb);
        fixtures.push(vec![
            function(2),
            function(1),
            ValueNode::String("é".into()),
            function(1),
            ValueNode::String("é".into()),
        ]);
        for nodes in fixtures {
            let original = admitted(nodes);
            let mut store = Store::new(16_000_000);
            let id = store.import_value(&original, Limits::default()).unwrap();
            let snapshot = store.snapshot(0).unwrap();
            let term = TermRef::new(&snapshot, id).unwrap();
            let actual: Vec<_> = Preorder::new(term).map(TermRef::descriptor).collect();
            let Value::Structured(original) = &original else {
                panic!("compound fixture")
            };
            let expected: Vec<_> = original.nodes().iter().map(ValueNode::view).collect();
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn wide_navigation_has_a_logarithmic_probe_bound() {
        let width = 256;
        let mut nodes = vec![function(width)];
        for _ in 0..width {
            nodes.extend([function(1), ValueNode::Number(0)]);
        }
        let original = admitted(nodes);
        let mut store = Store::new(16_000_000);
        let id = store.import_value(&original, Limits::default()).unwrap();
        let snapshot = store.snapshot(0).unwrap();
        let term = TermRef::new(&snapshot, id).unwrap();
        let mut cursor = Preorder::new(term);
        let mut probes = 0;
        let mut visited = 0;
        while cursor
            .next_with(&mut || {
                probes += 1;
                Ok::<_, Infallible>(())
            })
            .unwrap()
            .is_some()
        {
            visited += 1;
        }
        assert_eq!(visited, 2 * width + 1);
        // Each completed unary child returns through binary root selection.
        // A linear sibling rescan exceeds this bound for the same fixture.
        assert!(probes <= visited * (width.ilog2() as usize + 12));
    }

    #[test]
    fn foreign_deep_terms_compare_without_recursive_walks() {
        let depth = 10_000;
        let mut nodes = vec![function(1); depth];
        nodes.push(ValueNode::Number(0));
        let original = admitted(nodes);
        let limits = Limits {
            max_nodes: depth + 1,
            max_depth: depth + 1,
            max_bytes: 8_000_000,
        };
        let mut left = Store::new(32_000_000);
        let left_id = left.import_value(&original, limits).unwrap();
        let left = left.snapshot(0).unwrap();
        let mut right = Store::new(32_000_000);
        right.import_value(&Value::Number(99), limits).unwrap();
        let right_id = right.import_value(&original, limits).unwrap();
        let right = right.snapshot(0).unwrap();
        let left = TermRef::new(&left, left_id).unwrap();
        let right = TermRef::new(&right, right_id).unwrap();
        assert_eq!(left.cmp(&right), std::cmp::Ordering::Equal);
        assert_eq!(left.compare_terms(right), std::cmp::Ordering::Equal);
    }

    #[test]
    fn navigation_refusal_stops_at_its_callback() {
        let original = admitted(vec![
            function(2),
            function(1),
            ValueNode::Number(0),
            function(1),
            ValueNode::String("prefix".into()),
        ]);
        let mut store = Store::new(16_000_000);
        let id = store.import_value(&original, Limits::default()).unwrap();
        let snapshot = store.snapshot(0).unwrap();
        let term = TermRef::new(&snapshot, id).unwrap();
        let mut total = 0;
        term.compare_identity_with(&original, || {
            total += 1;
            Ok::<_, Infallible>(())
        })
        .unwrap();
        for limit in 0..total {
            let mut calls = 0;
            let result = term.compare_identity_with(&original, || {
                calls += 1;
                if calls > limit { Err(limit) } else { Ok(()) }
            });
            assert_eq!(result, Err(limit));
            assert_eq!(calls, limit + 1);
        }
    }
}
