//! Signed identity and coherence at the complete source-admission boundary.
//!
//! Constraints join equal tuples of opposite signs. They never supply support,
//! enumerate a domain product, or invent a predicate absent from the source.

use std::collections::BTreeMap;

use themelios_base::span::Location;
use zetesis_core::{
    AdmissionError, AdmissionLimits, AdmissionResource, AtomPattern, Predicate, Sign, Template,
    Term,
};

use crate::{AdmissionFailure, ExpansionResource};

fn patterns(template: &Template) -> impl Iterator<Item = &AtomPattern> {
    template
        .head()
        .into_iter()
        .chain(template.positive())
        .chain(template.gate_true())
        .chain(template.gate_false())
}

pub(crate) fn core_sign(sign: themelios_program::symbol::Sign) -> Sign {
    match sign {
        themelios_program::symbol::Sign::Positive => Sign::Positive,
        themelios_program::symbol::Sign::Negative => Sign::Negative,
    }
}

pub(crate) fn source_sign(sign: Sign) -> themelios_program::symbol::Sign {
    match sign {
        Sign::Positive => themelios_program::symbol::Sign::Positive,
        Sign::Negative => themelios_program::symbol::Sign::Negative,
    }
}

pub(crate) fn append<E: From<AdmissionFailure>>(
    templates: &mut Vec<Template>,
    origins: &mut Vec<Vec<Location>>,
    limits: AdmissionLimits,
    fallback: Location,
    mut charge: impl FnMut(ExpansionResource, u128, Location) -> Result<(), E>,
) -> Result<(), E> {
    // Preserve existing unsigned budgets and avoid a registry allocation on
    // the common unsigned route. Traversal is bounded by the source templates.
    if !templates
        .iter()
        .flat_map(patterns)
        .any(|pattern| pattern.predicate().sign() == Sign::Negative)
    {
        return Ok(());
    }
    let mut signatures = BTreeMap::new();
    for (index, template) in templates.iter().enumerate() {
        let location = origins[index].first().copied().unwrap_or(fallback);
        for pattern in patterns(template) {
            charge(ExpansionResource::TermWork, 1, location)?;
            signatures.entry(pattern.predicate()).or_insert(index);
        }
    }
    let mut generated = Vec::new();
    let mut evidence = Vec::new();
    for (&negative, &index) in &signatures {
        if negative.sign() != Sign::Negative {
            continue;
        }
        let location = origins[index].first().copied().unwrap_or(fallback);
        charge(
            ExpansionResource::ScalarBytes,
            negative.name().len() as u128,
            location,
        )?;
        let positive = Predicate::new(negative.name(), negative.arity())
            .expect("the original predicate has a nonempty name");
        let Some(&opposite) = signatures.get(&positive) else {
            continue;
        };
        check_core_limits(
            index,
            negative.arity(),
            templates.len() + generated.len() + 1,
            limits,
            location,
        )?;
        charge(ExpansionResource::Templates, 1, location)?;
        charge(
            ExpansionResource::TermWork,
            2 * negative.arity() as u128 + 2,
            location,
        )?;
        charge(
            ExpansionResource::ScalarBytes,
            negative.name().len() as u128,
            location,
        )?;
        charge(
            ExpansionResource::Origins,
            origins[index].len() as u128 + origins[opposite].len() as u128,
            location,
        )?;
        let pattern = |predicate: Predicate| {
            AtomPattern::new(
                predicate,
                (0..negative.arity()).map(Term::Variable).collect(),
            )
            .expect("one shared dense slot per argument")
        };
        generated.push(Template::new(
            None,
            vec![pattern(positive), pattern(negative.clone())],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ));
        let mut locations = origins[index].clone();
        locations.extend_from_slice(&origins[opposite]);
        locations.sort_unstable();
        locations.dedup();
        evidence.push(locations);
    }
    templates.extend(generated);
    origins.extend(evidence);
    Ok(())
}

fn check_core_limits(
    index: usize,
    arity: usize,
    count: usize,
    limits: AdmissionLimits,
    location: Location,
) -> Result<(), AdmissionFailure> {
    for (resource, actual, limit) in [
        (AdmissionResource::Templates, count, limits.max_templates),
        (
            AdmissionResource::PredicateArity,
            arity,
            limits.max_predicate_arity,
        ),
        (
            AdmissionResource::Variables,
            arity,
            limits.max_variables_per_template,
        ),
        (AdmissionResource::PositiveBody, 2, limits.max_positive_body),
    ] {
        if actual > limit {
            return Err(AdmissionFailure::Core {
                error: AdmissionError::LimitExceeded {
                    resource,
                    limit,
                    actual,
                    template: Some(index),
                },
                location,
            });
        }
    }
    Ok(())
}
