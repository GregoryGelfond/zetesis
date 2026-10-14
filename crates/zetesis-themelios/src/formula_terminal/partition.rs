//! Consuming, same-owner correspondence between source definitions and flat IR.
//!
//! Optional inapplicability retains every accepted work/expansion charge. A
//! checked allocation or resource failure is an error, never optional fallback.

mod index;
mod matching;
mod policy;
mod reads;
mod selection;
mod source;
mod symbols;
#[cfg(test)]
mod tests;
mod workspace;

use themelios_program::program::Program;
use zetesis_domain::terminal;

use crate::formula::Preparation;
use crate::formula_facts::{Item, Items};
use crate::formula_ir::RuleIr;
use crate::formula_support::{Counters, StorageLease};
use crate::{AnalysisBasis, FormulaFailure, ProgramSite};
use workspace::{Context, Scratch};

pub(super) struct OriginalAnalysis {
    pub(super) program: Program,
    pub(super) analysis: themelios_analysis::Analysis,
    pub(super) basis: AnalysisBasis,
}

/// Every outcome retains the same preparation history; only a successfully
/// certified partition carries deferred definitions and the original analysis.
pub(super) struct Partition {
    pub(super) base: Preparation,
    pub(super) terminal: Option<Definitions>,
}

pub(super) struct Definitions {
    pub(super) original: OriginalAnalysis,
    /// The deferred rules and fact blocks, in item order.
    pub(super) deferred: Items,
    pub(super) deferred_storage: StorageLease,
}

/// Named retained capacities of the certified flat rules and deferred fact
/// products. The enclosing owner already contains the sequence's header.
/// Canonical component payload is shared; upstream source AST/provenance
/// follows its separate logical copy policy.
pub(super) fn deferred_bytes(items: &Items) -> u128 {
    items.retained_bytes()
}

struct Selection {
    source: Program,
    analysis: themelios_analysis::Analysis,
    /// One flag per item: nonzero moves it into the deferred program.
    selected: Scratch<u8>,
    deferred: Scratch<RuleIr>,
}

pub(super) fn partition(mut preparation: Preparation) -> Result<Partition, FormulaFailure> {
    let mut counters = Counters::resume(
        std::mem::take(&mut preparation.accounting),
        crate::grounding_observer::Work::default(),
    )
    .with_cancellation(preparation.budget.cancellation());
    let selected = prepare(&mut preparation, &mut counters)?;
    let Some(selected) = selected else {
        preparation.accounting = counters.into_accounting();
        return Ok(Partition {
            base: preparation,
            terminal: None,
        });
    };
    // Component validation and movement permits precede the move, which
    // reserves before it moves. Each original IR occurrence remains present
    // exactly once: every rule and fact block, in item order.
    let deferred = preparation
        .program
        .split_items(&selected.selected.values)
        .map_err(|error| FormulaFailure::AtomAllocation {
            error,
            location: preparation.location,
        })?;
    let original = OriginalAnalysis {
        program: std::mem::replace(&mut preparation.program.analyzed, selected.source),
        analysis: std::mem::replace(&mut preparation.program.analysis, selected.analysis),
        basis: preparation.program.analysis_basis,
    };
    let (_, mut deferred_storage) = selected.deferred.into_leased_values();
    deferred_storage.observe(
        usize::try_from(deferred_bytes(&deferred)).expect("admitted deferred capacity"),
        preparation.location,
    )?;
    drop(selected.selected);
    preparation.accounting = counters.into_accounting();
    Ok(Partition {
        base: preparation,
        terminal: Some(Definitions {
            original,
            deferred,
            deferred_storage,
        }),
    })
}

fn prepare(
    preparation: &mut Preparation,
    counters: &mut Counters,
) -> Result<Option<Selection>, FormulaFailure> {
    let limits = &preparation.limits;
    let location = preparation.location;
    counters.work(limits, location)?;
    let program = &preparation.program;
    if program.analysis_basis != AnalysisBasis::NormalizedProgram
        || !program.objectives.is_empty()
        || !program.objective_declarations.is_empty()
        || !program.projection.is_empty()
        || program.project_selection.is_explicit()
    {
        return Ok(None);
    }
    let mut domain_limits = zetesis_domain::Limits::default();
    domain_limits.max_work = domain_limits
        .max_work
        .min(limits.max_work.saturating_sub(counters.accounting.work));
    let analysis = terminal::analyze(&program.analyzed, domain_limits);
    counters.charge_work(u128::from(analysis.statistics().work), limits, location)?;
    if analysis.status() != terminal::Status::Complete
        || analysis.definitions().is_empty()
        || !analysis.belongs_to(&program.analyzed)
    {
        return Ok(None);
    }
    // This capability borrows the already paired source authority. It imports
    // nothing and does not finish/rebind or replay preparation into an observer.
    let admission = preparation
        .catalog
        .component_admission(limits, counters, location)?;
    let components = admission.components(limits, counters, location)?;
    let mut context = Context {
        admission: &admission,
        components,
        limits,
        counters,
        location,
    };
    let Some(mut selected) = selection::select(program, analysis.definitions(), &mut context)?
    else {
        return Ok(None);
    };
    let definitions = policy::select(
        program,
        analysis.definitions(),
        &mut selected.values,
        &mut context,
    )?;
    if definitions.values.is_empty() {
        return Ok(None);
    }
    let (source, source_analysis) = source::base(
        &program.analyzed,
        &definitions.values,
        &mut preparation.budget,
        &mut context,
    )?;
    drop(definitions);
    let mut deferred = Scratch::<RuleIr>::new(&context)?;
    let mut count = 0_usize;
    let mut nested_bytes = 0_u128;
    for (selected_item, item) in selected.values.iter().zip(program.items().iter()) {
        context.work()?;
        if *selected_item != 0 {
            count += 1;
            nested_bytes += match item {
                Item::Rule(_, rule) => {
                    size_of::<RuleIr>() as u128
                        + rule.body.capacity() as u128
                            * size_of::<crate::formula_ir::LiteralIr>() as u128
                        + rule.origins.capacity() as u128 * size_of::<ProgramSite>() as u128
                }
                Item::Facts(product) => product.retained_bytes() as u128,
            };
        }
    }
    let retained = nested_bytes
        + count as u128 * crate::formula_facts::ITEM_BYTES as u128
        + size_of::<Scratch<RuleIr>>() as u128;
    let retained = usize::try_from(retained).map_err(|_| FormulaFailure::Limit {
        resource: crate::FormulaResource::SupportBytes,
        observed: retained,
        limit: limits.max_support_bytes as u128,
        location,
    })?;
    deferred.retain_existing(retained, &context)?;
    // One permitted read/move per original occurrence; no callbacks during
    // publication can expose a partially partitioned source owner.
    context
        .counters
        .charge_work(selected.values.len() as u128, limits, location)?;
    Ok(Some(Selection {
        source,
        analysis: source_analysis,
        selected,
        deferred,
    }))
}
