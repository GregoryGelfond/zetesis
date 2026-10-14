//! Argument domains over neutral producer plans.
//!
//! One lowering borrows source symbols; another may submit authenticated catalog
//! keys. Logical stops publish no bounds. Reader and allocation failures carry
//! the committed receipt, without a partial analysis. Bounds describe defined
//! values; they do not establish that arithmetic is total or that a binding exists.
//! Requested integer certificates separately establish defined arithmetic over
//! complete-binding domains, without proving a binding or a prefix extension.

mod input;
mod integer;
mod lattice;
mod engine;
mod transfer;

use crate::{Limits, Statistics, Stop, Widening};
pub use input::Input;
use std::{cell::Cell, cmp::Ordering, collections::TryReserveError};
use themelios_program::{
    program::{AggregateFunction, Relation},
    term::{BinaryOp, UnaryOp},
};

/// Whether every possible value is an integer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Integrality {
    /// All values are numbers, including the vacuous empty domain.
    Integer,
    /// Nonnumeric values may occur.
    Mixed,
}
/// A bound on only the numeric values in a domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interval {
    /// No numeric value occurs.
    Bottom,
    /// Inclusive i32 endpoints.
    Bounded {
        /// Least permitted numeric value.
        low: i32,
        /// Greatest permitted numeric value.
        high: i32,
    },
    /// No numeric bound has been established.
    Unbounded,
}
/// Numeric components of one argument or local slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bound {
    /// Whether nonnumeric values have been excluded.
    pub integrality: Integrality,
    /// Upper coverage of numeric values alone.
    pub interval: Interval,
}
/// Completion and a conservative logical stop are distinct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Every producer is covered by the completed bounds.
    FixedPoint,
    /// All queries are unavailable; no partial certificate survived.
    Stopped(Stop),
}

macro_rules! coordinate {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct $name(pub(super) usize);
    };
}
coordinate!(PredicateId, "A signed predicate coordinate in this input.");
coordinate!(ArgumentId, "An argument coordinate in this input.");
/// A classified constant in one analysis, ordered by registration coordinate.
/// This ordering is neither ASP term order nor meaningful across analyses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConstantId(pub(super) usize);
coordinate!(ExpressionId, "An expression coordinate in this input.");
coordinate!(RuleId, "A producer-rule coordinate in this input.");

