//! Borrowed domain restrictions and complete ordered row masks.

use std::{convert::Infallible, mem::size_of};

use zetesis_core::{Value, catalog::TermRef, relation::Relation};

use crate::Cancellation;

use super::{
    Failure, Limits, MeteredCause, MeteredFailure, Statistics, Table, WORD_BITS, Work, control,
    unmetered,
};

/// A permitted set of whole typed values, borrowed for one operation.
///
/// Empty finite domains permit no value. Unrestricted domains permit every
/// indexed value; they are not encoded by an empty list or a copied dictionary.
#[derive(Clone, Copy, Debug)]
pub enum Domain<'value> {
    /// No restriction on this variable beyond the supplied coherent relation.
    Unrestricted,
    /// Exactly one typed value, with no temporary one-element owner.
    Singleton(TermRef<'value>),
    /// A finite list with set semantics, including duplicates and absent values.
    Finite(Values<'value>),
}

/// A finite borrowed domain list, without an owned term dictionary.
///
/// Canonical callers supply a slice of [`TermRef`]s. Owned [`Value`] slices
/// are ingress descriptions and resolve to the same borrowed term interface.
/// Construction, indexing and iteration allocate nothing and copy only views;
/// the supplied payload and reference-list storage remain caller-owned.
///
/// ```
/// use zetesis_core::{Value, catalog::TermRef};
/// use zetesis_cpu::table::Values;
/// let ingress = [Value::Number(7)];
/// let references = [TermRef::from(&ingress[0])];
/// let source = Values::from(ingress.as_slice());
/// let borrowed = Values::from(references.as_slice());
/// assert!(source.iter().eq(borrowed.iter()));
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Values<'a>(ValueSource<'a>);

#[derive(Clone, Copy, Debug)]
enum ValueSource<'a> {
    Ingress(&'a [Value]),
    Terms(&'a [TermRef<'a>]),
}

impl<'a> From<&'a [Value]> for Values<'a> {
    fn from(values: &'a [Value]) -> Self {
        Self(ValueSource::Ingress(values))
    }
}
impl<'a> From<&'a [TermRef<'a>]> for Values<'a> {
    fn from(values: &'a [TermRef<'a>]) -> Self {
        Self(ValueSource::Terms(values))
    }
}
impl<'a> Values<'a> {
    /// Number of supplied values, including repetitions and unsupported values.
    #[must_use]
    pub const fn len(self) -> usize {
        match self.0 {
            ValueSource::Ingress(values) => values.len(),
            ValueSource::Terms(values) => values.len(),
        }
    }

    /// Whether this finite domain permits no value.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// Borrow the value at an in-range position, without copying payload.
    #[must_use]
    pub fn at(self, index: usize) -> Option<TermRef<'a>> {
        match self.0 {
            ValueSource::Ingress(values) => values.get(index).map(TermRef::from),
            ValueSource::Terms(values) => values.get(index).copied(),
        }
    }

    /// Original domain order, including repetitions. No term is materialized.
    #[must_use]
    pub fn iter(self) -> impl ExactSizeIterator<Item = TermRef<'a>> + DoubleEndedIterator + Clone {
        match self.0 {
            ValueSource::Ingress(values) => ValuesIter::Ingress(values.iter()),
            ValueSource::Terms(values) => ValuesIter::Terms(values.iter()),
        }
    }
}

/// Both input forms advance their own borrowed slice without an indexed lookup.
#[derive(Clone)]
enum ValuesIter<'a> {
    Ingress(std::slice::Iter<'a, Value>),
    Terms(std::slice::Iter<'a, TermRef<'a>>),
}
impl<'a> Iterator for ValuesIter<'a> {
    type Item = TermRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Ingress(values) => values.next().map(TermRef::from),
            Self::Terms(values) => values.next().copied(),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}
impl DoubleEndedIterator for ValuesIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Self::Ingress(values) => values.next_back().map(TermRef::from),
            Self::Terms(values) => values.next_back().copied(),
        }
    }
}
impl ExactSizeIterator for ValuesIter<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Ingress(values) => values.len(),
            Self::Terms(values) => values.len(),
        }
    }
}

/// Complete domain filtering over one exact immutable relation.
///
/// The mask owns its words and borrows only the relation, not the prepared
/// table or supplied domains. Preparation has already enforced every alias;
/// filtering has already checked every supplied domain. Dropping the index or
/// advancing a caller's bindings cannot change these selected row occurrences.
pub struct Selection<'owner, 'source> {
    relation: &'owner Relation<'source>,
    rows: Vec<u32>,
    statistics: Statistics,
}

