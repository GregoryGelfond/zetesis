//! Diagnostic use of scoped integer certificates; no grounding permission is issued.

use themelios_program::program::{DefaultNegation, Relation};
use zetesis_core::{TemplateComponentsRef, TemplateTerm};
use zetesis_domain::{Range, numeric};

use crate::formula_ir::{Comparison, Expression, HeadIr, LiteralIr, Operation, RuleIr};
use crate::formula_support::Counters;
use crate::{FormulaFailure, FormulaLimits, ProgramSite};

use super::Analysis;

/// Populations in one completed numeric analysis. Rule counts use non-fact
/// compiled occurrences; compact fact rows supply argument seeds, not rules.
/// Counts describe analysis results, never solver work saved or answer sets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NumericCensus {
    /// Predicate argument positions.
    pub positions: usize,
    /// Positions whose possible values are all integers, including empty domains.
    pub integer_positions: usize,
    /// Integer positions with nonempty bounded intervals.
    pub bounded_integer_positions: usize,
    /// Positions with a retained finite value set, including empty sets.
    pub finite_positions: usize,
    /// Entries across those finite value sets.
    pub finite_entries: usize,
    /// Empty domains: Integer integrality and Bottom numeric interval.
    pub empty_positions: usize,
    /// Positions that may contain nonnumeric values.
    pub mixed_positions: usize,
    /// Positions with no numeric interval bound; may overlap mixed positions.
    pub unbounded_positions: usize,
    /// Non-fact compiled rule occurrences, including unsupported head forms.
    pub rule_occurrences: usize,
    /// Occurrences in the initial ordinary-atom/scalar-comparison body profile.
    pub eligible_rules: usize,
    /// Occurrences outside that profile, including rich/local head forms.
    pub unsupported_rules: usize,
    /// Eligible rules with a provably empty complete-binding slot domain.
    pub empty_rules: usize,
    /// Requested scalar-comparison operands of normal rule occurrences.
    pub integer_queries: usize,
    /// Requests with complete-binding integer certificates.
    pub integer_certificates: usize,
    /// Requested operands without a certificate; no impossibility is implied.
    pub integer_declines: usize,
    /// Eligible scopes with no proved-empty slot and every body expression covered.
    pub total_bodies: usize,
    /// Total bodies with a normal atomic head; constraints are excluded.
    pub normal_head_total_bodies: usize,
    /// Comparisons true for every pair in their certified integer intervals.
    pub separated_true: usize,
    /// Comparisons false for every pair in their certified integer intervals.
    pub separated_false: usize,
    /// Separated true comparisons with the join's exclusion role.
    pub excluding_true: usize,
    /// Separated false comparisons with the join's exclusion role.
    pub excluding_false: usize,
}

/// Diagnostic totality of one top-level non-fact occurrence. None of these
/// outcomes certifies that a complete binding exists or a prefix extends to one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumericBody {
    /// Rich/local head, generated suffix, or a body outside the initial profile.
    Unsupported,
    /// An eligible complete-binding slot domain is empty; no family witness exists.
    Empty,
    /// Eligible shape, but at least one required input/expression is uncertified.
    Uncertified,
    /// Every relevant expression is total under the complete-binding domains.
    /// Constant and bound-variable leaves need not have numeric types.
    Total,
}

/// What establishes a scalar operand's definedness in this report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumericOperand {
    /// A defined integer expression under this rule's complete-binding domains.
    Integer(Range),
    /// An admitted constant or ordinarily bound variable leaf. It may be symbolic;
    /// no numeric type or interval follows from this case.
    Leaf,
    /// No supported totality premise; this is not proof of undefinedness.
    Uncertified,
}

