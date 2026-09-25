//! Objective policy over shared canonical template components. Construction
//! descriptions disappear after admission; closed queries retain ID-only nodes.

use std::convert::Infallible;

use zetesis_core::atom_interner::{AtomInterner, Failure as InternFailure, Limits as InternLimits};
use zetesis_core::catalog::Error as StorageError;
use zetesis_core::{
    AtomCatalog, FilterRef, PatternRef, TemplateCatalog, TemplateCatalogBuilder, TemplateTerm,
};

use super::{
    AdmissionError, AdmissionLimits, AdmissionResource, AdmittedTemplate, Data, ObjectiveElement,
    ObjectiveTemplate, admit_scope, admit_template, check_bound,
};
use crate::condition::PendingCondition;

pub(super) fn admit(
    templates: Vec<ObjectiveTemplate>,
    limits: AdmissionLimits,
) -> Result<Data, AdmissionError> {
    check_bound(
        AdmissionResource::Templates,
        templates.len(),
        limits.max_templates,
        None,
    )?;
    let retained_conditions = existing_conditions(
        &templates.iter().map(|template| &template.condition),
        None,
        limits,
    )?;
    let mut external = size_of::<Data>() as u128
        + size_of::<Vec<PendingCondition>>() as u128
        + retained_conditions;
    let mut builder =
        TemplateCatalogBuilder::new(limits.max_bytes).map_err(|error| storage(None, error))?;
    builder
        .set_external_bytes(external)
        .map_err(|error| storage(None, error))?;
    let mut admitted = reserve(templates.len(), &mut builder, &mut external)?;
    let mut priorities = reserve(templates.len(), &mut builder, &mut external)?;
    let mut pending = reserve(templates.len(), &mut builder, &mut external)?;
    let pending_bytes =
        size_of::<Vec<PendingCondition>>() as u128 + cells::<PendingCondition>(pending.capacity());
    let conditions = admit_templates(
        &templates,
        &mut builder,
        limits,
        &mut admitted,
        &mut priorities,
    )?;
    let catalog = builder.finish().map_err(|error| storage(None, error))?;
    // The writer shares the entire frozen base, including lookup indexes.
    // Its allowance therefore excludes only the other template metadata.
    let other_catalog = catalog.storage_bytes() - catalog.shared_vocabulary_bytes();
    let owner_limit = remaining(external + other_catalog, limits.max_bytes, None)?;
    let mut owner = AtomInterner::for_template_catalog(&catalog, owner_limit)
        .map_err(|error| storage(None, error))?;
    for (index, template) in templates.into_iter().enumerate() {
        let existing = template.condition.canonical_catalog().is_some();
        let available = remaining(external + other_catalog, limits.max_bytes, Some(index))?;
        let condition = template
            .condition
            .prepare_in(
                &mut owner,
                InternLimits {
                    max_atoms: conditions.max_atoms,
                    max_bytes: available as u128,
                },
                limits.max_condition_node_bytes,
            )
            .map_err(|error| AdmissionError::ConditionStorage {
                template: index,
                error,
            })?;
        // Existing shared node owners were counted once before construction.
        if !existing {
            external += condition.node_storage_bytes();
        }
        check_storage(
            external + other_catalog + owner.storage_bytes(),
            limits.max_bytes,
            Some(index),
        )?;
        pending.push(condition);
    }
    let available = remaining(external + other_catalog, limits.max_bytes, None)?;
    let atoms = owner
        .into_catalog_with(
            InternLimits {
                max_atoms: conditions.max_atoms,
                max_bytes: available as u128,
            },
            || Ok::<_, Infallible>(()),
        )
        .map_err(|error| storage(None, intern_error(error)))?;
    let atom_bytes = atoms.storage().bytes;
    for (index, (row, condition)) in admitted.iter_mut().zip(pending).enumerate() {
        row.condition =
            condition
                .finish(&atoms)
                .map_err(|error| AdmissionError::ConditionStorage {
                    template: index,
                    error,
                })?;
    }
    // Snapshot publication retains base payload, but not its lookup indexes.
    // The exact shared payload subtotal is removed once from the combined owner.
    let tuple_bytes = if conditions.has_ingress {
        atom_bytes - catalog.shared_payload_bytes()
    } else {
        0
    };
    let storage_bytes = external - pending_bytes + catalog.storage_bytes() + tuple_bytes;
    check_storage(storage_bytes, limits.max_bytes, None)?;
    Ok(Data {
        catalog: Some(catalog),
        templates: admitted,
        priorities,
        present: true,
        storage_bytes,
    })
}

