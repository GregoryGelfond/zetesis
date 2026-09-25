//! Sparse coordinates over an admitted program's immutable canonical vocabulary.

use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    iter::FusedIterator,
    sync::Arc,
};

use crate::{
    Program,
    catalog::{AtomRef, PredicateRef, TermRef},
    program::Predicates,
};

/// Carrier enumeration or lookup could not reserve tuple-coordinate storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarrierError {
    /// Signature, arity or argument ranks do not belong to this Program.
    Coordinates,
    /// Coordinate buffer reservation failed.
    Allocation,
    /// Named tuple header and coordinate capacity exceed the supplied allowance.
    Bytes {
        /// Requested or actual capacity, excluding Program and Arc bookkeeping.
        required: u128,
        /// Inclusive coordinate-storage allowance.
        limit: usize,
    },
}
impl fmt::Display for CarrierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Coordinates => {
                f.write_str("carrier coordinates lie outside the admitted program")
            }
            Self::Allocation => f.write_str("carrier tuple storage could not be reserved"),
            Self::Bytes { required, limit } => write!(
                f,
                "carrier coordinates require {required} bytes, allowance is {limit}"
            ),
        }
    }
}
impl std::error::Error for CarrierError {}

/// A checked sparse lookup stopped before publishing a coordinate token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CarrierFailure<E> {
    /// Coordinate storage could not be admitted.
    Storage(CarrierError),
    /// Caller work, cancellation or other injected refusal.
    Stopped(E),
}
impl<E: fmt::Display> fmt::Display for CarrierFailure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => error.fmt(f),
            Self::Stopped(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for CarrierFailure<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Storage(error) => error,
            Self::Stopped(error) => error,
        })
    }
}

#[derive(Clone, Copy)]
enum Query<'a> {
    Atom(AtomRef<'a>),
    Key(crate::AtomKey<'a>),
}
impl<'a> Query<'a> {
    fn predicate<E>(
        self,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<PredicateRef<'a>, E> {
        before()?;
        match self {
            Self::Atom(atom) => Ok(atom.predicate()),
            Self::Key(key) => key.predicate_with(before),
        }
    }
    fn argument<E>(
        self,
        column: usize,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<TermRef<'a>, E> {
        before()?;
        match self {
            Self::Atom(atom) => Ok(atom
                .values()
                .at(column)
                .expect("query column is within signed arity")),
            Self::Key(key) => key.argument_with(column, before),
        }
    }
}

/// One atom in a symbolic carrier, represented by signature and domain ranks.
///
/// The token retains its Program and O(arity) coordinate words, sharing the
/// program's canonical payload. It neither materializes an atom row nor needs
/// the full carrier cardinality to fit a machine integer. Clones share the
/// coordinate allocation. Arc envelopes follow Rust's infallible allocation
/// boundary; coordinate-buffer reservations are fallible.
///
/// Equality, ordering and hashing denote the logical atom across programs.
/// Program identity is separate and must be checked before candidate reuse.
#[derive(Clone, Debug)]
pub struct CarrierAtom {
    program: Program,
    tuple: Arc<Tuple>,
}
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Tuple {
    signature: usize,
    coordinates: Vec<usize>,
}
impl CarrierAtom {
    fn new(program: &Program, signature: usize, coordinates: Vec<usize>) -> Self {
        Self {
            program: program.clone(),
            tuple: Arc::new(Tuple {
                signature,
                coordinates,
            }),
        }
    }

    /// Borrow the logical row through its existing canonical vocabulary.
    #[must_use]
    pub fn atom(&self) -> AtomRef<'_> {
        self.into()
    }

    /// The immutable admitted instance establishing these coordinates.
    #[must_use]
    pub const fn program(&self) -> &Program {
        &self.program
    }

    /// Named coordinate storage, excluding the shared Program and Arc counters.
    /// Shared token clones name the same allocation, not additional payload.
    #[must_use]
    pub fn coordinate_bytes(&self) -> u128 {
        size_of::<Tuple>() as u128
            + self.tuple.coordinates.capacity() as u128 * size_of::<usize>() as u128
    }

    pub(crate) fn coordinates(&self) -> &[usize] {
        &self.tuple.coordinates
    }
    pub(crate) fn predicate(&self) -> PredicateRef<'_> {
        self.program
            .predicates()
            .get(self.tuple.signature)
            .expect("carrier signature was located in this immutable Program")
    }
    pub(crate) fn argument(&self, column: usize) -> Option<TermRef<'_>> {
        self.tuple.coordinates.get(column).map(|rank| {
            self.program
                .domain()
                .get(*rank)
                .expect("carrier coordinate was located in this immutable domain")
        })
    }
    pub(crate) fn same_identity(&self, other: &Self) -> bool {
        self.program.same_instance(&other.program) && self.tuple == other.tuple
    }
}
impl PartialEq for CarrierAtom {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for CarrierAtom {}
impl PartialOrd for CarrierAtom {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for CarrierAtom {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.program.same_instance(&other.program) {
            self.tuple.cmp(&other.tuple)
        } else {
            self.atom().cmp(&other.atom())
        }
    }
}
impl Hash for CarrierAtom {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.atom().hash(state);
    }
}

