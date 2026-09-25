//! Borrowed substitution keys with the exact identity of an owned atom.

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

use crate::catalog::{AtomRef, Error, PredicateRef, TermRef};
use crate::{
    Atom, AtomPattern, InstantiationError, PatternRef, Predicate, TemplateTerm, Value, ValueError,
    ValueLimits, ValueResource,
};

/// A complete atom identity borrowed from a pattern and immutable assignment.
///
/// Construction checks every referenced slot, preserving argument order and
/// repeated variables. The key cannot outlive either input; the shared borrows
/// prevent binding mutation while the key exists. It owns no value or vector.
/// Equality, ordering and hashing agree with the atom produced by [`Self::to_atom`],
/// including predicate sign and complete typed structural values. Ordering is
/// canonical storage order, not ASP term order.
#[derive(Clone, Copy, Debug)]
pub struct AtomKey<'a> {
    pattern: PatternRef<'a>,
    values: BindingView<'a>,
}

/// Read-only source variable lookup over complete or partial binding frames.
///
/// This view borrows its supplied slice and owns no values. Missing and
/// out-of-range slots both yield `None`; neither is a logical constant.
#[derive(Clone, Copy, Debug)]
pub struct BindingView<'a>(Values<'a>);

#[derive(Clone, Copy, Debug)]
enum Values<'a> {
    Complete(&'a [Value]),
    Partial(&'a [Option<Value>]),
    References(&'a [Option<&'a Value>]),
    Terms(&'a [TermRef<'a>]),
    PartialTerms(&'a [Option<TermRef<'a>>]),
    Canonical {
        read: crate::catalog::TermRead<'a>,
        values: &'a [Option<crate::catalog::storage::TermId>],
    },
}

impl<'a> BindingView<'a> {
    /// Number of source variable slots, including absent slots.
    #[must_use]
    pub fn len(self) -> usize {
        match self.0 {
            Values::Complete(values) => values.len(),
            Values::Partial(values) => values.len(),
            Values::References(values) => values.len(),
            Values::Terms(values) => values.len(),
            Values::PartialTerms(values) => values.len(),
            Values::Canonical { values, .. } => values.len(),
        }
    }
    /// Whether this binding view has no source variable slots.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    pub(crate) fn canonical(
        read: crate::catalog::TermRead<'a>,
        values: &'a [Option<crate::catalog::storage::TermId>],
    ) -> Self {
        Self(Values::Canonical { read, values })
    }
    fn is_canonical(self) -> bool {
        matches!(self.0, Values::Canonical { .. })
    }

    /// Borrow a present variable value without allocation or substitution.
    ///
    /// # Panics
    /// Panics if a canonical assignment refers outside its checked read prefix.
    /// Assignment binding validates that prefix before exposing this view.
    #[must_use]
    pub fn get(self, index: usize) -> Option<TermRef<'a>> {
        match self.0 {
            Values::Complete(values) => values.get(index).map(TermRef::from),
            Values::Partial(values) => values
                .get(index)
                .and_then(Option::as_ref)
                .map(TermRef::from),
            Values::References(values) => values.get(index).copied().flatten().map(TermRef::from),
            Values::Terms(values) => values.get(index).copied(),
            Values::PartialTerms(values) => values.get(index).copied().flatten(),
            Values::Canonical { read, values } => values
                .get(index)
                .copied()
                .flatten()
                .map(|id| read.resolve(id).expect("checked assignment prefix")),
        }
    }
}
impl<'a> From<&'a [Value]> for BindingView<'a> {
    fn from(values: &'a [Value]) -> Self {
        Self(Values::Complete(values))
    }
}
impl<'a> From<&'a [Option<Value>]> for BindingView<'a> {
    fn from(values: &'a [Option<Value>]) -> Self {
        Self(Values::Partial(values))
    }
}
impl<'a> From<&'a [Option<&'a Value>]> for BindingView<'a> {
    fn from(values: &'a [Option<&'a Value>]) -> Self {
        Self(Values::References(values))
    }
}

impl<'a> From<&'a [TermRef<'a>]> for BindingView<'a> {
    fn from(values: &'a [TermRef<'a>]) -> Self {
        Self(Values::Terms(values))
    }
}
impl<'a> From<&'a [Option<TermRef<'a>>]> for BindingView<'a> {
    fn from(values: &'a [Option<TermRef<'a>>]) -> Self {
        Self(Values::PartialTerms(values))
    }
}

impl AtomPattern {
    /// Borrow a complete substitution key without cloning values or allocating.
    /// Construction inspects the argument sequence once. Unreferenced partial
    /// slots may remain absent. The key establishes identity, not candidate truth.
    ///
    /// ```
    /// use zetesis_core::{AtomPattern, Predicate, Term, Value};
    /// let pattern = AtomPattern::new(Predicate::new("p", 1)?, vec![Term::Variable(1)])?;
    /// let complete = [Value::Number(0), Value::Number(7)];
    /// let partial = [None, Some(&complete[1])];
    /// assert_eq!(pattern.key(complete.as_slice())?, pattern.key(partial.as_slice())?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    /// Returns the first absent or out-of-range referenced variable.
    pub fn key<'a>(
        &'a self,
        assignment: impl Into<BindingView<'a>>,
    ) -> Result<AtomKey<'a>, InstantiationError> {
        PatternRef::from(self).key(assignment)
    }
}

