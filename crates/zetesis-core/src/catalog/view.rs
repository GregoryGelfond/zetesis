//! Borrowed logical views over canonical storage or owned ingress descriptions.
//!
//! References never own a second payload. Canonical IDs are interpreted only
//! with their snapshot; ingress borrows use the same semantic read interface.

use std::{
    cmp::Ordering,
    convert::Infallible,
    fmt,
    hash::{Hash, Hasher},
    iter::FusedIterator,
};

use crate::{
    Atom, Predicate, Sign, Value, ValueError, ValueLimits, ValueNode, ValueNodeRef, ValueResource,
};

use super::{
    compare,
    storage::{self, AtomId, Fault, PredicateId, Read, TermId},
};

/// One typed term borrowed from a canonical snapshot or an ingress description.
///
/// Copying this reference copies no payload. Equality, order and hash compare
/// logical contents across snapshots and independent catalogs. Storage order is
/// distinct from [`Self::compare_terms`]. Canonical same-catalog equality uses
/// stable IDs; IDs are never exposed as cross-catalog identity or term order.
/// Display spells the value with temporary iterative frames and no caller work
/// ceiling. Use [`Self::write_with`] for typed frame and interruption limits.
#[derive(Clone, Copy)]
pub struct TermRef<'a>(TermSource<'a>);

