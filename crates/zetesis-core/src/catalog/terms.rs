//! Scoped term metadata. Frames and sets retain IDs, never typed payload.

use std::{fmt, mem::size_of};

use super::{CatalogRead, Error, ReadError, TermRead, TermRef, storage};
use crate::BindingView;

/// A transferable term reference retaining only its vocabulary's identity.
/// Resolution requires a live compatible reader covering this term's prefix.
/// This is a transient capability; frames and sets keep one witness per container.
#[derive(Debug)]
pub struct TermKey {
    pub(super) scope: storage::VocabularyScope,
    pub(super) id: storage::TermId,
}

/// Optional variable slots in one vocabulary. Absence is not an ASP value.
/// Copying a frame uses the checked operation, not an implicit payload clone.
#[derive(Debug)]
pub struct TermAssignment {
    scope: storage::VocabularyScope,
    slots: Vec<Option<storage::TermId>>,
}

/// Borrowed slots retaining the source frame's exact vocabulary witness.
#[derive(Clone, Copy, Debug)]
pub struct AssignmentSlice<'a> {
    pub(super) scope: &'a storage::VocabularyScope,
    pub(super) slots: &'a [Option<storage::TermId>],
}

/// Exact selected-term membership; numeric ID order has no semantic ordering role.
/// Only explicitly inserted roots count. Interned children are not auto-selected.
#[derive(Debug)]
pub struct TermSet {
    scope: storage::VocabularyScope,
    words: Vec<u64>,
    count: usize,
}

/// A term frame or set cannot perform the requested operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssignmentError {
    /// Vocabulary or accessible-prefix mismatch.
    Read(ReadError),
    /// A requested slot or prefix end is outside the frame.
    Slot {
        /// Requested variable position or prefix end.
        slot: usize,
        /// Available slot count.
        len: usize,
    },
    /// An operation requires a present value at this valid variable slot.
    Unbound {
        /// Existing slot with no bound term.
        slot: usize,
    },
    /// A named metadata allocation or size bound refused.
    Storage(Error),
}

/// Caller work refusal is distinct from a frame/storage error.
#[derive(Debug)]
pub enum AssignmentFailure<E> {
    /// The frame or set operation is invalid or exceeds its capacity allowance.
    Assignment(AssignmentError),
    /// The caller stopped before the next operation.
    Stopped(E),
}
impl fmt::Display for AssignmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => error.fmt(f),
            Self::Slot { slot, len } => write!(
                f,
                "term slot or prefix {slot} is outside frame length {len}"
            ),
            Self::Storage(error) => error.fmt(f),
            Self::Unbound { slot } => write!(f, "term slot {slot} is unbound"),
        }
    }
}
impl std::error::Error for AssignmentError {}
impl<E: fmt::Display> fmt::Display for AssignmentFailure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Assignment(error) => error.fmt(f),
            Self::Stopped(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for AssignmentFailure<E> {}
impl<E> From<AssignmentError> for AssignmentFailure<E> {
    fn from(error: AssignmentError) -> Self {
        Self::Assignment(error)
    }
}
impl From<ReadError> for AssignmentError {
    fn from(error: ReadError) -> Self {
        Self::Read(error)
    }
}
impl From<Error> for AssignmentError {
    fn from(error: Error) -> Self {
        Self::Storage(error)
    }
}

impl<'a> TermRead<'a> {
    /// Name an existing canonical term, without import or semantic lookup.
    /// # Errors
    /// Refuses ingress, foreign vocabulary, and an inaccessible newer term.
    pub fn term_key(self, value: TermRef<'_>) -> Result<TermKey, ReadError> {
        let id = self.selected_term(value)?;
        Ok(TermKey {
            scope: self.scope(),
            id,
        })
    }

    /// Authenticate a borrowed coordinate without retaining another scope handle.
    pub(super) fn selected_term(self, value: TermRef<'_>) -> Result<storage::TermId, ReadError> {
        let (read, id) = value.scoped().ok_or(ReadError::Uninterned)?;
        if !self.same(read) {
            return Err(ReadError::ForeignCatalog);
        }
        if !self.contains(id) {
            return Err(ReadError::OutsidePrefix);
        }
        Ok(id)
    }

    /// Resolve a scoped key into a payload-borrowing view.
    /// # Errors
    /// Refuses a foreign vocabulary or a reader predating the term.
    pub fn term(self, key: &TermKey) -> Result<TermRef<'a>, ReadError> {
        if !self.accepts(&key.scope) {
            return Err(ReadError::ForeignCatalog);
        }
        self.resolve(key.id).ok_or(ReadError::OutsidePrefix)
    }

    /// Empty scoped metadata; clones the identity witness and allocates no vector.
    #[must_use]
    pub fn assignment(self) -> TermAssignment {
        TermAssignment {
            scope: self.scope(),
            slots: Vec::new(),
        }
    }

    /// Empty selected-term membership with this vocabulary's identity witness.
    #[must_use]
    pub fn term_set(self) -> TermSet {
        TermSet {
            scope: self.scope(),
            words: Vec::new(),
            count: 0,
        }
    }
}

