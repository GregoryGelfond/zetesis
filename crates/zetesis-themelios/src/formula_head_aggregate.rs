//! Finite numeric aggregate heads retain separate permission and measure.
//! Complete tuple validation precedes support or final lowering. Count permits
//! either tuple/atom alias direction; measured heads retain their checked
//! bijection profile. Neither bounds nor tuple weights supply bindings or support.
//! Positive conditions enumerate possible eligibility; default-negated gates
//! consume established bindings. Choice lowering retains every eligibility
//! formula. Support-table membership is never interpreted as truth.

use std::collections::BTreeMap;

use themelios_base::span::Location;
use themelios_program::program::{AggregateFunction, HeadAggregate};
use zetesis_core::{Atom, Term, Value};

use crate::diagnostic::unsupported;
use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_ir::{
    ChoiceIr, Compiler, Element, HeadMeasure, LiteralIr, Variables, value_bytes,
};
use crate::formula_support::{Counters, Join, Support};
use crate::{ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource, ProfileFeature};

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
            // Closed weight syntax has no binding dependency, so a false outer
            // guard cannot conceal an unsupported declared weight. Variables
            // are checked by the same selector on complete possible local rows.
            match tuple.first() {
                Some(Term::Constant(value)) => {
                    weight(measure, Some(value), self.location)?;
                }
                None => {
                    weight(measure, None, self.location)?;
                }
                Some(Term::Variable(_)) => {}
            }
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
                tuple: Some(tuple),
                head,
                condition,
                variables: local.count,
            });
        }
        Ok((measure, elements))
    }
}

/// Validate the entire instantiated group before the caller derives any support
/// or publishes a formula. The immutable completed support/binding is replayed
/// afterwards through the ordinary choice path. Both passes charge their work.
/// A present map certifies the stronger tuple/atom bijection used by optional
/// count planning. Nonbijective count groups are valid and return no certificate;
/// an ordinary choice has implicit atom keys and returns a present empty map.
pub(super) fn validate_group(
    group: &ChoiceIr,
    assignment: &[Value],
    support: &Support,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: Location,
) -> Result<Option<BTreeMap<Atom, Vec<Value>>>, FormulaFailure> {
    let ChoiceIr {
        measure, elements, ..
    } = group;
    let keyed = elements
        .first()
        .is_some_and(|element| element.tuple.is_some());
    // An explicit invariant at the common lowering boundary: groups cannot mix
    // ordinary atom-counting elements with tuple-keyed function elements.
    if elements
        .iter()
        .any(|element| element.tuple.is_some() != keyed)
    {
        return Err(unsupported(ProfileFeature::HeadAggregateAlias, location).into());
    }
    if !keyed {
        if *measure != HeadMeasure::Count && !elements.is_empty() {
            return Err(unsupported(ProfileFeature::HeadAggregateAlias, location).into());
        }
        return Ok(Some(BTreeMap::new()));
    }
    let mut tuples = BTreeMap::<Vec<Value>, Atom>::new();
    let mut atoms = BTreeMap::<Atom, Vec<Value>>::new();
    let mut bijective = true;
    for element in elements {
        let terms = element.tuple.as_ref().expect("uniform tuple group checked");
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
                let value = term
                    .resolve(&binding)
                    .expect("safe aggregate tuple assigned");
                budget.charge(
                    ExpansionResource::ScalarBytes,
                    2 * (std::mem::size_of::<Value>() as u128 + value_bytes(value)),
                    location,
                )?;
                tuple.push(value.clone());
            }
            weight(*measure, tuple.first(), location)?;
            let atom_bytes = element.head.predicate().name().len() as u128
                + element
                    .head
                    .terms()
                    .iter()
                    .map(|term| {
                        let value = term
                            .resolve(&binding)
                            .expect("safe aggregate head assigned");
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
                .expect("safe aggregate head assigned");
            if tuples.get(&tuple).is_some_and(|previous| *previous != atom)
                || atoms.get(&atom).is_some_and(|previous| *previous != tuple)
            {
                if *measure != HeadMeasure::Count {
                    return Err(unsupported(ProfileFeature::HeadAggregateAlias, location).into());
                }
                bijective = false;
            }
            if !tuples.contains_key(&tuple) {
                ceiling(
                    FormulaResource::AggregateElements,
                    tuples.len() as u128 + 1,
                    limits.aggregate.max_elements as u128,
                    location,
                )?;
                tuples.insert(tuple.clone(), atom.clone());
            }
            atoms.entry(atom).or_insert(tuple);
        }
    }
    // This optional certificate is consumed only by CountPlan. Ordinary count
    // semantics use the complete tuple activities even without a bijection.
    Ok(bijective.then_some(atoms))
}

