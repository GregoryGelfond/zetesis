//! Fact rows enter support by the same atom operation as a fact rule's head.
//!
//! A fact rule's one passing binding is the empty one, and its head pattern's
//! terms are all constants, so its head instantiation interns the pattern under
//! an empty assignment. A fact row interns the pattern of its predicate and its
//! admitted cells, under an empty assignment, through the same operation. No
//! join, plan, variant list or binding storage is built for it.

use zetesis_core::catalog::PredicateRef;
use zetesis_core::{PatternRef, TemplateTerm};

use super::{Computation, Counters, SourceAtom, Support, SupportAppend};
use crate::formula_binding::Binding;
use crate::formula_facts::{FactProduct, Item, Items, Row};
use crate::grounding_observer::Event;
use crate::{AdmissionFailure, FormulaFailure, FormulaLimits, ProgramSite};

/// One fact product's head: its predicate, read once, scratch for a row's
/// terms, and the empty assignment every row is interned under.
pub(crate) struct FactHead<'source> {
    predicate: PredicateRef<'source>,
    terms: Vec<TemplateTerm<'source>>,
    empty: Binding<'static>,
}

impl<'source> FactHead<'source> {
    pub(crate) fn new(
        product: FactProduct<'_>,
        computation: &Computation<'_, 'source>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let predicate =
            computation.static_predicate(product.predicate(), limits, counters, location)?;
        let mut terms = Vec::new();
        terms
            .try_reserve_exact(product.arity())
            .map_err(|error| FormulaFailure::AtomAllocation { error, location })?;
        let empty = Binding::new(computation, limits, counters, location)?;
        Ok(Self {
            predicate,
            terms,
            empty,
        })
    }

    /// Intern `row`'s atom, as a fact rule's head is interned.
    pub(crate) fn atom(
        &mut self,
        row: Row<'_>,
        computation: &mut Computation<'_, 'source>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<SourceAtom, FormulaFailure> {
        let location = row.location();
        self.terms.clear();
        for cell in row.cells() {
            let value = computation.static_scalar(cell, limits, counters, location)?;
            self.terms.push(TemplateTerm::Constant(value));
        }
        let pattern = PatternRef::from_parts(self.predicate, &self.terms)
            .map_err(|error| AdmissionFailure::Construction { error, location })?;
        computation.atom(pattern, &self.empty, limits, counters, location)
    }
}

/// Support completion's first round over the item sequence: each fact
/// product's rows enter the round's pending population at the product's item
/// position among the rules the round visits. Facts enter support in the first
/// round only: a fact rule's atom entered the delta in no later round.
pub(super) struct FirstRound<'items> {
    items: &'items Items,
    position: usize,
}

impl<'items> FirstRound<'items> {
    pub(super) fn new(items: &'items Items) -> Self {
        Self { items, position: 0 }
    }

    /// Intern every product that stands before the rule `index`, and move past
    /// that rule.
    pub(super) fn before_rule<'source>(
        &mut self,
        index: usize,
        delta: &mut SupportAppend<'source>,
        support: &Support<'source>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        while self.position < self.items.len() {
            let item = self.items.get(self.position);
            self.position += 1;
            match item {
                Item::Rule(rule, _) if rule == index => return Ok(()),
                Item::Rule(..) => {}
                Item::Facts(product) => intern(product, delta, support, limits, counters)?,
            }
        }
        Ok(())
    }

    /// Intern every product after the round's last visited rule.
    pub(super) fn finish<'source>(
        mut self,
        delta: &mut SupportAppend<'source>,
        support: &Support<'source>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        self.before_rule(usize::MAX, delta, support, limits, counters)
    }
}

/// Discover each row's atom and select it into the round's pending support.
fn intern<'source>(
    product: FactProduct<'_>,
    delta: &mut SupportAppend<'source>,
    support: &Support<'source>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<(), FormulaFailure> {
    let mut computation = Computation::new(delta, support);
    let mut head = FactHead::new(product, &computation, limits, counters, product.location())?;
    for row in product.rows() {
        let location = row.location();
        counters.work(limits, location)?;
        counters.observe_work(Event::SupportHeadWork, |counters| {
            let atom = head.atom(row, &mut computation, limits, counters)?;
            computation.support(&atom, limits, counters, location)
        })?;
    }
    Ok(())
}

/// Each support round's published atoms, in publication order, recorded on
/// this thread for tests that compare two compilations' rounds.
#[cfg(test)]
pub(crate) mod rounds {
    use std::cell::RefCell;

    use zetesis_core::ValueLimits;
    use zetesis_core::catalog::AtomRef;

    thread_local! {
        static ROUNDS: RefCell<Option<Vec<Vec<String>>>> = const { RefCell::new(None) };
    }

    pub(crate) fn record<'a>(atoms: impl Iterator<Item = AtomRef<'a>>) {
        ROUNDS.with(|rounds| {
            if let Some(rounds) = rounds.borrow_mut().as_mut() {
                rounds.push(
                    atoms
                        .map(|atom| format!("{:?}", atom.to_atom(ValueLimits::default())))
                        .collect(),
                );
            }
        });
    }

    /// Run `body`, returning the rounds it published.
    pub(crate) fn capture<T>(body: impl FnOnce() -> T) -> (T, Vec<Vec<String>>) {
        let previous = ROUNDS.with(|rounds| rounds.replace(Some(Vec::new())));
        let result = body();
        let captured = ROUNDS.with(|rounds| rounds.replace(previous));
        (result, captured.unwrap_or_default())
    }
}