impl<'a> CatalogRead<'a> {
    /// Name an admitted term in this exact canonical prefix.
    /// # Errors
    /// Refuses ingress, foreign scopes, and newer identities.
    pub fn term_key(self, value: TermRef<'_>) -> Result<TermKey, ReadError> {
        TermRead::from(self).term_key(value)
    }
    /// Resolve a key in this exact canonical prefix.
    /// # Errors
    /// Refuses foreign scopes and newer identities.
    pub fn term(self, key: &TermKey) -> Result<TermRef<'a>, ReadError> {
        TermRead::from(self).term(key)
    }
    /// Empty ID-only variable slots under this vocabulary's witness.
    #[must_use]
    pub fn assignment(self) -> TermAssignment {
        TermRead::from(self).assignment()
    }
    /// Empty selected-root membership under this vocabulary's witness.
    #[must_use]
    pub fn term_set(self) -> TermSet {
        TermRead::from(self).term_set()
    }
}

impl TermAssignment {
    /// Borrow slots without exposing raw IDs.
    #[must_use]
    pub fn as_slice(&self) -> AssignmentSlice<'_> {
        AssignmentSlice {
            scope: &self.scope,
            slots: &self.slots,
        }
    }
    /// Number of source variable slots, including unbound slots.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }
    /// Whether the frame has no variable slots.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
    /// Allocated source-variable slots, including reusable empty scratch slots.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.slots.capacity()
    }
    /// Frame header and retained slot capacity; canonical payload is separate.
    #[must_use]
    pub fn retained_bytes(&self) -> usize {
        size_of::<Self>() + self.slots.capacity() * size_of::<Option<storage::TermId>>()
    }
    /// Borrow the selected prefix; suffix IDs impose no read-prefix requirement.
    /// # Errors
    /// Refuses a prefix beyond the frame's length.
    pub fn prefix(&self, end: usize) -> Result<AssignmentSlice<'_>, AssignmentError> {
        self.as_slice().prefix(end)
    }
    /// Obtain an optional scoped key; a missing variable and invalid index differ.
    /// # Errors
    /// Refuses an out-of-range variable slot.
    pub fn key(&self, slot: usize) -> Result<Option<TermKey>, AssignmentError> {
        self.as_slice().key(slot)
    }

    /// Replace this frame's slots using same-vocabulary IDs only. All slot-copy
    /// work is admitted before changing logical contents; refused reservation
    /// capacity remains inspectable through `retained_bytes`.
    /// # Errors
    /// Refuses a foreign source, named capacity, or caller work.
    pub fn copy_from_with<E>(
        &mut self,
        source: AssignmentSlice<'_>,
        max_bytes: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<(), AssignmentFailure<E>> {
        before().map_err(AssignmentFailure::Stopped)?;
        check_scope(&self.scope, source.scope)?;
        reserve(
            &mut self.slots,
            source.len(),
            size_of::<Self>(),
            max_bytes,
            &mut before,
        )?;
        for _ in source.slots {
            before().map_err(AssignmentFailure::Stopped)?;
        }
        self.slots.clear();
        self.slots.extend_from_slice(source.slots);
        Ok(())
    }

    /// Empty scratch slots in constant time, retaining scope and capacity.
    /// IDs own no payload and need no destructor traversal. This cleanup cannot
    /// refuse and is suitable when unwinding a stopped evaluation.
    pub fn reset(&mut self) {
        self.slots.clear();
    }

    /// Resize metadata, initializing new slots to absent. Capacity remains on shrink.
    /// Bytes bound this frame's header and old/replacement slot capacities.
    /// All initialization/removal work is admitted before changing the frame.
    /// # Errors
    /// Returns capacity or caller refusal; reserved capacity may remain on refusal.
    pub fn resize_with<E>(
        &mut self,
        len: usize,
        max_bytes: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<(), AssignmentFailure<E>> {
        before().map_err(AssignmentFailure::Stopped)?;
        reserve(
            &mut self.slots,
            len,
            size_of::<Self>(),
            max_bytes,
            &mut before,
        )?;
        for _ in 0..self.slots.len().abs_diff(len) {
            before().map_err(AssignmentFailure::Stopped)?;
        }
        self.slots.resize(len, None);
        Ok(())
    }
    /// Remove a suffix while retaining reusable ID capacity.
    /// # Errors
    /// Refuses a longer prefix or caller work before changing any slot.
    pub fn truncate_with<E>(
        &mut self,
        len: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<(), AssignmentFailure<E>> {
        before().map_err(AssignmentFailure::Stopped)?;
        if len > self.len() {
            return Err(AssignmentError::Slot {
                slot: len,
                len: self.len(),
            }
            .into());
        }
        for _ in len..self.len() {
            before().map_err(AssignmentFailure::Stopped)?;
        }
        self.slots.truncate(len);
        Ok(())
    }
    /// Install a same-vocabulary key, storing only its ID in the frame.
    /// # Errors
    /// Refuses foreign identity, an invalid slot, or caller work before mutation.
    pub fn set_with<E>(
        &mut self,
        slot: usize,
        key: &TermKey,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<(), AssignmentFailure<E>> {
        before().map_err(AssignmentFailure::Stopped)?;
        check_scope(&self.scope, &key.scope)?;
        let len = self.len();
        let target = self
            .slots
            .get_mut(slot)
            .ok_or(AssignmentError::Slot { slot, len })?;
        before().map_err(AssignmentFailure::Stopped)?;
        *target = Some(key.id);
        Ok(())
    }
    /// Clear one existing variable slot without a logical sentinel value.
    /// # Errors
    /// Refuses an invalid slot or caller work before mutation.
    pub fn clear_with<E>(
        &mut self,
        slot: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<(), AssignmentFailure<E>> {
        before().map_err(AssignmentFailure::Stopped)?;
        let len = self.len();
        let target = self
            .slots
            .get_mut(slot)
            .ok_or(AssignmentError::Slot { slot, len })?;
        before().map_err(AssignmentFailure::Stopped)?;
        *target = None;
        Ok(())
    }

    /// Exchange two variable slots, including their bound or absent state.
    /// This is metadata movement, not semantic ordering of term identities.
    /// # Errors
    /// Refuses either invalid index or caller work before changing either slot.
    pub fn swap_with<E>(
        &mut self,
        left: usize,
        right: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<(), AssignmentFailure<E>> {
        before().map_err(AssignmentFailure::Stopped)?;
        for slot in [left, right] {
            if slot >= self.len() {
                return Err(AssignmentError::Slot {
                    slot,
                    len: self.len(),
                }
                .into());
            }
        }
        before().map_err(AssignmentFailure::Stopped)?;
        self.slots.swap(left, right);
        Ok(())
    }
}

impl<'a> AssignmentSlice<'a> {
    pub(super) fn validate_with<'read, E>(
        self,
        read: impl Into<TermRead<'read>>,
        selected: &[usize],
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<(), AssignmentFailure<E>> {
        let read = read.into();
        before().map_err(AssignmentFailure::Stopped)?;
        if !read.accepts(self.scope) {
            return Err(AssignmentError::Read(ReadError::ForeignCatalog).into());
        }
        for &slot in selected {
            before().map_err(AssignmentFailure::Stopped)?;
            let id = self
                .slots
                .get(slot)
                .ok_or(AssignmentError::Slot {
                    slot,
                    len: self.len(),
                })?
                .ok_or(AssignmentError::Unbound { slot })?;
            before().map_err(AssignmentFailure::Stopped)?;
            if !read.contains(id) {
                return Err(AssignmentError::Read(ReadError::OutsidePrefix).into());
            }
        }
        Ok(())
    }

    /// Resolve one variable slot directly, without cloning an identity witness
    /// or validating unrelated cells. Absence is distinct from an invalid slot.
    /// # Errors
    /// Refuses foreign scope, slot/prefix bounds, or the caller's work boundary.
    pub fn term_with<'read, E>(
        self,
        read: impl Into<TermRead<'read>>,
        slot: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<TermRef<'read>>, AssignmentFailure<E>> {
        let read = read.into();
        before().map_err(AssignmentFailure::Stopped)?;
        if !read.accepts(self.scope) {
            return Err(AssignmentError::Read(ReadError::ForeignCatalog).into());
        }
        before().map_err(AssignmentFailure::Stopped)?;
        let id = self.slots.get(slot).ok_or(AssignmentError::Slot {
            slot,
            len: self.len(),
        })?;
        let Some(id) = id else {
            return Ok(None);
        };
        before().map_err(AssignmentFailure::Stopped)?;
        read.resolve(*id)
            .map(Some)
            .ok_or_else(|| AssignmentError::Read(ReadError::OutsidePrefix).into())
    }

    /// Number of selected variable slots.
    #[must_use]
    pub fn len(self) -> usize {
        self.slots.len()
    }
    /// Whether this selected prefix has no slots.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.slots.is_empty()
    }
    /// Borrow a shorter source-variable prefix without copying metadata.
    /// # Errors
    /// Refuses an end beyond the selected slice.
    pub fn prefix(self, end: usize) -> Result<Self, AssignmentError> {
        let slots = self.slots.get(..end).ok_or(AssignmentError::Slot {
            slot: end,
            len: self.len(),
        })?;
        Ok(Self {
            scope: self.scope,
            slots,
        })
    }
    /// Read presence only; false denotes an actual absent slot.
    /// # Errors
    /// Refuses an invalid slot instead of treating it as absent.
    pub fn is_bound(self, slot: usize) -> Result<bool, AssignmentError> {
        Ok(self
            .slots
            .get(slot)
            .ok_or(AssignmentError::Slot {
                slot,
                len: self.len(),
            })?
            .is_some())
    }
    /// Retain one transient scope witness for a present term.
    /// # Errors
    /// Refuses an invalid slot; absent valid slots return None.
    pub fn key(self, slot: usize) -> Result<Option<TermKey>, AssignmentError> {
        Ok(self
            .slots
            .get(slot)
            .ok_or(AssignmentError::Slot {
                slot,
                len: self.len(),
            })?
            .map(|id| TermKey {
                scope: self.scope.clone(),
                id,
            }))
    }
    /// Compare a present slot and a key by canonical identity order.
    /// This order is useful for an index within one vocabulary. It is not ASP
    /// term order and must not determine a logical comparison or output order.
    /// # Errors
    /// Refuses a foreign scope, an invalid slot, or an absent value.
    pub fn compare_key(
        self,
        slot: usize,
        key: &TermKey,
    ) -> Result<std::cmp::Ordering, AssignmentError> {
        if !self.scope.same(&key.scope) {
            return Err(ReadError::ForeignCatalog.into());
        }
        let id = self
            .slots
            .get(slot)
            .ok_or(AssignmentError::Slot {
                slot,
                len: self.len(),
            })?
            .ok_or(AssignmentError::Unbound { slot })?;
        Ok(id.cmp(&key.id))
    }
    /// Copy only ID slots, with one witness for the resulting frame.
    /// # Errors
    /// Returns named capacity or caller refusal; no partial frame is returned.
    pub fn copy_with<E>(
        self,
        max_bytes: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<TermAssignment, AssignmentFailure<E>> {
        before().map_err(AssignmentFailure::Stopped)?;
        let mut result = TermAssignment {
            scope: self.scope.clone(),
            slots: Vec::new(),
        };
        result.copy_from_with(self, max_bytes, before)?;
        Ok(result)
    }
    /// Validate this exact selected slice and borrow a resolved binding view.
    /// Costs O(slots), allocates nothing, and checks empty slots too. A newer
    /// unselected suffix cannot reject this prefix. The reader bounds view life.
    /// # Errors
    /// Refuses foreign/missing-prefix identities or caller work, never as absence.
    pub fn bind_with<'read, E>(
        self,
        read: impl Into<TermRead<'read>>,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<BindingView<'read>, AssignmentFailure<E>>
    where
        'a: 'read,
    {
        let read = read.into();
        before().map_err(AssignmentFailure::Stopped)?;
        if !read.accepts(self.scope) {
            return Err(AssignmentError::Read(ReadError::ForeignCatalog).into());
        }
        for slot in self.slots {
            before().map_err(AssignmentFailure::Stopped)?;
            if slot.is_some_and(|id| !read.contains(id)) {
                return Err(AssignmentError::Read(ReadError::OutsidePrefix).into());
            }
        }
        Ok(BindingView::canonical(read, self.slots))
    }
}

impl TermSet {
    /// Exact number of explicitly selected roots.
    #[must_use]
    pub fn len(&self) -> usize {
        self.count
    }
    /// Whether no root is selected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    /// Set header and bitset capacity; referenced payload belongs to the catalog.
    #[must_use]
    pub fn retained_bytes(&self) -> usize {
        size_of::<Self>() + self.words.capacity() * size_of::<u64>()
    }
    /// Exact selected-root membership within this vocabulary.
    /// # Errors
    /// Refuses foreign keys or caller work before reporting membership.
    pub fn contains_with<E>(
        &self,
        key: &TermKey,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<bool, AssignmentFailure<E>> {
        before().map_err(AssignmentFailure::Stopped)?;
        check_scope(&self.scope, &key.scope)?;
        before().map_err(AssignmentFailure::Stopped)?;
        let position = key.id.position();
        Ok(self
            .words
            .get(position / 64)
            .is_some_and(|word| word & (1 << (position % 64)) != 0))
    }
    /// Select a root once. A duplicate does not reserve or charge another term.
    /// Bytes bound this set's header and old/replacement word capacities.
    /// # Errors
    /// Refuses scope, capacity, or caller work without publishing a new member.
    pub fn insert_with<E>(
        &mut self,
        key: &TermKey,
        max_bytes: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<bool, AssignmentFailure<E>> {
        ceiling(self.retained_bytes() as u128, max_bytes)?;
        if self.contains_with(key, &mut before)? {
            return Ok(false);
        }
        let count = self
            .count
            .checked_add(1)
            .ok_or(AssignmentError::Storage(Error::Overflow))?;
        let position = key.id.position();
        let word = position / 64;
        let len = word
            .checked_add(1)
            .ok_or(AssignmentError::Storage(Error::Overflow))?;
        reserve(
            &mut self.words,
            len,
            size_of::<Self>(),
            max_bytes,
            &mut before,
        )?;
        for _ in self.words.len()..len {
            before().map_err(AssignmentFailure::Stopped)?;
        }
        before().map_err(AssignmentFailure::Stopped)?;
        self.words.resize(self.words.len().max(len), 0);
        self.words[word] |= 1 << (position % 64);
        self.count = count;
        Ok(true)
    }
}

fn check_scope(
    left: &storage::VocabularyScope,
    right: &storage::VocabularyScope,
) -> Result<(), AssignmentError> {
    if left.same(right) {
        Ok(())
    } else {
        Err(ReadError::ForeignCatalog.into())
    }
}

fn reserve<T, E>(
    values: &mut Vec<T>,
    len: usize,
    header: usize,
    max_bytes: usize,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<(), AssignmentFailure<E>> {
    let current = header as u128 + values.capacity() as u128 * size_of::<T>() as u128;
    ceiling(current, max_bytes)?;
    if len <= values.capacity() {
        return Ok(());
    }
    let target = len.max(values.capacity().saturating_mul(2));
    ceiling(current + target as u128 * size_of::<T>() as u128, max_bytes)?;
    for _ in 0..values.len() {
        before().map_err(AssignmentFailure::Stopped)?;
    }
    before().map_err(AssignmentFailure::Stopped)?;
    values
        .try_reserve_exact(target - values.len())
        .map_err(|_| AssignmentError::Storage(Error::Allocation))?;
    ceiling(
        current + values.capacity() as u128 * size_of::<T>() as u128,
        max_bytes,
    )?;
    Ok(())
}
fn ceiling(required: u128, limit: usize) -> Result<(), AssignmentError> {
    if required > limit as u128 {
        Err(Error::Storage { required, limit }.into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
