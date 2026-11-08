//! The filtered analysis owner uses the established logical source-copy policy.

use themelios_program::program::{Program, Statement};
use themelios_program::provenance::WithProvenance;

use super::{
    index,
    workspace::{Context, Scratch},
};
use crate::expansion::Budget;
use crate::{ExpansionResource, FormulaFailure, FormulaResource};

pub(super) fn base(
    source: &Program,
    definitions: &[&WithProvenance<Statement>],
    budget: &mut Budget,
    context: &mut Context<'_, '_>,
) -> Result<(Program, themelios_analysis::Analysis), FormulaFailure> {
    // Definitions are found by address, so each statement costs a search.
    // Addresses differ from run to run; the search's charge must not.
    let mut addresses = Scratch::new(context)?;
    addresses.reserve(definitions.len(), context)?;
    for definition in definitions {
        context.work()?;
        addresses
            .values
            .push(std::ptr::from_ref(*definition).addr());
    }
    index::sort(&mut addresses.values, context)?;
    let mut statements = Scratch::new(context)?;
    let total = source.statements().count();
    statements.reserve(total.saturating_sub(definitions.len()), context)?;
    let mut nodes = 0_u128;
    for statement in source.statements() {
        context.work()?;
        if selected(statement, &addresses.values, context)? {
            continue;
        }
        let (work, bytes) = crate::formula_pool::source_copy_cost(statement.get());
        nodes = nodes.saturating_add(work);
        crate::formula::ceiling(
            FormulaResource::AnalysisNodes,
            nodes,
            context.limits.max_analysis_nodes as u128,
            context.location,
        )?;
        budget.charge(ExpansionResource::TermWork, work, context.location)?;
        budget.charge(ExpansionResource::ScalarBytes, bytes, context.location)?;
        if statements.values.len() == statements.values.capacity() {
            statements.reserve(1, context)?;
        }
        context.work()?;
        // Upstream cloning/Program collection uses infallible allocation under
        // these logical source limits. Other AST carriers, provenance and tree
        // allocator overhead remain outside this policy, as in normalization.
        statements.values.push(statement.clone());
    }
    context.work()?;
    let program = Program::of_nodes(std::mem::take(&mut statements.values));
    drop(statements);
    let analysis =
        crate::formula_analysis::analyze(&program, context.limits, budget, context.location)?;
    Ok((program, analysis))
}

/// Whether `source` is one of the definitions, by identity.
fn selected(
    source: &WithProvenance<Statement>,
    addresses: &[usize],
    context: &mut Context<'_, '_>,
) -> Result<bool, FormulaFailure> {
    contains(addresses, std::ptr::from_ref(source).addr(), |amount| {
        context.charge(amount)
    })
}

/// Whether the sorted `addresses` hold `address`. The search charges the bound
/// on its probes, ⌈log₂(n + 1)⌉ steps, as the sort charges its model: the
/// number of probes depends on where `address` falls among the others, and
/// allocation addresses change from run to run, so charging the probes would
/// make the same program's work receipts differ between runs.
pub(super) fn contains(
    addresses: &[usize],
    address: usize,
    mut charge: impl FnMut(u128) -> Result<(), FormulaFailure>,
) -> Result<bool, FormulaFailure> {
    charge(u128::from(usize::BITS - addresses.len().leading_zeros()))?;
    Ok(addresses.binary_search(&address).is_ok())
}
