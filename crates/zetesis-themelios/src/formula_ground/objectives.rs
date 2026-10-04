//! Resolve complete eligible objective rows into fixed-priority templates.
//!
//! Conditions remain original-model queries after specialization: a possible
//! binding never establishes an active contribution. Priority, weight and tuple
//! always come from one binding. Fixed priorities retain the lifted evaluator;
//! resolved priorities retain at most one bounded template per eligible row.

mod components;
mod rows;
use crate::formula_support::{Context, GroundingWork};

use crate::formula_binding::Binding;
use components::Components;
use rows::Rows;

use crate::ProgramSite;
use zetesis_core::ValueNodeRef;
use zetesis_core::catalog::{TermKey, TermRef};
use zetesis_objective::{AdmissionError, ObjectiveElement, ObjectiveProgram};

use super::scoped_body::{self, ValidatedBody};
use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_ir::{ObjectiveCondition, ObjectiveField, ObjectiveIr, Operation, Prepared};
use crate::formula_objective_dependencies::Presence;
use crate::formula_source_activity::model_query::{PendingCondition, condition as model_condition};
use crate::formula_source_activity::{Activity, Context as ActivityContext, SourceEligibility};
use crate::formula_support::family::{Evidence, Warnings};
use crate::formula_support::{
    self, CompletedQueries, Computation, Counters, Evaluation, Join, Publication, Support,
};
use crate::{ExpansionFailure, ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource};

pub(super) fn prepare<'source>(
    prepared: &Prepared,
    completed: &CompletedQueries<'source>,
    budget: &mut Budget,
    warnings: &mut Warnings,
    context: Context<'_, &mut Computation<'_, 'source>>,
) -> Result<PendingProgram, FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    let support = completed.support();
    let presence = crate::formula_objective_dependencies::check_presence(
        prepared,
        support,
        computation,
        limits,
        budget,
        counters,
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
            &mut ActivityContext {
                computation,
                limits,
                budget,
                counters,
                location,
            },
        )?)
    } else {
        None
    };
    let rows = Rows::new(computation, limits, counters, location)?;
    let mut preparation = Preparation {
        computation,
        rows,
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
            let may_have_numeric_weight = presence.may_have_numeric_weight(
                objective,
                preparation.computation,
                limits,
                preparation.counters,
            )?;
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
    Ok(PendingProgram {
        rows: preparation.rows,
        templates: preparation.templates,
        origins: preparation.origins,
    })
}

/// Canonical template coordinates and conditions wait for the final source
/// prefix. Neither description carries copied typed terms or atom payload.
pub(super) struct PendingProgram {
    rows: Rows,
    templates: Vec<PendingElement>,
    origins: Vec<Vec<ProgramSite>>,
}
struct PendingElement {
    row: usize,
    priority: i32,
    polarity: zetesis_objective::WeightPolarity,
    condition: Option<PendingCondition>,
}
impl PendingProgram {
    pub(super) fn publish(
        self,
        atoms: &zetesis_core::AtomCatalog,
        publication: &mut Publication<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(ObjectiveProgram, Vec<Vec<ProgramSite>>), FormulaFailure> {
        if self.templates.is_empty() {
            return Ok((ObjectiveProgram::none(), self.origins));
        }
        let catalog = self
            .rows
            .publish(atoms.clone(), publication, limits, counters, location)?;
        let mut elements = reserved(self.templates.len(), location)?;
        for template in self.templates {
            let mut element = ObjectiveElement::new(template.row, template.priority)
                .with_weight_polarity(template.polarity);
            if let Some(condition) = template.condition {
                element = element.with_condition(condition.publish(
                    publication,
                    limits,
                    counters,
                    location,
                )?);
            }
            elements.push(element);
        }
        let program = ObjectiveProgram::from_catalog(catalog, elements, limits.objective)
            .map_err(|error| FormulaFailure::Objective { error, location })?;
        Ok((program, self.origins))
    }
}

struct Preparation<'a, 'terms, 'source> {
    computation: &'a mut Computation<'terms, 'source>,
    rows: Rows,
    limits: &'a FormulaLimits,
    budget: &'a mut Budget,
    counters: &'a mut Counters,
    templates: Vec<PendingElement>,
    origins: Vec<Vec<ProgramSite>>,
    eligibility: Option<&'a SourceEligibility>,
    warnings: &'a mut Warnings,
    evaluation: Evaluation,
}

/// The complete scalar fields of one defined, numeric objective instance.
/// All independent fields were checked before this owner was constructed.
struct Resolved {
    weight: i32,
    priority: i32,
    tuple: Binding<'static>,
}

/// Arithmetic evidence for one condition row. An independently false scalar
/// condition excludes any erroneous fields, but those fields cannot witness
/// that the complete objective instance is defined.
struct Fields {
    selected: bool,
    failure: Option<ExpansionFailure>,
}

