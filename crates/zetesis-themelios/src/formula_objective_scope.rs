//! Scoped optimization observations reuse rule bodies without semantic producers.
//!
//! Its compiler has a private empty value domain. Objective-only constants and
//! aggregate tuples therefore never enlarge the original program's universe.

use themelios_program::program::{
    Body, BodyElement, Condition, DefaultNegation, LiteralInner, OptimizeElement, Statement,
    WeakConstraint, Weight,
};
#[path = "formula_objective_scope/selection.rs"]
mod selection;
use themelios_program::provenance::{Origin, WithProvenance};
use themelios_program::term::Term;

use super::{Compiler, LiteralIr, ObjectiveCondition, ObjectiveIr, Variables};
use crate::{ExpansionResource, FormulaFailure};
use std::collections::BTreeSet;
use themelios_base::span::Location;
use zetesis_objective::WeightPolarity;

impl Compiler<'_> {
    pub(super) fn objective_statement(
        &mut self,
        statement: &WithProvenance<Statement>,
        origins: &[Location],
        objectives: &mut Vec<ObjectiveIr>,
        declarations: &mut Vec<Location>,
        projection_nodes: &mut u128,
    ) -> Result<Option<Vec<WithProvenance<Statement>>>, FormulaFailure> {
        if let Statement::WeakConstraint(weak) = statement.get()
            && selection::scoped(weak, self.budget, self.location)?
        {
            let analyzed = self.conditional_projection(statement, projection_nodes)?;
            self.weak_objective(weak, origins, objectives, declarations)?;
            return Ok(Some(analyzed));
        }
        let normalized = crate::formula_weak::normalize(statement, self.budget, self.location)?;
        let statement = normalized.as_ref().unwrap_or(statement);
        if let Statement::Optimize(optimize) = statement.get() {
            self.objectives(
                optimize,
                origins,
                &crate::extended::parsed_origins(statement),
                objectives,
                declarations,
            )?;
            return Ok(Some(
                self.conditional_projection(statement, projection_nodes)?,
            ));
        }
        Ok(None)
    }

    pub(super) fn weak_objective(
        &mut self,
        weak: &WeakConstraint,
        origins: &[Location],
        objectives: &mut Vec<ObjectiveIr>,
        declarations: &mut Vec<Location>,
    ) -> Result<(), FormulaFailure> {
        let body_origins = || {
            weak.body().provenance().origins().chain(
                weak.body()
                    .get()
                    .elements()
                    .flat_map(|element| element.provenance().origins()),
            )
        };
        self.budget.charge(
            ExpansionResource::Origins,
            (origins.len() as u128)
                .saturating_mul(2)
                .saturating_add(body_origins().count() as u128),
            self.location,
        )?;
        let mut evidence = Vec::new();
        evidence
            .try_reserve_exact(origins.len().saturating_add(body_origins().count()))
            .map_err(|_| FormulaFailure::Objective {
                error: zetesis_objective::AdmissionError::Allocation,
                location: self.location,
            })?;
        evidence.extend_from_slice(origins);
        for origin in weak.body().provenance().origins().chain(
            weak.body()
                .get()
                .elements()
                .flat_map(|element| element.provenance().origins()),
        ) {
            if let Origin::Parsed(location) = origin {
                evidence.push(*location);
            }
        }
        evidence.sort_unstable();
        evidence.dedup();
        let fields: Vec<_> = std::iter::once(weak.weight().term())
            .chain(weak.weight().priority())
            .chain(weak.terms())
            .collect();
        declarations.extend_from_slice(origins);
        for body in self.body_alternatives(weak.body().get())? {
            let alternatives = self.local_alternatives(&fields, &Condition::new([]))?;
            for (terms, _) in &alternatives {
                let mut terms = terms.iter();
                let mut weight =
                    themelios_program::program::weight(terms.next().expect("weight field").clone());
                if weak.weight().priority().is_some() {
                    weight = weight.at_priority(terms.next().expect("priority field").clone());
                }
                self.budget.charge(
                    ExpansionResource::Origins,
                    evidence.len() as u128,
                    self.location,
                )?;
                let objective = self.scoped_objective(
                    &body,
                    &weight,
                    terms,
                    evidence.clone(),
                    WeightPolarity::AsWritten,
                )?;
                objectives.push(objective);
            }
        }
        Ok(())
    }

    pub(super) fn scoped_element(
        &mut self,
        element: &OptimizeElement,
        origins: Vec<Location>,
        polarity: WeightPolarity,
    ) -> Result<ObjectiveIr, FormulaFailure> {
        let body = crate::formula_weak::body(element.condition(), self.budget, self.location)?;
        self.scoped_objective(&body, element.weight(), element.terms(), origins, polarity)
    }

    pub(super) fn element_needs_scope(
        &mut self,
        element: &OptimizeElement,
    ) -> Result<bool, FormulaFailure> {
        for term in std::iter::once(element.weight().term())
            .chain(element.weight().priority())
            .chain(element.terms())
            .flat_map(Term::subterms)
        {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            if matches!(term, Term::Interval { .. }) {
                return Ok(true);
            }
        }
        selection::condition(element.condition(), self.budget, self.location)
    }

    fn scoped_objective<'source>(
        &mut self,
        source: &Body,
        weight: &Weight,
        terms: impl Iterator<Item = &'source Term>,
        origins: Vec<Location>,
        polarity: WeightPolarity,
    ) -> Result<ObjectiveIr, FormulaFailure> {
        let mut compiler = Compiler {
            options: self.options,
            limits: self.limits,
            budget: self.budget,
            domain: BTreeSet::new(),
            predicates: BTreeSet::new(),
            next_aggregate: self.next_aggregate,
            dependency_projection: false,
            location: self.location,
        };
        let objective = compiler.scoped_body(source, weight, terms, origins, polarity)?;
        self.next_aggregate = compiler.next_aggregate;
        self.dependency_projection |= compiler.dependency_projection;
        Ok(objective)
    }

    fn scoped_field(
        &mut self,
        term: &Term,
        variables: &mut Variables,
        body: &mut Vec<LiteralIr>,
    ) -> Result<super::ObjectiveField, FormulaFailure> {
        if term
            .subterms()
            .any(|term| matches!(term, Term::Interval { .. }))
        {
            return self
                .ranged_expression(term, variables, body)
                .map(super::ObjectiveField::Expression);
        }
        self.objective_field(term, variables)
    }

    fn scoped_body<'source>(
        &mut self,
        source: &Body,
        source_weight: &Weight,
        terms: impl Iterator<Item = &'source Term>,
        origins: Vec<Location>,
        polarity: WeightPolarity,
    ) -> Result<ObjectiveIr, FormulaFailure> {
        let mut variables = Variables::default();
        // Fields belong to the outer scope, even when only a local aggregate
        // mentions the same name. Local conditions cannot establish their safety.
        let mut body = Vec::new();
        let weight = self.scoped_field(source_weight.term(), &mut variables, &mut body)?;
        let priority = match source_weight.priority() {
            Some(term)
                if term
                    .subterms()
                    .any(|term| matches!(term, Term::Interval { .. })) =>
            {
                self.ranged_expression(term, &mut variables, &mut body)?
            }
            other => self.objective_priority(other, &mut variables)?,
        };
        let tuple = terms
            .map(|term| self.scoped_field(term, &mut variables, &mut body))
            .collect::<Result<Vec<_>, _>>()?;
        self.body_literals(source, &mut variables, &mut body)?;
        let guards = self.body_guards(source, &mut variables, &mut body)?;
        let assignments = self.assignment_targets(source, &guards, &mut variables)?;
        self.bindings(&mut body, &mut variables)?;
        variables.safety(self.location)?;
        self.body_aggregates(source, guards, assignments, &variables, &mut body)?;
        self.body_conditionals(source, &variables, &mut body)?;
        let bindings = self.assignment_plan(&body, variables.count, variables.count, &[])?;
        self.variable_limit(&variables)?;
        let positive = body
            .iter()
            .filter_map(|literal| match literal {
                LiteralIr::Atom(DefaultNegation::None, atom) => Some(atom.clone()),
                LiteralIr::PatternAtom(pattern) => Some(pattern.atom.clone()),
                _ => None,
            })
            .collect();
        Ok(ObjectiveIr {
            weight,
            priority,
            tuple,
            positive,
            filters: Vec::new(),
            polarity,
            priority_sources: BTreeSet::new(),
            needs_eligibility_query: true,
            condition: ObjectiveCondition::Body {
                literals: body,
                bindings,
                filters: source.elements().filter(|element| matches!(element.get(), BodyElement::Literal(literal) if matches!(literal.inner, LiteralInner::Comparison(_)))).count(),
            },
            variables: variables.count,
            origins,
            location: self.location,
        })
    }
}