impl<'owner, 'source> Table<'owner, 'source> {
    /// Select exact coherent rows without computing projected value domains.
    ///
    /// The first restriction initializes the result from coherent supports;
    /// an entirely unrestricted query copies the immutable coherent mask.
    /// Later singleton domains intersect one borrowed support directly; later
    /// finite domains use one reusable union scratch mask. Every domain is
    /// checked even when an earlier restriction leaves no surviving rows.
    /// No value, source row or selected-position vector is copied.
    ///
    /// For W row words, K variables, V indexed entries and D finite values,
    /// work is O(W + K + D*(1+log(V+1)+W) + K*W), plus typed comparison payload
    /// costs. An unrestricted variable costs one descriptor inspection; a
    /// singleton costs a lookup and W word operations. Retained result capacity
    /// includes only its header and mask. Operation peak additionally includes
    /// the borrowed relation/index and any temporary union mask. Other caller
    /// results are excluded; their simultaneous retention belongs to the caller.
    ///
    /// # Errors
    /// Refuses malformed domain count, finite limits, allocation failure or
    /// interrupted control. No partial mask is published. A refusal never means
    /// an empty relation or an inconsistent program.
    pub fn select(
        &self,
        domains: &[Domain<'_>],
        limits: Limits,
        cancellation: &Cancellation,
    ) -> Result<Selection<'owner, 'source>, Failure> {
        self.select_with(domains, limits, control(cancellation))
            .map_err(unmetered)
    }

    /// Select through the admission/control callback of [`Self::prepare_with`].
    /// Zero amounts poll control at entry/allocation/work boundaries; positive
    /// amounts request the next complete charged group before its effect. The
    /// same exact row selection and local limits implement [`Self::select`].
    /// Caller permits already accepted are retained on refusal, and the returned
    /// work receipt must not be charged to that caller a second time.
    ///
    /// # Errors
    /// Preserves local table failures and typed caller refusals with their
    /// admitted work and capacity prefixes. No partial selection escapes.
    pub fn select_with<E>(
        &self,
        domains: &[Domain<'_>],
        limits: Limits,
        before: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Selection<'owner, 'source>, MeteredFailure<E>> {
        let external = self.retained_inputs()?;
        let scratch_header = size_of::<Vec<u32>>();
        let mut work = Work::new(
            limits,
            before,
            external,
            size_of::<Selection<'_, '_>>() + scratch_header,
        )?;
        let result = (|| {
            self.check_domains(domains.len(), &work)?;
            let rows = self.restrict_rows(domains.iter().copied(), None, &mut work)?;
            work.release_bytes(scratch_header);
            Ok(Selection {
                relation: self.relation,
                rows,
                statistics: work.statistics(external),
            })
        })();
        result.map_err(|cause| work.failure(cause))
    }

    /// Restrict complete rows once. Full projection optionally records which
    /// support entries the domains permit while resolving those same lookups.
    /// This avoids both repeated lookup and testing every disallowed support.
    /// The caller has validated the domain count and owns the optional bitmap.
    pub(super) fn restrict_rows<'domain, E>(
        &self,
        domains: impl ExactSizeIterator<Item = Domain<'domain>>,
        mut permitted: Option<&mut [bool]>,
        work: &mut Work<impl FnMut(usize) -> Result<(), E>>,
    ) -> Result<Vec<u32>, MeteredCause<E>> {
        let mut rows = work.reserve(self.coherent.len())?;
        let mut initialized = false;
        let mut union = Vec::new();
        for (variable, domain) in domains.enumerate() {
            work.tick(1)?;
            match domain {
                Domain::Unrestricted => {
                    if let Some(permitted) = &mut permitted {
                        for entry in self.variables[variable].clone() {
                            permit(permitted, entry, work)?;
                        }
                    }
                }
                Domain::Singleton(value) => {
                    if let Some(entry) = self.lookup(variable, value, work)? {
                        if let Some(permitted) = &mut permitted {
                            permit(permitted, entry, work)?;
                        }
                        if initialized {
                            intersect(&mut rows, self.support(entry), work)?;
                        } else {
                            initialize(&mut rows, self.support(entry).iter().copied(), work)?;
                        }
                    } else if initialized {
                        clear(&mut rows, work)?;
                    } else {
                        initialize(&mut rows, std::iter::repeat_n(0, self.coherent.len()), work)?;
                    }
                    initialized = true;
                }
                Domain::Finite(values) => {
                    let target = if initialized {
                        if union.len() == rows.len() {
                            clear(&mut union, work)?;
                        } else {
                            union = work.zeros(rows.len())?;
                        }
                        &mut union
                    } else {
                        initialize(&mut rows, std::iter::repeat_n(0, self.coherent.len()), work)?;
                        &mut rows
                    };
                    self.union_supports(variable, values, target, &mut permitted, work)?;
                    if initialized {
                        intersect(&mut rows, &union, work)?;
                    }
                    initialized = true;
                }
            }
        }
        // Prepared supports contain only coherent original occurrences, so the
        // first restriction supplies its own base. No restriction (including a
        // nullary scope) still requires the explicit original coherent mask.
        if !initialized {
            initialize(&mut rows, self.coherent.iter().copied(), work)?;
        }
        work.release(union);
        Ok(rows)
    }

    fn union_supports<E>(
        &self,
        variable: usize,
        values: Values<'_>,
        target: &mut [u32],
        permitted: &mut Option<&mut [bool]>,
        work: &mut Work<impl FnMut(usize) -> Result<(), E>>,
    ) -> Result<(), MeteredCause<E>> {
        for value in values.iter() {
            work.tick(1)?;
            if let Some(entry) = self.lookup(variable, value, work)? {
                if let Some(permitted) = permitted {
                    permit(permitted, entry, work)?;
                }
                for (target, &word) in target.iter_mut().zip(self.support(entry)) {
                    work.tick(1)?;
                    *target |= word;
                }
            }
        }
        Ok(())
    }
}

impl<'owner, 'source> Selection<'owner, 'source> {
    /// Authoritative relation whose occurrence positions the mask denotes.
    #[must_use]
    pub const fn relation(&self) -> &'owner Relation<'source> {
        self.relation
    }

    /// Low-bit-first selected occurrences, with zero unused tail bits.
    #[must_use]
    pub fn words(&self) -> &[u32] {
        &self.rows
    }

    /// Whether an in-range original row occurrence is selected.
    #[must_use]
    pub fn contains(&self, row: usize) -> bool {
        row < self.relation.row_count()
            && self.rows[row / WORD_BITS] & (1 << (row % WORD_BITS)) != 0
    }

    /// First selected occurrence at or after `from`, or explicit exhaustion.
    ///
    /// Scans words from that position without allocating. Caller iteration work
    /// is separate from the completed selection's construction statistics.
    #[must_use]
    pub fn next_row(&self, from: usize) -> Option<usize> {
        next_row(&self.rows, from)
    }

    /// First selected occurrence, checking before each inspected mask word.
    ///
    /// Calls `before_word` once before reading each in-range word, including
    /// the first word and zero words traversed before exhaustion. No check is
    /// called when `from` lies beyond the mask's word storage. The callback can
    /// charge the consuming operation's work and poll its interruption control.
    /// The selection and the caller-supplied starting position remain unchanged;
    /// retrying repeats inspection from `from` with a fresh callback.
    ///
    /// # Errors
    /// Returns the callback's first error before inspecting that word. No row
    /// is published on error; successful earlier checks remain caller-owned work.
    pub fn next_row_with<E>(
        &self,
        from: usize,
        before_word: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<usize>, E> {
        next_row_with(&self.rows, from, before_word)
    }

    /// Increasing original row occurrences, without a position-vector owner.
    ///
    /// Complete traversal costs O(row words + selected rows), charged by the
    /// consuming operation rather than the earlier selection construction.
    pub fn rows(&self) -> impl Iterator<Item = usize> + Clone + '_ {
        Rows {
            words: &self.rows,
            next: 0,
        }
    }

    /// Selection work and owned result capacity, excluding earlier preparation.
    #[must_use]
    pub const fn statistics(&self) -> Statistics {
        self.statistics
    }
}

#[derive(Clone)]
struct Rows<'mask> {
    words: &'mask [u32],
    next: usize,
}

impl Iterator for Rows<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let row = next_row(self.words, self.next)?;
        // A valid occurrence is strictly below the relation's usize row count.
        self.next = row + 1;
        Some(row)
    }
}

