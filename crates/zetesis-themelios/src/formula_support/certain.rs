//! Certain predicates: those whose supported atoms hold in every answer set.
//!
//! A rule is *definite* when it has one ordinary head atom and its body holds
//! only positive atoms, literals its own join decides (comparisons, argument
//! checks, guards, binders and intervals) and exact assignment aggregates,
//! described below. A predicate is *certain* when every rule that can derive
//! one of its atoms is definite and reads only certain predicates. The certain
//! predicates are the greatest such set: a default negation, a projection, a
//! conditional literal, any other aggregate or a head guard in a body, and any
//! disjunctive, conditional or choice head, removes the rule's head
//! predicates, and a predicate that reads a removed one is removed in turn. A
//! predicate that only facts define is certain.
//!
//! The rules of the certain predicates form a definite program closed under
//! what it reads. Support completion derives a head from exactly the rows a
//! rule's positive atoms and decided literals admit, so the completed support
//! of a certain predicate is that program's least model: each supported atom
//! has a derivation from facts through definite rules. The roots of those
//! rules and facts stay in the emitted theory. By induction along a derivation
//! they force the atom into every model of the theory and into every model of
//! its reduct at such a model: the fact roots force the facts, and a rule root
//! whose body atoms are forced forces its head. A supported atom of a certain
//! predicate is therefore a premise wherever an explicit fact is one
//! (`formula_ground::fixed_facts`): a positive occurrence may be read as true.
//!
//! An assignment aggregate reads this through its elements. A tuple that an
//! element yields from certain atoms and decided literals alone is present in
//! every interpretation, as a tuple of an element with no atom is: it is
//! *required*. That holds for the completed support only. Support membership
//! grows between rounds, and a proposal made from an incomplete relation would
//! count too few required tuples, so an element is listed here only where its
//! tuples are read over complete relations: in a producer the component
//! schedule traverses once, after every predicate it reads reached its fixed
//! point (`producers::registration::components`), and in a constraint, which
//! proposes nothing and is grounded after completion. Final grounding reads
//! the same list over the completed support, so both sites propose from the
//! same required tuples. An objective's condition is not listed: its
//! assignments substitute every value its tuples allow.
//!
//! An assignment is *plain* when its keys are tuples and its conditions hold
//! positive atoms and decided literals only; an assignment is unnegated and
//! its one guard is the equality with its target. A plain assignment of such a
//! producer is *exact* when those atoms are certain: every tuple is required,
//! the carrier is the one measure of those tuples, and that is the aggregate's
//! value in every interpretation holding the certain atoms. The rule then
//! supports exactly the heads that hold, so it is definite. Final grounding
//! emits that equality as true, which makes the rule's root unconditional as a
//! definite rule's is.
//!
//! The analysis charges one unit per rule and per body literal classified,
//! with each predicate resolution's own charge, and for the removal one unit
//! per rule and per predicate it reads in each round. A round removes at
//! least one predicate or ends the removal, so the removal costs at most the
//! predicates and one more times the rules and their reads, reached by a
//! chain of predicates written head first. Its retained vectors are admitted
//! beside the registration's.

#[cfg(test)]
mod tests;

use std::cmp::Ordering;
use std::mem::size_of;

use zetesis_core::Sign;
use zetesis_core::catalog::PredicateRef;

use super::Counters;
use crate::formula_ir::AggregateElementIr;
use crate::{FormulaFailure, FormulaLimits, ProgramSite};

/// A certain predicate's signature, owned so the catalog can keep it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Signature {
    pub(super) sign: Sign,
    pub(super) name: Box<str>,
    pub(super) arity: usize,
}

impl Signature {
    fn order(&self, sign: Sign, name: &str, arity: usize) -> Ordering {
        self.sign
            .cmp(&sign)
            .then_with(|| (*self.name).cmp(name))
            .then_with(|| self.arity.cmp(&arity))
    }
}

/// The certain predicates of one prepared program and the assignment aggregate
/// elements they make required.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Certainty {
    /// Every predicate is certain: the whole source is positive and flat.
    all: bool,
    /// Sorted by sign, name and arity; distinct.
    predicates: Vec<Signature>,
    /// `(aggregate, element)` coordinates, sorted and distinct.
    elements: Vec<(usize, usize)>,
}

impl Certainty {
    /// Every predicate certain and no aggregate: a positive flat source.
    pub(super) fn all() -> Self {
        Self {
            all: true,
            predicates: Vec::new(),
            elements: Vec::new(),
        }
    }

    /// From the predicates and elements an analysis found, in any order.
    pub(super) fn new(mut predicates: Vec<Signature>, mut elements: Vec<(usize, usize)>) -> Self {
        predicates.sort_unstable_by(|left, right| left.order(right.sign, &right.name, right.arity));
        predicates.dedup();
        elements.sort_unstable();
        elements.dedup();
        Self {
            all: false,
            predicates,
            elements,
        }
    }

    /// Whether nothing is certain, so no reader needs this value.
    pub(super) fn is_empty(&self) -> bool {
        !self.all && self.predicates.is_empty()
    }

