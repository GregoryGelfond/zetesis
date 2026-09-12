//! Resolve complete eligible objective rows into fixed-priority templates.
//!
//! Conditions remain original-model queries after specialization: a possible
//! binding never establishes an active contribution. Priority, weight and tuple
//! always come from one binding. Fixed priorities retain the lifted evaluator;
//! resolved priorities retain at most one bounded template per eligible row.

use themelios_base::span::Location;
use zetesis_core::{AtomPattern, Term, Value};
use zetesis_objective::{AdmissionError, ObjectiveProgram, ObjectiveTemplate, WeightPolarity};

use super::scoped_body::{self, ValidatedBody};
use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_ir::{ObjectiveCondition, ObjectiveField, ObjectiveIr, Operation, Prepared};
use crate::formula_objective_dependencies::Presence;
use crate::formula_objective_dependencies::eligibility::{
    Activity, Context, SourceEligibility, model_condition,
};
use crate::formula_support::{self, CompletedSupport, Counters, Join, Support};
use crate::{ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource};

pub(super) fn prepare(
    prepared: &Prepared,
    completed: &CompletedSupport<'_>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: Location,
) -> Result<(ObjectiveProgram, Vec<Vec<Location>>), FormulaFailure> {
    let support = completed.relations();
    let presence = crate::formula_objective_dependencies::check_presence(
        prepared, support, limits, budget, counters,
    )?;
    let eligibility = if prepared
        .objectives
        .iter()
        .any(|objective| objective.needs_eligibility_query)
    {
        Some(SourceEligibility::build(
            prepared,
            completed,
            presence.retained_entries(),
            &mut Context {
                limits,
                budget,
                counters,
                location,
            },
        )?)
    } else {
        None
    };
    let mut preparation = Preparation {
        limits,
        budget,
        counters,
        templates: Vec::new(),
        origins: Vec::new(),
        eligibility: eligibility.as_ref(),
    };
    for objective in &prepared.objectives {
        let may_have_numeric_weight =
            presence.may_have_numeric_weight(objective, limits, preparation.counters)?;
        preparation.objective(objective, support, may_have_numeric_weight, &presence)?;
    }
    let program = if preparation.templates.is_empty() {
        ObjectiveProgram::none()
    } else {
        ObjectiveProgram::new(preparation.templates, limits.objective)
            .map_err(|error| FormulaFailure::Objective { error, location })?
    };
    Ok((program, preparation.origins))
}

struct Preparation<'a> {
    limits: &'a FormulaLimits,
    budget: &'a mut Budget,
    counters: &'a mut Counters,
    templates: Vec<ObjectiveTemplate>,
    origins: Vec<Vec<Location>>,
    eligibility: Option<&'a SourceEligibility>,
}

