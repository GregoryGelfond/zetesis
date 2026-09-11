//! Finite aggregate heads retain separate permission and measure.
//! Complete tuple validation precedes support or final lowering. Every measure
//! permits either tuple/atom alias direction. Neither bounds nor tuple weights
//! supply bindings or support.
//! Positive conditions enumerate possible eligibility; default-negated gates
//! consume established bindings. Choice lowering retains every eligibility
//! formula. Support-table membership is never interpreted as truth.

use std::collections::BTreeMap;

use themelios_base::span::Location;
use themelios_program::program::{AggregateFunction, HasGuards, HeadAggregate};
use zetesis_core::{Atom, Term, Value};

use crate::diagnostic::unsupported;
use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_ir::{
    ChoiceIr, Compiler, Element, HeadElementKey, HeadLiteral, HeadMeasure, HeadOperand, LiteralIr,
    Variables, value_bytes,
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
            // A present extremum value is validated even in a statically false rule.
            // Without bounds only the head choices remain, but source terms and
            // binding instructions are still compiled and validated below.
            if aggregate.left_guard().is_some() || aggregate.right_guard().is_some() {
                match tuple.first() {
                    Some(Term::Constant(value)) => {
                        contribution(measure, Some(value), self.location)?;
                    }
                    None => {
                        contribution(measure, None, self.location)?;
                    }
                    Some(Term::Variable(_)) => {}
                }
            }
            let head = self.head_literal(element.literal(), &mut local, &mut condition)?;
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
                key: HeadElementKey::Tuple(tuple),
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
/// count planning. Nonbijective function groups return no certificate; an
/// ordinary atom choice has implicit atom keys and returns a present empty map.
/// Signed or Boolean elements never certify the stronger unsigned atom-only contract.
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
        .is_some_and(|element| element.key.tuple().is_some());
    // An explicit invariant at the common lowering boundary: groups cannot mix
    // ordinary atom-counting elements with tuple-keyed function elements.
    if elements
        .iter()
        .any(|element| element.key.tuple().is_some() != keyed)
    {
        return Err(unsupported(ProfileFeature::HeadAggregateAlias, location).into());
    }
    if !keyed {
        if *measure != HeadMeasure::Count && !elements.is_empty() {
            return Err(unsupported(ProfileFeature::HeadAggregateAlias, location).into());
        }
        return Ok(elements
            .iter()
            .all(|element| element.head.positive_atom().is_some())
            .then(BTreeMap::new));
    }
    let mut tuples = BTreeMap::<Vec<Value>, HeadLiteral<Atom>>::new();
    let mut atoms = BTreeMap::<Atom, Vec<Value>>::new();
    let mut bijective = elements
        .iter()
        .all(|element| element.head.positive_atom().is_some());
    for element in elements {
        let terms = element.key.tuple().expect("uniform tuple group checked");
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
            if !group.guards.is_empty() {
                contribution(*measure, tuple.first(), location)?;
            }
            let head = head_identity(&element.head, &binding, budget, location)?;
            if tuples.get(&tuple).is_some_and(|previous| *previous != head)
                || head
                    .atom()
                    .and_then(|atom| atoms.get(atom))
                    .is_some_and(|previous| *previous != tuple)
            {
                bijective = false;
            }
            if !tuples.contains_key(&tuple) {
                ceiling(
                    FormulaResource::AggregateElements,
                    tuples.len() as u128 + 1,
                    limits.aggregate.max_elements as u128,
                    location,
                )?;
                tuples.insert(tuple.clone(), head.clone());
            }
            if let HeadOperand::Atom(atom) = head.operand {
                atoms.entry(atom).or_insert(tuple);
            }
        }
    }
    // This optional certificate is consumed only by CountPlan. All measures'
    // semantics use the complete tuple activities even without a bijection:
    // each key contributes once when any eligible occurrence selects its atom.
    Ok(bijective.then_some(atoms))
}

/// Resolve the semantic operand while charging copied atom payload. Constants
/// have no atom identity, and their tuple still passes the complete validation.
fn head_identity(
    head: &HeadLiteral,
    binding: &[Value],
    budget: &mut Budget,
    location: Location,
) -> Result<HeadLiteral<Atom>, FormulaFailure> {
    let atom = match &head.operand {
        HeadOperand::Atom(atom) => atom,
        HeadOperand::Boolean(value) => {
            return Ok(HeadLiteral {
                negation: head.negation,
                operand: HeadOperand::Boolean(*value),
            });
        }
    };
    let atom_bytes = atom.predicate().name().len() as u128
        + atom
            .terms()
            .iter()
            .map(|term| {
                let value = term.resolve(binding).expect("safe aggregate head assigned");
                std::mem::size_of::<Value>() as u128 + value_bytes(value)
            })
            .sum::<u128>();
    budget.charge(
        ExpansionResource::ScalarBytes,
        atom_bytes.saturating_mul(2),
        location,
    )?;
    Ok(HeadLiteral {
        negation: head.negation,
        operand: HeadOperand::Atom(
            atom.instantiate(binding)
                .expect("safe aggregate head assigned"),
        ),
    })
}

