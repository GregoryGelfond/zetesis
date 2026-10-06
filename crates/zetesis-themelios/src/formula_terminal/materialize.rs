//! Consume source certification and the actual retained base grounding owners.

use std::sync::Arc;

use super::{
    Admitted, Extension,
    partition::{self, Definitions, Partition},
};
use crate::formula::{Compiled, Preparation, ceiling};
use crate::formula_support::{Counters, GroundingWork, components};
use crate::{FormulaFailure, FormulaResource, GroundingObserver};

/// How a materialized core was grounded.
pub(crate) enum Core {
    /// Every rule instantiated.
    Eager,
    /// Eligible constraints streamed; absent when none was eligible.
    Hybrid(Option<Box<crate::formula_hybrid::Constraints>>),
}

pub(crate) struct Materialized {
    pub(crate) compiled: Compiled,
    pub(crate) core: Core,
    pub(crate) terminal: Option<Extension>,
}

/// Materialize with an eager base: every rule of the program, or of the base
/// when terminal definitions are deferred, is instantiated.
pub(crate) fn materialize(
    preparation: Preparation,
    observer: Option<&dyn GroundingObserver>,
) -> Result<Materialized, FormulaFailure> {
    materialize_with(preparation, observer, false)
}

/// Materialize with a hybrid base: eligible constraints are streamed, and
/// deferred terminal definitions are reconstructed per answer. The hybrid
/// schedule's restrictions apply after the partition has declined or
/// certified its groups, so its charges are retained either way.
pub(crate) fn materialize_lazy(
    preparation: Preparation,
    observer: Option<&dyn GroundingObserver>,
) -> Result<Materialized, FormulaFailure> {
    materialize_with(preparation, observer, true)
}

fn materialize_with(
    preparation: Preparation,
    observer: Option<&dyn GroundingObserver>,
    hybrid: bool,
) -> Result<Materialized, FormulaFailure> {
    let cancellation = preparation.budget.cancellation().cloned();
    let location = preparation.location;
    crate::grounding_observer::observe(observer, || {
        crate::formula::poll_control(cancellation.as_ref(), location)?;
        let Partition { base, terminal } = partition::partition(preparation)?;
        let materialized = match terminal {
            None if hybrid => crate::formula_ground::ground_hybrid(base, observer).map(
                |(compiled, constraints)| Materialized {
                    compiled,
                    core: Core::Hybrid(constraints.map(Box::new)),
                    terminal: None,
                },
            ),
            None => {
                crate::formula_ground::ground(base, observer, None).map(|compiled| Materialized {
                    compiled,
                    core: Core::Eager,
                    terminal: None,
                })
            }
            Some(definitions) => admit_terminal(base, definitions, observer, hybrid),
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
    hybrid: bool,
) -> Result<Materialized, FormulaFailure> {
    if let Some(observer) = observer {
        observer.terminal_definitions(if hybrid {
            super::BaseKind::Hybrid
        } else {
            super::BaseKind::Eager
        });
    }
    let Definitions {
        original,
        deferred,
        deferred_storage,
    } = definitions;
    let limits = base.limits;
    let location = base.location;
    let retained = crate::formula_ground::ground_retained(base, observer, hybrid)?;
    let envelope_bytes = retained.envelope_bytes();
    let crate::formula_ground::RetainedGrounding {
        mut compiled,
        catalog,
        accounting,
        budget,
        mut output_storage,
        streamed,
    } = retained;
    let mut counters = Counters::resume(accounting, crate::grounding_observer::Work::default())
        .with_cancellation(budget.cancellation());
    let (closed, constraints, relation_bytes) =
        close_base(catalog, streamed, &limits, &mut counters, location)?;
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
    // The closed source itself is counted by its storage; the owner holds a
    // shared handle to it.
    let fixed =
        (size_of::<Admitted>() - size_of::<Arc<crate::formula_support::ClosedSource>>()) as u128;
    let metadata_bytes = output_bytes
        + directory_bytes
        + fixed
        + partition::deferred_bytes(&deferred)
        + relation_bytes;
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
        core: if hybrid {
            Core::Hybrid(constraints.map(Box::new))
        } else {
            Core::Eager
        },
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

/// Close the base catalog once. For a hybrid base with a stream, the close
/// keeps the relations its constraints read, and both they and reconstruction
/// share the closed source; otherwise the catalog closes for reconstruction
/// alone. Returns the closed source, the streamed constraints and the kept
/// relations' bytes.
fn close_base(
    catalog: crate::formula_support::CompletedCatalog,
    streamed: Option<(Vec<crate::formula_ir::RuleIr>, u64)>,
    limits: &crate::FormulaLimits,
    counters: &mut Counters,
    location: crate::ProgramSite,
) -> Result<
    (
        Arc<crate::formula_support::ClosedSource>,
        Option<crate::formula_hybrid::Constraints>,
        u128,
    ),
    FormulaFailure,
> {
    match streamed {
        Some((rules, instances)) if !rules.is_empty() => {
            let support =
                catalog.into_streamed(&rules, GroundingWork::new(limits, counters, location))?;
            let closed = Arc::clone(support.closed());
            let relation_bytes = support.relation_bytes() as u128;
            let constraints = crate::formula_hybrid::Constraints {
                support,
                rules,
                instances,
                limits: *limits,
                location,
            };
            Ok((closed, Some(constraints), relation_bytes))
        }
        _ => {
            let closed = catalog
                .into_closed(0, GroundingWork::new(limits, counters, location))
                .map_err(|failure| {
                    let (failure, _actual_source_peak) = failure.into_parts();
                    // The close operation records its actual composed peak
                    // before returning the original typed refusal.
                    failure
                })?;
            Ok((Arc::new(closed), None, 0))
        }
    }
}
