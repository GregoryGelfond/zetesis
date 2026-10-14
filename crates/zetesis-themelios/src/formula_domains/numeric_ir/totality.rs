//! Static scalar totality for possible-head production.
//!
//! The lowering requests every scalar operand of an eligible rule, in literal
//! order, against that exact neutral rule. Its complete-binding domains include
//! every compiled producer, rather than one intermediate support snapshot.
//! Successful requests retain only the original rule borrow and occurrence.
//! They grant prefix selection, not a computed-key map, source-family evidence
//! or a witness that a complete binding exists.

use zetesis_core::TemplateComponentsRef;
use zetesis_domain::numeric;

use super::{Analysis, census};
use crate::formula_ir::{HeadIr, LiteralIr, RuleIr};
use crate::formula_support::Counters;
use crate::{FormulaFailure, FormulaLimits};

pub(super) struct Request<'source> {
    pub(super) index: usize,
    pub(super) source: &'source RuleIr,
    pub(super) rule: numeric::RuleId,
}

/// Every complete binding of this rule has defined scalar body expressions.
/// Private construction requires the finished all-producer analysis. A prefix
/// can still contain an out-of-domain value; its evaluator stays checked.
pub(crate) struct TotalBody<'source> {
    source: &'source RuleIr,
}

impl TotalBody<'_> {
    pub(crate) fn belongs_to(&self, source: &RuleIr) -> bool {
        std::ptr::eq(self.source, source)
    }
}

/// Request execution evidence only where Complete coverage otherwise prevents
/// scalar prefix selection. The shared readiness check establishes positive
/// slot coverage after the analysis; syntax alone never issues a certificate.
pub(super) fn requested(source: &RuleIr) -> bool {
    matches!(source.head, HeadIr::Normal(Some(_)))
        && source.variables == source.body_variables
        && source.bindings.is_none()
        && crate::formula_support::family::partial(&source.body)
        && source.body.iter().all(|literal| {
            matches!(
                literal,
                LiteralIr::Atom(..) | LiteralIr::Compare(_) | LiteralIr::ArgumentCheck(_)
            )
        })
}

impl<'source> Analysis<'source> {
    /// Retain sparse certificates in original occurrence order. The temporary
    /// query graph and numeric result are released with this analysis. Metadata
    /// is bounded by admitted rules and each attempted publication is charged.
    pub(crate) fn total_bodies(
        &self,
        components: TemplateComponentsRef<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<Vec<(usize, TotalBody<'source>)>, FormulaFailure> {
        let mut result = Vec::new();
        if self.values.status() != numeric::Status::FixedPoint {
            return Ok(result);
        }
        for request in &self.totality {
            counters.work(limits, request.source.location)?;
            if !self.total(request, components, limits, counters)? {
                continue;
            }
            counters.work(limits, request.source.location)?;
            result
                .try_reserve(1)
                .map_err(|error| FormulaFailure::AtomAllocation {
                    error,
                    location: request.source.location,
                })?;
            result.push((
                request.index,
                TotalBody {
                    source: request.source,
                },
            ));
        }
        Ok(result)
    }

    fn total(
        &self,
        request: &Request<'_>,
        components: TemplateComponentsRef<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<bool, FormulaFailure> {
        let source = request.source;
        let missing = || crate::formula_support::components::missing(source.location);
        let bounds = self.values.rule(request.rule).ok_or_else(missing)?;
        let Some(binding) = census::readiness(source, components, limits, counters)? else {
            return Ok(false);
        };
        if bounds.domains().len() != binding.len() {
            return Err(missing());
        }
        for (depth, domain) in binding.iter().zip(bounds.domains()) {
            counters.work(limits, source.location)?;
            if depth.is_none()
                || (domain.integrality == numeric::Integrality::Integer
                    && domain.interval == numeric::Interval::Bottom)
            {
                return Ok(false);
            }
        }
        let mut query = 0;
        let mut total = true;
        for literal in &source.body {
            counters.work(limits, source.location)?;
            let comparison = match literal {
                LiteralIr::Compare(comparison) | LiteralIr::ArgumentCheck(comparison) => comparison,
                LiteralIr::Atom(..) => continue,
                _ => return Err(missing()),
            };
            for expression in [comparison.left(), comparison.right()] {
                counters.work(limits, source.location)?;
                total &=
                    bounds.integer(query).is_some() || census::leaf(expression, Some(&binding));
                query += 1;
            }
        }
        if query != bounds.integer_query_count() {
            return Err(missing());
        }
        Ok(total)
    }
}