impl<'a> PatternRef<'a> {
    /// Borrow a complete substitution over admitted or construction patterns.
    /// The key retains both borrows and copies no value or argument vector.
    ///
    /// # Errors
    /// Returns the first absent or out-of-range referenced variable.
    pub fn key(
        self,
        assignment: impl Into<BindingView<'a>>,
    ) -> Result<AtomKey<'a>, InstantiationError> {
        AtomKey::checked(self, assignment.into())
    }
}

impl<'a> AtomKey<'a> {
    fn checked(
        pattern: PatternRef<'a>,
        values: BindingView<'a>,
    ) -> Result<Self, InstantiationError> {
        for variable in pattern.terms().variables() {
            if values.get(variable).is_none() {
                return Err(InstantiationError { variable });
            }
        }
        Ok(Self { pattern, values })
    }

    /// Full signed predicate identity.
    #[must_use]
    pub fn predicate(&self) -> PredicateRef<'a> {
        self.pattern.predicate()
    }

    pub(crate) fn predicate_with<E>(
        &self,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<PredicateRef<'a>, E> {
        if !self.pattern.is_ingress() {
            before()?;
        }
        Ok(self.predicate())
    }

    /// Borrow one resolved argument; only an out-of-arity column is absent.
    #[must_use]
    pub fn value(&self, column: usize) -> Option<TermRef<'a>> {
        match self.pattern.terms().get(column)? {
            TemplateTerm::Constant(value) => Some(value),
            TemplateTerm::Variable(variable) => self.values.get(variable),
        }
    }

    // Internal consumers pass only columns bounded by the checked predicate.
    pub(crate) fn argument(&self, column: usize) -> TermRef<'a> {
        self.value(column).expect("checked key arity")
    }

    pub(crate) fn argument_with<E>(
        &self,
        column: usize,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<TermRef<'a>, E> {
        if !self.pattern.is_ingress() || self.values.is_canonical() {
            before()?;
        }
        Ok(self.argument(column))
    }

    /// Exact typed comparison with an owned atom, without materialization.
    /// Cost includes visited predicate and value payloads.
    #[must_use]
    pub fn compare(&self, atom: &Atom) -> Ordering {
        compare(self, atom)
    }

    /// Compare complete typed identity without materializing a substitution.
    /// Ingress assignments preserve [`Value::compare_identity_with`]'s visited
    /// descriptor/text trace, including the signed predicate. Canonical terms
    /// additionally charge their storage/navigation probes before work.
    ///
    /// # Errors
    /// Returns the first caller refusal before that comparison; no ordering or
    /// owned atom is produced after refusal.
    pub fn compare_identity_with<E>(
        &self,
        atom: &Atom,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Ordering, E> {
        let order = self
            .predicate_with(&mut before)?
            .compare_identity_with(atom.predicate(), &mut before)?;
        if !order.is_eq() {
            return Ok(order);
        }
        self.compare_values_with(atom, before)
    }

    /// Compare the arguments with an atom of this key's predicate, in column
    /// order, charging as [`Self::compare_identity_with`] does after the
    /// predicate.
    ///
    /// # Errors
    /// Returns the first caller refusal before that comparison.
    pub(crate) fn compare_values_with<E>(
        &self,
        atom: &Atom,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Ordering, E> {
        for (column, value) in atom.values().iter().enumerate() {
            let order = self
                .argument_with(column, &mut before)?
                .compare_identity_with(value, &mut before)?;
            if !order.is_eq() {
                return Ok(order);
            }
        }
        Ok(Ordering::Equal)
    }

    /// Compare with a canonical or ingress atom view without materialization.
    /// Predicate and argument contents determine order across independent stores.
    #[must_use]
    pub fn compare_ref(&self, atom: AtomRef<'_>) -> Ordering {
        compare(self, &atom)
    }

    /// Checked comparison with a borrowed atom view, in key-versus-atom order.
    /// Canonical storage/navigation work is charged before each operation.
    ///
    /// # Errors
    /// Returns the first callback refusal without producing an ordering.
    pub fn compare_ref_with<E>(
        &self,
        atom: AtomRef<'_>,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<Ordering, E> {
        atom.compare_key_with(self, before).map(Ordering::reverse)
    }

    /// Find this complete identity in a canonically ordered atom set.
    /// This performs membership lookup and borrows the original stored atom;
    /// it allocates nothing and never manufactures another atom owner.
    #[must_use]
    pub fn get<'set>(&self, atoms: &'set BTreeSet<Atom>) -> Option<&'set Atom> {
        atoms.get::<dyn View + '_>(self)
    }

    /// Explicitly materialize an owned ingress atom for an external boundary.
    /// This allocates the argument vector and expands each referenced value;
    /// it is not an execution lookup, interning adapter or implicit clone.
    /// Node/depth ceilings apply per value; bytes bound the named output and
    /// temporary value construction after reserving the argument vector.
    ///
    /// # Errors
    /// Refuses construction limits, size overflow or failed buffer reservations.
    /// No partial atom is returned. Owned constructors retain their existing
    /// infallible Arc-envelope behavior; universal allocation recovery is not
    /// promised.
    pub fn to_atom(self, limits: ValueLimits) -> Result<Atom, Error> {
        let arity = self.predicate().arity();
        let requested = arity as u128 * std::mem::size_of::<Value>() as u128
            + self.predicate().name().len() as u128;
        materialization_ceiling(requested, limits.max_bytes)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(arity)
            .map_err(|_| Error::Allocation)?;
        let mut name = String::new();
        name.try_reserve_exact(self.predicate().name().len())
            .map_err(|_| Error::Allocation)?;
        let mut bytes = values.capacity() as u128 * std::mem::size_of::<Value>() as u128
            + name.capacity() as u128;
        materialization_ceiling(bytes, limits.max_bytes)?;
        name.push_str(self.predicate().name());
        for column in 0..arity {
            let remaining =
                limits.max_bytes - usize::try_from(bytes).map_err(|_| Error::Overflow)?;
            let value = self.argument(column).to_value(ValueLimits {
                max_bytes: remaining,
                ..limits
            })?;
            bytes += value
                .checked_payload_capacity_bytes()
                .ok_or(Error::Overflow)?;
            materialization_ceiling(bytes, limits.max_bytes)?;
            values.push(value);
        }
        let predicate =
            Predicate::with_sign(name, arity, self.predicate().sign()).map_err(|_| Error::Shape)?;
        Ok(Atom::from_valid_parts(predicate, values))
    }
}