/// One scalar comparison's diagnostic result. Certified ranges cover complete
/// bindings only. A separated verdict does not remove or replace the literal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumericComparison {
    /// Literal position in this exact compiled body's occurrence array.
    pub literal: usize,
    /// The source comparison relation.
    pub relation: Relation,
    /// Left operand's integer certificate, explicit leaf case, or decline.
    pub left: NumericOperand,
    /// Right operand's integer certificate, explicit leaf case, or decline.
    pub right: NumericOperand,
    /// Constant endpoint verdict, only when both operands are certified integers.
    pub separated: Option<bool>,
    /// Whether ordinary positive inputs decide this comparison with no prefix.
    /// Unsupported body profiles are unclassified, represented by None.
    pub excluding: Option<bool>,
}

/// Borrowed report for an authentic compiled rule occurrence in this analysis.
/// Its ordinal and source site are reporting coordinates, not transferable IDs.
#[derive(Clone, Copy)]
pub struct NumericRuleAnalysis<'a> {
    pub(super) rule: &'a Occurrence<'a>,
    pub(super) values: &'a Values,
    pub(super) occurrence: usize,
}
impl NumericRuleAnalysis<'_> {
    /// Position among non-fact compiled occurrences, independent of source sites.
    #[must_use]
    pub fn occurrence(self) -> usize {
        self.occurrence
    }
    /// Original source/statement evidence; distinct expansions may share it.
    #[must_use]
    pub fn site(self) -> ProgramSite {
        self.rule.source.location
    }
    /// Whole-body result, separate from individual integer certificates.
    #[must_use]
    pub fn body(self) -> NumericBody {
        self.rule.body
    }
    /// Whether this occurrence has a normal atomic head rather than a constraint.
    #[must_use]
    pub fn normal_head(self) -> bool {
        matches!(self.rule.source.head, HeadIr::Normal(Some(_)))
    }
}
impl<'a> NumericRuleAnalysis<'a> {
    /// Scalar comparisons in literal order; unsupported rich/local occurrences
    /// have no requests in this initial report. No source payload is copied.
    ///
    /// # Panics
    /// Panics if an internal mapping names a missing neutral rule or comparison.
    /// Completed census preparation checks that mapping before publication.
    pub fn comparisons(self) -> impl Iterator<Item = NumericComparison> + 'a {
        self.rule.requests.iter().flat_map(move |requests| {
            let bounds = self
                .values
                .rule(requests.rule)
                .expect("completed mapped rule");
            requests.comparisons.iter().map(move |query| {
                let comparison = comparison(self.rule.source, query.literal)
                    .expect("checked comparison occurrence");
                NumericComparison {
                    literal: query.literal,
                    relation: comparison.relation(),
                    left: operand(bounds.integer(query.first), query.leaves[0]),
                    right: operand(bounds.integer(query.first + 1), query.leaves[1]),
                    separated: query.separated,
                    excluding: query.excluding,
                }
            })
        })
    }
}

pub(super) struct Query {
    literal: usize,
    first: usize,
    roots: [numeric::ExpressionId; 2],
    leaves: [bool; 2],
    separated: Option<bool>,
    excluding: Option<bool>,
}
impl Query {
    pub(super) fn new(literal: usize, first: usize, roots: [numeric::ExpressionId; 2]) -> Self {
        Self {
            literal,
            first,
            roots,
            leaves: [false; 2],
            separated: None,
            excluding: None,
        }
    }
}
pub(super) struct Requests {
    pub(super) rule: numeric::RuleId,
    pub(super) comparisons: Vec<Query>,
}
pub(super) struct Occurrence<'a> {
    source: &'a RuleIr,
    requests: Option<Requests>,
    body: NumericBody,
}
impl<'a> Occurrence<'a> {
    pub(super) fn new(source: &'a RuleIr, requests: Option<Requests>) -> Self {
        Self {
            source,
            requests,
            body: NumericBody::Unsupported,
        }
    }
}