impl Program {
    /// Retain one explicitly addressed carrier tuple after checking every rank.
    /// The signature indexes the full predicate view. The supplied ranks index
    /// the explicit domain, not arbitrary interned subterms. Only O(arity) words
    /// are copied; no Cartesian enumeration or canonical row import occurs.
    ///
    /// # Errors
    /// Refuses invalid coordinates, coordinate storage or the first callback
    /// stop before a read, allocation, copy or final publication.
    pub fn carrier_atom_with<E>(
        &self,
        signature: usize,
        coordinates: &[usize],
        max_coordinate_bytes: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<CarrierAtom, CarrierFailure<E>> {
        let mut checked = || before().map_err(CarrierFailure::Stopped);
        checked()?;
        let predicate = self
            .predicates()
            .get(signature)
            .ok_or(CarrierFailure::Storage(CarrierError::Coordinates))?;
        if coordinates.len() != predicate.arity() {
            return Err(CarrierFailure::Storage(CarrierError::Coordinates));
        }
        coordinate_allowance(coordinates.len(), max_coordinate_bytes)
            .map_err(CarrierFailure::Storage)?;
        checked()?;
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(coordinates.len())
            .map_err(|_| CarrierFailure::Storage(CarrierError::Allocation))?;
        coordinate_allowance(retained.capacity(), max_coordinate_bytes)
            .map_err(CarrierFailure::Storage)?;
        for &coordinate in coordinates {
            checked()?;
            if coordinate >= self.domain().len() {
                return Err(CarrierFailure::Storage(CarrierError::Coordinates));
            }
            retained.push(coordinate);
        }
        checked()?;
        Ok(CarrierAtom::new(self, signature, retained))
    }

    /// Locate a sparse supplied atom without expanding the conceptual carrier.
    /// When `gates_only` is true, its signature must occur in a candidate gate.
    /// Accepted payload is borrowed only for lookup; the returned token retains
    /// integer coordinates into this Program. Canonical subterms outside the
    /// explicit domain are not admissible substitutions.
    ///
    /// # Errors
    /// Refuses unavailable coordinate storage. An outside-carrier atom is
    /// `Ok(None)`, distinct from this storage refusal. Lookup does not compute a
    /// full-carrier ordinal, even when that cardinality would overflow.
    pub fn locate_atom<'a>(
        &self,
        atom: impl Into<AtomRef<'a>>,
        gates_only: bool,
    ) -> Result<Option<CarrierAtom>, CarrierError> {
        self.locate_atom_with(atom, gates_only, usize::MAX, || {
            Ok::<_, std::convert::Infallible>(())
        })
        .map_err(|error| match error {
            CarrierFailure::Storage(error) => error,
            CarrierFailure::Stopped(never) => match never {},
        })
    }

    /// Checked sparse atom lookup under a coordinate-storage allowance.
    /// Callback admission precedes source reads, typed lookup probes, allocation,
    /// coordinate copies and publication. No token escapes after a refusal.
    ///
    /// # Errors
    /// Returns a coordinate-storage or callback refusal. `Ok(None)` means a
    /// completed lookup found a signature or value outside the supplied carrier.
    pub fn locate_atom_with<'a, E>(
        &self,
        atom: impl Into<AtomRef<'a>>,
        gates_only: bool,
        max_coordinate_bytes: usize,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<CarrierAtom>, CarrierFailure<E>> {
        self.locate(
            Query::Atom(atom.into()),
            gates_only,
            max_coordinate_bytes,
            before,
        )
    }

