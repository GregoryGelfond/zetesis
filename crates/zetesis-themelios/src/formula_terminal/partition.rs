//! Consuming, same-owner correspondence between source definitions and flat IR.
//!
//! Optional inapplicability retains every accepted work/expansion charge. A
//! checked allocation or resource failure is an error, never optional fallback.

mod matching;
mod reads;
mod selection;
mod source;
mod symbols;
mod workspace;
#[cfg(test)]
mod tests;

use themelios_program::program::Program;
use zetesis_domain::terminal;

use crate::formula::Preparation;
use crate::formula_ir::{LiteralIr, RuleIr};
use crate::formula_support::{Counters, StorageLease};
use crate::{AnalysisBasis, FormulaFailure};
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
    pub(super) deferred: Vec<RuleIr>,
    pub(super) deferred_storage: StorageLease,
}

/// Named retained capacities of the certified flat rules. The enclosing owner
/// already contains the vector header. Canonical component payload is shared;
/// upstream source AST/provenance follows its separate logical copy policy.
pub(super) fn deferred_bytes(rules: &Vec<RuleIr>) -> u128 {
    rules.capacity() as u128 * size_of::<RuleIr>() as u128
        + rules
            .iter()
            .map(|rule| {
                rule.body.capacity() as u128 * size_of::<LiteralIr>() as u128
                    + rule.origins.capacity() as u128
                        * size_of::<themelios_base::span::Location>() as u128
            })
            .sum::<u128>()
}

struct Selection {
    source: Program,
    analysis: themelios_analysis::Analysis,
    selected: Scratch<u8>,
    base: Scratch<RuleIr>,
    deferred: Scratch<RuleIr>,
}

pub(super) fn partition(mut preparation: Preparation) -> Result<Partition, FormulaFailure> {
    let mut counters = Counters::resume(
        std::mem::take(&mut preparation.accounting),
        crate::grounding_observer::Work::default(),
    );
    let selected = prepare(&mut preparation, &mut counters)?;
    let Some(mut selected) = selected else {
        preparation.accounting = counters.into_accounting();
        return Ok(Partition {
            base: preparation,
            terminal: None,
        });
    };
    // All allocations, component validation and movement permits precede these
    // infallible moves. Each original IR occurrence remains present exactly once.
    for (rule, selected_rule) in std::mem::take(&mut preparation.program.rules)
        .into_iter()
        .zip(&selected.selected.values)
    {
        if *selected_rule == 0 {
            selected.base.values.push(rule);
        } else {
            selected.deferred.values.push(rule);
        }
    }
    preparation.program.rules = selected.base.into_values();
    let original = OriginalAnalysis {
        program: std::mem::replace(&mut preparation.program.analyzed, selected.source),
        analysis: std::mem::replace(&mut preparation.program.analysis, selected.analysis),
        basis: preparation.program.analysis_basis,
    };
    let (deferred, mut deferred_storage) = selected.deferred.into_leased_values();
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
    let Some(selected) = selection::select(program, analysis.definitions(), &mut context)? else {
        return Ok(None);
    };
    let (source, source_analysis) = source::base(
        &program.analyzed,
        analysis.definitions(),
        &mut preparation.budget,
        &mut context,
    )?;
    let mut base = Scratch::new(&context)?;
    let mut deferred = Scratch::new(&context)?;
    let mut count = 0;
    let mut nested_bytes = 0_u128;
    for (selected_rule, rule) in selected.values.iter().zip(&program.rules) {
        context.work()?;
        if *selected_rule != 0 {
            count += 1;
            nested_bytes += rule.body.capacity() as u128 * size_of::<LiteralIr>() as u128
                + rule.origins.capacity() as u128
                    * size_of::<themelios_base::span::Location>() as u128;
        }
    }
    base.reserve(selected.values.len() - count, &mut context)?;
    deferred.reserve(count, &mut context)?;
    let retained = nested_bytes
        + deferred.values.capacity() as u128 * size_of::<RuleIr>() as u128
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
        base,
        deferred,
    }))
}