impl<'source> Preparation<'_, '_, 'source> {
    fn objective(
        &mut self,
        objective: &ObjectiveIr,
        support: &Support<'source>,
        may_have_numeric_weight: bool,
        presence: &Presence<'_>,
    ) -> Result<Evidence, FormulaFailure> {
        if objective.priority_sources.is_empty()
            && !objective.needs_eligibility_query
            && let [Operation::Constant(priority)] = objective.priority.nodes.as_slice()
            && let ValueNodeRef::Number(priority) = self
                .computation
                .static_scalar(*priority, self.limits, self.counters, objective.location)?
                .descriptor()
            && objective.weight.term().is_some()
            && objective.tuple.iter().all(|field| field.term().is_some())
        {
            return self.lifted_objective(objective, support, may_have_numeric_weight, priority);
        }
        let mut bindings = Join::objective(
            objective,
            support,
            self.computation,
            self.limits,
            self.budget,
            self.counters,
        )?;
        bindings.evidence();
        let mut family = Evidence::default();
        while let Some(row) = bindings.next_row(
            self.computation,
            self.limits,
            self.budget,
            self.counters,
            objective.location,
        )? {
            let binding = row.values;
            if row.passes {
                self.validate_scopes(objective, &binding, support)?;
            }
            if !presence.eligible(
                objective,
                &binding,
                self.computation,
                self.limits,
                self.counters,
            )? {
                continue;
            }
            let body = if matches!(objective.condition, ObjectiveCondition::Body { .. }) {
                Some(scoped_body::validate(
                    objective.condition.literals(),
                    &binding,
                    support,
                    &mut ActivityContext {
                        computation: self.computation,
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
                let mut context = ActivityContext {
                    computation: self.computation,
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

    fn lifted_objective(
        &mut self,
        objective: &ObjectiveIr,
        support: &Support<'source>,
        may_have_numeric_weight: bool,
        priority: i32,
    ) -> Result<Evidence, FormulaFailure> {
        let (numeric, family) = self.numeric_rows(objective, support)?;
        if may_have_numeric_weight && numeric {
            let mut components = Components::new(
                self.computation,
                self.limits,
                self.counters,
                objective.location,
            )?;
            components.lifted_scalar(
                objective.weight.term().expect("simple weight"),
                self.computation,
                self.limits,
                self.counters,
                objective.location,
            )?;
            for field in &objective.tuple {
                components.lifted_scalar(
                    field.term().expect("simple tuple field"),
                    self.computation,
                    self.limits,
                    self.counters,
                    objective.location,
                )?;
            }
            for pattern in &objective.positive {
                components.pattern(
                    *pattern,
                    None,
                    self.computation,
                    self.limits,
                    self.counters,
                    objective.location,
                )?;
            }
            for filter in &objective.filters {
                components.filter(
                    *filter,
                    self.computation,
                    self.limits,
                    self.counters,
                    objective.location,
                )?;
            }
            let row = components.append(
                &mut self.rows,
                self.computation,
                self.limits,
                self.counters,
                objective.location,
            )?;
            self.retain(
                objective,
                PendingElement {
                    row,
                    priority,
                    polarity: objective.polarity,
                    condition: None,
                },
            )?;
        }
        Ok(family)
    }

    fn numeric_rows(
        &mut self,
        objective: &ObjectiveIr,
        support: &Support<'source>,
    ) -> Result<(bool, Evidence), FormulaFailure> {
        let mut bindings = Join::objective(
            objective,
            support,
            self.computation,
            self.limits,
            self.budget,
            self.counters,
        )?;
        bindings.evidence();
        let mut numeric = false;
        let mut family = Evidence::default();
        while let Some(row) = bindings.next_row(
            self.computation,
            self.limits,
            self.budget,
            self.counters,
            objective.location,
        )? {
            if row.passes {
                self.validate_scopes(objective, &row.values, support)?;
            }
            let value = self.field(&objective.weight, &row.values, objective.location)?;
            if let ValueNodeRef::Number(weight) =
                self.value(&value, objective.location)?.descriptor()
            {
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
        support: &Support<'source>,
    ) -> Result<(), FormulaFailure> {
        super::arithmetic::body(
            objective.condition.literals(),
            binding,
            support,
            self.budget,
            self.warnings,
            Context::new(
                &mut *self.computation,
                self.limits,
                self.counters,
                objective.location,
            ),
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
        let mut tuple = Binding::new(
            self.computation,
            self.limits,
            self.counters,
            objective.location,
        )?;
        tuple.extend_scope(
            objective.tuple.len(),
            self.computation,
            self.limits,
            self.counters,
            objective.location,
        )?;
        for (slot, field) in objective.tuple.iter().enumerate() {
            if let Some(value) =
                self.resolve_field(field, binding, objective.location, &mut fields)?
            {
                tuple.set(slot, &value, self.limits, self.counters, objective.location)?;
            }
        }
        // Polarity normalization belongs only to numeric contributions;
        // required authored expressions above are checked for every row.
        let priority_number = priority
            .as_ref()
            .map(|key| self.number(key, objective.location))
            .transpose()?
            .flatten();
        let weight_number = weight
            .as_ref()
            .map(|key| self.number(key, objective.location))
            .transpose()?
            .flatten();
        if priority_number.is_some()
            && let Some(weight) = weight_number
            && objective.polarity.normalize(weight).is_none()
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
        let Some(weight) = weight_number else {
            family.defined = true;
            return Ok(None);
        };
        family.defined = true;
        let Some(priority) = priority_number else {
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
        location: ProgramSite,
        fields: &mut Fields,
    ) -> Result<Option<TermKey>, FormulaFailure> {
        self.counters.work(self.limits, location)?;
        match field {
            ObjectiveField::Term(term) => self.term(term, binding, location).map(Some),
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
        location: ProgramSite,
        fields: &mut Fields,
    ) -> Result<Option<TermKey>, FormulaFailure> {
        let result = self.evaluation.source_expression(
            expression,
            |variable| binding.key(variable, location),
            self.computation,
            self.limits,
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
    ) -> Result<PendingElement, FormulaFailure> {
        let Resolved {
            weight,
            priority,
            tuple,
        } = resolved;
        let mut components = Components::new(
            self.computation,
            self.limits,
            self.counters,
            objective.location,
        )?;
        let weight =
            self.computation
                .number(weight, self.limits, self.counters, objective.location)?;
        components.scalar(
            &weight,
            self.computation,
            self.limits,
            self.counters,
            objective.location,
        )?;
        for slot in 0..tuple.len() {
            let key = tuple.key(slot, objective.location)?;
            components.scalar(
                &key,
                self.computation,
                self.limits,
                self.counters,
                objective.location,
            )?;
        }
        let condition = if objective.needs_eligibility_query {
            let mut context = ActivityContext {
                computation: self.computation,
                limits: self.limits,
                budget: self.budget,
                counters: self.counters,
                location: objective.location,
            };
            Some(if let Some(body) = body {
                body.condition(&mut context)?
            } else {
                model_condition(objective.condition.literals(), binding, &mut context)?
            })
        } else {
            for atom in &objective.positive {
                components.pattern(
                    *atom,
                    Some(binding),
                    self.computation,
                    self.limits,
                    self.counters,
                    objective.location,
                )?;
            }
            None
        };
        let row = components.append(
            &mut self.rows,
            self.computation,
            self.limits,
            self.counters,
            objective.location,
        )?;
        Ok(PendingElement {
            row,
            priority,
            polarity: objective.polarity,
            condition,
        })
    }

    fn field(
        &mut self,
        field: &ObjectiveField,
        binding: &Binding,
        location: ProgramSite,
    ) -> Result<TermKey, FormulaFailure> {
        self.counters.work(self.limits, location)?;
        match field {
            ObjectiveField::Term(term) => self.term(term, binding, location),
            ObjectiveField::Expression(expression) => formula_support::expression(
                expression,
                binding,
                self.computation,
                self.limits,
                self.counters,
                location,
            ),
        }
    }
    fn term(
        &mut self,
        term: &crate::formula_support::components::Term,
        binding: &Binding,
        location: ProgramSite,
    ) -> Result<TermKey, FormulaFailure> {
        match term {
            crate::formula_support::components::Term::Variable(slot) => {
                binding.key(*slot, location)
            }
            crate::formula_support::components::Term::Constant(value) => self
                .computation
                .static_key(*value, self.limits, self.counters, location),
        }
    }
    fn value(&self, key: &TermKey, location: ProgramSite) -> Result<TermRef<'_>, FormulaFailure> {
        self.computation.read().term(key).map_err(|error| {
            crate::formula_binding::assignment(
                zetesis_core::catalog::AssignmentError::Read(error),
                location,
            )
        })
    }
    fn number(&self, key: &TermKey, location: ProgramSite) -> Result<Option<i32>, FormulaFailure> {
        Ok(match self.value(key, location)?.descriptor() {
            ValueNodeRef::Number(number) => Some(number),
            _ => None,
        })
    }

    fn capacity(&self, location: ProgramSite) -> Result<(), FormulaFailure> {
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
        template: PendingElement,
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

fn reserved<T>(count: usize, location: ProgramSite) -> Result<Vec<T>, FormulaFailure> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| allocation(location))?;
    Ok(result)
}

fn allocation(location: ProgramSite) -> FormulaFailure {
    FormulaFailure::Objective {
        error: AdmissionError::Allocation,
        location,
    }
}