    /// Locate a checked substitution using the same sparse coordinate operation.
    /// No owned atom, predicate spelling or argument payload is constructed.
    ///
    /// # Errors
    /// Same storage, work and absence contract as [`Self::locate_atom_with`].
    pub fn locate_key_with<E>(
        &self,
        key: &crate::AtomKey<'_>,
        gates_only: bool,
        max_coordinate_bytes: usize,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<CarrierAtom>, CarrierFailure<E>> {
        self.locate(Query::Key(*key), gates_only, max_coordinate_bytes, before)
    }

    fn locate<E>(
        &self,
        query: Query<'_>,
        gates_only: bool,
        limit: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<CarrierAtom>, CarrierFailure<E>> {
        let mut checked = || before().map_err(CarrierFailure::Stopped);
        let predicate = query.predicate(&mut checked)?;
        if gates_only
            && self
                .gate_predicates()
                .binary_search_with(predicate, &mut checked)?
                .is_err()
        {
            return Ok(None);
        }
        let Ok(signature) = self
            .predicates()
            .binary_search_with(predicate, &mut checked)?
        else {
            return Ok(None);
        };
        coordinate_allowance(predicate.arity(), limit).map_err(CarrierFailure::Storage)?;
        checked()?;
        let mut coordinates = Vec::new();
        coordinates
            .try_reserve_exact(predicate.arity())
            .map_err(|_| CarrierFailure::Storage(CarrierError::Allocation))?;
        coordinate_allowance(coordinates.capacity(), limit).map_err(CarrierFailure::Storage)?;
        for column in 0..predicate.arity() {
            let value = query.argument(column, &mut checked)?;
            let Ok(rank) = self.domain().binary_search_with(value, &mut checked)? else {
                return Ok(None);
            };
            checked()?;
            coordinates.push(rank);
        }
        checked()?;
        Ok(Some(CarrierAtom::new(self, signature, coordinates)))
    }
}

/// Fallible canonical Cartesian enumeration retaining only integer coordinates.
/// Creating the iterator allocates nothing. Each next tuple reserves O(arity)
/// words; a refusal is returned once and terminates iteration. A returned token
/// shares the Program's term/predicate payload, never a copied owned Value.
///
/// Unique signatures and domain values are ordered by semantic storage order.
/// Advancing the last coordinate fastest therefore agrees with Atom ordering.
/// Nullary signatures contribute one tuple even for an empty domain; positive
/// arities contribute none there. This is not ASP term ordering or insertion ID
/// order, and enumeration does not first count the whole carrier.
#[derive(Clone)]
pub struct AtomIter<'a> {
    program: &'a Program,
    predicates: Predicates<'a>,
    current: usize,
    coordinates: Vec<usize>,
    started: bool,
    finished: bool,
}
impl<'a> AtomIter<'a> {
    pub(crate) fn new(program: &'a Program, predicates: Predicates<'a>) -> Self {
        Self {
            program,
            predicates,
            current: 0,
            coordinates: Vec::new(),
            started: false,
            finished: false,
        }
    }
    fn next_atom(&mut self) -> Result<Option<CarrierAtom>, CarrierError> {
        loop {
            let Some(predicate) = self.predicates.get(self.current) else {
                return Ok(None);
            };
            let domain = self.program.domain();
            if !self.started {
                if domain.is_empty() && predicate.arity() != 0 {
                    self.current += 1;
                    continue;
                }
                self.coordinates.clear();
                self.coordinates
                    .try_reserve_exact(predicate.arity())
                    .map_err(|_| CarrierError::Allocation)?;
                self.coordinates.resize(predicate.arity(), 0);
                self.started = true;
            } else if !advance(&mut self.coordinates, domain.len()) {
                self.current += 1;
                self.started = false;
                continue;
            }
            let signature = self
                .program
                .predicates()
                .binary_search(predicate)
                .expect("carrier subset contains only this Program's signatures");
            let mut coordinates = Vec::new();
            coordinates
                .try_reserve_exact(self.coordinates.len())
                .map_err(|_| CarrierError::Allocation)?;
            coordinates.extend_from_slice(&self.coordinates);
            return Ok(Some(CarrierAtom::new(self.program, signature, coordinates)));
        }
    }
}
impl Iterator for AtomIter<'_> {
    type Item = Result<CarrierAtom, CarrierError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        match self.next_atom() {
            Ok(Some(atom)) => Some(Ok(atom)),
            Ok(None) => {
                self.finished = true;
                None
            }
            Err(error) => {
                self.finished = true;
                Some(Err(error))
            }
        }
    }
}
impl FusedIterator for AtomIter<'_> {}

fn coordinate_allowance(capacity: usize, limit: usize) -> Result<(), CarrierError> {
    let required = size_of::<Tuple>() as u128 + capacity as u128 * size_of::<usize>() as u128;
    if required > limit as u128 {
        Err(CarrierError::Bytes { required, limit })
    } else {
        Ok(())
    }
}

/// Advance last-coordinate-first; an empty tuple has one assignment.
pub(crate) fn advance(coordinates: &mut [usize], radix: usize) -> bool {
    for index in coordinates.iter_mut().rev() {
        if *index < radix.saturating_sub(1) {
            *index += 1;
            return true;
        }
        *index = 0;
    }
    false
}