#[derive(Clone, Copy)]
enum TermSource<'a> {
    Canonical {
        snapshot: Read<'a>,
        id: TermId,
    },
    Derived {
        arena: &'a storage::DerivedTerms<'a>,
        id: TermId,
    },
    Ingress(&'a Value),
    Nodes(&'a [ValueNode]),
}

// A transient dereference, never another payload owner. Canonical handles are
// checked when constructed; a read borrow prevents mutation and snapshots keep
// their prefix. All operations below therefore read the same admitted term.
#[derive(Clone, Copy)]
enum ResolvedTerm<'a> {
    Canonical(storage::Term<'a>),
    Derived(storage::DerivedTerm<'a>),
    Ingress(&'a Value),
    Nodes(&'a [ValueNode]),
}

impl<'a> ResolvedTerm<'a> {
    fn descriptor(self) -> ValueNodeRef<'a> {
        match self {
            Self::Canonical(term) => term.descriptor(),
            Self::Derived(term) => term.descriptor(),
            Self::Ingress(value) => value.root_view(),
            Self::Nodes(nodes) => nodes[0].view(),
        }
    }

    fn expanded_nodes(self) -> usize {
        match self {
            Self::Canonical(term) => term.expanded_nodes(),
            Self::Derived(term) => term.expanded_nodes(),
            Self::Ingress(Value::Structured(value)) => value.nodes().len(),
            Self::Ingress(_) => 1,
            Self::Nodes(nodes) => nodes.len(),
        }
    }

    fn depth(self) -> usize {
        match self {
            Self::Canonical(term) => term.depth(),
            Self::Derived(term) => term.depth(),
            Self::Ingress(Value::Structured(value)) => value.depth(),
            Self::Ingress(_) => 1,
            Self::Nodes(nodes) => flat_depth(nodes),
        }
    }

    fn flat_node(self, index: usize) -> Option<ValueNodeRef<'a>> {
        match self {
            Self::Canonical(_) | Self::Derived(_) => None,
            Self::Ingress(Value::Structured(value)) => {
                value.nodes().get(index).map(ValueNode::view)
            }
            Self::Ingress(value) => (index == 0).then(|| value.root_view()),
            Self::Nodes(nodes) => nodes.get(index).map(ValueNode::view),
        }
    }
}

#[derive(Clone, Copy)]
enum Length {
    Canonical,
    Rendered,
}

impl Length {
    fn stored(self, term: storage::Term<'_>) -> usize {
        match self {
            Self::Canonical => term.canonical_bytes(),
            Self::Rendered => term.rendered_bytes(),
        }
    }

    fn descriptor(self, node: ValueNodeRef<'_>) -> u128 {
        match self {
            Self::Canonical => node.canonical_bytes(),
            Self::Rendered => node.rendered_bytes(),
        }
    }
}

impl<'a> From<&'a Value> for TermRef<'a> {
    fn from(value: &'a Value) -> Self {
        Self(TermSource::Ingress(value))
    }
}

impl<'a> TermRef<'a> {
    pub(super) fn canonical(self) -> Option<(Read<'a>, TermId)> {
        match self.0 {
            TermSource::Canonical { snapshot, id } => Some((snapshot, id)),
            TermSource::Derived { .. } | TermSource::Ingress(_) | TermSource::Nodes(_) => None,
        }
    }

    pub(crate) fn new(snapshot: impl Into<Read<'a>>, id: TermId) -> Option<Self> {
        let snapshot = snapshot.into();
        snapshot.term(id)?;
        Some(Self(TermSource::Canonical { snapshot, id }))
    }

    pub(super) fn scoped(self) -> Option<(super::TermRead<'a>, TermId)> {
        match self.0 {
            TermSource::Canonical { snapshot, id } => {
                Some((super::CatalogRead(snapshot).into(), id))
            }
            TermSource::Derived { arena, id } => Some((arena.read(), id)),
            _ => None,
        }
    }

    pub(super) fn derived(arena: &'a storage::DerivedTerms<'a>, id: TermId) -> Option<Self> {
        arena
            .contains(id)
            .then_some(Self(TermSource::Derived { arena, id }))
    }

    fn read(self) -> ResolvedTerm<'a> {
        match self.0 {
            TermSource::Canonical { snapshot, id } => ResolvedTerm::Canonical(
                snapshot
                    .term(id)
                    .expect("validated term remains in its read prefix"),
            ),
            TermSource::Derived { arena, id } => ResolvedTerm::Derived(arena.term(id)),
            TermSource::Ingress(value) => ResolvedTerm::Ingress(value),
            TermSource::Nodes(nodes) => ResolvedTerm::Nodes(nodes),
        }
    }

    fn stored(self) -> Option<storage::Term<'a>> {
        match self.read() {
            ResolvedTerm::Canonical(term) => Some(term),
            ResolvedTerm::Derived(_) | ResolvedTerm::Ingress(_) | ResolvedTerm::Nodes(_) => None,
        }
    }

    pub(super) fn ingress(self) -> Option<&'a Value> {
        match self.0 {
            TermSource::Ingress(value) => Some(value),
            TermSource::Canonical { .. } | TermSource::Derived { .. } | TermSource::Nodes(_) => {
                None
            }
        }
    }

    pub(super) fn is_canonical(self) -> bool {
        matches!(
            self.0,
            TermSource::Canonical { .. } | TermSource::Derived { .. }
        )
    }

    pub(super) fn same_identity(self, other: Self) -> bool {
        if let Some(equal) = self.canonical_equality(other) {
            return equal;
        }
        match (self.0, other.0) {
            (TermSource::Ingress(left), TermSource::Ingress(right)) => std::ptr::eq(left, right),
            (TermSource::Nodes(left), TermSource::Nodes(right)) => std::ptr::eq(left, right),
            _ => false,
        }
    }

    // Exact scope plus admitted IDs is sufficient because each scope interns
    // equal typed terms to one ID. Prefix validation belongs to construction of
    // these views. Different scopes need content comparison, even for equal IDs.
    fn canonical_equality(self, other: Self) -> Option<bool> {
        let ((left, left_id), (right, right_id)) = (self.scoped()?, other.scoped()?);
        left.same(right).then_some(left_id == right_id)
    }

    /// The root's typed description, borrowing its text. No traversal or
    /// allocation occurs; canonical resolution includes segment-directory lookup.
    #[must_use]
    pub fn descriptor(self) -> ValueNodeRef<'a> {
        self.read().descriptor()
    }

    /// Traverse borrowed node descriptions in logical preorder. Repeated
    /// subterms appear at each occurrence. The cursor allocates no storage and
    /// retains constant state; recovering ancestors can take quadratic work on
    /// deep trees. Use [`TermNodes::next_with`] for a caller-controlled work bound.
    #[must_use]
    pub fn nodes(self) -> TermNodes<'a> {
        TermNodes::new(self)
    }

    /// Borrow the subterm beginning at a logical preorder position. Repeated
    /// occurrences select the same canonical identity without copying payload.
    /// Canonical selection follows child prefix ends (logarithmic in arity at
    /// each visited level); ingress selection scans the selected flat subtree.
    /// No storage is allocated. An out-of-range position returns `None`.
    ///
    /// # Errors
    /// Returns the caller's refusal before the next navigation or descriptor read.
    pub fn subterm_with<E>(
        self,
        position: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<Self>, E> {
        before()?;
        if self.is_canonical() {
            if position >= self.expanded_nodes() {
                return Ok(None);
            }
            return compare::subterm_with(self, position, &mut before).map(Some);
        }
        let Some(nodes) = self.flat_nodes() else {
            return Ok((position == 0).then_some(self));
        };
        if position >= nodes.len() {
            return Ok(None);
        }
        let mut end = position;
        let mut remaining = 1;
        while remaining != 0 {
            before()?;
            let node = nodes[end].view();
            remaining = remaining - 1 + compare::arity(node);
            end += 1;
        }
        Ok(Some(Self(TermSource::Nodes(&nodes[position..end]))))
    }

    /// Write canonical ASP spelling directly from borrowed node descriptions.
    /// No owned value or complete rendered copy is constructed. The callback
    /// admits each navigation/frame step and each output fragment's UTF-8 bytes
    /// plus one before emission. String decoding is charged before each scalar.
    /// Canonical navigation has the bound documented on [`Self::nodes`].
    ///
    /// `max_frame_bytes` bounds frame capacity, including old/new growth overlap;
    /// the vector header and caller-owned output storage are separate. Frame
    /// depth is bounded by the admitted value's depth. A sink such as `String` can
    /// allocate independently, so callers needing typed output admission must
    /// pre-reserve it or supply a bounded `fmt::Write` implementation.
    ///
    /// # Errors
    /// Returns typed scratch, caller or writer refusal. The sink can contain an
    /// already-written prefix; this operation does not roll back external output.
    pub fn write_with<E>(
        self,
        output: &mut impl fmt::Write,
        max_frame_bytes: usize,
        mut before: impl FnMut(u128) -> Result<(), E>,
    ) -> Result<(), crate::ValueWriteError<E>> {
        let mut frames = Vec::new();
        let mut nodes = self.nodes();
        while let Some(node) = nodes
            .next_with(|| before(1))
            .map_err(crate::ValueWriteError::Stopped)?
        {
            crate::structured::spelling::reserve(&mut frames, node, max_frame_bytes, &mut before)?;
            crate::structured::spelling::node(node, &mut frames, output, &mut before)?;
        }
        Ok(())
    }

    pub(super) fn flat_node(self, index: usize) -> Option<ValueNodeRef<'a>> {
        self.read().flat_node(index)
    }

    fn flat_nodes(self) -> Option<&'a [ValueNode]> {
        match self.0 {
            TermSource::Ingress(Value::Structured(value)) => Some(value.nodes()),
            TermSource::Nodes(nodes) => Some(nodes),
            TermSource::Canonical { .. } | TermSource::Derived { .. } | TermSource::Ingress(_) => {
                None
            }
        }
    }

    /// Borrow an immediate child. Canonical children require only storage
    /// resolution; an ingress subtree scans preceding flat child descriptions.
    /// An out-of-arity child is absent, never a placeholder logical constant.
    #[must_use]
    pub fn child(self, index: usize) -> Option<Self> {
        if let TermSource::Derived { arena, id } = self.0 {
            return arena.term(id).child(index);
        }
        if let TermSource::Canonical { snapshot, id } = self.0 {
            let child = snapshot.term(id)?.child(index)?;
            return Self::new(snapshot, child);
        }
        let nodes = self.flat_nodes()?;
        if index >= compare::arity(nodes[0].view()) {
            return None;
        }
        let mut start = 1;
        for _ in 0..index {
            start = subtree_end(nodes, start);
        }
        Some(Self(TermSource::Nodes(
            &nodes[start..subtree_end(nodes, start)],
        )))
    }

    pub(super) fn child_end(self, index: usize) -> Option<usize> {
        if let TermSource::Derived { arena, id } = self.0 {
            return arena.term(id).child_end(index);
        }
        if let Some(stored) = self.stored() {
            return stored.child_end(index);
        }
        let nodes = self.flat_nodes()?;
        if index >= compare::arity(nodes[0].view()) {
            return None;
        }
        let mut end = 1;
        for _ in 0..=index {
            end = subtree_end(nodes, end);
        }
        Some(end - 1)
    }

    /// Expanded preorder node count, including repeated occurrences of shared
    /// subterms. Constant time after canonical segment resolution.
    #[must_use]
    pub fn expanded_nodes(self) -> usize {
        self.read().expanded_nodes()
    }

    /// Root-inclusive logical depth. Canonical terms and complete ingress values
    /// use cached depth. A borrowed ingress subtree uses an allocation-free
    /// interval scan, with quadratic worst-case work in its expanded nodes.
    #[must_use]
    pub fn depth(self) -> usize {
        self.read().depth()
    }

    /// Portable typed encoding length, including this term's root tag. Canonical
    /// terms use their admitted cached length; ingress descriptions are scanned.
    /// This measure excludes allocation capacity and is not a wire fingerprint.
    #[must_use]
    pub fn canonical_bytes(self) -> usize {
        self.length(Length::Canonical)
    }

    /// Measure portable typed encoding length with an explicit work boundary.
    /// A canonical term charges one cached-measure read. An ingress term charges
    /// each descriptor; string lengths are read without scanning their bytes.
    /// This does not measure retained capacity or copy any payload.
    ///
    /// # Errors
    /// Returns the caller's first refusal before inspecting the next descriptor.
    pub fn canonical_bytes_with<E>(
        self,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<usize, E> {
        self.length_with(Length::Canonical, &mut before)
    }

    /// Length of the canonical ASP spelling. Canonical terms use their cached
    /// length; ingress descriptions scan nodes and escaped text without rendering.
    #[must_use]
    pub fn rendered_bytes(self) -> usize {
        self.length(Length::Rendered)
    }

    fn length(self, measure: Length) -> usize {
        match self.length_with(measure, &mut || Ok::<(), Infallible>(())) {
            Ok(bytes) => bytes,
            Err(never) => match never {},
        }
    }

    // One read projection and one flat-descriptor traversal serve both measures.
    // Complete ingress values have admitted lengths; a subtree's nonnegative
    // contribution cannot exceed its complete source. Canonical lengths are
    // checked at interning. The final narrowing is consequently invariant-only.
    fn length_with<E>(
        self,
        measure: Length,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<usize, E> {
        before()?;
        let term = self.read();
        match term {
            ResolvedTerm::Canonical(stored) => return Ok(measure.stored(stored)),
            ResolvedTerm::Derived(stored) => {
                return Ok(match measure {
                    Length::Canonical => stored.canonical_bytes(),
                    Length::Rendered => stored.rendered_bytes(),
                });
            }
            _ => {}
        }
        let mut bytes = 0_u128;
        for index in 0..term.expanded_nodes() {
            before()?;
            bytes += measure.descriptor(term.flat_node(index).expect("admitted flat extent"));
        }
        Ok(usize::try_from(bytes).expect("admitted length fits usize"))
    }

    /// Compare in ASP term order without allocation or recursion. Canonical
    /// traversal uses constant cursor space; worst-case work is
    /// O(V × D × log(A + 1) × R + B), for visited expanded nodes V, depth D,
    /// arity A, storage-resolution cost R and compared text bytes B. Ingress
    /// preorder traversal is linear in its visited nodes and bytes.
    #[must_use]
    pub fn compare_terms(self, other: Self) -> Ordering {
        compare::asp(self, other)
    }

    /// Compare in ASP term order with a check before every navigation step,
    /// descriptor comparison and compared text-byte pair. Equal canonical
    /// identities require one check and no payload traversal. No payload is copied.
    ///
    /// # Errors
    /// Returns the caller's first refusal without claiming an ordering.
    pub fn compare_terms_with<E>(
        self,
        other: Self,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Ordering, E> {
        compare::asp_with(self, other, &mut before)
    }

    /// Compare storage identity with an owned ingress value, borrowing both.
    /// The traversal bound is the same as [`Self::compare_terms`].
    #[must_use]
    pub fn compare(self, other: &Value) -> Ordering {
        compare::term_value(self, other)
    }

    /// Checked storage comparison against another borrowed term. The callback
    /// precedes every logical storage/navigation probe, descriptor comparison,
    /// compared text-byte pair and sequence termination. Navigation events make
    /// this a different work schedule from [`Value::compare_identity_with`].
    /// It allocates nothing and does not inspect an unneeded payload suffix.
    /// Equal canonical identities require one check and no payload traversal.
    /// Two complete ingress values retain their legacy comparison trace.
    ///
    /// # Errors
    /// Returns the first callback error before its operation, without an ordering.
    pub fn compare_ref_with<E>(
        self,
        other: Self,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Ordering, E> {
        if let (Some(left), Some(right)) = (self.ingress(), other.ingress()) {
            return left.compare_identity_with(right, before);
        }
        compare::term_with(self, other, &mut before)
    }

    /// Test typed equality without allocation. Terms in the same canonical
    /// scope require one check and no payload traversal, whether equal or not.
    /// This relies on that scope's unique interning of equal typed terms.
    /// Different scopes use checked content comparison; two complete ingress
    /// values retain their legacy comparison trace. IDs never determine term order.
    ///
    /// # Errors
    /// Returns the first callback error before its operation, without an answer.
    pub fn equals_ref_with<E>(
        self,
        other: Self,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<bool, E> {
        if self.is_canonical() && other.is_canonical() {
            before()?;
            if let Some(equal) = self.canonical_equality(other) {
                return Ok(equal);
            }
            return compare::term_contents_with(self, other, &mut before).map(Ordering::is_eq);
        }
        self.compare_ref_with(other, before).map(Ordering::is_eq)
    }

    /// Checked storage comparison against an ingress value, with the navigation
    /// and comparison boundaries of [`Self::compare_ref_with`].
    ///
    /// # Errors
    /// Returns the first callback error before its operation, without an ordering.
    pub fn compare_identity_with<E>(
        self,
        other: &Value,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Ordering, E> {
        if let Some(value) = self.ingress() {
            return value.compare_identity_with(other, before);
        }
        compare::term_value_with(self, other, &mut before)
    }

    /// Explicitly construct an owned ingress value. Copies the expanded flat
    /// descriptions once; this is not an execution lookup or an implicit clone.
    /// Limits cover expanded nodes, depth, owned buffers and constructor scratch.
    /// Canonical traversal has the bound of [`Self::compare_terms`].
    ///
    /// # Errors
    /// Refuses exceeded construction limits or failed buffer reservations. No
    /// partial value is returned. Arc envelopes follow [`Value::from_nodes`]'s
    /// allocation behavior; this is not universal allocation recovery.
    pub fn to_value(self, limits: ValueLimits) -> Result<Value, ValueError> {
        ceiling(
            ValueResource::Nodes,
            self.expanded_nodes() as u128,
            limits.max_nodes,
        )?;
        ceiling(ValueResource::Depth, self.depth() as u128, limits.max_depth)?;
        let frames = self.expanded_nodes().min(limits.max_depth);
        let scratch = frames as u128
            * (std::mem::size_of::<usize>() + std::mem::size_of::<(usize, bool, bool)>()) as u128;
        let mut bytes =
            self.expanded_nodes() as u128 * std::mem::size_of::<ValueNode>() as u128 + scratch;
        ceiling(ValueResource::Bytes, bytes, limits.max_bytes)?;
        for node in TermNodes::new(self) {
            bytes += node.text_bytes() as u128;
            ceiling(ValueResource::Bytes, bytes, limits.max_bytes)?;
        }
        bytes += self.rendered_bytes() as u128;
        ceiling(ValueResource::Bytes, bytes, limits.max_bytes)?;
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(self.expanded_nodes())
            .map_err(|_| ValueError::Allocation)?;
        bytes += (nodes.capacity() - self.expanded_nodes()) as u128
            * std::mem::size_of::<ValueNode>() as u128;
        ceiling(ValueResource::Bytes, bytes, limits.max_bytes)?;
        for node in TermNodes::new(self) {
            nodes.push(copy_node(node, &mut bytes, limits.max_bytes)?);
        }
        Value::from_nodes(nodes, limits)
    }
}

impl fmt::Display for TermRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_with(f, usize::MAX, |_| Ok::<_, Infallible>(()))
            .map_err(|_| fmt::Error)
    }
}

impl PartialEq for TermRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.canonical_equality(*other)
            .unwrap_or_else(|| compare::term(*self, *other).is_eq())
    }
}
impl Eq for TermRef<'_> {}
impl PartialOrd for TermRef<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for TermRef<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        compare::term(*self, *other)
    }
}
impl Hash for TermRef<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let descriptor = self.descriptor();
        crate::term_hash::root(descriptor, state);
        if compare::structured(descriptor) {
            self.expanded_nodes().hash(state);
            for node in TermNodes::new(*self) {
                crate::term_hash::descriptor(node, state);
            }
        } else {
            crate::term_hash::descriptor(descriptor, state);
        }
    }
}
impl fmt::Debug for TermRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(TermNodes::new(*self)).finish()
    }
}
impl PartialEq<Value> for TermRef<'_> {
    fn eq(&self, other: &Value) -> bool {
        self.compare(other).is_eq()
    }
}
impl PartialOrd<Value> for TermRef<'_> {
    fn partial_cmp(&self, other: &Value) -> Option<Ordering> {
        Some(self.compare(other))
    }
}