/// A selected tuple contributes either an integer or a complete logical value.
/// Borrowing preserves allocation-free validation; final lowering charges the
/// retained extremum value before copying its scalar or structural payload.
pub(super) enum Contribution<'a> {
    Numeric(i32),
    Extremum(&'a Value),
}

/// Contribution is independent of permission to select the head. Count ignores
/// tuple values; missing/nonnumeric sum values have weight zero. Numeric
/// nonpositive sum+ weights also contribute nothing. All retain permission.
/// Missing extremum values are neutral; complete values retain their logical
/// order and admitted domain. An unbounded head never calls this operation
/// because it has no measure bound.
pub(super) fn contribution(
    measure: HeadMeasure,
    first: Option<&Value>,
    location: Location,
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
    let Some(Value::Number(value)) = first else {
        return Ok(None);
    };
    Ok((measure != HeadMeasure::SumPlus || *value > 0).then_some(Contribution::Numeric(*value)))
}

#[cfg(test)]
mod tests {
    use super::{
        Budget, ChoiceIr, Counters, Element, FormulaLimits, HeadElementKey, HeadLiteral,
        HeadMeasure, HeadOperand, Support, validate_group,
    };
    use crate::{AdmissionOptions, ExpansionLimits, FormulaFailure, ProfileFeature};
    use themelios_base::source::{Source, SourceId};
    use themelios_base::span::Location;
    use themelios_program::program::DefaultNegation;
    use zetesis_core::{AtomPattern, Predicate, Term, Value};

    fn element(keyed: bool) -> Element {
        Element {
            key: if keyed {
                HeadElementKey::Tuple(vec![Term::Constant(Value::Number(1))])
            } else {
                HeadElementKey::Atom
            },
            head: HeadLiteral {
                negation: DefaultNegation::None,
                operand: HeadOperand::Atom(
                    AtomPattern::new(Predicate::new("p", 0).unwrap(), vec![]).unwrap(),
                ),
            },
            condition: vec![],
            variables: 0,
        }
    }

    fn validate_measure(
        measure: HeadMeasure,
        elements: Vec<Element>,
    ) -> Result<bool, FormulaFailure> {
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
        .map(|certificate| certificate.is_some())
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

    #[test]
    fn numeric_aliases_never_certify_a_bijection() {
        for measure in [
            HeadMeasure::Count,
            HeadMeasure::Sum,
            HeadMeasure::SumPlus,
            HeadMeasure::Min,
            HeadMeasure::Max,
        ] {
            let mut same_tuple = element(true);
            same_tuple.head.operand = HeadOperand::Atom(
                AtomPattern::new(Predicate::new("q", 0).unwrap(), vec![]).unwrap(),
            );
            assert!(!validate_measure(measure, vec![element(true), same_tuple]).unwrap());
            let mut same_atom = element(true);
            same_atom.key = HeadElementKey::Tuple(vec![Term::Constant(Value::Number(2))]);
            assert!(!validate_measure(measure, vec![element(true), same_atom]).unwrap());
        }
    }

    #[test]
    fn boolean_elements_never_certify_atom_planning() {
        for value in [false, true] {
            let mut ordinary = element(false);
            ordinary.head.operand = HeadOperand::Boolean(value);
            assert!(!validate_measure(HeadMeasure::Count, vec![ordinary]).unwrap());
            for measure in [
                HeadMeasure::Count,
                HeadMeasure::Sum,
                HeadMeasure::SumPlus,
                HeadMeasure::Min,
                HeadMeasure::Max,
            ] {
                let mut keyed = element(true);
                keyed.head.operand = HeadOperand::Boolean(value);
                assert!(!validate_measure(measure, vec![keyed]).unwrap());
            }
        }
    }
    #[test]
    fn signed_elements_never_certify_atom_planning() {
        for negation in [DefaultNegation::Not, DefaultNegation::NotNot] {
            for keyed in [false, true] {
                let mut signed = element(keyed);
                signed.head.negation = negation;
                assert!(!validate_measure(HeadMeasure::Count, vec![signed]).unwrap());
                assert!(validate_measure(HeadMeasure::Count, vec![element(keyed)]).unwrap());
            }
        }
    }
}
