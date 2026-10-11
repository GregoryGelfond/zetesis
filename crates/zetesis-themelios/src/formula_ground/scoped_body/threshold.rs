//! The shared physical threshold recurrence targets original-model query nodes.

use crate::formula_source_activity::Context;
use crate::formula_source_activity::model_query::Query;
use crate::formula_support::Buffer;
use crate::{FormulaFailure, ProgramSite};
use zetesis_ferraris::{
    AdmissionError, ExtremumCut, Threshold, extremum_lowering, threshold_lowering,
};
use zetesis_objective::ConditionNode;

enum Failure {
    Formula(FormulaFailure),
    Physical(threshold_lowering::Error),
}

impl From<threshold_lowering::Error> for Failure {
    fn from(error: threshold_lowering::Error) -> Self {
        Self::Physical(error)
    }
}

impl Failure {
    fn located(self, location: ProgramSite, malformed: AdmissionError) -> FormulaFailure {
        match self {
            Self::Formula(error) => error,
            Self::Physical(error) => FormulaFailure::Theory {
                error: match error {
                    threshold_lowering::Error::Invalid => malformed,
                    threshold_lowering::Error::Capacity => AdmissionError::Limit,
                },
                location,
            },
        }
    }
}

pub(super) fn lower(
    threshold: Threshold<'_>,
    mapping: &[usize],
    query: &mut Query,
    context: &mut Context<'_, '_, '_>,
) -> Result<usize, FormulaFailure> {
    let shape = threshold_lowering::Shape::new(threshold, || {
        context
            .counters
            .work(context.limits, context.location)
            .map_err(Failure::Formula)
    })
    .map_err(|error: Failure| error.located(context.location, AdmissionError::ThresholdShape))?;
    let constants = if shape.constant().is_some() { 1 } else { 2 };
    admit_output(query.len(), shape.gates, constants, context)?;
    let mut scratch = Buffer::new(
        context.computation,
        context.limits,
        context.counters,
        context.location,
    )?;
    scratch.resize(
        shape.scratch,
        0,
        context.computation,
        context.limits,
        context.counters,
        context.location,
    )?;
    threshold_lowering::lower(
        threshold,
        scratch.slice_mut(),
        &mut Sink {
            query,
            mapping,
            context,
        },
    )
    .map_err(|error| error.located(context.location, AdmissionError::ThresholdShape))
}

fn admit_output(
    length: usize,
    gates: usize,
    constants: u128,
    context: &Context<'_, '_, '_>,
) -> Result<(), FormulaFailure> {
    let actual = length as u128 + gates as u128 + constants;
    let limit = context.limits.objective.max_condition_nodes;
    if actual > limit as u128 {
        return Err(FormulaFailure::Objective {
            error: zetesis_objective::AdmissionError::Limit {
                resource: zetesis_objective::AdmissionResource::ConditionNodes,
                template: None,
                actual: usize::try_from(actual).unwrap_or(usize::MAX),
                limit,
            },
            location: context.location,
        });
    }
    Ok(())
}

pub(super) fn lower_extremum(
    cut: ExtremumCut<'_>,
    mapping: &[usize],
    query: &mut Query,
    context: &mut Context<'_, '_, '_>,
) -> Result<usize, FormulaFailure> {
    let shape = extremum_lowering::Shape::new(cut, || {
        context
            .counters
            .work(context.limits, context.location)
            .map_err(Failure::Formula)
    })
    .map_err(|error: Failure| error.located(context.location, AdmissionError::OrdinalCut))?;
    admit_output(query.len(), shape.gates, 1, context)?;
    extremum_lowering::lower(
        cut,
        &mut Sink {
            query,
            mapping,
            context,
        },
    )
    .map_err(|error| error.located(context.location, AdmissionError::OrdinalCut))
}

struct Sink<'a, 'scope, 'terms, 'source> {
    query: &'a mut Query,
    mapping: &'a [usize],
    context: &'a mut Context<'scope, 'terms, 'source>,
}

impl threshold_lowering::Sink for Sink<'_, '_, '_, '_> {
    type Value = usize;
    type Error = Failure;

    fn tick(&mut self) -> Result<(), Failure> {
        self.context
            .counters
            .work(self.context.limits, self.context.location)
            .map_err(Failure::Formula)
    }

    fn constant(&mut self, value: bool) -> Result<usize, Failure> {
        self.query
            .node(ConditionNode::Boolean(value), self.context)
            .map_err(Failure::Formula)
    }

    fn condition(&mut self, index: usize) -> Result<usize, Failure> {
        self.mapping
            .get(index)
            .copied()
            .ok_or(Failure::Physical(threshold_lowering::Error::Invalid))
    }

    fn gate(&mut self, disjunction: bool, left: usize, right: usize) -> Result<usize, Failure> {
        self.query
            .node(
                if disjunction {
                    ConditionNode::Or(left, right)
                } else {
                    ConditionNode::And(left, right)
                },
                self.context,
            )
            .map_err(Failure::Formula)
    }
}