/// A signed predicate signature borrowed from canonical storage or ingress.
/// Identity and order use name, arity and sign, independently of catalog IDs.
#[derive(Clone, Copy)]
pub struct PredicateRef<'a>(PredicateSource<'a>);

#[derive(Clone, Copy)]
enum PredicateSource<'a> {
    Canonical {
        snapshot: Read<'a>,
        id: PredicateId,
    },
    Ingress(&'a Predicate),
    Signed {
        name: &'a str,
        arity: usize,
        sign: Sign,
    },
}
// Canonical resolution happens once before reading the signature fields.
// This projection borrows the same admitted prefix as its PredicateRef.
#[derive(Clone, Copy)]
enum PredicateRead<'a> {
    Canonical(storage::Predicate<'a>),
    Ingress(&'a Predicate),
    Signed {
        name: &'a str,
        arity: usize,
        sign: Sign,
    },
}
impl<'a> PredicateRead<'a> {
    fn signature(self) -> (&'a str, usize, Sign) {
        match self {
            Self::Canonical(predicate) => (predicate.name(), predicate.arity(), predicate.sign()),
            Self::Ingress(predicate) => (predicate.name(), predicate.arity(), predicate.sign()),
            Self::Signed { name, arity, sign } => (name, arity, sign),
        }
    }
}
impl<'a> From<&'a Predicate> for PredicateRef<'a> {
    fn from(predicate: &'a Predicate) -> Self {
        Self(PredicateSource::Ingress(predicate))
    }
}
impl<'a> PredicateRef<'a> {
    pub(crate) fn new(snapshot: impl Into<Read<'a>>, id: PredicateId) -> Option<Self> {
        let snapshot = snapshot.into();
        snapshot.predicate(id)?;
        Some(Self(PredicateSource::Canonical { snapshot, id }))
    }

    pub(super) fn canonical(self) -> Option<(Read<'a>, PredicateId)> {
        match self.0 {
            PredicateSource::Canonical { snapshot, id } => Some((snapshot, id)),
            PredicateSource::Ingress(_) | PredicateSource::Signed { .. } => None,
        }
    }