/// Numeric contribution is independent of permission to select the head.
/// Count ignores tuple values; other measures require a numeric first term. Zero sum+
/// contributes nothing but never removes an eligible head. Negative sum+ values
/// retain an explicit profile refusal pending a separate semantic contract.
pub(super) fn weight(
    measure: HeadMeasure,
    first: Option<&Value>,
    location: Location,
) -> Result<Option<i32>, FormulaFailure> {
    if measure == HeadMeasure::Count {
        return Ok(Some(1));
    }
    let Some(Value::Number(value)) = first else {
        return Err(unsupported(ProfileFeature::HeadAggregateWeight, location).into());
    };
    if matches!(measure, HeadMeasure::Min | HeadMeasure::Max) {
        crate::formula_assignment::extremum_value(&Value::Number(*value), location)?;
    }
    if measure == HeadMeasure::SumPlus && *value < 0 {
        return Err(unsupported(ProfileFeature::HeadAggregateWeight, location).into());
    }
    Ok((measure != HeadMeasure::SumPlus || *value > 0).then_some(*value))
}

#[cfg(test)]
mod tests {
    use super::{
        Budget, ChoiceIr, Counters, Element, FormulaLimits, HeadMeasure, Support, validate_group,
    };
    use crate::{AdmissionOptions, ExpansionLimits, FormulaFailure, ProfileFeature};
    use themelios_base::source::{Source, SourceId};
    use themelios_base::span::Location;
    use zetesis_core::{AtomPattern, Predicate, Term, Value};

    fn element(keyed: bool) -> Element {
        Element {
            tuple: keyed.then(|| vec![Term::Constant(Value::Number(1))]),
            head: AtomPattern::new(Predicate::new("p", 0).unwrap(), vec![]).unwrap(),
            condition: vec![],
            variables: 0,
        }
    }

    fn validate_measure(
        measure: HeadMeasure,
        elements: Vec<Element>,
    ) -> Result<(), FormulaFailure> {
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
            &ChoiceIr {
                measure,
                guards: vec![],
                elements,
            },
            &[],
            &Support::default(),
            &FormulaLimits::default(),
            &mut budget,
            &mut Counters::default(),
            location,
        )
        .map(|_| ())
    }

    #[test]
    fn mixed_key_metadata_is_rejected() {
        for elements in [
            vec![element(false), element(true)],
            vec![element(true), element(false)],
        ] {
            assert!(matches!(
                validate_measure(HeadMeasure::Count, elements),
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
    fn homogeneous_count_groups_satisfy_the_key_invariant() {
        for elements in [
            vec![],
            vec![element(false), element(false)],
            vec![element(true), element(true)],
        ] {
            assert!(validate_measure(HeadMeasure::Count, elements).is_ok());
        }
    }

    #[test]
    fn measured_groups_require_tuple_keys() {
        for measure in [
            HeadMeasure::Sum,
            HeadMeasure::SumPlus,
            HeadMeasure::Min,
            HeadMeasure::Max,
        ] {
            let error = validate_measure(measure, vec![element(false)]).unwrap_err();
            assert!(matches!(
                error,
                FormulaFailure::Expansion(crate::ExpansionFailure::Admission(
                    crate::AdmissionFailure::Profile {
                        feature: ProfileFeature::HeadAggregateAlias,
                        ..
                    }
                ))
            ));
        }
    }

    #[test]
    fn empty_measured_groups_satisfy_the_key_invariant() {
        for measure in [
            HeadMeasure::Sum,
            HeadMeasure::SumPlus,
            HeadMeasure::Min,
            HeadMeasure::Max,
        ] {
            validate_measure(measure, vec![]).unwrap();
        }
    }
}