impl Preparation<'_> {
    fn objective(
        &mut self,
        objective: &ObjectiveIr,
        support: &Support,
        may_have_numeric_weight: bool,
        presence: &Presence<'_>,
    ) -> Result<(), FormulaFailure> {
        if objective.priority_sources.is_empty()
            && !objective.needs_eligibility_query
            && let [Operation::Constant(Value::Number(priority))] =
                objective.priority.nodes.as_slice()
            && objective.weight.term().is_some()
            && objective.tuple.iter().all(|field| field.term().is_some())
        {
            if may_have_numeric_weight && self.has_numeric_row(objective, support)? {
                self.retain(
                    objective,
                    objective
                        .template(*priority)
                        .expect("simple objective fields"),
                )?;
            }
            return Ok(());
        }
        let mut bindings = Join::objective(objective, support, self.budget)?;
        while let Some(row) =
            bindings.next_row(self.limits, self.budget, self.counters, objective.location)?
        {
            let binding = row.values;
            if !presence.eligible(objective, &binding, self.limits, self.counters)? {
                continue;
            }
            let body = if matches!(objective.condition, ObjectiveCondition::Body { .. }) {
                Some(scoped_body::validate(
                    objective.condition.literals(),
                    &binding,
                    support,
                    &mut Context {
                        limits: self.limits,
                        budget: self.budget,
                        counters: self.counters,
                        location: objective.location,
                    },
                )?)
            } else {
                None
            };
            if !row.passes {
                continue;
            }
            if objective.needs_eligibility_query {
                let eligibility = self
                    .eligibility
                    .expect("selected source eligibility prepared");
                let mut context = Context {
                    limits: self.limits,
                    budget: self.budget,
                    counters: self.counters,
                    location: objective.location,
                };
                if eligibility.activity(objective.condition.literals(), &binding, &mut context)?
                    == Activity::Absent
                {
                    continue;
                }
            }
            let priority = formula_support::expression(
                &objective.priority,
                &binding,
                self.limits,
                self.budget,
                self.counters,
                objective.location,
            )?;
            if let Value::Number(priority) = priority
                && may_have_numeric_weight
                && let Some(weight) = self.weight(objective, &binding)?
            {
                self.capacity(objective.location)?;
                let template = self.specialize(objective, &binding, weight, priority, body)?;
                self.retain(objective, template)?;
            }
        }
        Ok(())
    }

    fn has_numeric_row(
        &mut self,
        objective: &ObjectiveIr,
        support: &Support,
    ) -> Result<bool, FormulaFailure> {
        let mut bindings = Join::objective(objective, support, self.budget)?;
        let mut numeric = false;
        while let Some(binding) =
            bindings.next(self.limits, self.budget, self.counters, objective.location)?
        {
            if self.weight(objective, &binding)?.is_some() {
                numeric = true;
                // Maximization inspects every eligible row: a later MIN weight
                // must not be hidden by an earlier representable contribution.
                if objective.polarity == WeightPolarity::AsWritten {
                    break;
                }
            }
        }
        Ok(numeric)
    }

    fn weight(
        &mut self,
        objective: &ObjectiveIr,
        binding: &[Value],
    ) -> Result<Option<i32>, FormulaFailure> {
        let value = self.field(&objective.weight, binding, objective.location)?;
        let Value::Number(weight) = value else {
            return Ok(None);
        };
        objective.polarity.normalize(weight).ok_or_else(|| {
            crate::diagnostic::unsupported(
                crate::ProfileFeature::NumericOverflow,
                objective.location,
            )
        })?;
        Ok(Some(weight))
    }

    fn specialize(
        &mut self,
        objective: &ObjectiveIr,
        binding: &[Value],
        weight: i32,
        priority: i32,
        body: Option<ValidatedBody>,
    ) -> Result<ObjectiveTemplate, FormulaFailure> {
        let mut tuple = reserved(objective.tuple.len(), objective.location)?;
        for field in &objective.tuple {
            tuple.push(Term::Constant(self.field(
                field,
                binding,
                objective.location,
            )?));
        }
        if objective.needs_eligibility_query {
            let mut context = Context {
                limits: self.limits,
                budget: self.budget,
                counters: self.counters,
                location: objective.location,
            };
            let query = if let Some(body) = body {
                body.condition(&mut context)?
            } else {
                model_condition(objective.condition.literals(), binding, &mut context)?
            };
            return Ok(ObjectiveTemplate::new(
                Term::Constant(Value::Number(weight)),
                priority,
                tuple,
                Vec::new(),
                Vec::new(),
            )
            .with_weight_polarity(objective.polarity)
            .with_condition(query));
        }
        let mut positive = reserved(objective.positive.len(), objective.location)?;
        for atom in &objective.positive {
            self.counters.work(self.limits, objective.location)?;
            let terms = self.terms(atom.terms(), binding, objective.location)?;
            self.budget.charge(
                ExpansionResource::ScalarBytes,
                atom.predicate().name().len() as u128,
                objective.location,
            )?;
            positive.push(
                AtomPattern::new(atom.predicate().clone(), terms).expect("same source arity"),
            );
        }
        Ok(ObjectiveTemplate::new(
            Term::Constant(Value::Number(weight)),
            priority,
            tuple,
            positive,
            Vec::new(),
        )
        .with_weight_polarity(objective.polarity))
    }

    fn field(
        &mut self,
        field: &ObjectiveField,
        binding: &[Value],
        location: Location,
    ) -> Result<Value, FormulaFailure> {
        self.counters.work(self.limits, location)?;
        match field {
            ObjectiveField::Term(term) => formula_support::copy(
                term.resolve(binding).expect("safe objective field"),
                self.budget,
                location,
            ),
            ObjectiveField::Expression(expression) => formula_support::expression(
                expression,
                binding,
                self.limits,
                self.budget,
                self.counters,
                location,
            ),
        }
    }

    fn terms(
        &mut self,
        source: &[Term],
        binding: &[Value],
        location: Location,
    ) -> Result<Vec<Term>, FormulaFailure> {
        let mut result = reserved(source.len(), location)?;
        for term in source {
            self.counters.work(self.limits, location)?;
            result.push(Term::Constant(formula_support::copy(
                term.resolve(binding).expect("safe objective field"),
                self.budget,
                location,
            )?));
        }
        Ok(result)
    }

    fn capacity(&self, location: Location) -> Result<(), FormulaFailure> {
        ceiling(
            FormulaResource::ObjectiveElements,
            self.templates.len() as u128 + 1,
            self.limits.objective.max_templates as u128,
            location,
        )
    }

    fn retain(
        &mut self,
        objective: &ObjectiveIr,
        template: ObjectiveTemplate,
    ) -> Result<(), FormulaFailure> {
        self.capacity(objective.location)?;
        self.budget.charge(
            ExpansionResource::Origins,
            objective.origins.len() as u128,
            objective.location,
        )?;
        self.templates
            .try_reserve(1)
            .map_err(|_| allocation(objective.location))?;
        self.origins
            .try_reserve(1)
            .map_err(|_| allocation(objective.location))?;
        let mut origins = reserved(objective.origins.len(), objective.location)?;
        origins.extend_from_slice(&objective.origins);
        self.templates.push(template);
        self.origins.push(origins);
        Ok(())
    }
}

fn reserved<T>(count: usize, location: Location) -> Result<Vec<T>, FormulaFailure> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| allocation(location))?;
    Ok(result)
}

fn allocation(location: Location) -> FormulaFailure {
    FormulaFailure::Objective {
        error: AdmissionError::Allocation,
        location,
    }
}