    fn canonical_equality(self, other: Self) -> Option<bool> {
        let ((left, left_id), (right, right_id)) = (self.canonical()?, other.canonical()?);
        left.same_vocabulary(right).then_some(left_id == right_id)
    }

    fn read(self) -> PredicateRead<'a> {
        match self.0 {
            PredicateSource::Canonical { snapshot, id } => PredicateRead::Canonical(
                snapshot
                    .predicate(id)
                    .expect("validated predicate remains in its read prefix"),
            ),
            PredicateSource::Ingress(predicate) => PredicateRead::Ingress(predicate),
            PredicateSource::Signed { name, arity, sign } => {
                PredicateRead::Signed { name, arity, sign }
            }
        }
    }
    /// Borrow this name and arity with an explicit classical sign. No text is
    /// copied and no signature is interned. A changed sign carries no canonical
    /// ID; identity comparison still includes its complete borrowed signature.
    #[must_use]
    pub fn with_sign(self, sign: Sign) -> Self {
        let (name, arity, previous) = self.read().signature();
        if previous == sign {
            self
        } else {
            Self(PredicateSource::Signed { name, arity, sign })
        }
    }
    /// Exact borrowed predicate name. Canonical access includes segment resolution.
    #[must_use]
    pub fn name(self) -> &'a str {
        self.read().signature().0
    }
    /// Number of arguments, without visiting them.
    #[must_use]
    pub fn arity(self) -> usize {
        self.read().signature().1
    }
    /// Classical predicate sign, independent of default negation.
    #[must_use]
    pub fn sign(self) -> Sign {
        self.read().signature().2
    }
    /// Compare with an owned signature using exact name bytes, arity and sign.
    #[must_use]
    pub fn compare(self, other: &Predicate) -> Ordering {
        self.cmp(&PredicateRef::from(other))
    }
    /// Checked signature comparison. Borrowed ingress pairs preserve the legacy
    /// shared-name shortcut and callback trace. Equal canonical identities need
    /// one check; other canonical comparisons charge resolution and text reads.
    ///
    /// # Errors
    /// Returns the first callback refusal before its operation.
    pub fn compare_ref_with<E>(
        self,
        other: Self,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Ordering, E> {
        if let (PredicateSource::Ingress(left), PredicateSource::Ingress(right)) = (self.0, other.0)
        {
            return crate::identity::predicate(left, right, &mut before);
        }
        if matches!(self.0, PredicateSource::Canonical { .. })
            && matches!(other.0, PredicateSource::Canonical { .. })
        {
            before()?;
            if self.canonical_equality(other) == Some(true) {
                return Ok(Ordering::Equal);
            }
        }
        self.compare_signature_with(other, &mut before)
    }

    fn compare_signature_with<E>(
        self,
        other: Self,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Ordering, E> {
        before()?;
        let left = self.read().signature();
        before()?;
        let right = other.read().signature();
        before()?;
        Ok(
            crate::identity::bytes(left.0.as_bytes(), right.0.as_bytes(), before)?
                .then_with(|| left.1.cmp(&right.1))
                .then_with(|| left.2.cmp(&right.2)),
        )
    }
    /// Checked comparison with ingress, using [`Self::compare_ref_with`].
    ///
    /// # Errors
    /// Returns the first callback refusal before its operation.
    pub fn compare_identity_with<E>(
        self,
        other: &Predicate,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<Ordering, E> {
        self.compare_ref_with(PredicateRef::from(other), before)
    }

    /// Test signed predicate equality. A shared canonical vocabulary interns
    /// each name, arity and sign once, so its IDs answer after one check without
    /// reading text. Other pairs use checked signature comparison; ingress pairs
    /// preserve their legacy callback trace. IDs do not determine predicate order.
    ///
    /// # Errors
    /// Returns the first callback refusal before its operation.
    pub fn equals_ref_with<E>(
        self,
        other: Self,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<bool, E> {
        if matches!(self.0, PredicateSource::Canonical { .. })
            && matches!(other.0, PredicateSource::Canonical { .. })
        {
            before()?;
            if let Some(equal) = self.canonical_equality(other) {
                return Ok(equal);
            }
            return self
                .compare_signature_with(other, &mut before)
                .map(Ordering::is_eq);
        }
        self.compare_ref_with(other, before).map(Ordering::is_eq)
    }
}
impl PartialEq for PredicateRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.canonical_equality(*other)
            .unwrap_or_else(|| self.cmp(other).is_eq())
    }
}
impl Eq for PredicateRef<'_> {}
impl PartialOrd for PredicateRef<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for PredicateRef<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.read().signature().cmp(&other.read().signature())
    }
}
impl Hash for PredicateRead<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let (name, arity, sign) = self.signature();
        name.hash(state);
        arity.hash(state);
        sign.hash(state);
    }
}
impl Hash for PredicateRef<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.read().hash(state);
    }
}
impl fmt::Debug for PredicateRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (name, arity, sign) = self.read().signature();
        f.debug_struct("Predicate")
            .field("name", &name)
            .field("arity", &arity)
            .field("sign", &sign)
            .finish()
    }
}
impl PartialEq<Predicate> for PredicateRef<'_> {
    fn eq(&self, other: &Predicate) -> bool {
        self.compare(other).is_eq()
    }
}

/// One complete atom borrowed from a canonical snapshot or owned ingress atom.
/// Clone/Copy preserves the borrow without copying arguments. Eq/Ord/Hash use
/// signed predicate and complete argument contents across independent catalogs.
#[derive(Clone, Copy)]
pub struct AtomRef<'a>(AtomSource<'a>);

#[derive(Clone, Copy)]
enum AtomSource<'a> {
    Canonical { snapshot: Read<'a>, id: AtomId },
    Ingress(&'a Atom),
    Carrier(&'a crate::carrier::CarrierAtom),
}
// Every atom row was admitted only after its predicate and argument IDs.
// Its references therefore remain valid in this same read prefix; following a
// row link does not need another membership check or a copied argument payload.
// One resolution serves a whole traversal: the row cannot change while its
// prefix is borrowed.
#[derive(Clone, Copy)]
pub(super) enum AtomRead<'a> {
    Canonical {
        snapshot: Read<'a>,
        row: storage::Atom<'a>,
    },
    Ingress(&'a Atom),
    Carrier(&'a crate::carrier::CarrierAtom),
}
impl<'a> AtomRead<'a> {
    pub(super) fn predicate(self) -> PredicateRef<'a> {
        match self {
            Self::Canonical { snapshot, row } => PredicateRef(PredicateSource::Canonical {
                snapshot,
                id: row.predicate(),
            }),
            Self::Ingress(atom) => PredicateRef::from(atom.predicate()),
            Self::Carrier(atom) => atom.predicate(),
        }
    }

    pub(super) fn argument(self, column: usize) -> Option<TermRef<'a>> {
        match self {
            Self::Canonical { snapshot, row } => Some(TermRef(TermSource::Canonical {
                snapshot,
                id: row.argument(column)?,
            })),
            Self::Ingress(atom) => atom.values().get(column).map(TermRef::from),
            Self::Carrier(atom) => atom.argument(column),
        }
    }

    // Callers have established column < arity() from this same resolved row.
    // Iterators use this invariant to provide their exact-size contract; checked
    // comparisons invoke it only after admitting the corresponding lookup.
    pub(super) fn at_valid_column(self, column: usize) -> TermRef<'a> {
        self.argument(column)
            .expect("column is within the admitted atom arity")
    }

    // A carrier atom holds one domain coordinate per argument of its signature.
    pub(super) fn arity(self) -> usize {
        match self {
            Self::Canonical { row, .. } => row.arity(),
            Self::Ingress(atom) => atom.predicate().arity(),
            Self::Carrier(atom) => atom.coordinates().len(),
        }
    }

    pub(super) fn arguments(self) -> ArgumentIter<'a> {
        ArgumentIter {
            atom: self,
            front: 0,
            back: self.arity(),
        }
    }
}
impl<'a> From<&'a Atom> for AtomRef<'a> {
    fn from(atom: &'a Atom) -> Self {
        Self(AtomSource::Ingress(atom))
    }
}
impl<'a> From<&'a crate::carrier::CarrierAtom> for AtomRef<'a> {
    fn from(atom: &'a crate::carrier::CarrierAtom) -> Self {
        Self(AtomSource::Carrier(atom))
    }
}
impl<'a> AtomRef<'a> {
    pub(super) fn canonical(self) -> Option<(Read<'a>, AtomId)> {
        match self.0 {
            AtomSource::Canonical { snapshot, id } => Some((snapshot, id)),
            AtomSource::Ingress(_) | AtomSource::Carrier(_) => None,
        }
    }

