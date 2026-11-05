//! Storage-transparent packed words and allocation-free borrowed operations.

use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

/// Packed physical lanes in caller-chosen storage, without an implicit universe.
///
/// Wrapping moves only the existing storage handle: it does not allocate, copy,
/// validate or clear tail bits. The semantic owner retains its extent, identity,
/// allocation policy and error accounting. `Vec`, boxed slices, borrowed slices
/// and inline arrays therefore keep their original storage and layout.
#[repr(transparent)]
#[derive(PartialEq, Eq)]
pub struct Words<L, S = Vec<L>> {
    storage: S,
    lane: PhantomData<L>,
}

impl<L, S> Words<L, S> {
    /// Wrap existing storage without reading or changing any lane.
    #[must_use]
    pub const fn new(storage: S) -> Self {
        Self {
            storage,
            lane: PhantomData,
        }
    }

    /// Borrow the original backing value, preserving its exact storage type.
    /// No lane traversal, normalization or allocation is performed.
    #[inline]
    #[must_use]
    pub fn storage(&self) -> &S {
        &self.storage
    }

    /// Borrow the original backing owner for its existing allocation operations.
    /// This accessor performs no allocation, resizing or lane traversal itself.
    #[inline]
    pub fn storage_mut(&mut self) -> &mut S {
        &mut self.storage
    }

    /// Return the same backing storage, preserving ownership and capacity.
    #[must_use]
    pub fn into_storage(self) -> S {
        self.storage
    }
}

impl<L, S: Default> Default for Words<L, S> {
    fn default() -> Self {
        Self::new(S::default())
    }
}

impl<L, S> From<S> for Words<L, S> {
    fn from(storage: S) -> Self {
        Self::new(storage)
    }
}

impl<L, S: std::fmt::Debug> std::fmt::Debug for Words<L, S> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.storage.fmt(formatter)
    }
}

impl<L, S: Clone> Clone for Words<L, S> {
    fn clone(&self) -> Self {
        Self::new(self.storage.clone())
    }

    fn clone_from(&mut self, source: &Self) {
        // Preserve the backing owner's reuse policy, notably Box<[L]> and Vec<L>.
        self.storage.clone_from(&source.storage);
    }
}

impl<L, S: Copy> Copy for Words<L, S> {}

impl<L, S: AsRef<[L]>> Deref for Words<L, S> {
    type Target = [L];

    #[inline]
    fn deref(&self) -> &[L] {
        self.storage.as_ref()
    }
}

impl<L, S: AsRef<[L]> + AsMut<[L]>> DerefMut for Words<L, S> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [L] {
        self.storage.as_mut()
    }
}

impl<'a, L, S: AsRef<[L]>> IntoIterator for &'a Words<L, S> {
    type Item = &'a L;
    type IntoIter = std::slice::Iter<'a, L>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<L> Words<L> {
    /// Actual capacity of the existing vector; no allocator overhead is included.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.storage.capacity()
    }
}

/// The ascending set-bit positions of a single physical lane.
/// No semantic extent or owner identity is implied by a position.
#[derive(Clone, Debug)]
pub struct SetBits<L> {
    remaining: L,
}

