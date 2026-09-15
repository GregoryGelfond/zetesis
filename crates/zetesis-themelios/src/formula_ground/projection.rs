//! Complete projection declarations against the original possible-source owner.
//!
//! Instances contribute a fixed domain, never logical heads. Source activity
//! rejects statically absent literals; optional conditions are not tested for
//! joint satisfiability and are never reevaluated against a selected answer.

use std::mem::size_of;

use themelios_base::span::Location;
use zetesis_core::atom_interner::{AtomInterner, Limits as InternerLimits};
use zetesis_core::{Atom, AtomKey, AtomPattern, Predicate, Term, Value};

use super::{Purpose, atoms, scoped_body};
use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_ir::{HeadIr, Prepared, RuleIr};
use crate::formula_source_activity::{Activity, Context, SourceEligibility};
use crate::formula_support::{self, CompletedQueries, Counters, Join};
use crate::{
    ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource, PreparedProjection,
};

pub(super) fn prepare(
    prepared: &Prepared,
    completed: &CompletedQueries<'_>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: Location,
) -> Result<PreparedProjection, FormulaFailure> {
    if !prepared.project_selection.is_explicit() {
        return Ok(PreparedProjection::default());
    }
    let mut context = Context {
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
    let mut domain = Domain {
        atoms: AtomInterner::new(),
        payload: 0,
    };
    for predicate in prepared.project_selection.signatures() {
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
    Ok(PreparedProjection {
        explicit: true,
        atoms: domain.finish(&mut context)?,
    })
}

fn instances(
    declaration: &RuleIr,
    completed: &CompletedQueries<'_>,
    eligibility: &SourceEligibility,
    domain: &mut Domain,
    context: &mut Context<'_>,
) -> Result<(), FormulaFailure> {
    context.location = declaration.location;
    let support = completed.support();
    let mut rows = Join::rule(declaration, support, context.budget)?;
    while let Some(row) = rows.next_row(
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
        let key = head
            .key(row.values.slots())
            .map_err(|error| FormulaFailure::UnsafeVariable {
                variable: error.variable,
                location,
            })?;
        if !support.contains(&key, context.limits, context.counters, location)? {
            continue;
        }
        domain.insert(key, context)?;
    }
    Ok(())
}

/// Signature rows borrow the existing dictionary values. One reusable frame
/// and one variable-only pattern supply checked keys without copying a tuple.
fn signature(
    predicate: &Predicate,
    completed: &CompletedQueries<'_>,
    domain: &mut Domain,
    context: &mut Context<'_>,
) -> Result<(), FormulaFailure> {
    let count = predicate.arity();
    context.budget.charge(
        ExpansionResource::ScalarBytes,
        predicate.name().len() as u128
            + (count as u128)
                .saturating_mul((size_of::<Term>() + size_of::<Option<&Value>>()) as u128),
        context.location,
    )?;
    let mut terms = Vec::new();
    terms
        .try_reserve_exact(count)
        .map_err(|error| FormulaFailure::MetadataAllocation {
            error,
            location: context.location,
        })?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|error| FormulaFailure::MetadataAllocation {
            error,
            location: context.location,
        })?;
    for column in 0..count {
        context.counters.work(context.limits, context.location)?;
        terms.push(Term::Variable(column));
    }
    let pattern = AtomPattern::new(predicate.clone(), terms).expect("unchanged selector arity");
    for row in completed.support().rows(predicate) {
        context.counters.work(context.limits, context.location)?;
        values.clear();
        for value in formula_support::row_values(row) {
            context.counters.work(context.limits, context.location)?;
            values.push(Some(value));
        }
        // Key validation visits each positional variable before the borrowed
        // frame can enter the interner's separately metered identity lookup.
        context
            .counters
            .charge_work(count as u128, context.limits, context.location)?;
        let key = pattern
            .key(values.as_slice())
            .expect("complete selector row");
        domain.insert(key, context)?;
    }
    Ok(())
}

/// One unique atom owner during construction. The population-derived interner
/// envelope bounds its temporary index, paths, vector overlap and ordered IDs.
/// `ProjectBytes` separately admits retained atom payload and the final vector;
/// allocator metadata and support remain outside that retained-domain bound.
struct Domain {
    atoms: AtomInterner,
    payload: u128,
}

impl Domain {
    fn insert(
        &mut self,
        key: AtomKey<'_>,
        context: &mut Context<'_>,
    ) -> Result<(), FormulaFailure> {
        let limits = InternerLimits::for_atoms(context.limits.max_project_atoms);
        let bound = (
            FormulaResource::ProjectAtoms,
            context.limits.max_project_atoms,
        );
        let count = self.atoms.len() as u128 + 1;
        let entry = self
            .atoms
            .entry_key_with(key, limits, || {
                context.counters.work(context.limits, context.location)
            })
            .map_err(|error| atoms::failure(error, bound, context.location))?;
        if entry.position().is_some() {
            return Ok(());
        }
        ceiling(
            FormulaResource::ProjectAtoms,
            count,
            bound.1 as u128,
            context.location,
        )?;
        let estimated = key_payload(key, context)?;
        domain_bytes(self.payload.saturating_add(estimated), count, context)?;
        // Only the vacant entry copies values. This cumulative conservative
        // copy charge includes the argument cells and referenced nested buffers.
        context
            .budget
            .charge(ExpansionResource::ScalarBytes, estimated, context.location)?;
        let id = entry
            .insert_with(limits, || {
                context.counters.work(context.limits, context.location)
            })
            .map_err(|error| atoms::failure(error, bound, context.location))?;
        let atom = self.atoms.get(id).expect("published projection identity");
        for value in atom.values() {
            account_value(value, context)?;
        }
        self.payload = self
            .payload
            .saturating_add(atom.checked_payload_capacity_bytes().unwrap_or(u128::MAX));
        domain_bytes(self.payload, count, context)
    }

    /// Move the sole payload owner into canonical order. The interner supplies
    /// a complete rank-to-insertion-ID permutation after one prefix commit.
    /// Each cycle swaps payload owners without cloning values; completed cycle
    /// entries become identities. At most n swaps and n terminal writes occur.
    fn finish(mut self, context: &mut Context<'_>) -> Result<Vec<Atom>, FormulaFailure> {
        let limits = InternerLimits::for_atoms(context.limits.max_project_atoms);
        let bound = (
            FormulaResource::ProjectAtoms,
            context.limits.max_project_atoms,
        );
        self.atoms
            .commit_with(limits, || {
                context.counters.work(context.limits, context.location)
            })
            .map_err(|error| atoms::failure(error, bound, context.location))?;
        let mut order = self
            .atoms
            .ordered_ids_with(limits, || {
                context.counters.work(context.limits, context.location)
            })
            .map_err(|error| atoms::failure(error, bound, context.location))?;
        let mut atoms = self
            .atoms
            .into_atoms_with(limits, || {
                context.counters.work(context.limits, context.location)
            })
            .map_err(|error| atoms::failure(error, bound, context.location))?;
        domain_bytes(self.payload, atoms.capacity() as u128, context)?;
        for start in 0..order.len() {
            let mut position = start;
            loop {
                context.counters.work(context.limits, context.location)?;
                let next = order[position];
                if next == start {
                    context.counters.work(context.limits, context.location)?;
                    order[position] = position;
                    break;
                }
                context
                    .counters
                    .charge_work(2, context.limits, context.location)?;
                atoms.swap(position, next);
                order[position] = position;
                position = next;
            }
        }
        Ok(atoms)
    }
}

fn key_payload(key: AtomKey<'_>, context: &mut Context<'_>) -> Result<u128, FormulaFailure> {
    let mut bytes = key.predicate().payload_capacity_bytes() as u128
        + key.predicate().arity() as u128 * size_of::<Value>() as u128;
    for column in 0..key.predicate().arity() {
        let value = key.value(column).expect("checked projection key");
        account_value(value, context)?;
        bytes = bytes.saturating_add(value.checked_payload_capacity_bytes().unwrap_or(u128::MAX));
    }
    Ok(bytes)
}

/// Capacity inspection visits structural descriptors; text capacities are read
/// in constant time. Core separately charges exact identity/copy operations.
fn account_value(value: &Value, context: &mut Context<'_>) -> Result<(), FormulaFailure> {
    let nodes = match value {
        Value::Structured(value) => value.nodes().len() as u128,
        _ => 0,
    };
    context
        .counters
        .charge_work(nodes + 1, context.limits, context.location)
}

fn domain_bytes(
    payload: u128,
    capacity: u128,
    context: &Context<'_>,
) -> Result<(), FormulaFailure> {
    let bytes = payload.saturating_add(capacity.saturating_mul(size_of::<Atom>() as u128));
    ceiling(
        FormulaResource::ProjectBytes,
        bytes,
        context.limits.max_project_bytes as u128,
        context.location,
    )
}
