//! Required inputs and stable finite-value scheduling for aggregate consumers.
//!
//! The plan references existing body instructions; it never rewrites semantic
//! literals. A step's inputs are relationally bound or produced by earlier
//! steps. Aggregate outputs are proposals, not established aggregate truth.
//! Local scopes retain their independent compiler and cursor paths.

use themelios_program::program::DefaultNegation;
use zetesis_core::TemplateTerm;

use crate::formula_binding_cursor::target;
use crate::formula_ir::{AggregateGuard, Compiler, LiteralIr};
use crate::{ExpansionResource, FormulaFailure};

pub(crate) struct Plan {
    pub steps: Vec<Step>,
    /// An outer filter, negative gate, conditional, scalar/range binding,
    /// aggregate producer, nonbinding aggregate (guard, element key or element
    /// condition), or head bound reads an aggregate proposal or its descendant.
    /// Only the original body frame participates; local witness indices and
    /// the later synthetic head suffix are not interchangeable outer inputs.
    /// Objective admission consumes this fact; scheduling does not certify a
    /// total objective observer.
    pub consumers: bool,
    /// A normal rule's complete continuation reads, projected onto
    /// relational outer slots. The checked rule compiler populates this only
    /// after its head and all generator/filter instructions exist. None leaves
    /// support witness traversal unchanged. Checked formula factorization also
    /// borrows this summary to retain every positive activation while sharing
    /// only equal continuations; the list alone never authorizes that rewrite.
    pub continuation_inputs: Option<Vec<usize>>,
}

/// One existing instruction, with a single output and complete outer reads.
pub(crate) struct Step {
    pub literal: usize,
    inputs: Inputs,
    pub produced: usize,
}

/// Aggregate instructions borrow the summary already owned by their source
/// occurrence. Scalar and range instructions own their separately compiled reads.
/// `Aggregate` is constructed only for the binding aggregate at `Step::literal`;
/// its output is exactly `Step::produced`. A plan stays paired with that body.
enum Inputs {
    Aggregate,
    Slots(Vec<usize>),
}

#[cfg(test)]
impl Plan {
    /// Keep the execution inputs fixed while a reference test changes only
    /// aggregate cache-key metadata. Production plans borrow one summary.
    pub(crate) fn retain_required_for_test(&mut self, body: &[LiteralIr]) {
        for step in &mut self.steps {
            step.inputs = Inputs::Slots(step.required(body).to_vec());
        }
    }
}

impl Step {
    pub(crate) fn required<'a>(&'a self, body: &'a [LiteralIr]) -> &'a [usize] {
        match &self.inputs {
            Inputs::Slots(slots) => slots,
            Inputs::Aggregate => match &body[self.literal] {
                LiteralIr::Aggregate(aggregate) if aggregate.binding == Some(self.produced) => {
                    &aggregate.family_inputs
                }
                _ => unreachable!("aggregate step selects its original binding aggregate"),
            },
        }
    }
}

impl Compiler<'_> {
    /// Allocate only for rules containing aggregate proposals. Plan storage is
    /// O(steps + scalar/range input occurrences + variables). Aggregate reads
    /// borrow the source occurrence's already compiled family-input summary;
    /// scalar/range summaries scan each slot twice across expression structure.
    /// Stable selection can scan
    /// O(steps² · inputs) and charges every inspection. Each successful pass
    /// removes one pending step; a pass without progress reports a dependency.
    pub(super) fn assignment_plan(
        &mut self,
        body: &[LiteralIr],
        variables: usize,
        body_variables: usize,
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
                let inputs = self.instruction_inputs(literal, variables)?;
                pending.push(Some(Step {
                    literal: index,
                    inputs,
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
                let required = step.required(body);
                self.scope_work(required.len())?;
                if required.iter().all(|input| ready[*input]) {
                    selected = Some(index);
                    break;
                }
            }
            let Some(index) = selected else {
                return Err(self.dependency_failure(&pending, body, &ready, &producers)?);
            };
            let step = pending[index].take().expect("selected pending instruction");
            let required = step.required(body);
            self.scope_work(required.len())?;
            let dependent = required.iter().any(|input| aggregate_values[*input]);
            let aggregate = matches!(body[step.literal], LiteralIr::Aggregate(_));
            // A dependent aggregate selects its own candidate carrier from
            // this completed predecessor row. The cursor resets that carrier
            // before any predecessor changes; original equalities still
            // determine whether the full proposal row is realized.
            ready[step.produced] = true;
            aggregate_values[step.produced] = aggregate || dependent;
            steps.push(step);
        }
        let consumers =
            self.assignment_context(body, guards, &aggregate_values[..body_variables])?;
        Ok(Some(Plan {
            steps,
            consumers,
            continuation_inputs: None,
        }))
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
                    let atom = self.source.pattern_ref(
                        *atom,
                        self.limits,
                        self.counters,
                        self.location,
                    )?;
                    self.budget.charge(
                        ExpansionResource::TermWork,
                        atom.terms().len() as u128,
                        self.location,
                    )?;
                    for term in atom.terms() {
                        if let TemplateTerm::Variable(slot) = term {
                            ready[slot] = true;
                        }
                    }
                }
                LiteralIr::PatternAtom(pattern) => {
                    let flat = self.source.pattern_ref(
                        pattern.atom,
                        self.limits,
                        self.counters,
                        self.location,
                    )?;
                    let pattern = pattern.bind(flat);
                    self.budget.charge(
                        ExpansionResource::TermWork,
                        pattern.node_count() as u128,
                        self.location,
                    )?;
                    for slot in pattern.slots() {
                        ready[slot] = true;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn instruction_inputs(
        &mut self,
        literal: &LiteralIr,
        variables: usize,
    ) -> Result<Inputs, FormulaFailure> {
        if matches!(literal, LiteralIr::Aggregate(_)) {
            // The aggregate summary was compiled before head-only slots exist.
            // Its original body frame and required slot storage remain authoritative.
            return Ok(Inputs::Aggregate);
        }
        // Count before reserving exact logical capacity. Scalar head instructions
        // may read the complete body frame. Charge both scans of their expressions.
        let mut count = 0;
        for input in 0..variables {
            count += usize::from(self.instruction_uses(literal, input)?);
        }
        self.plan_storage::<usize>(count)?;
        let mut slots = Vec::with_capacity(count);
        for input in 0..variables {
            if self.instruction_uses(literal, input)? {
                slots.push(input);
            }
        }
        Ok(Inputs::Slots(slots))
    }

    fn instruction_uses(
        &mut self,
        literal: &LiteralIr,
        input: usize,
    ) -> Result<bool, FormulaFailure> {
        // Every scalar or range slot inspection consumes source work.
        self.scope_work(1)?;
        match literal {
            LiteralIr::Bind { value, .. } => self.expression_uses(value, input),
            LiteralIr::Range { lower, upper, .. } => {
                Ok(self.expression_uses(lower, input)? || self.expression_uses(upper, input)?)
            }
            _ => unreachable!("aggregate inputs borrow their existing summary"),
        }
    }

    fn dependency_failure(
        &mut self,
        pending: &[Option<Step>],
        body: &[LiteralIr],
        ready: &[bool],
        producers: &[Option<usize>],
    ) -> Result<FormulaFailure, FormulaFailure> {
        let mut blocked = None;
        for step in pending {
            self.scope_work(1)?;
            let Some(step) = step else { continue };
            for &variable in step.required(body) {
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
