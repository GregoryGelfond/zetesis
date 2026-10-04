//! Complete projection declarations select the original shared source authority.
//!
//! Instances contribute a fixed domain, never logical heads. Source activity
//! rejects statically absent literals; optional conditions are not tested for
//! joint satisfiability and are never reevaluated against a selected answer.

use crate::ProgramSite;
use zetesis_core::AtomCatalog;
use zetesis_core::catalog::PredicateRef;

use super::{Purpose, scoped_body};
use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_ir::{HeadIr, Prepared, RuleIr};
use crate::formula_source_activity::{Activity, Context, SourceEligibility};
use crate::formula_support::{
    CompletedQueries, Computation, Counters, Join, Publication, SourceAtom, SourceSelection,
};
use crate::{FormulaFailure, FormulaLimits, FormulaResource, PreparedProjection};

/// None denotes no explicit directive; an empty selection is an explicit empty
/// domain. Nonempty selections already have semantic atom order at this boundary.
pub(crate) struct PendingProjection {
    atoms: Option<SourceSelection>,
}

impl PendingProjection {
    pub(crate) fn publish(
        self,
        publication: &mut Publication<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<PreparedProjection, FormulaFailure> {
        let Some(selection) = self.atoms else {
            return Ok(PreparedProjection::default());
        };
        // An explicit empty projection retains no unrelated source payload and
        // keeps the existing zero-capable projection byte contract.
        let atoms = if selection.len() == 0 {
            AtomCatalog::default()
        } else {
            publication.atoms(selection, limits, counters, location)?
        };
        if !atoms.atoms().is_empty() {
            let storage = atoms.storage_with(|| counters.work(limits, location))?;
            ceiling(
                FormulaResource::ProjectBytes,
                storage.bytes,
                limits.max_project_bytes as u128,
                location,
            )?;
        }
        Ok(PreparedProjection {
            explicit: true,
            atoms,
        })
    }
}

pub(super) fn prepare(
    prepared: &Prepared,
    completed: &CompletedQueries<'_>,
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<PendingProjection, FormulaFailure> {
    if !prepared.project_selection.is_explicit() {
        return Ok(PendingProjection { atoms: None });
    }
    let mut context = Context {
        computation,
        limits,
        budget,
        counters,
        location,
    };
    let eligibility = SourceEligibility::build_from_conditions(
        prepared,
        completed,
        0,
        prepared.projection.iter().map(|rule| rule.body.as_slice()),
        &mut context,
    )?;
    let mut domain = SourceSelection::new(context.computation, limits, context.counters, location)?;
    for predicate in prepared.project_selection.signatures().iter() {
        signature(predicate, completed, &mut domain, &mut context)?;
    }
    for declaration in &prepared.projection {
        instances(
            declaration,
            completed,
            &eligibility,
            &mut domain,
            &mut context,
        )?;
    }
    let atoms = domain.order(
        context.computation,
        context.limits,
        context.counters,
        context.location,
    )?;
    Ok(PendingProjection { atoms: Some(atoms) })
}

fn instances(
    declaration: &RuleIr,
    completed: &CompletedQueries<'_>,
    eligibility: &SourceEligibility,
    domain: &mut SourceSelection,
    context: &mut Context<'_, '_, '_>,
) -> Result<(), FormulaFailure> {
    context.location = declaration.location;
    let support = completed.support();
    let mut rows = Join::rule(
        declaration,
        support,
        context.computation,
        context.limits,
        context.budget,
        context.counters,
    )?;
    while let Some(row) = rows.next_row(
        context.computation,
        context.limits,
        context.budget,
        context.counters,
        context.location,
    )? {
        let body_binding = declaration.body_binding(&row.values);
        let body = scoped_body::validate_with_purpose(
            &declaration.body,
            &body_binding,
            support,
            context,
            Purpose::Validation,
        )?;
        if !row.passes
            || body.is_false()
            || body.source_activity(eligibility, context)? == Activity::Absent
        {
            continue;
        }
        let HeadIr::Normal(Some(head)) = &declaration.head else {
            return Err(crate::diagnostic::unsupported(
                crate::ProfileFeature::Statement,
                context.location,
            )
            .into());
        };
        let location = context.location;
        let head = context.computation.static_pattern(
            *head,
            context.limits,
            context.counters,
            location,
        )?;
        let binding = row.values.view(
            context.computation.read(),
            context.limits,
            context.counters,
            location,
        )?;
        context
            .counters
            .charge_work(head.terms().len() as u128, context.limits, location)?;
        let key = head
            .key(binding)
            .map_err(|error| FormulaFailure::UnsafeVariable {
                variable: error.variable,
                location,
            })?;
        if !support.contains(&key, context.limits, context.counters, location)? {
            continue;
        }
        let source = context.computation.atom(
            head,
            &row.values,
            context.limits,
            context.counters,
            location,
        )?;
        insert(domain, &source, context)?;
    }
    Ok(())
}

/// Signature rows already borrow the shared source dictionary. Selecting their
/// identities needs no cloned predicate, variable-only pattern or value frame.
fn signature(
    predicate: PredicateRef<'_>,
    completed: &CompletedQueries<'_>,
    domain: &mut SourceSelection,
    context: &mut Context<'_, '_, '_>,
) -> Result<(), FormulaFailure> {
    for row in completed.support().rows(predicate) {
        context.counters.work(context.limits, context.location)?;
        let source = context.computation.atom_ref(
            row.atom(),
            context.limits,
            context.counters,
            context.location,
        )?;
        insert(domain, &source, context)?;
    }
    Ok(())
}

fn insert(
    domain: &mut SourceSelection,
    atom: &SourceAtom,
    context: &mut Context<'_, '_, '_>,
) -> Result<(), FormulaFailure> {
    domain
        .insert(
            atom,
            (
                FormulaResource::ProjectAtoms,
                context.limits.max_project_atoms,
            ),
            context.computation,
            context.limits,
            context.counters,
            context.location,
        )
        .map(|_| ())
}

#[cfg(test)]
mod tests;