macro_rules! lane {
    ($lane:ty) => {
        impl<S> Words<$lane, S> {
            /// The lane index and one-bit mask of a physical bit position.
            #[inline]
            #[must_use]
            pub const fn locate(index: usize) -> (usize, $lane) {
                (
                    index / <$lane>::BITS as usize,
                    1 << (index % <$lane>::BITS as usize),
                )
            }

            /// Number of set bits in one physical lane, including padding.
            #[inline]
            #[must_use]
            pub const fn popcount(word: $lane) -> u32 {
                word.count_ones()
            }

            /// Index of a lane's least set bit, or the lane width for zero.
            /// The zero result is a sentinel, not an in-lane bit position.
            #[inline]
            #[must_use]
            pub const fn first_bit(word: $lane) -> usize {
                word.trailing_zeros() as usize
            }

            /// Index of a lane's greatest set bit, or the lane width for zero.
            /// The zero result is a sentinel, not an in-lane bit position.
            #[inline]
            #[must_use]
            pub const fn last_bit(word: $lane) -> usize {
                if word == 0 {
                    <$lane>::BITS as usize
                } else {
                    (<$lane>::BITS - 1 - word.leading_zeros()) as usize
                }
            }

            /// Isolate a lane's least set bit; zero remains zero.
            #[inline]
            #[must_use]
            pub const fn lowest_bit(word: $lane) -> $lane {
                word & word.wrapping_neg()
            }

            /// Iterate one lane's set bits, least significant first.
            #[inline]
            #[must_use]
            pub const fn bits(word: $lane) -> SetBits<$lane> {
                SetBits { remaining: word }
            }
        }

        impl<S: AsRef<[$lane]>> Words<$lane, S> {
            /// Test a physical bit. The owner checks its logical extent separately.
            ///
            /// # Panics
            /// Panics if the bit's lane is outside the backing storage.
            #[inline]
            #[must_use]
            pub fn contains(&self, index: usize) -> bool {
                let (word, bit) = Self::locate(index);
                self[word] & bit != 0
            }

            /// Test an allocated physical bit, or return absence beyond storage.
            /// The lane is checked before computing its bit mask. The owner
            /// supplies any meaning for an unallocated lane or logical tail.
            #[inline]
            #[must_use]
            pub fn get_bit(&self, index: usize) -> Option<bool> {
                self.get(index / <$lane>::BITS as usize)
                    .map(|word| word & (1 << (index % <$lane>::BITS as usize)) != 0)
            }

            /// Ascending physical set-bit positions, without masking the tail.
            /// Visits each lane and each selected bit once, with constant space.
            pub fn positions(&self) -> impl Iterator<Item = usize> + '_ {
                self.iter().enumerate().flat_map(|(index, &word)| {
                    Self::bits(word).map(move |bit| index * <$lane>::BITS as usize + bit)
                })
            }

            /// Find the least physical set bit at or after `from`.
            /// Tail bits participate; no owner extent is inferred.
            #[inline]
            #[must_use]
            pub fn next_from(&self, from: usize) -> Option<usize> {
                match self.next_from_with(from, || Ok::<(), std::convert::Infallible>(())) {
                    Ok(position) => position,
                    Err(impossible) => match impossible {},
                }
            }

            /// Find the first set bit at or after `from`, polling before each lane.
            ///
            /// The first lane masks positions below `from`; later lanes are read
            /// in order. A match stops the scan immediately. A starting position
            /// beyond storage returns absence without calling `before_word`.
            /// Work is linear in the visited lanes, with constant auxiliary space.
            ///
            /// # Errors
            /// Returns the exact first callback refusal before reading that lane.
            #[inline]
            pub fn next_from_with<E>(
                &self,
                from: usize,
                mut before_word: impl FnMut() -> Result<(), E>,
            ) -> Result<Option<usize>, E> {
                let width = <$lane>::BITS as usize;
                let mut index = from / width;
                let mut mask = <$lane>::MAX << (from % width);
                while index < self.len() {
                    before_word()?;
                    let selected = self[index] & mask;
                    if selected != 0 {
                        return Ok(Some(index * width + Self::first_bit(selected)));
                    }
                    index += 1;
                    mask = <$lane>::MAX;
                }
                Ok(None)
            }

            /// Whether every stored bit here also occurs in `other`.
            ///
            /// Equal-length lanes are visited in ascending order. Each visited
            /// pair calls `before` separately before NOT, AND and the zero test.
            /// A missing bit decides false immediately; later lanes and callbacks
            /// are not visited. Empty storage decides true without a callback.
            /// All stored bits participate, including caller-owned padding.
            ///
            /// No mutation, allocation or retained state occurs. Work is three
            /// primitive operations per visited lane, with constant space.
            /// The completed prefix has no bit absent from `other`; a nonzero
            /// difference is the deciding witness. Each step shortens the suffix.
            ///
            /// # Errors
            /// Returns the exact first callback refusal before its operation.
            /// An unfinished scan proves neither containment nor its negation.
            ///
            /// # Panics
            /// Panics before any callback if the storage lengths differ.
            #[inline]
            pub fn is_subset_of<E>(
                &self,
                other: Words<$lane, &[$lane]>,
                before: &mut impl FnMut() -> Result<(), E>,
            ) -> Result<bool, E> {
                assert_eq!(self.len(), other.len(), "packed subset shapes agree");
                for (left, right) in self.iter().zip(other.iter()) {
                    before()?;
                    let absent = !*right;
                    before()?;
                    let missing = *left & absent;
                    before()?;
                    if missing != 0 {
                        return Ok(false);
                    }
                }
                Ok(true)
            }

            /// Whether the common lane prefix has any shared set bit.
            ///
            /// Calls `before` once before each pair's AND-and-test, in ascending
            /// order, stopping at the first match. Missing lanes contribute no
            /// match. This preserves a caller's pair-level accounting contract;
            /// it does not substitute for kernels that charge the two operations
            /// separately. No mutation or allocation occurs.
            ///
            /// # Errors
            /// Returns the callback's exact error before examining that pair.
            #[inline]
            pub fn intersects<E>(
                &self,
                other: Words<$lane, &[$lane]>,
                before: &mut impl FnMut() -> Result<(), E>,
            ) -> Result<bool, E> {
                for (&left, &right) in self.iter().zip(other.iter()) {
                    before()?;
                    if left & right != 0 {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }

        impl<S: AsRef<[$lane]> + AsMut<[$lane]>> Words<$lane, S> {
            /// Fill existing lanes in order, admitting each write separately.
            /// No allocation, resizing or tail normalization occurs.
            ///
            /// # Errors
            /// Returns a refusal before its write; only earlier lanes change.
            #[inline]
            pub fn fill_with<E>(
                &mut self,
                value: $lane,
                mut before: impl FnMut() -> Result<(), E>,
            ) -> Result<(), E> {
                for lane in self.iter_mut() {
                    before()?;
                    *lane = value;
                }
                Ok(())
            }

            /// Union the common lane prefix, admitting each OR before mutation.
            /// Unpaired lanes are untouched; no allocation or tail masking occurs.
            ///
            /// # Errors
            /// Returns the first refusal, preserving that lane and the suffix.
            #[inline]
            pub fn union_assign<E>(
                &mut self,
                other: Words<$lane, &[$lane]>,
                before: impl FnMut() -> Result<(), E>,
            ) -> Result<(), E> {
                self.combine(other, before, |left, right| left | right)
            }

            /// Intersect the common lane prefix, admitting each AND before mutation.
            /// Unpaired lanes are untouched; no allocation or tail masking occurs.
            ///
            /// # Errors
            /// Returns the first refusal, preserving that lane and the suffix.
            #[inline]
            pub fn intersect_assign<E>(
                &mut self,
                other: Words<$lane, &[$lane]>,
                before: impl FnMut() -> Result<(), E>,
            ) -> Result<(), E> {
                self.combine(other, before, |left, right| left & right)
            }

            /// Symmetric difference over the common prefix, admitting each XOR.
            /// Unpaired lanes are untouched; no allocation or tail masking occurs.
            ///
            /// # Errors
            /// Returns the first refusal, preserving that lane and the suffix.
            #[inline]
            pub fn xor_assign<E>(
                &mut self,
                other: Words<$lane, &[$lane]>,
                before: impl FnMut() -> Result<(), E>,
            ) -> Result<(), E> {
                self.combine(other, before, |left, right| left ^ right)
            }

            #[inline]
            fn combine<E>(
                &mut self,
                other: Words<$lane, &[$lane]>,
                mut before: impl FnMut() -> Result<(), E>,
                operation: impl Fn($lane, $lane) -> $lane,
            ) -> Result<(), E> {
                for (left, &right) in self.iter_mut().zip(other.iter()) {
                    before()?;
                    *left = operation(*left, right);
                }
                Ok(())
            }

            /// Set one physical bit without inspecting or changing other lanes.
            ///
            /// # Panics
            /// Panics if the bit's lane is outside the backing storage.
            #[inline]
            pub fn insert(&mut self, index: usize) {
                let (word, bit) = Self::locate(index);
                self[word] |= bit;
            }

            /// Clear one physical bit without inspecting or changing other lanes.
            ///
            /// # Panics
            /// Panics if the bit's lane is outside the backing storage.
            #[inline]
            pub fn remove(&mut self, index: usize) {
                let (word, bit) = Self::locate(index);
                self[word] &= !bit;
            }
        }

        impl SetBits<$lane> {
            /// Advance one selected bit only after its work has been admitted.
            /// An empty lane makes no callback; a refusal leaves the cursor intact.
            ///
            /// # Errors
            /// Returns the callback's exact error before selecting or clearing a bit.
            #[inline]
            pub fn next_with<E>(
                &mut self,
                before: impl FnOnce() -> Result<(), E>,
            ) -> Result<Option<usize>, E> {
                self.advance(before, Ok)
            }

            /// Visit the least selected bit, clearing it only after success.
            /// An empty cursor makes no callback. Failed visitor effects remain
            /// the caller's responsibility; the cursor itself has not advanced.
            ///
            /// # Errors
            /// Returns the visitor's exact error, retaining the selected bit.
            #[inline]
            pub fn visit_next<E, T>(
                &mut self,
                visit: impl FnOnce(usize) -> Result<T, E>,
            ) -> Result<Option<T>, E> {
                self.advance(|| Ok(()), visit)
            }

            #[inline]
            fn advance<E, T>(
                &mut self,
                before: impl FnOnce() -> Result<(), E>,
                visit: impl FnOnce(usize) -> Result<T, E>,
            ) -> Result<Option<T>, E> {
                if self.remaining == 0 {
                    return Ok(None);
                }
                before()?;
                let bit = Words::<$lane>::first_bit(self.remaining);
                let result = visit(bit)?;
                self.remaining &= self.remaining - 1;
                Ok(Some(result))
            }
        }

        impl Iterator for SetBits<$lane> {
            type Item = usize;

            #[inline]
            fn next(&mut self) -> Option<Self::Item> {
                match self.next_with(|| Ok::<(), std::convert::Infallible>(())) {
                    Ok(position) => position,
                    Err(impossible) => match impossible {},
                }
            }
        }

        impl std::iter::FusedIterator for SetBits<$lane> {}
    };
}

lane!(u64);
lane!(u32);

impl<S: AsRef<[u64]>> Words<u64, S> {
    /// Extract one numeric 32-bit half, low half first, independently of byte order.
    /// This does not choose a logical export length or normalize unused bits.
    ///
    /// # Panics
    /// Panics if the containing 64-bit lane is outside the backing storage.
    #[inline]
    #[must_use]
    pub fn half32(&self, index: usize) -> u32 {
        let word = self[index / 2];
        let bytes = (word >> ((index % 2) * 32)).to_le_bytes();
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }
}