    pub(super) fn new(snapshot: impl Into<Read<'a>>, id: AtomId) -> Option<Self> {
        let snapshot = snapshot.into();
        snapshot.atom(id)?;
        Some(Self(AtomSource::Canonical { snapshot, id }))
    }
    pub(super) fn read(self) -> AtomRead<'a> {
        match self.0 {
            AtomSource::Canonical { snapshot, id } => AtomRead::Canonical {
                snapshot,
                row: snapshot
                    .atom(id)
                    .expect("validated atom remains in its read prefix"),
            },
            AtomSource::Ingress(atom) => AtomRead::Ingress(atom),
            AtomSource::Carrier(atom) => AtomRead::Carrier(atom),
        }
    }
    pub(super) fn same_identity(self, other: Self) -> bool {
        if let Some(equal) = self.canonical_equality(other) {
            return equal;
        }
        match (self.0, other.0) {
            (AtomSource::Ingress(left), AtomSource::Ingress(right)) => std::ptr::eq(left, right),
            (AtomSource::Carrier(left), AtomSource::Carrier(right)) => left.same_identity(right),
            _ => false,
        }
    }

    // An atom scope interns each complete signed tuple to one ID. The view's
    // constructor has already admitted its prefix. Sharing vocabulary alone is
    // insufficient: independent tuple writers have different atom scopes.
    fn canonical_equality(self, other: Self) -> Option<bool> {
        let ((left, left_id), (right, right_id)) = (self.canonical()?, other.canonical()?);
        left.same_atoms(right).then_some(left_id == right_id)
    }
    /// Borrow the signed signature without copying its name.
    #[must_use]
    pub fn predicate(self) -> PredicateRef<'a> {
        self.read().predicate()
    }
    /// Borrow the ordered argument columns. No temporary row is allocated.
    #[must_use]
    pub const fn values(self) -> Arguments<'a> {
        Arguments { atom: self }
    }
    /// Borrow the ordered arguments; the same view as [`Self::values`].
    #[must_use]
    pub const fn arguments(self) -> Arguments<'a> {
        self.values()
    }
    /// Compare with an owned ingress atom without materializing a row.
    #[must_use]
    pub fn compare(self, other: &Atom) -> Ordering {
        compare::atom_value(self, other)
    }
    /// Checked semantic comparison with another borrowed atom. Charges logical
    /// signature/argument resolution, term navigation and compared text bytes.
    /// Equal canonical row identities require one check without visiting fields.
    /// Two ingress atoms retain the legacy comparison trace and name sharing.
    ///
    /// # Errors
    /// Returns the first callback refusal before its operation.
    pub fn compare_ref_with<E>(
        self,
        other: Self,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Ordering, E> {
        if let (AtomSource::Ingress(left), AtomSource::Ingress(right)) = (self.0, other.0) {
            return crate::identity::atom(left, right, &mut before);
        }
        compare::atom_with(self, other, &mut before)
    }

    /// Test complete typed equality without deriving an order from row IDs.
    /// Two admitted canonical rows from the same atom authority require one
    /// permit before checking scope and IDs, whether equal or unequal. Foreign
    /// authorities compare predicate and argument contents; sharing only a
    /// frozen vocabulary does not establish shared atom identity. Two ingress
    /// atoms retain the legacy comparison trace and name sharing.
    ///
    /// # Errors
    /// Returns the first callback refusal before its operation, never absence.
    pub fn equals_ref_with<E>(
        self,
        other: Self,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<bool, E> {
        if self.canonical().is_some() && other.canonical().is_some() {
            before()?;
            if let Some(equal) = self.canonical_equality(other) {
                return Ok(equal);
            }
            return compare::atom_contents_with(self, other, &mut before).map(Ordering::is_eq);
        }
        self.compare_ref_with(other, before).map(Ordering::is_eq)
    }
    /// Checked semantic comparison with ingress, using the work boundaries of
    /// [`Self::compare_ref_with`], not the legacy flat-value callback trace.
    ///
    /// # Errors
    /// Returns the first callback refusal before its operation.
    pub fn compare_identity_with<E>(
        self,
        other: &Atom,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Ordering, E> {
        if let AtomSource::Ingress(atom) = self.0 {
            return crate::identity::atom(atom, other, &mut before);
        }
        compare::atom_value_with(self, other, &mut before)
    }
    /// Checked storage comparison with a borrowed substitution key, in stored
    /// atom versus query order. Borrowed ingress preserves its legacy trace.
    ///
    /// # Errors
    /// Returns the first callback refusal before its operation.
    pub fn compare_key_with<E>(
        self,
        other: &crate::AtomKey<'_>,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Ordering, E> {
        if let AtomSource::Ingress(atom) = self.0 {
            return other
                .compare_identity_with(atom, before)
                .map(Ordering::reverse);
        }
        before()?;
        let atom = self.read();
        let predicate = atom.predicate();
        let query_predicate = other.predicate_with(&mut before)?;
        let order = predicate.compare_ref_with(query_predicate, &mut before)?;
        if !order.is_eq() {
            return Ok(order);
        }
        for column in 0..atom.arity() {
            before()?;
            let term = atom.at_valid_column(column);
            let query_term = other.argument_with(column, &mut before)?;
            let order = term.compare_ref_with(query_term, &mut before)?;
            if !order.is_eq() {
                return Ok(order);
            }
        }
        Ok(Ordering::Equal)
    }
    /// Explicitly copy an atom into the owned ingress representation. Each term
    /// obeys the node/depth limits; the byte allowance also includes the argument
    /// vector and predicate text, and is reduced by completed argument payloads.
    /// This allocates and must not be used as an implicit lookup adapter.
    ///
    /// # Errors
    /// Refuses exceeded construction limits or failed buffer reservations.
    /// No partial atom is returned. Shared owner envelopes retain the allocation
    /// behavior of the owned constructors, not universal allocation recovery.
    pub fn to_atom(self, limits: ValueLimits) -> Result<Atom, Fault> {
        let atom = self.read();
        let (name, arity, sign) = atom.predicate().read().signature();
        let requested = arity as u128 * std::mem::size_of::<Value>() as u128 + name.len() as u128;
        ceiling(ValueResource::Bytes, requested, limits.max_bytes)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(arity)
            .map_err(|_| Fault::Allocation)?;
        let mut bytes =
            values.capacity() as u128 * std::mem::size_of::<Value>() as u128 + name.len() as u128;
        ceiling(ValueResource::Bytes, bytes, limits.max_bytes)?;
        let name = copy_text(name, &mut bytes, limits.max_bytes)?;
        for term in atom.arguments() {
            let remaining =
                limits.max_bytes - usize::try_from(bytes).map_err(|_| Fault::Overflow)?;
            let value = term.to_value(ValueLimits {
                max_bytes: remaining,
                ..limits
            })?;
            bytes += value
                .checked_payload_capacity_bytes()
                .ok_or(Fault::Overflow)?;
            ceiling(ValueResource::Bytes, bytes, limits.max_bytes)?;
            values.push(value);
        }
        let predicate = Predicate::with_sign(name, arity, sign).map_err(|_| Fault::Shape)?;
        Ok(Atom::from_valid_parts(predicate, values))
    }
}
impl PartialEq for AtomRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.canonical_equality(*other)
            .unwrap_or_else(|| compare::atom(*self, *other).is_eq())
    }
}
impl Eq for AtomRef<'_> {}
impl PartialOrd for AtomRef<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for AtomRef<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        compare::atom(*self, *other)
    }
}
impl Hash for AtomRef<'_> {
    // One row and one signature resolution produce the writes of the ingress
    // atom with equal contents.
    fn hash<H: Hasher>(&self, state: &mut H) {
        let atom = self.read();
        atom.predicate().read().hash(state);
        atom.arity().hash(state);
        for term in atom.arguments() {
            term.hash(state);
        }
    }
}
impl fmt::Debug for AtomRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Atom")
            .field("predicate", &self.predicate())
            .field("values", &self.values())
            .finish()
    }
}
impl PartialEq<Atom> for AtomRef<'_> {
    fn eq(&self, other: &Atom) -> bool {
        self.compare(other).is_eq()
    }
}
impl PartialOrd<Atom> for AtomRef<'_> {
    fn partial_cmp(&self, other: &Atom) -> Option<Ordering> {
        Some(self.compare(other))
    }
}