/// Condition ingress bounds discovered while admitting the objective rows.
struct ConditionPopulation {
    max_atoms: usize,
    has_ingress: bool,
}

/// Admit shared template components before publishing the closed vocabulary.
/// Priority slots refer to the descending distinct priority sequence.
fn admit_templates(
    templates: &[ObjectiveTemplate],
    builder: &mut TemplateCatalogBuilder,
    limits: AdmissionLimits,
    admitted: &mut Vec<AdmittedTemplate>,
    priorities: &mut Vec<i32>,
) -> Result<ConditionPopulation, AdmissionError> {
    let mut max_atoms = 0usize;
    let mut has_ingress = false;
    for (index, template) in templates.iter().enumerate() {
        // Safety scratch has its own short lifetime; the registered external
        // envelope already includes every concurrently retained input catalog.
        let available = remaining(builder.storage_bytes(), limits.max_bytes, Some(index))?;
        let variables = admit_template(
            template,
            AdmissionLimits {
                max_bytes: available,
                ..limits
            },
            index,
        )?;
        let row = builder
            .append(
                std::iter::once(TemplateTerm::from(&template.weight))
                    .chain(template.tuple.iter().map(TemplateTerm::from)),
                template.positive.iter().map(PatternRef::from),
                template.filters.iter().map(FilterRef::from),
            )
            .map_err(|error| storage(Some(index), error))?;
        for atom in template.condition.ingress_atoms() {
            builder
                .include_atom_vocabulary(atom)
                .map_err(|error| storage(Some(index), error))?;
        }
        if template.condition.canonical_catalog().is_none() {
            has_ingress = true;
            max_atoms = max_atoms
                .checked_add(template.condition.nodes().len())
                .ok_or(AdmissionError::Overflow {
                    template: Some(index),
                })?;
        }
        priorities.push(template.priority);
        admitted.push(AdmittedTemplate {
            row,
            polarity: template.polarity,
            condition: crate::Condition::default(),
            variables,
            slot: 0,
        });
    }
    priorities.sort_unstable_by(|left, right| right.cmp(left));
    priorities.dedup();
    for (index, (row, original)) in admitted.iter_mut().zip(templates).enumerate() {
        row.slot = priorities
            .binary_search_by(|value| original.priority.cmp(value))
            .map_err(|_| AdmissionError::MissingPriority { template: index })?;
    }
    Ok(ConditionPopulation {
        max_atoms,
        has_ingress,
    })
}

/// No payload import is performed by this entry point. The semantic checks are
/// shared with ingress admission; only storage construction differs.
pub(super) fn from_catalog(
    catalog: TemplateCatalog,
    rows: Vec<ObjectiveElement>,
    limits: AdmissionLimits,
) -> Result<Data, AdmissionError> {
    check_bound(
        AdmissionResource::Templates,
        rows.len(),
        limits.max_templates,
        None,
    )?;
    for (index, element) in rows.iter().enumerate() {
        if element.condition.canonical_catalog().is_none() && !element.condition.nodes().is_empty()
        {
            return Err(AdmissionError::ConditionNotCanonical { template: index });
        }
    }
    let conditions = existing_conditions(
        &rows.iter().map(|element| &element.condition),
        catalog.source_catalog(),
        limits,
    )?;
    let mut retained = size_of::<Data>() as u128 + catalog.storage_bytes() + conditions;
    check_storage(retained, limits.max_bytes, None)?;
    let mut admitted = reserve_retained::<AdmittedTemplate>(rows.len(), &mut retained, limits)?;
    let mut priorities = reserve_retained::<i32>(rows.len(), &mut retained, limits)?;
    for (index, element) in rows.iter().enumerate() {
        let row = catalog.at(element.row).ok_or(AdmissionError::CatalogRow {
            template: index,
            row: element.row,
        })?;
        let fields = row.terms();
        if fields.is_empty() {
            return Err(AdmissionError::MissingWeight {
                template: index,
                row: element.row,
            });
        }
        element.condition.validate(limits, index)?;
        let available = remaining(retained, limits.max_bytes, Some(index))?;
        let variables = admit_scope(
            fields.len() - 1,
            fields
                .iter()
                .chain(row.patterns().iter().flat_map(PatternRef::terms))
                .chain(row.filters().iter().flat_map(|filter| {
                    let (left, right) = filter.terms();
                    [left, right]
                })),
            row.patterns().iter(),
            row.filters().len(),
            AdmissionLimits {
                max_bytes: available,
                ..limits
            },
            index,
        )?;
        priorities.push(element.priority);
        admitted.push(AdmittedTemplate {
            row: element.row,
            polarity: element.polarity,
            condition: crate::Condition::default(),
            variables,
            slot: 0,
        });
    }
    priorities.sort_unstable_by(|left, right| right.cmp(left));
    priorities.dedup();
    for (index, (template, element)) in admitted.iter_mut().zip(rows).enumerate() {
        template.slot = priorities
            .binary_search_by(|value| element.priority.cmp(value))
            .map_err(|_| AdmissionError::MissingPriority { template: index })?;
        template.condition = element.condition;
    }
    Ok(Data {
        catalog: Some(catalog),
        templates: admitted,
        priorities,
        present: true,
        storage_bytes: retained,
    })
}

