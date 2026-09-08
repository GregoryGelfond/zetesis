//! Required inputs and stable finite-value scheduling for aggregate consumers.
//!
//! The plan references existing body instructions; it never rewrites semantic
//! literals. A step's inputs are relationally bound or produced by earlier
//! steps. Aggregate outputs are proposals, not established aggregate truth.
//! Local scopes retain their independent compiler and cursor paths.

use themelios_program::program::DefaultNegation;
use zetesis_core::Term;

use crate::formula_binding_cursor::target;
use crate::formula_ir::{AggregateGuard, Compiler, LiteralIr};
use crate::{ExpansionResource, FormulaFailure};

pub(crate) struct Plan {
    pub steps: Vec<Step>,
    /// An outer filter, negative gate, scalar/range binding, aggregate producer
    /// or head bound reads an aggregate proposal or a value derived from one.
    /// Objective admission consumes this fact; scheduling does not certify a
    /// total objective observer.
    pub consumers: bool,
}

/// One existing instruction, with a single output and complete outer reads.
pub(crate) struct Step {
    pub literal: usize,
    pub required: Vec<usize>,
    pub produced: usize,
}

impl Compiler<'_> {
    /// Allocate only for rules containing aggregate proposals. Plan storage is
    /// O(steps + input occurrences + variables). Summary construction scans each
    /// outer slot twice across each instruction's expression/element structure:
    /// O(variables · total instruction structure). Stable selection can scan
    /// O(steps² · inputs) and charges every inspection. Each successful pass
    /// removes one pending step; a pass without progress reports a dependency.
    pub(super) fn assignment_plan(
        &mut self,
        body: &[LiteralIr],
        variables: usize,
        guards: &[AggregateGuard],
    ) -> Result<Option<Plan>, FormulaFailure> {
        // Preserve the established source-work charge on aggregate-free rules.
        self.scope_work(body.len())?;
        if !body.iter().any(|literal| {
            matches!(literal, LiteralIr::Aggregate(aggregate) if aggregate.binding.is_some())
        }) {
            return Ok(None);
        }
        self.scope_work(body.len())?;
        let count = body.iter().filter_map(target).count();
        self.plan_storage::<Option<Step>>(count)?;
        self.plan_storage::<Step>(count)?;
        self.plan_storage::<bool>(variables)?;
        self.plan_storage::<bool>(variables)?;
        self.plan_storage::<Option<usize>>(variables)?;
        let mut pending = Vec::with_capacity(count);
        let mut ready = vec![false; variables];
        let mut aggregate_values = vec![false; variables];
        let mut producers = vec![None; variables];
        self.relational_inputs(body, &mut ready)?;

        // Legacy priority is scalar/range instructions followed by aggregate
        // proposals. Dependencies can delay a step, never reverse ready peers.
        for aggregates in [false, true] {
            for (index, literal) in body.iter().enumerate() {
                self.scope_work(1)?;
                let Some(produced) = target(literal) else {
                    continue;
                };
                if matches!(literal, LiteralIr::Aggregate(_)) != aggregates {
                    continue;
                }
                assert!(producers[produced].is_none(), "one instruction per target");
                producers[produced] = Some(pending.len());
                // Count before reserving exact logical capacity. Repeated
                // source scans are charged; growing/reallocating summaries
                // must not hide additional copies of earlier input cells.
                let mut inputs = 0;
                for input in 0..variables {
                    inputs += usize::from(self.instruction_uses(literal, input)?);
                }
                self.plan_storage::<usize>(inputs)?;
                let mut required = Vec::with_capacity(inputs);
                for input in 0..variables {
                    if self.instruction_uses(literal, input)? {
                        required.push(input);
                    }
                }
                pending.push(Some(Step {
                    literal: index,
                    required,
                    produced,
                }));
            }
        }
        let mut steps = Vec::with_capacity(count);
        while steps.len() < count {
            let mut selected = None;
            for (index, step) in pending.iter().enumerate() {
                self.scope_work(1)?;
                let Some(step) = step else { continue };
                self.scope_work(step.required.len())?;
                if step.required.iter().all(|input| ready[*input]) {
                    selected = Some(index);
                    break;
                }
            }
            let Some(index) = selected else {
                return Err(self.dependency_failure(&pending, &ready, &producers)?);
            };
            let step = pending[index].take().expect("selected pending instruction");
            self.scope_work(step.required.len())?;
            let dependent = step.required.iter().any(|input| aggregate_values[*input]);
            let aggregate = matches!(body[step.literal], LiteralIr::Aggregate(_));
            // A dependent aggregate selects its own candidate carrier from
            // this completed predecessor row. The cursor resets that carrier
            // before any predecessor changes; original equalities still
            // determine whether the full proposal row is realized.
            ready[step.produced] = true;
            aggregate_values[step.produced] = aggregate || dependent;
            steps.push(step);
        }
        let consumers = self.assignment_context(body, guards, &aggregate_values)?;
        Ok(Some(Plan { steps, consumers }))
    }

    fn plan_storage<T>(&mut self, count: usize) -> Result<(), FormulaFailure> {
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            count as u128 * std::mem::size_of::<T>() as u128,
            self.location,
        )?;
        Ok(())
    }

    fn relational_inputs(
        &mut self,
        body: &[LiteralIr],
        ready: &mut [bool],
    ) -> Result<(), FormulaFailure> {
        for literal in body {
            self.scope_work(1)?;
            match literal {
                LiteralIr::Atom(DefaultNegation::None, atom) => {
                    self.scope_work(atom.terms().len())?;
                    for term in atom.terms() {
                        if let Term::Variable(slot) = term {
                            ready[*slot] = true;
                        }
                    }
                }
                LiteralIr::PatternAtom(pattern) => {
                    self.scope_work(pattern.node_count())?;
                    for slot in pattern.slots() {
                        ready[slot] = true;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn instruction_uses(
        &mut self,
        literal: &LiteralIr,
        input: usize,
    ) -> Result<bool, FormulaFailure> {
        // Even an empty aggregate performs an outer-slot readiness inspection.
        self.scope_work(1)?;
        match literal {
            LiteralIr::Bind { value, .. } => self.expression_uses(value, input),
            LiteralIr::Range { lower, upper, .. } => {
                Ok(self.expression_uses(lower, input)? || self.expression_uses(upper, input)?)
            }
            LiteralIr::Aggregate(aggregate) => {
                // Its own equality target is an output, not a guard input.
                // Compiler::rule finishes ordinary body/head/guard registration
                // before body_aggregates clones Variables for each element.
                // Later conditional/choice compilation borrows that outer frame
                // and extends only clones. Thus local slots cannot alias a
                // subsequently allocated outer slot or enter this summary.
                for element in &aggregate.elements {
                    if self.element_uses(element, input)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            _ => unreachable!("only instructions enter a plan"),
        }
    }

    fn dependency_failure(
        &mut self,
        pending: &[Option<Step>],
        ready: &[bool],
        producers: &[Option<usize>],
    ) -> Result<FormulaFailure, FormulaFailure> {
        let mut blocked = None;
        for step in pending {
            self.scope_work(1)?;
            let Some(step) = step else { continue };
            for &variable in &step.required {
                self.scope_work(1)?;
                if !ready[variable] {
                    if producers[variable].is_none() {
                        return Ok(FormulaFailure::UnboundValueInput {
                            variable,
                            location: self.location,
                        });
                    }
                    blocked.get_or_insert(variable);
                }
            }
        }
        let variable = blocked.expect("blocked instruction has an input");
        Ok(FormulaFailure::CyclicValueInput {
            variable,
            location: self.location,
        })
    }
}