/// Borrowed ordered argument columns. Indexing returns a reference value, not a
/// reference to a separately stored row cell. Canonical length and indexing each
/// resolve the row; an iterator resolves it once for its whole traversal.
/// Ingress access borrows its original argument slice.
#[derive(Clone, Copy)]
pub struct Arguments<'a> {
    atom: AtomRef<'a>,
}
impl<'a> Arguments<'a> {
    /// Argument count, without visiting argument payloads or the predicate name.
    #[must_use]
    pub fn len(self) -> usize {
        self.atom.read().arity()
    }
    /// Whether this is a nullary atom's empty tuple.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    /// Borrow the argument at a column; only out-of-arity indices are absent.
    #[must_use]
    pub fn at(self, column: usize) -> Option<TermRef<'a>> {
        self.atom.read().argument(column)
    }

    /// Borrow an argument, with the same meaning as [`Self::at`].
    #[must_use]
    pub fn get(self, column: usize) -> Option<TermRef<'a>> {
        self.at(column)
    }
    /// Exact-size, double-ended traversal of one resolved row; cloning copies
    /// only cursor state.
    #[must_use]
    pub fn iter(self) -> ArgumentIter<'a> {
        self.atom.read().arguments()
    }
}
impl fmt::Debug for Arguments<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
impl PartialEq for Arguments<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}
impl Eq for Arguments<'_> {}
impl PartialOrd for Arguments<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Arguments<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}
impl<'a> IntoIterator for Arguments<'a> {
    type Item = TermRef<'a>;
    type IntoIter = ArgumentIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Exact borrowed argument traversal. The row is resolved once, when the
/// traversal starts. No node, text or argument vector is copied.
#[derive(Clone)]
pub struct ArgumentIter<'a> {
    atom: AtomRead<'a>,
    front: usize,
    back: usize,
}
impl fmt::Debug for ArgumentIter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}
impl<'a> Iterator for ArgumentIter<'a> {
    type Item = TermRef<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        let column = self.front;
        self.front += 1;
        Some(self.atom.at_valid_column(column))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.back - self.front;
        (len, Some(len))
    }
}
impl DoubleEndedIterator for ArgumentIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        self.back -= 1;
        Some(self.atom.at_valid_column(self.back))
    }
}
impl ExactSizeIterator for ArgumentIter<'_> {}
impl FusedIterator for ArgumentIter<'_> {}

/// An allocation-free preorder traversal of one borrowed logical term.
/// A failed checked step leaves the cursor at its previous node.
pub struct TermNodes<'a> {
    term: TermRef<'a>,
    position: usize,
    canonical: Option<compare::Preorder<'a>>,
}
impl<'a> TermNodes<'a> {
    fn new(term: TermRef<'a>) -> Self {
        Self {
            term,
            position: 0,
            canonical: term.is_canonical().then(|| compare::Preorder::new(term)),
        }
    }
}
impl<'a> TermNodes<'a> {
    /// Read the next descriptor, checking the caller's allowance before each
    /// canonical navigation step or ingress read. No text is copied.
    ///
    /// # Errors
    /// Returns the caller's refusal without consuming the next descriptor.
    pub fn next_with<E>(
        &mut self,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<ValueNodeRef<'a>>, E> {
        if let Some(cursor) = &mut self.canonical {
            let mut staged = cursor.clone();
            let descriptor = if let Some(term) = staged.next_with(&mut before)? {
                before()?;
                Some(term.descriptor())
            } else {
                None
            };
            *cursor = staged;
            return Ok(descriptor);
        }
        before()?;
        let node = self.term.flat_node(self.position);
        if node.is_some() {
            self.position += 1;
        }
        Ok(node)
    }
}
impl<'a> Iterator for TermNodes<'a> {
    type Item = ValueNodeRef<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.next_with(|| Ok::<_, Infallible>(())) {
            Ok(node) => node,
            Err(never) => match never {},
        }
    }
}
impl FusedIterator for TermNodes<'_> {}

fn subtree_end(nodes: &[ValueNode], start: usize) -> usize {
    let mut pending = 1;
    let mut end = start;
    while pending != 0 {
        pending = pending - 1 + compare::arity(nodes[end].view());
        end += 1;
    }
    end
}

