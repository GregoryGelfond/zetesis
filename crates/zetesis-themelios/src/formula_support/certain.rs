//! Certain predicates: those whose supported atoms hold in every answer set.
//!
//! A rule is *definite* when it has one ordinary head atom and its body holds
//! only positive atoms, literals its own join decides (comparisons, argument
//! checks, guards, binders and intervals), decided default negations and
//! exact assignment aggregates, both described below. A predicate is
//! *certain* when every rule that can derive one of its atoms is definite and
//! reads only certain predicates. The certain predicates are the greatest
//! such set: any other default negation, a projection, a conditional literal,
//! any other aggregate or a head guard in a body, and any disjunctive,
//! conditional or choice head, removes the rule's head predicates, and a
//! predicate that reads a removed one is removed in turn. A predicate that
//! only facts define is certain.
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
//! A default negation is *decided* when it stands at the top level of a
//! rule's body, over a flat atom of a certain predicate, and every traversal
//! of the rule reads that atom's relation complete. The component schedule
//! supplies the last condition (`producers::registration::components`): the
//! atom is *awaited* when its component is decidable and precedes the rule's
//! heads, and the rule's producer is then held back until that component
//! reaches its fixed point, or is deferred for everything it reads. A
//! certain predicate lies in such a component: were it in its reader's own,
//! the reader's undecided negation would remove the reader's head and with it
//! every predicate of the component. Over a complete relation of a certain
//! predicate, a supported atom holds in every answer set, as above, and an
//! unsupported one holds in none, since no rule can derive it. So `not a` is
//! false for a supported `a` and true otherwise, and `not not a` the reverse.
//! Support completion derives no head from a row under which a decided
//! negation is false ([`super::decided_negations_hold`]): no answer set
//! satisfies that row's body. The rule then supports exactly the heads its
//! other literals admit under true negations, as a definite rule does, and
//! emission reads each such negation as true and drops each instance whose
//! negation is false (`formula_ground::decided_negation`), so the root that
//! remains is unconditional. The certain predicates therefore still form a
//! program whose completed support is its one model: stratified along the
//! component order, each component's rules reading the components before it
//! complete.
//!
//! An assignment aggregate reads this through its elements. A tuple that an
//! element yields from certain atoms and decided literals alone is present in
//! every interpretation, as a tuple of an element with no atom is: it is
//! *required*. That holds for the completed support only. Support membership
//! grows between rounds, and a proposal made from an incomplete relation would
//! count too few required tuples, so an element is listed here only where its
//! tuples are read over complete relations:
//!
//! - in a producer the component schedule traverses once, after every
//!   predicate it reads reached its fixed point
//!   (`producers::registration::components`);
//! - in any other producer, for the elements the schedule holds it back for:
//!   it is not traversed before the components those elements read reach
//!   their fixed point, so each of its traversals counts the same required
//!   tuples. With those fixed, its proposals only grow as the support it
//!   reads grows;
//! - in a constraint, which proposes nothing and is grounded after
//!   completion.
//!
//! Final grounding reads the same list over the completed support, so both
//! sites propose from the same required tuples. An objective's condition is
//! not listed: its assignments substitute every value its tuples allow.
//!
//! The second case decides whether some recursive assignments reach a fixed
//! point at all. In `f(3). {p(N)} :- N = #count{V : p(V); V : f(V)}.` the
//! fact's tuple is required, so the count is at least one and its values end
//! at 3, whose head adds no tuple. Were that tuple optional, 0 would be
//! proposed, `p(0)` would add a tuple, and each round would propose one value
//! more. A tuple is required through its element's predicates, never through
//! one row: a fact of a predicate that another rule also derives through a
//! choice or an assignment of its own leaves that predicate uncertain, and
//! `p(1). {p(N)} :- N = #count{V : p(V)}.` still proposes without end.
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
    /// `(rule, occurrence)` coordinates of the decided default-negated body
    /// atoms, sorted and distinct. A rule's coordinate is its position among
    /// the prepared rules as support completion saw them. Emission compacts
    /// that vector as it retires materialized rules, so these coordinates are
    /// read while a rule is completed, staged or emitted, never after.
    negations: Vec<(usize, usize)>,
    /// The owned payload's bytes: the vectors' capacities and the names.
    /// Nothing changes them after construction, and the catalog reads them
    /// at every admission, so they are summed once.
    retained: usize,
}

