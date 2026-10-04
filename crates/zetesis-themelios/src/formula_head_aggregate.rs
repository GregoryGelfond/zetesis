//! Finite aggregate heads retain separate permission and measure.
//! Complete tuple validation precedes support or final lowering. Every measure
//! permits either tuple/atom alias direction. Neither bounds nor tuple weights
//! supply bindings or support.
//! Positive conditions enumerate possible eligibility; default-negated gates
//! consume established bindings. Choice lowering retains every eligibility
//! formula. Support-table membership is never interpreted as truth.

mod validation;
pub(crate) use validation::{Bijection, validate_group};

use crate::ProgramSite;
use crate::formula_support::components::Term;
use themelios_program::program::{AggregateFunction, HasGuards, HeadAggregate};
use zetesis_core::ValueNodeRef;
use zetesis_core::catalog::TermRef;

use crate::formula_ir::{
    Compiler, Element, HeadElementKey, HeadMeasure, LiteralIr, LocalFamily, Variables,
};
use crate::{ExpansionResource, FormulaFailure};

impl Compiler<'_> {
    pub(super) fn aggregate_head_elements(
        &mut self,
        aggregate: &HeadAggregate,
        variables: &Variables,
    ) -> Result<(HeadMeasure, Vec<Element>), FormulaFailure> {
        let measure = match aggregate.function() {
            AggregateFunction::Count => HeadMeasure::Count,
            AggregateFunction::Sum => HeadMeasure::Sum,
            AggregateFunction::SumPlus => HeadMeasure::SumPlus,
            AggregateFunction::Min => HeadMeasure::Min,
            AggregateFunction::Max => HeadMeasure::Max,
        };
        let mut elements = Vec::new();
        for (index, element) in aggregate.elements().enumerate() {
            let family = LocalFamily(index);
            let element = element.get();
            let fields: Vec<_> = element.terms().collect();
            for (terms, source_condition) in
                self.local_alternatives(&fields, element.condition())?
            {
                for literal in self.literal_alternatives(element.literal())? {
                    // Preserve the existing per-literal preflight before compiling the
                    // shared choice-condition profile. Negative gates supply no inputs;
                    // their original polarity survives both support and final lowering.
                    for _ in element.condition().literals() {
                        self.budget
                            .charge(ExpansionResource::TermWork, 1, self.location)?;
                    }
                    let mut local = variables.clone();
                    self.head_global_literal(&literal, &mut local)?;
                    let mut condition = self.condition(&source_condition, &mut local)?;
                    let tuple = terms
                        .iter()
                        .map(|term| {
                            if matches!(
                                term,
                                themelios_program::term::Term::Variable(_)
                                    | themelios_program::term::Term::Symbolic(_)
                            ) {
                                self.aggregate_term(term, &mut local)
                            } else {
                                self.generated_term(term, &mut local, &mut condition)
                            }
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    // A present extremum value is validated even in a statically false rule.
                    // Without bounds only the head choices remain, but source terms and
                    // binding instructions are still compiled and validated below.
                    if aggregate.left_guard().is_some() || aggregate.right_guard().is_some() {
                        match tuple.first() {
                            Some(Term::Constant(value)) => {
                                let value = self.source.scalar_ref(
                                    *value,
                                    self.limits,
                                    self.counters,
                                    self.location,
                                )?;
                                contribution(measure, Some(value), self.location)?;
                            }
                            None => {
                                contribution(measure, None, self.location)?;
                            }
                            Some(Term::Variable(_)) => {}
                        }
                    }
                    let (head, body_variables) =
                        self.element_head(&literal, &mut local, &mut condition)?;
                    self.variable_limit(&local)?;
                    local.safety(self.location)?;
                    debug_assert!(condition.iter().all(|literal| matches!(
                        literal,
                        LiteralIr::Bind { .. }
                            | LiteralIr::Atom(..)
                            | LiteralIr::ProjectedAtom(..)
                            | LiteralIr::PatternAtom(_)
                            | LiteralIr::ArgumentCheck { .. }
                            | LiteralIr::Range { .. }
                            | LiteralIr::Compare(..)
                            | LiteralIr::TupleCompare(..)
                            | LiteralIr::Guard(_)
                    )));
                    elements.push(Element {
                        family,
                        key: HeadElementKey::Tuple(tuple),
                        head,
                        condition,
                        body_variables,
                        variables: local.count,
                    });
                }
            }
        }
        Ok((measure, elements))
    }
}

/// A selected tuple contributes either an integer or a complete logical value.
/// Borrowing preserves allocation-free validation. Retained consumers name the
/// value through the shared computation vocabulary rather than copying payload.
pub(super) enum Contribution<'a> {
    Numeric(i32),
    Extremum(TermRef<'a>),
}

/// Contribution is independent of permission to select the head. Count ignores
/// tuple values; missing/nonnumeric sum values have weight zero. Numeric
/// nonpositive sum+ weights also contribute nothing. All retain permission.
/// Missing extremum values are neutral; complete values retain their logical
/// order and admitted domain. An unbounded head never calls this operation
/// because it has no measure bound.
pub(super) fn contribution(
    measure: HeadMeasure,
    first: Option<TermRef<'_>>,
    location: ProgramSite,
) -> Result<Option<Contribution<'_>>, FormulaFailure> {
    if measure == HeadMeasure::Count {
        return Ok(Some(Contribution::Numeric(1)));
    }
    if matches!(measure, HeadMeasure::Min | HeadMeasure::Max) {
        let Some(value) = first else {
            return Ok(None);
        };
        crate::formula_assignment::extremum_value(value, location)?;
        return Ok(Some(Contribution::Extremum(value)));
    }
    let Some(ValueNodeRef::Number(value)) = first.map(TermRef::descriptor) else {
        return Ok(None);
    };
    Ok((measure != HeadMeasure::SumPlus || value > 0).then_some(Contribution::Numeric(value)))
}

#[cfg(test)]
mod tests;
