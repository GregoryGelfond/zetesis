//! The filtered analysis owner uses the established logical source-copy policy.

use themelios_program::program::{Program, Statement};
use themelios_program::provenance::WithProvenance;

use super::workspace::{Context, Scratch};
use crate::expansion::Budget;
use crate::{ExpansionResource, FormulaFailure, FormulaResource};

pub(super) fn base(
    source: &Program,
    definitions: &[&WithProvenance<Statement>],
    budget: &mut Budget,
    context: &mut Context<'_, '_>,
) -> Result<(Program, themelios_analysis::Analysis), FormulaFailure> {
    let mut statements = Scratch::new(context)?;
    let mut nodes = 0_u128;
    for statement in source.statements() {
        context.work()?;
        if selected(statement, definitions, context)? {
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
        statements.reserve(1, context)?;
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

fn selected(
    source: &WithProvenance<Statement>,
    definitions: &[&WithProvenance<Statement>],
    context: &mut Context<'_, '_>,
) -> Result<bool, FormulaFailure> {
    for definition in definitions {
        context.work()?;
        if std::ptr::eq(source, *definition) {
            return Ok(true);
        }
    }
    Ok(false)
}