impl Analysis<'_> {
    pub(crate) fn prepare_census(
        &mut self,
        components: TemplateComponentsRef<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        if self.values.status() != numeric::Status::FixedPoint {
            return Ok(());
        }
        let mut census = NumericCensus::default();
        for (argument, bound) in self.values.arguments() {
            counters.work(limits, location)?;
            census.positions += 1;
            match bound.integrality {
                numeric::Integrality::Integer => {
                    census.integer_positions += 1;
                    census.bounded_integer_positions +=
                        usize::from(matches!(bound.interval, numeric::Interval::Bounded { .. }));
                    census.empty_positions +=
                        usize::from(bound.interval == numeric::Interval::Bottom);
                }
                numeric::Integrality::Mixed => census.mixed_positions += 1,
            }
            census.unbounded_positions +=
                usize::from(bound.interval == numeric::Interval::Unbounded);
            counters.work(limits, location)?;
            if let Some(values) = self.values.values(argument) {
                census.finite_positions += 1;
                census.finite_entries += values.len();
            }
        }
        for occurrence in &mut self.occurrences {
            counters.work(limits, occurrence.source.location)?;
            census.rule_occurrences += 1;
            prepare_rule(
                occurrence,
                &self.values,
                components,
                limits,
                counters,
                &mut census,
            )?;
        }
        self.census = Some(census);
        Ok(())
    }
}

type Values = numeric::Analysis<zetesis_core::catalog::TermKey, ProgramSite>;

fn prepare_rule(
    occurrence: &mut Occurrence<'_>,
    values: &Values,
    components: TemplateComponentsRef<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    census: &mut NumericCensus,
) -> Result<(), FormulaFailure> {
    let source = occurrence.source;
    let Some(requests) = &mut occurrence.requests else {
        census.unsupported_rules += 1;
        return Ok(());
    };
    counters.work(limits, source.location)?;
    let bounds = values.rule(requests.rule).ok_or_else(|| missing(source))?;
    if bounds.integer_query_count() != requests.comparisons.len() * 2 {
        return Err(missing(source));
    }
    let binding = readiness(source, components, limits, counters)?;
    let mut empty = false;
    let mut total = binding.is_some();
    if let Some(binding) = &binding {
        census.eligible_rules += 1;
        if bounds.domains().len() != source.variables {
            return Err(missing(source));
        }
        for (depth, domain) in binding.iter().zip(bounds.domains()) {
            counters.work(limits, source.location)?;
            total &= depth.is_some();
            empty |= domain.integrality == numeric::Integrality::Integer
                && domain.interval == numeric::Interval::Bottom;
        }
    } else {
        census.unsupported_rules += 1;
    }
    for query in &mut requests.comparisons {
        counters.work(limits, source.location)?;
        let comparison = comparison(source, query.literal).ok_or_else(|| missing(source))?;
        let mut ranges = [None, None];
        for (side, expression) in [comparison.left(), comparison.right()]
            .into_iter()
            .enumerate()
        {
            counters.work(limits, source.location)?;
            census.integer_queries += 1;
            if let Some(certificate) = bounds.integer(query.first + side) {
                if certificate.expression() != query.roots[side] {
                    return Err(missing(source));
                }
                ranges[side] = Some(certificate.range());
                census.integer_certificates += 1;
            } else {
                census.integer_declines += 1;
            }
            counters.work(limits, source.location)?;
            query.leaves[side] = leaf(expression, binding.as_deref());
            total &= ranges[side].is_some() || query.leaves[side];
        }
        counters.work(limits, source.location)?;
        query.separated = ranges[0]
            .zip(ranges[1])
            .and_then(|(left, right)| separated(left, comparison.relation(), right));
        if let Some(binding) = &binding {
            counters.absorb(
                (comparison.left().nodes.len() as u128) + (comparison.right().nodes.len() as u128),
                limits,
                source.location,
            )?;
            query.excluding = Some(
                crate::formula_support::comparison_depth(
                    [comparison.left(), comparison.right()],
                    binding,
                )
                .is_some(),
            );
        }
        match query.separated {
            Some(true) => {
                census.separated_true += 1;
                census.excluding_true += usize::from(query.excluding == Some(true));
            }
            Some(false) => {
                census.separated_false += 1;
                census.excluding_false += usize::from(query.excluding == Some(true));
            }
            None => {}
        }
    }
    occurrence.body = body_result(source, binding.is_some(), empty, total, census);
    Ok(())
}

