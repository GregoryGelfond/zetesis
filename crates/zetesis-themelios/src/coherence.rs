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

use crate::compile::{Compiled, CompiledRule, Item, Occurrence};
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
    compiled: &mut Compiled<Location>,
    limits: AdmissionLimits,
    fallback: impl Into<ProgramSite>,
    mut charge: impl FnMut(ExpansionResource, u128, ProgramSite) -> Result<(), E>,
) -> Result<(), E> {
    let fallback = fallback.into();
    append_with(
        compiled,
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
    compiled: &mut Compiled<P>,
    limits: AdmissionLimits,
    mut charge: impl FnMut(ExpansionResource, u128, &[P]) -> Result<(), E>,
    failure: impl Fn(AdmissionError, &[P]) -> E,
) -> Result<(), E> {
    // A block contributes one signature inspection, not one per fact row.
    if !compiled
        .templates
        .iter()
        .flat_map(patterns)
        .any(|pattern| pattern.predicate().sign() == Sign::Negative)
        && !compiled
            .facts
            .predicates()
            .any(|(predicate, _)| predicate.sign() == Sign::Negative)
    {
        return Ok(());
    }
    let signatures = signatures(compiled, &mut charge)?;
    let mut generated = Vec::new();
    for (&negative, &occurrence) in &signatures {
        if negative.sign() != Sign::Negative {
            continue;
        }
        let origin = compiled.origins(occurrence);
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
            occurrence,
            negative.arity(),
            compiled.len() + generated.len() + 1,
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
        let opposite = compiled.origins(opposite);
        charge(
            ExpansionResource::Origins,
            origin.len() as u128 + opposite.len() as u128,
            origin,
        )?;
        let pattern = |predicate: Predicate| {
            AtomPattern::new(
                predicate,
                (0..negative.arity()).map(Term::Variable).collect(),
            )
            .expect("one shared dense slot per argument")
        };
        let template = Template::new(
            None,
            vec![pattern(positive), pattern(negative.clone())],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let mut locations = origin.to_vec();
        locations.extend_from_slice(opposite);
        locations.sort_unstable();
        locations.dedup();
        generated.push((template, locations));
    }
    for (template, origins) in generated {
        compiled
            .push(CompiledRule::Template(template), origins.clone())
            .map_err(|error| failure(error, &origins))?;
    }
    Ok(())
}

/// First signature evidence follows source order even when the same signed
/// predicate occurs in both streams. Fact runs can cross adjacent families;
/// their first occurrence retains the first family's evidence.
fn signatures<'a, P: Clone, E>(
    compiled: &'a Compiled<P>,
    charge: &mut impl FnMut(ExpansionResource, u128, &[P]) -> Result<(), E>,
) -> Result<BTreeMap<&'a Predicate, Occurrence>, E> {
    let mut signatures = BTreeMap::new();
    let mut facts = compiled.facts.predicates().peekable();
    for item in &compiled.order {
        match *item {
            Item::Template(index) => {
                for pattern in patterns(&compiled.templates[index]) {
                    let occurrence = Occurrence::Template(index);
                    charge(ExpansionResource::TermWork, 1, compiled.origins(occurrence))?;
                    signatures.entry(pattern.predicate()).or_insert(occurrence);
                }
            }
            Item::Facts { end } => {
                while let Some(&(predicate, index)) = facts.peek() {
                    if index >= end {
                        break;
                    }
                    facts.next();
                    let occurrence = Occurrence::Fact(index);
                    charge(ExpansionResource::TermWork, 1, compiled.origins(occurrence))?;
                    signatures.entry(predicate).or_insert(occurrence);
                }
            }
        }
    }
    Ok(signatures)
}

fn check_core_limits(
    occurrence: Occurrence,
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
            return Err(match occurrence {
                Occurrence::Template(index) => AdmissionError::LimitExceeded {
                    resource,
                    limit,
                    actual,
                    template: Some(index),
                },
                Occurrence::Fact(fact) => AdmissionError::FactLimitExceeded {
                    resource,
                    limit,
                    actual,
                    fact,
                },
            });
        }
    }
    Ok(())
}