fn next_row(words: &[u32], from: usize) -> Option<usize> {
    match next_row_with(words, from, || Ok::<(), Infallible>(())) {
        Ok(row) => row,
        Err(never) => match never {},
    }
}

fn next_row_with<E>(
    words: &[u32],
    from: usize,
    mut before_word: impl FnMut() -> Result<(), E>,
) -> Result<Option<usize>, E> {
    let first = from / WORD_BITS;
    let mut index = first;
    while index < words.len() {
        before_word()?;
        let mut word = words[index];
        if index == first {
            word &= u32::MAX << (from % WORD_BITS);
        }
        if word != 0 {
            return Ok(Some(index * WORD_BITS + word.trailing_zeros() as usize));
        }
        index += 1;
    }
    Ok(None)
}

fn permit<E>(
    permitted: &mut [bool],
    entry: usize,
    work: &mut Work<impl FnMut(usize) -> Result<(), E>>,
) -> Result<(), MeteredCause<E>> {
    work.tick(1)?;
    permitted[entry] = true;
    Ok(())
}

fn initialize<E>(
    rows: &mut Vec<u32>,
    words: impl Iterator<Item = u32>,
    work: &mut Work<impl FnMut(usize) -> Result<(), E>>,
) -> Result<(), MeteredCause<E>> {
    for word in words {
        work.tick(1)?;
        rows.push(word);
    }
    Ok(())
}

fn clear<E>(
    words: &mut [u32],
    work: &mut Work<impl FnMut(usize) -> Result<(), E>>,
) -> Result<(), MeteredCause<E>> {
    for word in words {
        work.tick(1)?;
        *word = 0;
    }
    Ok(())
}

fn intersect<E>(
    rows: &mut [u32],
    support: &[u32],
    work: &mut Work<impl FnMut(usize) -> Result<(), E>>,
) -> Result<(), MeteredCause<E>> {
    for (row, &word) in rows.iter_mut().zip(support) {
        work.tick(1)?;
        *row &= word;
    }
    Ok(())
}
