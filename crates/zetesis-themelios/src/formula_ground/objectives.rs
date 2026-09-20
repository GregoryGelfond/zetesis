//! Resolve complete eligible objective rows into fixed-priority templates.
//!
//! Conditions remain original-model queries after specialization: a possible
//! binding never establishes an active contribution. Priority, weight and tuple
//! always come from one binding. Fixed priorities retain the lifted evaluator;
//! resolved priorities retain at most one bounded template per eligible row.

use crate::formula_binding::Binding;

use themelios_base::span::Location;
use zetesis_core::{AtomPattern, Term, Value};
use zetesis_objective::{AdmissionError, ObjectiveProgram, ObjectiveTemplate};

use super::scoped_body::{self, ValidatedBody};
use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_ir::{ObjectiveCondition, ObjectiveField, ObjectiveIr, Operation, Prepared};
use crate::formula_objective_dependencies::Presence;
use crate::formula_source_activity::model_query::condition as model_condition;
use crate::formula_source_activity::{Activity, Context, SourceEligibility};
use crate::formula_support::family::{Evidence, Warnings};
use crate::formula_support::{self, CompletedQueries, Counters, Evaluation, Join, Support};
use crate::{ExpansionFailure, ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource};

pub(super) fn prepare(
    prepared: &Prepared,
    completed: &CompletedQueries<'_>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: Location,
    warnings: &mut Warnings,
) -> Result<(ObjectiveProgram, Vec<Vec<Location>>), FormulaFailure> {
    let support = completed.support();
    let presence = crate::formula_objective_dependencies::check_presence(
        prepared, support, limits, budget, counters,
    )?;
    let eligibility = if prepared
        .objectives
        .iter()
        .any(|objective| objective.needs_eligibility_query)
    {
        Some(SourceEligibility::build_from_conditions(
            prepared,
            completed,
            presence.retained_entries(),
            prepared
                .objectives
                .iter()
                .filter(|objective| objective.needs_eligibility_query)
                .map(|objective| objective.condition.literals()),
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
        warnings,
        evaluation: Evaluation::default(),
    };
    // The source compiler emits each original element's alternatives together.
    // A constant-size accumulator preserves that family without merging other
    // elements sharing the same enclosing directive's diagnostic location.
    for fragments in prepared
        .objectives
        .chunk_by(|left, right| left.family == right.family)
    {
        let mut family = Evidence::default();
        for objective in fragments {
            let may_have_numeric_weight =
                presence.may_have_numeric_weight(objective, limits, preparation.counters)?;
            family.merge(preparation.objective(
                objective,
                support,
                may_have_numeric_weight,
                &presence,
            )?);
        }
        preparation.warnings.family(
            family,
            limits,
            preparation.counters,
            fragments
                .first()
                .expect("a family chunk is nonempty")
                .location,
        )?;
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
    warnings: &'a mut Warnings,
    evaluation: Evaluation,
}

/// The complete scalar fields of one defined, numeric objective instance.
/// All independent fields were checked before this owner was constructed.
struct Resolved {
    weight: i32,
    priority: i32,
    tuple: Vec<Term>,
}

/// Arithmetic evidence for one condition row. An independently false scalar
/// condition excludes any erroneous fields, but those fields cannot witness
/// that the complete objective instance is defined.
struct Fields {
    selected: bool,
    failure: Option<ExpansionFailure>,
}

impl Preparation<'_> {
    fn objective(
        &mut self,
        objective: &ObjectiveIr,
        support: &Support,
        may_have_numeric_weight: bool,
        presence: &Presence<'_>,
    ) -> Result<Evidence, FormulaFailure> {
        if objective.priority_sources.is_empty()
            && !objective.needs_eligibility_query
            && let [Operation::Constant(Value::Number(priority))] =
                objective.priority.nodes.as_slice()
            && objective.weight.term().is_some()
            && objective.tuple.iter().all(|field| field.term().is_some())
        {
            let (numeric, family) = self.numeric_rows(objective, support)?;
            if may_have_numeric_weight && numeric {
                self.retain(
                    objective,
                    objective
                        .template(*priority)
                        .expect("simple objective fields"),
                )?;
            }
            return Ok(family);
        }
        let mut bindings = Join::objective(objective, support, self.budget)?;
        bindings.evidence();
        let mut family = Evidence::default();
        while let Some(row) =
            bindings.next_row(self.limits, self.budget, self.counters, objective.location)?
        {
            let binding = row.values;
            if row.passes {
                self.validate_scopes(objective, &binding, support)?;
            }
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
                let _ = self.resolve(objective, &binding, false, &mut family)?;
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
            if let Some(resolved) = self.resolve(objective, &binding, true, &mut family)?
                && may_have_numeric_weight
            {
                self.capacity(objective.location)?;
                let template = self.specialize(objective, &binding, resolved, body)?;
                self.retain(objective, template)?;
            }
        }
        // A condition-only witness cannot rescue an instance whose scalar
        // fields were all undefined. Keep only the join's zero-divisor evidence.
        let joined = bindings.take_family();
        if family.zero.is_none() {
            family.zero = joined.zero;
        }
        Ok(family)
    }

    fn numeric_rows(
        &mut self,
        objective: &ObjectiveIr,
        support: &Support,
    ) -> Result<(bool, Evidence), FormulaFailure> {
        let mut bindings = Join::objective(objective, support, self.budget)?;
        bindings.evidence();
        let mut numeric = false;
        let mut family = Evidence::default();
        while let Some(row) =
            bindings.next_row(self.limits, self.budget, self.counters, objective.location)?
        {
            if row.passes {
                self.validate_scopes(objective, &row.values, support)?;
            }
            let value = self.field(&objective.weight, &row.values, objective.location)?;
            if let Value::Number(weight) = value {
                if objective.polarity.normalize(weight).is_none() {
                    if !row.passes {
                        continue;
                    }
                    return Err(crate::diagnostic::unsupported(
                        crate::ProfileFeature::NumericOverflow,
                        objective.location,
                    )
                    .into());
                }
                numeric |= row.passes;
            }
            family.defined = true;
        }
        family.zero = bindings.take_family().zero;
        Ok((numeric, family))
    }

    fn validate_scopes(
        &mut self,
        objective: &ObjectiveIr,
        binding: &Binding,
        support: &Support,
    ) -> Result<(), FormulaFailure> {
        super::arithmetic::body(
            (objective.condition.literals(), objective.location),
            binding,
            support,
            self.limits,
            self.budget,
            self.counters,
            self.warnings,
        )
    }

    fn resolve(
        &mut self,
        objective: &ObjectiveIr,
        binding: &Binding,
        selected: bool,
        family: &mut Evidence,
    ) -> Result<Option<Resolved>, FormulaFailure> {
        let mut fields = Fields {
            selected,
            failure: None,
        };
        let priority = self.evaluate(
            &objective.priority,
            binding,
            objective.location,
            &mut fields,
        )?;
        let weight =
            self.resolve_field(&objective.weight, binding, objective.location, &mut fields)?;
        let mut tuple = reserved(objective.tuple.len(), objective.location)?;
        for field in &objective.tuple {
            if let Some(value) =
                self.resolve_field(field, binding, objective.location, &mut fields)?
            {
                tuple.push(Term::Constant(value));
            }
        }
        // Polarity normalization belongs only to numeric contributions;
        // required authored expressions above are checked for every row.
        if matches!(&priority, Some(Value::Number(_)))
            && let Some(Value::Number(weight)) = &weight
            && objective.polarity.normalize(*weight).is_none()
        {
            if !selected {
                return Ok(None);
            }
            return Err(crate::diagnostic::unsupported(
                crate::ProfileFeature::NumericOverflow,
                objective.location,
            )
            .into());
        }
        if fields.failure.is_some() {
            if selected && family.zero.is_none() {
                family.zero = fields.failure;
            }
            return Ok(None);
        }
        let Some(Value::Number(weight)) = weight else {
            family.defined = true;
            return Ok(None);
        };
        family.defined = true;
        let Some(Value::Number(priority)) = priority else {
            return Ok(None);
        };
        Ok(Some(Resolved {
            weight,
            priority,
            tuple,
        }))
    }

    fn resolve_field(
        &mut self,
        field: &ObjectiveField,
        binding: &Binding,
        location: Location,
        fields: &mut Fields,
    ) -> Result<Option<Value>, FormulaFailure> {
        self.counters.work(self.limits, location)?;
        match field {
            ObjectiveField::Term(term) => {
                formula_support::copy(binding.resolve(term, location)?, self.budget, location)
                    .map(Some)
            }
            ObjectiveField::Expression(expression) => {
                self.evaluate(expression, binding, location, fields)
            }
        }
    }

    /// Classify only the evaluator's typed numeric zero-divisor cause. Continue
    /// through the row's other fields so a skipped instance cannot hide a fatal
    /// overflow, type error or undefined power in an independent expression.
    fn evaluate(
        &mut self,
        expression: &crate::formula_ir::Expression,
        binding: &Binding,
        location: Location,
        fields: &mut Fields,
    ) -> Result<Option<Value>, FormulaFailure> {
        let result = self.evaluation.source_expression(
            expression,
            |variable| binding.read(variable, location),
            self.limits,
            self.budget,
            self.counters,
            location,
        );
        match result {
            Ok(value) => Ok(Some(value)),
            Err(FormulaFailure::Expansion(error @ ExpansionFailure::Evaluation { .. }))
                if self.evaluation.zero_divisor() || !fields.selected =>
            {
                if fields.failure.is_none() {
                    fields.failure = Some(error);
                }
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    fn specialize(
        &mut self,
        objective: &ObjectiveIr,
        binding: &Binding,
        resolved: Resolved,
        body: Option<ValidatedBody>,
    ) -> Result<ObjectiveTemplate, FormulaFailure> {
        let Resolved {
            weight,
            priority,
            tuple,
        } = resolved;
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
        binding: &Binding,
        location: Location,
    ) -> Result<Value, FormulaFailure> {
        self.counters.work(self.limits, location)?;
        match field {
            ObjectiveField::Term(term) => {
                formula_support::copy(binding.resolve(term, location)?, self.budget, location)
            }
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
        binding: &Binding,
        location: Location,
    ) -> Result<Vec<Term>, FormulaFailure> {
        let mut result = reserved(source.len(), location)?;
        for term in source {
            self.counters.work(self.limits, location)?;
            result.push(Term::Constant(formula_support::copy(
                binding.resolve(term, location)?,
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
