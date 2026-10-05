//! Consume source certification and the actual retained base grounding owners.

use super::{
    Admitted, Extension,
    partition::{self, Definitions, Partition},
};
use crate::formula::{Compiled, Preparation, ceiling};
use crate::formula_support::{Counters, GroundingWork, components};
use crate::{FormulaFailure, FormulaResource, GroundingObserver};

pub(crate) struct Materialized {
    pub(crate) compiled: Compiled,
    pub(crate) terminal: Option<Extension>,
}

pub(crate) fn materialize(
    preparation: Preparation,
    observer: Option<&dyn GroundingObserver>,
) -> Result<Materialized, FormulaFailure> {
    let cancellation = preparation.budget.cancellation().cloned();
    let location = preparation.location;
    crate::grounding_observer::observe(observer, || {
        crate::formula::poll_control(cancellation.as_ref(), location)?;
        let Partition { base, terminal } = partition::partition(preparation)?;
        let materialized = match terminal {
            None => {
                crate::formula_ground::ground(base, observer, None).map(|compiled| Materialized {
                    compiled,
                    terminal: None,
                })
            }
            Some(definitions) => admit_terminal(base, definitions, observer),
        }?;
        crate::formula::poll_control(cancellation.as_ref(), location)?;
        Ok(materialized)
    })
}

/// Transfer the completed base owner and its accepted accounting into one
/// reconstruction plan. The source partition has already established eligibility.
fn admit_terminal(
    base: Preparation,
    definitions: Definitions,
    observer: Option<&dyn GroundingObserver>,
) -> Result<Materialized, FormulaFailure> {
    if let Some(observer) = observer {
        observer.terminal_definitions();
    }
    let Definitions {
        original,
        deferred,
        deferred_storage,
    } = definitions;
    let limits = base.limits;
    let location = base.location;
    let retained = crate::formula_ground::ground_retained(base, observer)?;
    let envelope_bytes = retained.envelope_bytes();
    let crate::formula_ground::RetainedGrounding {
        mut compiled,
        catalog,
        accounting,
        budget,
        mut output_storage,
    } = retained;
    let mut counters = Counters::resume(accounting, crate::grounding_observer::Work::default())
        .with_cancellation(budget.cancellation());
    let closed = catalog
        .into_closed(0, GroundingWork::new(&limits, &mut counters, location))
        .map_err(|failure| {
            let (failure, _actual_source_peak) = failure.into_parts();
            // The close operation records its actual composed peak
            // before returning the original typed refusal.
            failure
        })?;
    let directory_bytes = closed
        .storage
        .prior_publication_metadata_bytes(&compiled.atoms)
        .map_err(|error| FormulaFailure::TemplateCatalog {
            error: zetesis_core::TemplateCatalogFailure::Read(error),
            location,
        })?
        - compiled.atoms.publication_bytes();
    // Output maps and transferred buffers already have a live lease.
    // Replace the temporary grounding envelope; charge independent
    // old snapshot directories only after authenticating that prefix.
    let output_bytes = output_storage
        .bytes()
        .checked_sub(envelope_bytes)
        .ok_or_else(|| components::missing(location))? as u128;
    let fixed = (size_of::<Admitted>() - size_of::<crate::formula_support::ClosedSource>()) as u128;
    let metadata_bytes =
        output_bytes + directory_bytes + fixed + partition::deferred_bytes(&deferred);
    // Before retiring leases, admit both the final owner and the
    // current source-history workspace. Publication never resets work.
    let overlap = closed.storage_bytes() + metadata_bytes + counters.workspace_bytes() as u128
        - output_storage.bytes() as u128
        - deferred_storage.bytes() as u128;
    ceiling(
        FormulaResource::SupportBytes,
        overlap,
        limits.max_support_bytes as u128,
        location,
    )?;
    output_storage.observe(
        usize::try_from(output_bytes).map_err(|_| super::storage::overflow(location))?,
        location,
    )?;
    // These metadata owners now belong to the admitted extension, not a mutable
    // workspace. No canonical payload is moved or reimported here.
    drop(output_storage);
    drop(deferred_storage);
    let baseline = counters
        .into_accounting()
        .into_flat_baseline()
        .map_err(|_| components::missing(location))?;
    ceiling(
        FormulaResource::GeneratedValues,
        baseline.generated_values as u128,
        limits.max_generated_values as u128,
        location,
    )?;
    compiled.expansion = budget.usage();
    let final_bytes = closed.storage_bytes() + metadata_bytes;
    ceiling(
        FormulaResource::SupportBytes,
        final_bytes,
        limits.max_support_bytes as u128,
        location,
    )?;
    Ok(Materialized {
        compiled,
        terminal: Some(Extension {
            original,
            deferred,
            closed,
            baseline,
            limits,
            location,
            metadata_bytes,
        }),
    })
}