fn reserve_retained<T>(
    count: usize,
    retained: &mut u128,
    limits: AdmissionLimits,
) -> Result<Vec<T>, AdmissionError> {
    check_storage(*retained + cells::<T>(count), limits.max_bytes, None)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| AdmissionError::Allocation)?;
    *retained += cells::<T>(values.capacity());
    check_storage(*retained, limits.max_bytes, None)?;
    Ok(values)
}

/// Count occurrence owners, exact snapshot allocations and node owners once.
/// Different immutable prefixes may share segments, but are conservatively
/// counted independently because partial overlap is not an equality witness.
fn existing_conditions<'a>(
    conditions: &(impl Iterator<Item = &'a crate::Condition> + Clone),
    source: Option<&AtomCatalog>,
    limits: AdmissionLimits,
) -> Result<u128, AdmissionError> {
    let mut bytes = 0u128;
    for (index, condition) in conditions.clone().enumerate() {
        if let Some(catalog) = condition.canonical_catalog() {
            if condition.node_storage_bytes() > limits.max_condition_node_bytes as u128 {
                return Err(AdmissionError::ConditionStorage {
                    template: index,
                    error: crate::ConditionError::Storage(StorageError::Storage {
                        required: condition.node_storage_bytes(),
                        limit: limits.max_condition_node_bytes,
                    }),
                });
            }
            if !conditions
                .clone()
                .take(index)
                .any(|other| condition.same_node_owner(other))
            {
                bytes += condition.node_storage_bytes();
            }
            let previous = || {
                conditions
                    .clone()
                    .take(index)
                    .filter_map(crate::Condition::canonical_catalog)
            };
            if !source.is_some_and(|other| catalog.same_owner(other))
                && !previous().any(|other| catalog.same_owner(other))
            {
                bytes += catalog.publication_bytes();
                if !source.is_some_and(|other| catalog.shares_snapshot(other))
                    && !previous().any(|other| catalog.shares_snapshot(other))
                {
                    bytes += catalog.shared_snapshot_bytes();
                }
            }
            check_storage(bytes, limits.max_bytes, Some(index))?;
        }
    }
    Ok(bytes)
}

fn reserve<T>(
    count: usize,
    builder: &mut TemplateCatalogBuilder,
    external: &mut u128,
) -> Result<Vec<T>, AdmissionError> {
    builder
        .set_external_bytes(*external + cells::<T>(count))
        .map_err(|error| storage(None, error))?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| AdmissionError::Allocation)?;
    *external += cells::<T>(values.capacity());
    builder
        .set_external_bytes(*external)
        .map_err(|error| storage(None, error))?;
    Ok(values)
}
fn cells<T>(count: usize) -> u128 {
    count as u128 * size_of::<T>() as u128
}
fn remaining(used: u128, limit: usize, template: Option<usize>) -> Result<usize, AdmissionError> {
    check_storage(used, limit, template)?;
    Ok(limit - usize::try_from(used).map_err(|_| AdmissionError::Overflow { template })?)
}
pub(super) fn check_storage(
    required: u128,
    limit: usize,
    template: Option<usize>,
) -> Result<(), AdmissionError> {
    if required > limit as u128 {
        Err(storage(template, StorageError::Storage { required, limit }))
    } else {
        Ok(())
    }
}
fn storage(template: Option<usize>, error: StorageError) -> AdmissionError {
    AdmissionError::Storage { template, error }
}
fn intern_error(error: InternFailure<Infallible>) -> StorageError {
    match error {
        InternFailure::Catalog(error) => error,
        InternFailure::Allocation(_) => StorageError::Allocation,
        InternFailure::Bytes { required, limit } => StorageError::Storage {
            required,
            limit: usize::try_from(limit).unwrap_or(usize::MAX),
        },
        InternFailure::Overflow | InternFailure::Atoms { .. } => StorageError::Overflow,
        InternFailure::Stopped(never) => match never {},
    }
}