impl Hash for AtomKey<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash(self, state);
    }
}

pub(crate) fn hash_atom<H: Hasher>(atom: &Atom, state: &mut H) {
    hash(atom, state);
}

// Both owners use one hash operation sequence. This does not depend on the
// standard library's private Vec/slice hash encoding or a particular hasher.
fn hash<V: View, H: Hasher>(value: &V, state: &mut H) {
    value.predicate().hash(state);
    value.predicate().arity().hash(state);
    for column in 0..value.predicate().arity() {
        value.argument(column).hash(state);
    }
}

impl PartialEq for AtomKey<'_> {
    fn eq(&self, other: &Self) -> bool {
        compare(self, other).is_eq()
    }
}
impl Eq for AtomKey<'_> {}
impl PartialOrd for AtomKey<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for AtomKey<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        compare(self, other)
    }
}

// The heterogeneous BTree borrow bridge is private. Only checked core owners
// implement it, so no caller-supplied comparison can contradict Atom::Ord.
trait View {
    fn predicate(&self) -> PredicateRef<'_>;
    fn argument(&self, column: usize) -> TermRef<'_>;
}
impl View for Atom {
    fn predicate(&self) -> PredicateRef<'_> {
        PredicateRef::from(self.predicate())
    }
    fn argument(&self, column: usize) -> TermRef<'_> {
        TermRef::from(&self.values()[column])
    }
}
impl View for AtomKey<'_> {
    fn predicate(&self) -> PredicateRef<'_> {
        self.predicate()
    }
    fn argument(&self, column: usize) -> TermRef<'_> {
        self.argument(column)
    }
}
impl View for AtomRef<'_> {
    fn predicate(&self) -> PredicateRef<'_> {
        AtomRef::predicate(*self)
    }
    fn argument(&self, column: usize) -> TermRef<'_> {
        self.values().at(column).expect("admitted atom arity")
    }
}

impl<'a> Borrow<dyn View + 'a> for Atom {
    fn borrow(&self) -> &(dyn View + 'a) {
        self
    }
}
impl PartialEq for dyn View + '_ {
    fn eq(&self, other: &Self) -> bool {
        compare(self, other).is_eq()
    }
}
impl Eq for dyn View + '_ {}
impl PartialOrd for dyn View + '_ {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for dyn View + '_ {
    fn cmp(&self, other: &Self) -> Ordering {
        compare(self, other)
    }
}
fn compare<L: View + ?Sized, R: View + ?Sized>(left: &L, right: &R) -> Ordering {
    let predicate = left.predicate().cmp(&right.predicate());
    if !predicate.is_eq() {
        return predicate;
    }
    for column in 0..left.predicate().arity() {
        let order = left.argument(column).cmp(&right.argument(column));
        if !order.is_eq() {
            return order;
        }
    }
    Ordering::Equal
}

fn materialization_ceiling(observed: u128, limit: usize) -> Result<(), Error> {
    if observed > limit as u128 {
        Err(ValueError::Limit {
            resource: ValueResource::Bytes,
            observed,
            limit,
        }
        .into())
    } else {
        Ok(())
    }
}