/// Classification and finite-symbol admission are independent.
pub struct Classification {
    /// Numeric value, or None for a nonnumeric term (including infinities).
    pub number: Option<i32>,
    /// Whether the key fits every finite-symbol structural and payload ceiling.
    pub retain: bool,
}
/// Borrow a compatible owner without importing or copying its term payload.
/// Implementations charge actual inspected structure and semantic comparison
/// work through the supplied budget. A refused read is never an unknown value.
pub trait Values {
    /// Authentic source key; neither Clone nor Ord is required.
    type Key;
    /// The owner's typed read/control failure.
    type Error;
    /// Classify one key and check the configured finite-symbol ceilings.
    /// # Errors
    /// Preserve owner/control, allocation and logical-limit failures.
    fn classify(
        &self,
        key: &Self::Key,
        limits: &Limits,
        budget: &Budget,
    ) -> Result<Classification, Error<Self::Error>>;
    /// Compare keys by a consistent total order whose equality is semantic equality.
    /// The engine does not interpret this order as the ASP relational order.
    /// # Errors
    /// Preserve owner/control, allocation and logical-limit failures.
    fn compare(
        &self,
        left: &Self::Key,
        right: &Self::Key,
        budget: &Budget,
    ) -> Result<Ordering, Error<Self::Error>>;
}
/// Shared logical meter for lowering, key inspection and abstract transfer.
/// Finite-set entries are live reservations, released with their owning sets.
pub struct Budget {
    pub(super) limits: Limits,
    pub(super) statistics: Cell<Statistics>,
}
impl Budget {
    /// Admit work before performing it; a refused charge is not committed.
    /// # Errors
    /// Reports the inclusive work ceiling.
    pub fn work(&self, amount: u64) -> Result<(), Stop> {
        let mut statistics = self.statistics.get();
        crate::limits::check(
            crate::Resource::Work,
            u128::from(statistics.work) + u128::from(amount),
            u128::from(self.limits.max_work),
        )?;
        statistics.work += amount;
        self.statistics.set(statistics);
        Ok(())
    }
    /// Admit inspected name/string bytes before examining their payload.
    /// # Errors
    /// Reports the inclusive inspected-byte ceiling.
    pub fn bytes(&self, amount: u64) -> Result<(), Stop> {
        let mut statistics = self.statistics.get();
        crate::limits::check(
            crate::Resource::InspectedBytes,
            u128::from(statistics.inspected_bytes) + u128::from(amount),
            u128::from(self.limits.max_inspected_bytes),
        )?;
        statistics.inspected_bytes += amount;
        self.statistics.set(statistics);
        Ok(())
    }
    /// The committed logical receipt at this point.
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        self.statistics.get()
    }
}
/// A refused operation; logical limits alone become a stopped analysis.
#[derive(Debug)]
pub enum Error<E> {
    /// No part of the analysis may be used after this global stop.
    Limit(Stop),
    /// Host metadata allocation failed.
    Allocation(TryReserveError),
    /// The key owner refused a read or observed control.
    Read(E),
    /// A coordinate, interval or dependency in the neutral plan is invalid.
    InvalidInput,
}
impl<E> From<Stop> for Error<E> {
    fn from(stop: Stop) -> Self {
        Self::Limit(stop)
    }
}
impl<E> From<TryReserveError> for Error<E> {
    fn from(error: TryReserveError) -> Self {
        Self::Allocation(error)
    }
}
/// Failed lowering or transfer, with no partial result.
#[derive(Debug)]
pub struct Failure<E, C> {
    /// Typed owner, allocation or input failure.
    pub error: Error<E>,
    /// Work completed before the failure.
    pub statistics: Statistics,
    /// Original context of the failed operation.
    pub context: Option<C>,
}
/// Nodes reference only earlier expressions; the plan is acyclic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expression {
    /// An authenticated source constant.
    Constant(ConstantId),
    /// A local slot in the current rule.
    Slot(usize),
    /// Unary arithmetic on one expression.
    Unary(UnaryOp, ExpressionId),
    /// Absolute value; retained explicitly even when its static bound is unknown.
    Absolute(ExpressionId),
    /// Binary arithmetic preserving operand order.
    Binary(BinaryOp, ExpressionId, ExpressionId),
    /// A local transfer without a more precise bound. This is a summary, not
    /// a denotational identity: even reusing its coordinate cannot establish
    /// correlation or preserve an expression-specific guard refinement.
    Unknown {
        /// Every defined result is an integer.
        integer: bool,
        /// Why the finite component lost precision.
        reason: Widening,
    },
}
/// Complete-binding constraints; comparisons do not belong here.
#[derive(Clone, Copy, Debug)]
pub enum Binding {
    /// One ordinary positive occurrence; repeated occurrences meet.
    Positive {
        /// Local variable slot.
        slot: usize,
        /// Predicate argument supplying the occurrence.
        argument: ArgumentId,
    },
    /// A copy, constant or arithmetic assignment.
    Assign {
        /// Assigned slot.
        slot: usize,
        /// Assigned expression.
        value: ExpressionId,
    },
    /// A generating interval; membership tests instead use `Guard::Membership`.
    Range {
        /// Generated slot.
        slot: usize,
        /// Lower endpoint expression.
        low: ExpressionId,
        /// Upper endpoint expression.
        high: ExpressionId,
    },
    /// A conservative aggregate assignment, without a claimed tuple envelope.
    Aggregate {
        /// Assigned slot.
        slot: usize,
        /// Aggregate function.
        function: AggregateFunction,
    },
}
/// Ordered conditions refining only the values that can produce heads.
#[derive(Clone, Copy, Debug)]
pub enum Guard {
    /// A comparison; neither operand establishes integrality by comparison alone.
    Compare {
        /// Left expression.
        left: ExpressionId,
        /// Source relation.
        relation: Relation,
        /// Right expression.
        right: ExpressionId,
    },
    /// An already-bound slot tested against an interval.
    Membership {
        /// Bound target slot.
        slot: usize,
        /// Lower endpoint expression.
        low: ExpressionId,
        /// Upper endpoint expression.
        high: ExpressionId,
    },
}
/// One output argument of a producer rule.
#[derive(Clone, Copy, Debug)]
pub struct Head {
    /// Produced predicate argument.
    pub argument: ArgumentId,
    /// Its expression over produced slot ranges.
    pub value: ExpressionId,
}
/// One independent variable scope and its ordered producer transfer.
#[derive(Debug, Default)]
pub struct Rule {
    /// Number of slots; every referenced slot must lie in this extent.
    pub slots: usize,
    /// Positive occurrences followed by assignments in dependency order.
    pub bindings: Vec<Binding>,
    /// Source-order guard refinements, applied once.
    pub guards: Vec<Guard>,
    /// Output argument occurrences; duplicates retain their provenance.
    pub heads: Vec<Head>,
    /// Expressions to certify against complete-binding domains after convergence.
    /// Request order is retained, including duplicates. An empty list adds no
    /// certificate work. Requests neither refine bounds nor change head transfer.
    /// Covered arithmetic is negation, addition/subtraction, multiplication by a
    /// literal, division by a literal and remainder by a bounded nonzero interval.
    /// Every operand and intermediate must have a defined integer range.
    pub integer_queries: Vec<ExpressionId>,
}

