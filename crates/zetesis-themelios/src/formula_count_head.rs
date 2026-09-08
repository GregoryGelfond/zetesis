//! Finite count heads enter ordinary choice lowering only after a complete
//! tuple/atom correspondence check. Neither count bounds nor tuple values supply
//! bindings or support; both alias directions remain explicit profile refusals.
//! Positive conditions enumerate possible eligibility; default-negated gates
//! consume established bindings. Choice lowering retains every eligibility
//! formula. Support-table membership is never interpreted as truth.

use std::collections::BTreeMap;

use themelios_base::span::Location;
use themelios_program::program::{AggregateFunction, HeadAggregate};
use zetesis_core::{Atom, Value};

use crate::diagnostic::unsupported;
use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_ir::{Compiler, Element, LiteralIr, Variables, value_bytes};
use crate::formula_support::{Counters, Join, Support};
use crate::{ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource, ProfileFeature};

impl Compiler<'_> {
    pub(super) fn count_head_elements(
        &mut self,
        aggregate: &HeadAggregate,
        variables: &Variables,
    ) -> Result<Vec<Element>, FormulaFailure> {
        if aggregate.function() != AggregateFunction::Count {
            return Err(unsupported(ProfileFeature::Head, self.location).into());
        }
        let mut elements = Vec::new();
        for element in aggregate.elements() {
            let element = element.get();
            // Preserve the existing per-literal preflight before compiling the
            // shared choice-condition profile. Negative gates supply no inputs;
            // their original polarity survives both support and final lowering.
            for _ in element.condition().literals() {
                self.budget
                    .charge(ExpansionResource::TermWork, 1, self.location)?;
            }
            let mut local = variables.clone();
            let mut condition = self.condition(element.condition(), &mut local)?;
            let tuple = element
                .terms()
                .map(|term| self.aggregate_term(term, &mut local))
                .collect::<Result<Vec<_>, _>>()?;
            let head = self.choice_head(element.literal(), &mut local, &mut condition)?;
            self.bindings(&mut condition, &mut local)?;
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
                count_tuple: Some(tuple),
                head,
                condition,
                variables: local.count,
            });
        }
        Ok(elements)
    }
}

/// Validate the entire instantiated group before the caller derives any support
/// or publishes a formula. The immutable completed support/binding is replayed
/// afterwards through the ordinary choice path. Both passes charge their work.
pub(super) fn validate_group(
    elements: &[Element],
    assignment: &[Value],
    support: &Support,
    limits: FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: Location,
) -> Result<(), FormulaFailure> {
    let counted = elements
        .first()
        .is_some_and(|element| element.count_tuple.is_some());
    // An explicit invariant at the common lowering boundary: groups cannot mix
    // ordinary atom-counting elements with tuple-keyed function elements.
    if elements
        .iter()
        .any(|element| element.count_tuple.is_some() != counted)
    {
        return Err(unsupported(ProfileFeature::HeadAggregateAlias, location).into());
    }
    if !counted {
        return Ok(());
    }
    let mut tuples = BTreeMap::<Vec<Value>, Atom>::new();
    let mut atoms = BTreeMap::<Atom, Vec<Value>>::new();
    for element in elements {
        let terms = element
            .count_tuple
            .as_ref()
            .expect("uniform count group checked");
        let mut local = Join::new(
            &element.condition,
            assignment,
            element.variables,
            support,
            budget,
            location,
        )?;
        while let Some(binding) = local.next(limits, budget, counters, location)? {
            counters.work(limits, location)?;
            let mut tuple = Vec::new();
            for term in terms {
                counters.work(limits, location)?;
                let value = term.resolve(&binding).expect("safe count tuple assigned");
                budget.charge(
                    ExpansionResource::ScalarBytes,
                    2 * (std::mem::size_of::<Value>() as u128 + value_bytes(value)),
                    location,
                )?;
                tuple.push(value.clone());
            }
            let atom_bytes = element.head.predicate().name().len() as u128
                + element
                    .head
                    .terms()
                    .iter()
                    .map(|term| {
                        let value = term.resolve(&binding).expect("safe count head assigned");
                        std::mem::size_of::<Value>() as u128 + value_bytes(value)
                    })
                    .sum::<u128>();
            budget.charge(
                ExpansionResource::ScalarBytes,
                atom_bytes.saturating_mul(2),
                location,
            )?;
            let atom = element
                .head
                .instantiate(&binding)
                .expect("safe count head assigned");
            if tuples.get(&tuple).is_some_and(|previous| *previous != atom)
                || atoms.get(&atom).is_some_and(|previous| *previous != tuple)
            {
                return Err(unsupported(ProfileFeature::HeadAggregateAlias, location).into());
            }
            if !tuples.contains_key(&tuple) {
                ceiling(
                    FormulaResource::AggregateElements,
                    tuples.len() as u128 + 1,
                    limits.aggregate.max_elements as u128,
                    location,
                )?;
                tuples.insert(tuple.clone(), atom.clone());
                atoms.insert(atom, tuple);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Budget, Counters, Element, FormulaLimits, Support, validate_group};
    use crate::{AdmissionOptions, ExpansionLimits, FormulaFailure, ProfileFeature};
    use themelios_base::source::{Source, SourceId};
    use themelios_base::span::Location;
    use zetesis_core::{AtomPattern, Predicate, Term, Value};

    fn element(keyed: bool) -> Element {
        Element {
            count_tuple: keyed.then(|| vec![Term::Constant(Value::Number(1))]),
            head: AtomPattern::new(Predicate::new("p", 0).unwrap(), vec![]).unwrap(),
            condition: vec![],
            variables: 0,
        }
    }

    fn validate(elements: &[Element]) -> Result<(), FormulaFailure> {
        let source = Source::new(SourceId::new(0), String::new()).unwrap();
        let location = Location {
            source: source.id(),
            span: source.span(),
        };
        let mut budget = Budget::new(
            ExpansionLimits::default(),
            AdmissionOptions::default().core_limits.max_templates,
        );
        validate_group(
            elements,
            &[],
            &Support::default(),
            FormulaLimits::default(),
            &mut budget,
            &mut Counters::default(),
            location,
        )
    }

    #[test]
    fn mixed_metadata_is_rejected_in_either_order_at_the_shared_boundary() {
        for elements in [
            vec![element(false), element(true)],
            vec![element(true), element(false)],
        ] {
            assert!(matches!(
                validate(&elements),
                Err(FormulaFailure::Expansion(
                    crate::ExpansionFailure::Admission(crate::AdmissionFailure::Profile {
                        feature: ProfileFeature::HeadAggregateAlias,
                        ..
                    })
                ))
            ));
        }
    }

    #[test]
    fn homogeneous_and_empty_groups_satisfy_the_boundary_invariant() {
        for elements in [
            vec![],
            vec![element(false), element(false)],
            vec![element(true), element(true)],
        ] {
            assert!(validate(&elements).is_ok());
        }
    }
}