fn body_result(
    source: &RuleIr,
    eligible: bool,
    empty: bool,
    total: bool,
    census: &mut NumericCensus,
) -> NumericBody {
    if !eligible {
        NumericBody::Unsupported
    } else if empty {
        census.empty_rules += 1;
        NumericBody::Empty
    } else if total {
        census.total_bodies += 1;
        census.normal_head_total_bodies +=
            usize::from(matches!(source.head, HeadIr::Normal(Some(_))));
        NumericBody::Total
    } else {
        NumericBody::Uncertified
    }
}

/// Match the initial ordinary join profile. This collects only its positive
/// slot coverage; the shared Decisions fold remains the readiness authority.
fn readiness(
    source: &RuleIr,
    components: TemplateComponentsRef<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<Option<Vec<Option<usize>>>, FormulaFailure> {
    counters.work(limits, source.location)?;
    if source.variables != source.body_variables || source.bindings.is_some() {
        return Ok(None);
    }
    let mut bound = Vec::new();
    bound
        .try_reserve_exact(source.variables)
        .map_err(|error| FormulaFailure::AtomAllocation {
            error,
            location: source.location,
        })?;
    for _ in 0..source.variables {
        counters.work(limits, source.location)?;
        bound.push(None);
    }
    for literal in &source.body {
        counters.work(limits, source.location)?;
        match literal {
            LiteralIr::Atom(DefaultNegation::None, pattern) => {
                let pattern = pattern.get(components, limits, counters, source.location)?;
                for column in 0..pattern.terms().len() {
                    counters.work(limits, source.location)?;
                    if let Some(TemplateTerm::Variable(slot)) = pattern.terms().at(column) {
                        *bound.get_mut(slot).ok_or_else(|| missing(source))? = Some(0);
                    }
                }
            }
            LiteralIr::Atom(DefaultNegation::Not | DefaultNegation::NotNot, _)
            | LiteralIr::Compare(_)
            | LiteralIr::ArgumentCheck(_) => {}
            _ => return Ok(None),
        }
    }
    Ok(Some(bound))
}

fn operand(certificate: Option<numeric::IntegerCertificate<'_>>, leaf: bool) -> NumericOperand {
    if let Some(certificate) = certificate {
        NumericOperand::Integer(certificate.range())
    } else if leaf {
        NumericOperand::Leaf
    } else {
        NumericOperand::Uncertified
    }
}

fn leaf(expression: &Expression, bound: Option<&[Option<usize>]>) -> bool {
    match expression.nodes.as_slice() {
        [Operation::Constant(_)] => true,
        [Operation::Variable(slot)] => bound
            .and_then(|bound| bound.get(*slot))
            .is_some_and(Option::is_some),
        _ => false,
    }
}

fn comparison(source: &RuleIr, literal: usize) -> Option<&Comparison> {
    match source.body.get(literal)? {
        LiteralIr::Compare(value) | LiteralIr::ArgumentCheck(value) => Some(value),
        _ => None,
    }
}

fn missing(source: &RuleIr) -> FormulaFailure {
    crate::formula_support::components::missing(source.location)
}

fn separated(left: Range, relation: Relation, right: Range) -> Option<bool> {
    let (low, high) = left.bounds();
    let (right_low, right_high) = right.bounds();
    match relation {
        Relation::Eq if high < right_low || right_high < low => Some(false),
        Relation::Eq if low == high && right_low == right_high => Some(low == right_low),
        Relation::Neq => separated(left, Relation::Eq, right).map(|equal| !equal),
        Relation::Lt if high < right_low => Some(true),
        Relation::Lt if low >= right_high => Some(false),
        Relation::Le if high <= right_low => Some(true),
        Relation::Le if low > right_high => Some(false),
        Relation::Gt => separated(right, Relation::Lt, left),
        Relation::Ge => separated(right, Relation::Le, left),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