#[derive(Debug)]
pub(super) struct Constant<K> {
    key: Option<K>,
    number: Option<i32>,
}
#[derive(Debug)]
pub(super) struct Argument<C> {
    bound: Bound,
    values: Option<Vec<usize>>,
    producers: Vec<C>,
    widening: Option<Widening>,
}
/// Completed argument domains and original submitted producer contexts.
#[derive(Debug)]
pub struct Analysis<K, C> {
    constants: Vec<Constant<K>>,
    arguments: Vec<Argument<C>>,
    rules: Vec<RuleBounds>,
    context: Option<C>,
    statistics: Statistics,
    status: Status,
}
/// Distinct complete-binding and produced bounds for one submitted rule.
#[derive(Debug)]
pub struct RuleBounds {
    domains: Vec<Bound>,
    produced: Vec<Bound>,
    integers: Vec<integer::Evaluation>,
}
/// Defined integer arithmetic over one completed rule's binding domains.
///
/// Every operand and intermediate in the supported grammar is checked. This
/// establishes neither a complete binding's existence nor a prefix guarantee,
/// and it does not certify arithmetic in other expressions or in the bindings
/// that supply a slot. The token borrows the exact rule result; callers retain
/// responsibility for mapping that submitted scope to their original owner.
#[derive(Clone, Copy, Debug)]
pub struct IntegerCertificate<'rule> {
    scope: &'rule RuleBounds,
    query: usize,
    range: crate::Range,
}
impl IntegerCertificate<'_> {
    /// The requested expression coordinate, meaningful only in this analysis.
    #[must_use]
    pub fn expression(self) -> ExpressionId {
        self.scope.integers[self.query].expression
    }
    /// Every complete binding has a defined integer result within this interval.
    #[must_use]
    pub fn range(self) -> crate::Range {
        self.range
    }
}
impl RuleBounds {
    /// Number of original requests, including conservative declines.
    #[must_use]
    pub fn integer_query_count(&self) -> usize {
        self.integers.len()
    }
    /// Certify one request by its position in this rule's submitted list.
    ///
    /// None means an absent request or a conservative decline, including empty,
    /// mixed or unbounded operands, unsupported arithmetic and a possibly
    /// undefined intermediate. It does not establish an impossible binding.
    /// Guard-refined produced bounds never justify this result.
    #[must_use]
    pub fn integer(&self, query: usize) -> Option<IntegerCertificate<'_>> {
        let range = self.integers.get(query)?.range?;
        Some(IntegerCertificate {
            scope: self,
            query,
            range,
        })
    }
    /// Complete-binding coverage, without guard refinements or prefix guarantees.
    #[must_use]
    pub fn domains(&self) -> &[Bound] {
        &self.domains
    }
    /// Bounds after one ordered guard pass, suitable only for head transfer.
    #[must_use]
    pub fn produced(&self) -> &[Bound] {
        &self.produced
    }
}
impl<K, C: Copy> Analysis<K, C> {
    /// Whether all published bounds are complete or all were cleared.
    #[must_use]
    pub fn status(&self) -> Status {
        self.status
    }
    /// Completed charges, including failed attempts' committed prefix.
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        self.statistics
    }
    /// Stop context; absent on a completed analysis.
    #[must_use]
    pub fn context(&self) -> Option<C> {
        self.context
    }
    /// Numeric components of a valid completed argument.
    #[must_use]
    pub fn argument(&self, argument: ArgumentId) -> Option<Bound> {
        self.arguments.get(argument.0).map(|value| value.bound)
    }
    /// Arguments in registration order.
    pub fn arguments(&self) -> impl ExactSizeIterator<Item = (ArgumentId, Bound)> {
        self.arguments
            .iter()
            .enumerate()
            .map(|(index, value)| (ArgumentId(index), value.bound))
    }
    /// Finite values, or None for Unknown/unavailable. Order is registration
    /// identity, not semantic term order; callers must not infer term order.
    /// # Panics
    /// Iteration panics if an internal finite entry names an unretained key.
    /// Completed analyses maintain this invariant.
    #[must_use]
    pub fn values(&self, argument: ArgumentId) -> Option<impl ExactSizeIterator<Item = &K>> {
        self.arguments
            .get(argument.0)?
            .values
            .as_ref()
            .map(|values| {
                values.iter().map(|&index| {
                    self.constants[index]
                        .key
                        .as_ref()
                        .expect("only retained keys enter finite sets")
                })
            })
    }
    /// Finite entries in increasing analysis-local registration order. These
    /// coordinates may compare sets from this same analysis without reading
    /// payloads. They do not authenticate a different analysis or key owner.
    /// # Panics
    /// Iteration panics if an internal finite entry names an unretained key.
    /// Completed analyses maintain this invariant.
    #[must_use]
    pub fn entries(
        &self,
        argument: ArgumentId,
    ) -> Option<impl ExactSizeIterator<Item = (ConstantId, &K)>> {
        self.arguments
            .get(argument.0)?
            .values
            .as_ref()
            .map(|values| {
                values.iter().map(|&index| {
                    (
                        ConstantId(index),
                        self.constants[index]
                            .key
                            .as_ref()
                            .expect("only retained keys enter finite sets"),
                    )
                })
            })
    }
    /// Direct original contexts for every submitted output occurrence.
    #[must_use]
    pub fn producers(&self, argument: ArgumentId) -> Option<&[C]> {
        self.arguments
            .get(argument.0)
            .map(|value| value.producers.as_slice())
    }
    /// The first recorded loss of finite precision for this argument.
    #[must_use]
    pub fn widening(&self, argument: ArgumentId) -> Option<Widening> {
        self.arguments
            .get(argument.0)
            .and_then(|value| value.widening)
    }
    /// Local bounds recomputed from the completed argument result.
    #[must_use]
    pub fn rule(&self, rule: RuleId) -> Option<&RuleBounds> {
        self.rules.get(rule.0)
    }
}
/// Lower and analyze one complete producer input with one shared logical meter.
/// A logical stop returns a cleared Analysis; other failures return no Analysis.
/// The reader and callback are synchronous and are not retained in the result.
/// # Errors
/// Returns owner/read, allocation or invalid-plan failures with their receipt.
pub fn analyze<R: Values, C: Copy>(
    reader: &R,
    limits: Limits,
    lower: impl FnOnce(&mut Input<'_, R, C>) -> Result<(), Error<R::Error>>,
) -> Result<Analysis<R::Key, C>, Failure<R::Error, C>> {
    let budget = Budget {
        limits,
        statistics: Cell::new(Statistics::default()),
    };
    let mut input = Input::new(reader, &budget);
    let attempt = lower(&mut input).and_then(|()| engine::run(&mut input));
    match attempt {
        Ok(engine::Completed { arguments, rules }) => input.finish(arguments, rules),
        Err(Error::Limit(stop)) => Ok(Analysis {
            constants: Vec::new(),
            arguments: Vec::new(),
            rules: Vec::new(),
            context: input.context,
            statistics: budget.statistics(),
            status: Status::Stopped(stop),
        }),
        Err(error) => Err(Failure {
            error,
            statistics: budget.statistics(),
            context: input.context,
        }),
    }
}

#[cfg(test)]
mod tests;