    /// Whether `predicate` is certain: every supported atom of it holds in
    /// every answer set. One unit per comparison, with the compared names'
    /// lengths.
    pub(crate) fn certain(
        &self,
        predicate: PredicateRef<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        if self.all {
            counters.work(limits, location)?;
            return Ok(true);
        }
        let (sign, name, arity) = (predicate.sign(), predicate.name(), predicate.arity());
        let mut start = 0;
        let mut end = self.predicates.len();
        while start < end {
            let middle = start + (end - start) / 2;
            let entry = &self.predicates[middle];
            counters.charge_work(
                1 + entry.name.len() as u128 + name.len() as u128,
                limits,
                location,
            )?;
            match entry.order(sign, name, arity) {
                Ordering::Less => start = middle + 1,
                Ordering::Greater => end = middle,
                Ordering::Equal => return Ok(true),
            }
        }
        Ok(false)
    }

    /// Whether the element at `element` of the assignment aggregate
    /// `aggregate` is listed: every tuple it yields over the completed support
    /// is required. A binary search of the listed coordinates, covered by its
    /// caller's charge for the element.
    fn lists(&self, aggregate: usize, element: usize) -> bool {
        self.elements.binary_search(&(aggregate, element)).is_ok()
    }

    /// Whether every tuple `element`, at `position` of the assignment
    /// aggregate `aggregate`, yields is required: by its own condition, which
    /// the local join decides, or by `certainty`'s listing over the completed
    /// support. Support completion's proposals and final grounding both ask
    /// here, so they count the same required tuples.
    pub(crate) fn requires(
        certainty: Option<&Self>,
        aggregate: usize,
        position: usize,
        element: &AggregateElementIr,
    ) -> bool {
        element.required || certainty.is_some_and(|certainty| certainty.lists(aggregate, position))
    }

    /// The certain predicates listed, by name and arity; none when all are.
    #[cfg(test)]
    pub(super) fn predicates(&self) -> impl Iterator<Item = (&str, usize)> + '_ {
        self.signed_predicates()
            .map(|(_, name, arity)| (name, arity))
    }

    /// The certain predicates listed, each with its sign.
    #[cfg(test)]
    pub(super) fn signed_predicates(&self) -> impl Iterator<Item = (Sign, &str, usize)> + '_ {
        self.predicates
            .iter()
            .map(|signature| (signature.sign, &*signature.name, signature.arity))
    }

    /// The listed element coordinates.
    #[cfg(test)]
    pub(super) fn elements(&self) -> &[(usize, usize)] {
        &self.elements
    }

    /// Whether every predicate is certain, as [`Self::all`] makes it.
    #[cfg(test)]
    pub(super) fn is_all(&self) -> bool {
        self.all
    }

    /// Owned payload: the vectors' capacities and the names.
    pub(super) fn retained_bytes(&self) -> usize {
        self.predicates.capacity() * size_of::<Signature>()
            + self
                .predicates
                .iter()
                .map(|signature| signature.name.len())
                .sum::<usize>()
            + self.elements.capacity() * size_of::<(usize, usize)>()
    }
}

/// Whether support completion leaves certainty out, as the test-only
/// reference does. Always false outside tests.
pub(super) fn left_out() -> bool {
    #[cfg(test)]
    {
        testing::left_out()
    }
    #[cfg(not(test))]
    {
        false
    }
}

/// Whether support completion treats every predicate as certain, as one
/// test-only fault does. Always false outside tests.
pub(super) fn every_predicate_assumed() -> bool {
    #[cfg(test)]
    {
        testing::every_predicate_assumed()
    }
    #[cfg(not(test))]
    {
        false
    }
}

/// Two scoped, thread-local switches, each restored after an unwind and
/// confined to its thread, as the registration's switch is.
///
/// The reference certainty is compared against: support completion installs
/// no certain predicate, so every atom stays in the bodies that read it and
/// every tuple with an atom in its condition stays optional.
///
/// The fault emission must refuse: support completion calls every predicate
/// certain, whatever derives it.
#[cfg(test)]
pub(crate) mod testing {
    use std::cell::Cell;

    thread_local! {
        static LEFT_OUT: Cell<bool> = const { Cell::new(false) };
        static ASSUMED: Cell<bool> = const { Cell::new(false) };
    }

    pub(super) fn left_out() -> bool {
        LEFT_OUT.get()
    }

    pub(super) fn every_predicate_assumed() -> bool {
        ASSUMED.get()
    }

    /// Restores a switch when its scope ends.
    struct Restore(&'static std::thread::LocalKey<Cell<bool>>, bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            self.0.set(self.1);
        }
    }

    /// Run `action` with no predicate treated as certain.
    pub(crate) fn without_certainty<T>(action: impl FnOnce() -> T) -> T {
        let _restore = Restore(&LEFT_OUT, LEFT_OUT.replace(true));
        action()
    }

    /// Run `action` with every predicate wrongly treated as certain.
    pub(crate) fn with_every_predicate_assumed<T>(action: impl FnOnce() -> T) -> T {
        let _restore = Restore(&ASSUMED, ASSUMED.replace(true));
        action()
    }
}
