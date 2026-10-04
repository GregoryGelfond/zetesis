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

use crate::{AdmissionFailure, ExpansionResource, ProgramSite};

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
    fallback: impl Into<ProgramSite>,
    mut charge: impl FnMut(ExpansionResource, u128, ProgramSite) -> Result<(), E>,
) -> Result<(), E> {
    let fallback = fallback.into();
    append_with(
        templates,
        origins,
        limits,
        |resource, count, evidence| {
            charge(
                resource,
                count,
                evidence
                    .first()
                    .copied()
                    .map_or(fallback, ProgramSite::from),
            )
        },
        |error, evidence| {
            AdmissionFailure::Core {
                error,
                location: evidence
                    .first()
                    .copied()
                    .map_or(fallback, ProgramSite::from),
            }
            .into()
        },
    )
}

/// The coherence operation is independent of whether evidence is a source span
/// or a borrowed logical statement. Each generated constraint keeps both origins.
pub(crate) fn append_with<P: Clone + Ord, E>(
    templates: &mut Vec<Template>,
    origins: &mut Vec<Vec<P>>,
    limits: AdmissionLimits,
    mut charge: impl FnMut(ExpansionResource, u128, &[P]) -> Result<(), E>,
    failure: impl Fn(AdmissionError, &[P]) -> E,
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
        let origin = &origins[index];
        for pattern in patterns(template) {
            charge(ExpansionResource::TermWork, 1, origin)?;
            signatures.entry(pattern.predicate()).or_insert(index);
        }
    }
    let mut generated = Vec::new();
    let mut evidence = Vec::new();
    for (&negative, &index) in &signatures {
        if negative.sign() != Sign::Negative {
            continue;
        }
        let origin = &origins[index];
        charge(
            ExpansionResource::ScalarBytes,
            negative.name().len() as u128,
            origin,
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
        )
        .map_err(|error| failure(error, origin))?;
        charge(ExpansionResource::Templates, 1, origin)?;
        charge(
            ExpansionResource::TermWork,
            2 * negative.arity() as u128 + 2,
            origin,
        )?;
        charge(
            ExpansionResource::ScalarBytes,
            negative.name().len() as u128,
            origin,
        )?;
        charge(
            ExpansionResource::Origins,
            origins[index].len() as u128 + origins[opposite].len() as u128,
            origin,
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
) -> Result<(), AdmissionError> {
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
            return Err(AdmissionError::LimitExceeded {
                resource,
                limit,
                actual,
                template: Some(index),
            });
        }
    }
    Ok(())
}