// A backwards scan skips complete preceding sibling trees until their parent
// has an unused child position. Following parents scans disjoint earlier spans,
// so each node's depth takes at most a linear scan of its prefix.
fn flat_depth(nodes: &[ValueNode]) -> usize {
    let mut maximum = 1;
    for position in 1..nodes.len() {
        let mut ancestor = position;
        let mut depth = 1;
        while ancestor != 0 {
            let mut siblings = 0;
            for candidate in (0..ancestor).rev() {
                let children = compare::arity(nodes[candidate].view());
                if children > siblings {
                    ancestor = candidate;
                    depth += 1;
                    break;
                }
                siblings = siblings + 1 - children;
            }
        }
        maximum = maximum.max(depth);
    }
    maximum
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

fn copy_text(text: &str, bytes: &mut u128, limit: usize) -> Result<String, ValueError> {
    let mut owned = String::new();
    owned
        .try_reserve_exact(text.len())
        .map_err(|_| ValueError::Allocation)?;
    *bytes += (owned.capacity() - text.len()) as u128;
    ceiling(ValueResource::Bytes, *bytes, limit)?;
    owned.push_str(text);
    Ok(owned)
}

fn copy_node(
    node: ValueNodeRef<'_>,
    bytes: &mut u128,
    limit: usize,
) -> Result<ValueNode, ValueError> {
    Ok(match node {
        ValueNodeRef::Infimum => ValueNode::Infimum,
        ValueNodeRef::Supremum => ValueNode::Supremum,
        ValueNodeRef::Number(number) => ValueNode::Number(number),
        ValueNodeRef::String(text) => ValueNode::String(copy_text(text, bytes, limit)?),
        ValueNodeRef::Symbol(text) => ValueNode::Symbol(copy_text(text, bytes, limit)?),
        ValueNodeRef::Function { name, sign, arity } => ValueNode::Function {
            name: copy_text(name, bytes, limit)?,
            sign,
            arity,
        },
        ValueNodeRef::Tuple { arity } => ValueNode::Tuple { arity },
    })
}

#[cfg(test)]
#[path = "equality_tests.rs"]
mod equality_tests;
#[cfg(test)]
#[path = "atom_equality_tests.rs"]
mod atom_equality_tests;

#[cfg(test)]
mod tests {
    use std::{
        cmp::Ordering,
        convert::Infallible,
        hash::{Hash, Hasher},
    };

    use crate::catalog::{
        Limits,
        storage::{AtomId, Snapshot, Store},
    };
    use crate::{Atom, AtomPattern, Predicate, Sign, Term, Value, ValueLimits, ValueNode};

    use super::{AtomRef, TermRef};

    fn structure() -> Value {
        Value::from_nodes(
            vec![
                ValueNode::Function {
                    name: "f".into(),
                    sign: Sign::Negative,
                    arity: 2,
                },
                ValueNode::Tuple { arity: 1 },
                ValueNode::String("é\n".into()),
                ValueNode::Function {
                    name: "g".into(),
                    sign: Sign::Negative,
                    arity: 0,
                },
            ],
            ValueLimits::default(),
        )
        .unwrap()
    }

    fn atom() -> Atom {
        Atom::new(
            Predicate::with_sign("p", 4, Sign::Negative).unwrap(),
            vec![
                Value::Number(-7),
                Value::String("same".into()),
                Value::Symbol("same".into()),
                structure(),
            ],
        )
        .unwrap()
    }

    fn snapshot(atom: &Atom, pad: bool) -> (Snapshot, AtomId) {
        let mut store = Store::new(16_000_000);
        if pad {
            store
                .import_value(&Value::Number(99), Limits::default())
                .unwrap();
        }
        let id = store.import_atom(atom, Limits::default()).unwrap();
        (store.snapshot(0).unwrap(), id)
    }

    #[derive(Default)]
    struct HashWrites(Vec<Vec<u8>>);
    impl Hasher for HashWrites {
        fn finish(&self) -> u64 {
            0
        }
        fn write(&mut self, bytes: &[u8]) {
            self.0.push(bytes.to_vec());
        }
    }
    fn hash_writes(value: &impl Hash) -> Vec<Vec<u8>> {
        let mut writes = HashWrites::default();
        value.hash(&mut writes);
        writes.0
    }

    #[test]
    fn semantic_hashes_agree_across_read_interfaces() {
        let atom = atom();
        let (left, left_id) = snapshot(&atom, false);
        let (right, right_id) = snapshot(&atom, true);
        let left = AtomRef::new(&left, left_id).unwrap();
        let right = AtomRef::new(&right, right_id).unwrap();
        let ingress = AtomRef::from(&atom);
        assert_eq!(left, right);
        assert_eq!(left, ingress);
        assert_eq!(hash_writes(&left), hash_writes(&atom));
        assert_eq!(hash_writes(&left), hash_writes(&right));
        assert_eq!(
            hash_writes(&left.predicate()),
            hash_writes(atom.predicate())
        );
        for (view, value) in left.values().iter().zip(atom.values()) {
            assert_eq!(hash_writes(&view), hash_writes(value));
            assert_eq!(view, *value);
        }
        let pattern = AtomPattern::new(
            atom.predicate().clone(),
            atom.values().iter().cloned().map(Term::Constant).collect(),
        )
        .unwrap();
        let empty: &[Value] = &[];
        assert_eq!(
            hash_writes(&left),
            hash_writes(&pattern.key(empty).unwrap())
        );
    }

    #[test]
    fn independent_snapshots_preserve_semantic_term_order() {
        let values = [
            Value::Infimum,
            Value::Number(-7),
            Value::String("z".into()),
            Value::Symbol("a".into()),
            structure(),
            Value::Supremum,
        ];
        let mut left = Store::new(16_000_000);
        let left_ids: Vec<_> = values
            .iter()
            .map(|value| left.import_value(value, Limits::default()).unwrap())
            .collect();
        let left = left.snapshot(0).unwrap();
        let mut right = Store::new(16_000_000);
        let right_ids: Vec<_> = values
            .iter()
            .rev()
            .map(|value| right.import_value(value, Limits::default()).unwrap())
            .collect();
        let right = right.snapshot(0).unwrap();
        for (i, value) in values.iter().enumerate() {
            let a = TermRef::new(&left, left_ids[i]).unwrap();
            for (j, other) in values.iter().enumerate() {
                let b = TermRef::new(&right, right_ids[values.len() - 1 - j]).unwrap();
                assert_eq!(a.cmp(&b), value.cmp(other));
                assert_eq!(a.compare_terms(b), value.compare_terms(other));
                assert_eq!(a.compare(other), value.cmp(other));
            }
        }
    }

    #[test]
    fn append_keeps_old_references_semantically_ordered() {
        let mut store = Store::new(16_000_000);
        let b = store
            .import_value(&Value::Symbol("b".into()), Limits::default())
            .unwrap();
        let z = store
            .import_value(&Value::Symbol("z".into()), Limits::default())
            .unwrap();
        let old = store.snapshot(0).unwrap();
        let a = store
            .import_value(&Value::Symbol("a".into()), Limits::default())
            .unwrap();
        let new = store.snapshot(0).unwrap();
        assert!(TermRef::new(&old, a).is_none());
        let old_b = TermRef::new(&old, b).unwrap();
        let new_b = TermRef::new(&new, b).unwrap();
        assert_eq!(old_b, new_b);
        assert!(TermRef::new(&old, z).unwrap() > new_b);
        assert!(TermRef::new(&new, a).unwrap() < old_b);
    }

    #[test]
    fn argument_iteration_preserves_both_ends() {
        let atom = atom();
        let (snapshot, id) = snapshot(&atom, false);
        for atom_ref in [AtomRef::new(&snapshot, id).unwrap(), AtomRef::from(&atom)] {
            let mut arguments = atom_ref.values().iter();
            assert_eq!(arguments.len(), 4);
            assert_eq!(arguments.next().unwrap(), Value::Number(-7));
            assert_eq!(arguments.next_back().unwrap(), structure());
            assert_eq!(arguments.len(), 2);
            assert_eq!(arguments.next_back().unwrap(), Value::Symbol("same".into()));
            assert_eq!(arguments.next().unwrap(), Value::String("same".into()));
            assert_eq!(arguments.len(), 0);
            assert!(arguments.next().is_none());
            assert!(arguments.next_back().is_none());
            assert!(atom_ref.values().at(4).is_none());
        }
    }

    fn nested(depth: usize) -> Value {
        let mut nodes = vec![
            ValueNode::Function {
                name: "h".into(),
                sign: Sign::Positive,
                arity: 1,
            };
            depth
        ];
        nodes.push(ValueNode::Symbol("x".into()));
        Value::from_nodes(nodes, ValueLimits::default()).unwrap()
    }

    // Arities zero, one and four, scalar and nested arguments; `-p/4` recurs in
    // a later publication with new vocabulary.
    fn traversed_atoms() -> Vec<Atom> {
        vec![
            atom(),
            Atom::new(Predicate::new("q", 0).unwrap(), Vec::new()).unwrap(),
            Atom::new(
                Predicate::with_sign("r", 1, Sign::Negative).unwrap(),
                vec![Value::Number(1)],
            )
            .unwrap(),
            Atom::new(
                Predicate::with_sign("p", 4, Sign::Negative).unwrap(),
                vec![
                    Value::Number(1),
                    Value::Symbol("a".into()),
                    Value::String("b".into()),
                    nested(3),
                ],
            )
            .unwrap(),
        ]
    }

    // One publication per atom: each row and its new vocabulary occupy their
    // own segment, and every earlier snapshot keeps its shorter prefix.
    fn segmented(atoms: &[Atom]) -> (Store, Vec<Snapshot>, Vec<AtomId>) {
        let mut store = Store::new(16_000_000);
        let mut snapshots = Vec::new();
        let mut ids = Vec::new();
        for atom in atoms {
            ids.push(store.import_atom(atom, Limits::default()).unwrap());
            snapshots.push(store.snapshot(0).unwrap());
        }
        (store, snapshots, ids)
    }

    #[test]
    fn argument_traversal_agrees_across_segments_and_readers() {
        let atoms = traversed_atoms();
        let (store, snapshots, ids) = segmented(&atoms);
        let latest = snapshots.last().unwrap();
        for (index, (atom, &id)) in atoms.iter().zip(&ids).enumerate() {
            for view in [
                AtomRef::new(&snapshots[index], id).unwrap(),
                AtomRef::new(latest, id).unwrap(),
                AtomRef::new(&store, id).unwrap(),
            ] {
                let arguments = view.values();
                assert_eq!(arguments.len(), atom.values().len());
                assert!(arguments.iter().eq(atom.values().iter().cloned()));
                assert!(
                    arguments
                        .iter()
                        .rev()
                        .eq(atom.values().iter().rev().cloned())
                );
                for (column, value) in atom.values().iter().enumerate() {
                    assert_eq!(arguments.at(column).unwrap(), *value);
                }
                assert!(arguments.at(atom.values().len()).is_none());
            }
        }
    }

    #[test]
    fn atom_hash_writes_agree_across_segments_and_readers() {
        let atoms = traversed_atoms();
        let (store, snapshots, ids) = segmented(&atoms);
        let latest = snapshots.last().unwrap();
        for (index, (atom, &id)) in atoms.iter().zip(&ids).enumerate() {
            for view in [
                AtomRef::new(&snapshots[index], id).unwrap(),
                AtomRef::new(latest, id).unwrap(),
                AtomRef::new(&store, id).unwrap(),
            ] {
                assert_eq!(hash_writes(&view), hash_writes(atom));
            }
        }
    }

    #[test]
    fn atom_order_agrees_with_ingress_across_owners() {
        // Reversed publication gives the right owner different IDs and segment
        // boundaries; the order must depend on contents alone.
        let atoms = traversed_atoms();
        let reversed: Vec<_> = atoms.iter().rev().cloned().collect();
        let (_, left, left_ids) = segmented(&atoms);
        let (_, right, right_ids) = segmented(&reversed);
        let (left, right) = (left.last().unwrap(), right.last().unwrap());
        for (left_atom, &left_id) in atoms.iter().zip(&left_ids) {
            let left_view = AtomRef::new(left, left_id).unwrap();
            for (right_atom, &right_id) in reversed.iter().zip(&right_ids) {
                let right_view = AtomRef::new(right, right_id).unwrap();
                let expected = AtomRef::from(left_atom).cmp(&AtomRef::from(right_atom));
                assert_eq!(left_view.cmp(&right_view), expected);
                assert_eq!(left_view.compare(right_atom), expected);
                assert_eq!(
                    left_view.compare_ref_with(right_view, || Ok::<_, Infallible>(())),
                    Ok(expected)
                );
            }
        }
    }

    #[test]
    fn checked_canonical_key_comparison_keeps_its_permit_schedule() {
        // A canonical atom compared with a substitution key reads its row
        // once; one permit still precedes each signature, argument and term
        // step, and a refusal stops before the step it would admit.
        let atom = atom();
        let (snapshot, id) = snapshot(&atom, false);
        let view = AtomRef::new(&snapshot, id).unwrap();
        let empty: &[Value] = &[];
        let key_of = |atom: &Atom| {
            AtomPattern::new(
                atom.predicate().clone(),
                atom.values().iter().cloned().map(Term::Constant).collect(),
            )
            .unwrap()
        };
        let pattern = key_of(&atom);
        let key = pattern.key(empty).unwrap();
        let mut permits = 0;
        assert_eq!(
            view.compare_key_with(&key, || {
                permits += 1;
                Ok::<_, Infallible>(())
            }),
            Ok(Ordering::Equal)
        );
        assert_eq!(permits, 70);
        for cutoff in 0..permits {
            let mut accepted = 0;
            assert_eq!(
                view.compare_key_with(&key, || {
                    if accepted == cutoff {
                        return Err("stop");
                    }
                    accepted += 1;
                    Ok(())
                }),
                Err("stop")
            );
            assert_eq!(accepted, cutoff);
        }
        for other in traversed_atoms() {
            let pattern = key_of(&other);
            let key = pattern.key(empty).unwrap();
            assert_eq!(
                view.compare_key_with(&key, || Ok::<_, Infallible>(())),
                AtomRef::from(&atom).compare_key_with(&key, || Ok::<_, Infallible>(()))
            );
        }
    }

    #[test]
    fn checked_foreign_atom_comparison_keeps_its_permit_schedule() {
        // Independent owners compare contents: one permit precedes each
        // signature, argument and term step, and a refusal stops before it.
        let atom = atom();
        let (_, left, left_ids) = segmented(std::slice::from_ref(&atom));
        let (_, right, right_ids) = segmented(&traversed_atoms());
        let left = AtomRef::new(left.last().unwrap(), left_ids[0]).unwrap();
        let right = AtomRef::new(right.last().unwrap(), right_ids[0]).unwrap();
        let mut permits = 0;
        assert_eq!(
            left.compare_ref_with(right, || {
                permits += 1;
                Ok::<_, Infallible>(())
            }),
            Ok(Ordering::Equal)
        );
        assert_eq!(permits, 96);
        for cutoff in 0..permits {
            let mut accepted = 0;
            assert_eq!(
                left.compare_ref_with(right, || {
                    if accepted == cutoff {
                        return Err("stop");
                    }
                    accepted += 1;
                    Ok(())
                }),
                Err("stop")
            );
            assert_eq!(accepted, cutoff);
        }
    }

    #[test]
    fn explicit_conversion_preserves_typed_payloads() {
        let atom = atom();
        let (snapshot, id) = snapshot(&atom, false);
        let canonical = AtomRef::new(&snapshot, id).unwrap();
        assert_eq!(canonical.to_atom(ValueLimits::default()).unwrap(), atom);
        for term in [
            canonical.values().at(3).unwrap(),
            TermRef::from(&atom.values()[3]),
        ] {
            let tuple = term.child(0).unwrap();
            assert_eq!(tuple.depth(), 2);
            assert_eq!(tuple.child(0).unwrap(), Value::String("é\n".into()));
            assert_eq!(
                tuple.to_value(ValueLimits::default()).unwrap(),
                Value::from_nodes(
                    vec![
                        ValueNode::Tuple { arity: 1 },
                        ValueNode::String("é\n".into())
                    ],
                    ValueLimits::default()
                )
                .unwrap()
            );
        }
    }

    #[test]
    fn explicit_conversion_refuses_tighter_logical_limits() {
        let atom = atom();
        let (snapshot, id) = snapshot(&atom, false);
        let term = AtomRef::new(&snapshot, id).unwrap().values().at(3).unwrap();
        assert!(matches!(
            term.to_value(ValueLimits {
                max_nodes: 1,
                ..ValueLimits::default()
            }),
            Err(crate::ValueError::Limit {
                resource: crate::ValueResource::Nodes,
                ..
            })
        ));
        assert!(matches!(
            term.to_value(ValueLimits {
                max_depth: 1,
                ..ValueLimits::default()
            }),
            Err(crate::ValueError::Limit {
                resource: crate::ValueResource::Depth,
                ..
            })
        ));
        assert!(matches!(
            term.to_value(ValueLimits {
                max_bytes: 0,
                ..ValueLimits::default()
            }),
            Err(crate::ValueError::Limit {
                resource: crate::ValueResource::Bytes,
                ..
            })
        ));
    }

    #[test]
    fn borrowed_ingress_keeps_legacy_comparison_work() {
        let left = atom();
        let right = left.clone();
        let mut legacy = 0;
        crate::identity::atom(&left, &right, &mut || {
            legacy += 1;
            Ok::<_, Infallible>(())
        })
        .unwrap();
        let mut unified = 0;
        assert_eq!(
            AtomRef::from(&left)
                .compare_ref_with(AtomRef::from(&right), || {
                    unified += 1;
                    Ok::<_, Infallible>(())
                })
                .unwrap(),
            Ordering::Equal
        );
        assert_eq!(unified, legacy);
        let pattern = AtomPattern::new(
            left.predicate().clone(),
            left.values().iter().cloned().map(Term::Constant).collect(),
        )
        .unwrap();
        let empty: &[Value] = &[];
        let key = pattern.key(empty).unwrap();
        let mut key_legacy = 0;
        key.compare_identity_with(&right, || {
            key_legacy += 1;
            Ok::<_, Infallible>(())
        })
        .unwrap();
        let mut key_unified = 0;
        AtomRef::from(&right)
            .compare_key_with(&key, || {
                key_unified += 1;
                Ok::<_, Infallible>(())
            })
            .unwrap();
        assert_eq!(key_unified, key_legacy);
    }
}
