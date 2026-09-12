//! Borrowed substitution keys with the exact identity of an owned atom.

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

use crate::{Atom, AtomPattern, InstantiationError, Predicate, Term, Value};

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
    pattern: &'a AtomPattern,
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
}

impl<'a> BindingView<'a> {
    /// Borrow a present variable value without allocation or substitution.
    #[must_use]
    pub fn get(self, index: usize) -> Option<&'a Value> {
        match self.0 {
            Values::Complete(values) => values.get(index),
            Values::Partial(values) => values.get(index).and_then(Option::as_ref),
            Values::References(values) => values.get(index).copied().flatten(),
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
        AtomKey::checked(self, assignment.into())
    }
}

impl<'a> AtomKey<'a> {
    fn checked(
        pattern: &'a AtomPattern,
        values: BindingView<'a>,
    ) -> Result<Self, InstantiationError> {
        for term in pattern.terms() {
            if let Term::Variable(variable) = term
                && values.get(*variable).is_none()
            {
                return Err(InstantiationError {
                    variable: *variable,
                });
            }
        }
        Ok(Self { pattern, values })
    }

    /// Full signed predicate identity.
    #[must_use]
    pub fn predicate(&self) -> &Predicate {
        self.pattern.predicate()
    }

    /// Borrow one resolved argument; only an out-of-arity column is absent.
    #[must_use]
    pub fn value(&self, column: usize) -> Option<&'a Value> {
        match self.pattern.terms().get(column)? {
            Term::Constant(value) => Some(value),
            Term::Variable(variable) => self.values.get(*variable),
        }
    }

    // Internal consumers pass only columns bounded by the checked predicate.
    pub(crate) fn argument(&self, column: usize) -> &'a Value {
        self.value(column).expect("checked key arity")
    }

    /// Exact typed comparison with an owned atom, without materialization.
    /// Cost includes visited predicate and value payloads.
    #[must_use]
    pub fn compare(&self, atom: &Atom) -> Ordering {
        compare(self, atom)
    }

    /// Find this complete identity in a canonically ordered atom set.
    /// This performs membership lookup and borrows the original stored atom;
    /// it allocates nothing and never manufactures another atom owner.
    #[must_use]
    pub fn get<'set>(&self, atoms: &'set BTreeSet<Atom>) -> Option<&'set Atom> {
        atoms.get::<dyn View + '_>(self)
    }

    /// Copy the predicate and resolved argument values into one owned atom.
    /// Unlike borrowing or membership lookup, this allocates the value vector
    /// and clones value payloads according to [`Value`]'s ownership contract.
    #[must_use]
    pub fn to_atom(self) -> Atom {
        let values = (0..self.predicate().arity())
            .map(|column| self.argument(column).clone())
            .collect();
        Atom::from_valid_parts(self.predicate().clone(), values)
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
    fn predicate(&self) -> &Predicate;
    fn argument(&self, column: usize) -> &Value;
}
impl View for Atom {
    fn predicate(&self) -> &Predicate {
        self.predicate()
    }
    fn argument(&self, column: usize) -> &Value {
        &self.values()[column]
    }
}
impl View for AtomKey<'_> {
    fn predicate(&self) -> &Predicate {
        self.predicate()
    }
    fn argument(&self, column: usize) -> &Value {
        self.argument(column)
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
    let predicate = left.predicate().cmp(right.predicate());
    if !predicate.is_eq() {
        return predicate;
    }
    for column in 0..left.predicate().arity() {
        let order = left.argument(column).cmp(right.argument(column));
        if !order.is_eq() {
            return order;
        }
    }
    Ordering::Equal
}
