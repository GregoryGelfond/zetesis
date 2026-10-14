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
use crate::formula_facts::{Item, Items, SplitRoom};
use crate::formula_support::{Counters, StorageLease};
use crate::{AnalysisBasis, FormulaFailure, ProgramSite};
use workspace::{Context, Lease, Scratch};

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
/// blocks. The enclosing owner already contains the sequence's header.
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
    /// The room the move fills, reserved before it.
    room: SplitRoom,
    /// The deferred program's storage: the vectors reserved for what moves,
    /// and the moving rules' bodies and origins, whose owner the move changes.
    deferred: Lease,
    /// The kept rules' and items' replacement vectors, admitted while the
    /// vectors they replace are live. The base program takes them over at the
    /// move, and this lease is then released, as the base program's own
    /// storage is not leased.
    kept: Lease,
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
    // Component validation, movement permits and every reservation the move
    // fills precede it, so the move itself allocates nothing. Each original IR
    // occurrence remains present exactly once: every rule and fact block, in
    // item order.
    let deferred = preparation
        .program
        .split_items(&selected.selected.values, selected.room);
    drop(selected.kept);
    let original = OriginalAnalysis {
        program: std::mem::replace(&mut preparation.program.analyzed, selected.source),
        analysis: std::mem::replace(&mut preparation.program.analysis, selected.analysis),
        basis: preparation.program.analysis_basis,
    };
    let mut deferred_storage = selected.deferred.into_lease();
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
    let mut deferred = Lease::new(&context)?;
    let mut kept = Lease::new(&context)?;
    let room = program.items().reserve_split(
        &selected.values,
        &mut context,
        deferred.get_mut(),
        kept.get_mut(),
    )?;
    // The moving rules' bodies and origins are live already; the move only
    // changes their owner. The moved fact blocks are copies, reserved above.
    let mut nested_bytes = 0_u128;
    for (selected_item, item) in selected.values.iter().zip(program.items().iter()) {
        context.work()?;
        if *selected_item != 0
            && let Item::Rule(_, rule) = item
        {
            nested_bytes += rule.body.capacity() as u128
                * size_of::<crate::formula_ir::LiteralIr>() as u128
                + rule.origins.capacity() as u128 * size_of::<ProgramSite>() as u128;
        }
    }
    let nested = usize::try_from(nested_bytes).map_err(|_| FormulaFailure::Limit {
        resource: crate::FormulaResource::SupportBytes,
        observed: nested_bytes,
        limit: limits.max_support_bytes as u128,
        location,
    })?;
    deferred.retain_existing(nested, &context)?;
    // One permitted read/move per original occurrence; no callbacks during
    // publication can expose a partially partitioned source owner.
    context
        .counters
        .charge_work(selected.values.len() as u128, limits, location)?;
    Ok(Some(Selection {
        source,
        analysis: source_analysis,
        selected,
        room,
        deferred,
        kept,
    }))
}