impl Certainty {
    /// Every predicate certain and no aggregate: a positive flat source.
    pub(super) fn all() -> Self {
        Self {
            all: true,
            ..Self::default()
        }
    }

    /// From the predicates, elements and negated atoms an analysis found, in
    /// any order.
    pub(super) fn new(
        mut predicates: Vec<Signature>,
        mut elements: Vec<(usize, usize)>,
        mut negations: Vec<(usize, usize)>,
    ) -> Self {
        predicates.sort_unstable_by(|left, right| left.order(right.sign, &right.name, right.arity));
        predicates.dedup();
        elements.sort_unstable();
        elements.dedup();
        negations.sort_unstable();
        negations.dedup();
        let retained = predicates.capacity() * size_of::<Signature>()
            + predicates
                .iter()
                .map(|signature| signature.name.len())
                .sum::<usize>()
            + (elements.capacity() + negations.capacity()) * size_of::<(usize, usize)>();
        Self {
            all: false,
            predicates,
            elements,
            negations,
            retained,
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

    /// The body positions of the default-negated atoms that every traversal
    /// of the rule at `rule` decides, in order: the atom's predicate is
    /// certain and its relation complete whenever the rule is traversed, so
    /// the atom holds in every answer set when it is supported and in none
    /// otherwise. A binary search of the listed coordinates.
    pub(crate) fn decided(&self, rule: usize) -> impl Iterator<Item = usize> + '_ {
        let start = self.negations.partition_point(|&(listed, _)| listed < rule);
        self.negations[start..]
            .iter()
            .take_while(move |&&(listed, _)| listed == rule)
            .map(|&(_, occurrence)| occurrence)
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

    /// Owned payload: the vectors' capacities and the names. One read.
    pub(super) fn retained_bytes(&self) -> usize {
        self.retained
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

/// Whether no default-negated atom is decided, as the test-only reference
/// has it: the analysis admits none in a definite rule and lists none, and
/// emission keeps each as written. Always false outside tests.
pub(crate) fn negation_left_out() -> bool {
    #[cfg(test)]
    {
        testing::negation_left_out()
    }
    #[cfg(not(test))]
    {
        false
    }
}

/// Three scoped, thread-local switches, each restored after an unwind and
/// confined to its thread, as the registration's switch is.
///
/// The reference certainty is compared against: support completion installs
/// no certain predicate, so every atom stays in the bodies that read it and
/// every tuple with an atom in its condition stays optional.
///
/// The fault emission must refuse: support completion calls every predicate
/// certain, whatever derives it.
///
/// The reference decided negation is compared against: a default negation
/// removes its rule's head predicates, no row is tested against a negated
/// atom, and emission keeps every negated atom as written. Nothing being
/// decided, no ordinary rule is held back for the atoms it negates.
#[cfg(test)]
pub(crate) mod testing {
    use std::cell::Cell;

    thread_local! {
        static LEFT_OUT: Cell<bool> = const { Cell::new(false) };
        static ASSUMED: Cell<bool> = const { Cell::new(false) };
        static NEGATION_LEFT_OUT: Cell<bool> = const { Cell::new(false) };
    }

    pub(super) fn left_out() -> bool {
        LEFT_OUT.get()
    }

    pub(super) fn every_predicate_assumed() -> bool {
        ASSUMED.get()
    }

    pub(super) fn negation_left_out() -> bool {
        NEGATION_LEFT_OUT.get()
    }

    /// Run `action` with no default-negated atom decided.
    pub(crate) fn without_decided_negation<T>(action: impl FnOnce() -> T) -> T {
        let _restore = Restore(&NEGATION_LEFT_OUT, NEGATION_LEFT_OUT.replace(true));
        action()
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
