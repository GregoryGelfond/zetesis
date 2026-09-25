//! Scoped finite aggregate tuples and guards, independent of propositional lowering.

use themelios_program::program::{
    Aggregate, AggregateFunction, Body, BodyElement, Choice, Condition, DefaultNegation, Guard,
    HasGuards, Relation, SetElement,
};
use themelios_program::provenance::WithProvenance;
use themelios_program::symbol::Symbol;
use themelios_program::term::Term;

use crate::diagnostic::unsupported;
use crate::formula_ir::{
    AggregateElementIr, AggregateGuard, AggregateIr, AggregateKey, Compiler, Expression, LiteralIr,
    LocalFamily, Operation, Variables,
};
use crate::formula_support::components::Term as CoreTerm;
use crate::{AdmissionFailure, FormulaFailure, InputLimit, ProfileFeature};
use zetesis_core::Value;

impl Compiler<'_> {
    pub(super) fn condition(
        &mut self,
        condition: &Condition,
        variables: &mut Variables,
    ) -> Result<Vec<LiteralIr>, FormulaFailure> {
        let mut result = Vec::new();
        for (index, literal) in condition.literals().enumerate() {
            if index >= self.options.max_body_elements {
                return Err(AdmissionFailure::Limit {
                    resource: InputLimit::BodyElements,
                    limit: self.options.max_body_elements,
                    observed: index + 1,
                    location: self.location,
                }
                .into());
            }
            self.literal_into(literal.get(), variables, &mut result)?;
        }
        Ok(result)
    }
    pub(super) fn comparison(
        &mut self,
        left: &Term,
        relation: Relation,
        right: &Term,
        variables: &mut Variables,
    ) -> Result<LiteralIr, FormulaFailure> {
        // Closed tuples are ordinary complete values. Tuple destructuring is
        // retained only when a syntactic tuple still contains local variables.
        if matches!(left, Term::Tuple(_)) || matches!(right, Term::Tuple(_)) {
            if !matches!(relation, Relation::Eq | Relation::Neq) {
                return Err(unsupported(ProfileFeature::Term, self.location).into());
            }
            Ok(LiteralIr::TupleCompare(
                self.tuple_expressions(left, variables)?,
                relation,
                self.tuple_expressions(right, variables)?,
            ))
        } else {
            Ok(LiteralIr::Compare(
                self.expression(left, variables)?,
                relation,
                self.expression(right, variables)?,
            ))
        }
    }
    fn tuple_expressions(
        &mut self,
        term: &Term,
        variables: &mut Variables,
    ) -> Result<Vec<Expression>, FormulaFailure> {
        match term {
            Term::Tuple(terms) => terms
                .iter()
                .map(|term| self.expression(term, variables))
                .collect(),
            Term::Symbolic(Symbol::Tuple(values)) => values
                .iter()
                .map(|value| self.expression(&Term::Symbolic(value.clone()), variables))
                .collect(),
            _ => Err(unsupported(ProfileFeature::Term, self.location).into()),
        }
    }
    pub(super) fn choice_guards(
        &mut self,
        choice: &Choice,
        variables: &mut Variables,
        values: &mut Vec<LiteralIr>,
    ) -> Result<Vec<AggregateGuard>, FormulaFailure> {
        self.guards(
            choice.left_guard().map(WithProvenance::get),
            choice.right_guard().map(WithProvenance::get),
            variables,
            values,
        )
    }
    pub(super) fn guards(
        &mut self,
        left: Option<&Guard>,
        right: Option<&Guard>,
        variables: &mut Variables,
        values: &mut Vec<LiteralIr>,
    ) -> Result<Vec<AggregateGuard>, FormulaFailure> {
        let mut guards = Vec::new();
        if let Some(left) = left {
            guards.push(AggregateGuard {
                relation: reverse(left.relation.unwrap_or(Relation::Le)),
                bound: self.aggregate_expression(&left.term, variables, values)?,
            });
        }
        if let Some(right) = right {
            guards.push(AggregateGuard {
                relation: right.relation.unwrap_or(Relation::Le),
                bound: self.aggregate_expression(&right.term, variables, values)?,
            });
        }
        Ok(guards)
    }
    fn aggregate_expression(
        &mut self,
        term: &Term,
        variables: &mut Variables,
        values: &mut Vec<LiteralIr>,
    ) -> Result<Expression, FormulaFailure> {
        if let Some(value) = sentinel(term) {
            let key = self.value(&value)?;
            return Ok(Expression {
                nodes: vec![Operation::Constant(self.source.scalar_key(
                    &key,
                    self.limits,
                    self.counters,
                    self.location,
                )?)],
            });
        }
        self.ranged_expression(term, variables, values)
    }
    pub(super) fn body_guards(
        &mut self,
        source: &Body,
        variables: &mut Variables,
        values: &mut Vec<LiteralIr>,
    ) -> Result<Vec<Vec<AggregateGuard>>, FormulaFailure> {
        let mut result = Vec::new();
        for element in source.elements() {
            if let BodyElement::Aggregate { aggregate, .. } = element.get() {
                let guarded: &dyn HasGuards = match aggregate {
                    Aggregate::Function(value) => value,
                    Aggregate::Set(value) => value,
                };
                result.push(self.guards(
                    guarded.left_guard().map(WithProvenance::get),
                    guarded.right_guard().map(WithProvenance::get),
                    variables,
                    values,
                )?);
            }
        }
        Ok(result)
    }
    pub(super) fn body_aggregates(
        &mut self,
        source: &Body,
        guards: Vec<Vec<AggregateGuard>>,
        assignments: Vec<Option<usize>>,
        variables: &Variables,
        body: &mut Vec<LiteralIr>,
    ) -> Result<(), FormulaFailure> {
        let mut guards = guards.into_iter();
        let mut assignments = assignments.into_iter();
        for element in source.elements() {
            if let BodyElement::Aggregate {
                negation,
                aggregate,
            } = element.get()
            {
                let binding = assignments
                    .next()
                    .expect("one binding marker per aggregate");
                let aggregate = self.aggregate(
                    *negation,
                    aggregate,
                    guards.next().expect("one guard list per aggregate"),
                    variables,
                    binding,
                )?;
                self.assignment_scope(&aggregate)?;
                body.push(LiteralIr::Aggregate(aggregate));
            }
        }
        Ok(())
    }
    pub(super) fn aggregate_term(
        &mut self,
        term: &Term,
        variables: &mut Variables,
    ) -> Result<CoreTerm, FormulaFailure> {
        if !matches!(term, Term::Variable(_) | Term::Symbolic(_)) {
            return Err(unsupported(ProfileFeature::Aggregate, self.location).into());
        }
        self.domain_term(term, variables)
    }

    /// Explicit scalar roots in rule, aggregate and witness contexts. Objective
    /// fields use `objective_term` instead and do not enlarge this root selection.
    pub(super) fn domain_term(
        &mut self,
        term: &Term,
        variables: &mut Variables,
    ) -> Result<CoreTerm, FormulaFailure> {
        match term {
            Term::Symbolic(symbol) => {
                self.budget
                    .charge(crate::ExpansionResource::TermWork, 1, self.location)?;
                let value = crate::compile::scalar(symbol, self.location)?;
                Ok(CoreTerm::Constant(self.root_scalar(&value)?))
            }
            _ => self.objective_term(term, variables),
        }
    }
    fn aggregate(
        &mut self,
        negation: DefaultNegation,
        aggregate: &Aggregate,
        guards: Vec<AggregateGuard>,
        variables: &Variables,
        binding: Option<usize>,
    ) -> Result<AggregateIr, FormulaFailure> {
        let mut elements = Vec::new();
        let function = match aggregate {
            Aggregate::Function(aggregate) => {
                if !matches!(
                    aggregate.function(),
                    AggregateFunction::Count
                        | AggregateFunction::Sum
                        | AggregateFunction::SumPlus
                        | AggregateFunction::Min
                        | AggregateFunction::Max
                ) {
                    return Err(unsupported(ProfileFeature::Aggregate, self.location).into());
                }
                for (index, element) in aggregate.elements().enumerate() {
                    let family = LocalFamily(index);
                    let fields: Vec<_> = element.get().terms().collect();
                    for (terms, source_condition) in
                        self.local_alternatives(&fields, element.get().condition())?
                    {
                        elements.push(self.aggregate_tuple_element(
                            &terms,
                            &source_condition,
                            aggregate.function(),
                            variables,
                            family,
                        )?);
                    }
                }
                aggregate.function()
            }
            Aggregate::Set(aggregate) => {
                for (index, element) in aggregate.elements().enumerate() {
                    let family = LocalFamily(index);
                    let (literal, source_condition) = match element.get() {
                        SetElement::Literal(literal) => (literal, Condition::new([])),
                        SetElement::ConditionalLiteral(value) => {
                            (&value.literal, value.condition.clone())
                        }
                    };
                    for alternative in self.condition_alternatives(&source_condition)? {
                        for selected in self.literal_alternatives(literal)? {
                            let mut local = variables.clone();
                            let mut condition = self.condition(&alternative, &mut local)?;
                            let themelios_program::program::LiteralInner::Atom(atom) =
                                &selected.inner
                            else {
                                return Err(
                                    unsupported(ProfileFeature::Aggregate, self.location).into()
                                );
                            };
                            if selected.negation != DefaultNegation::None {
                                return Err(
                                    unsupported(ProfileFeature::Aggregate, self.location).into()
                                );
                            }
                            let atom =
                                self.positive_atom_key(atom.get(), &mut local, &mut condition)?;
                            self.bindings(&mut condition, &mut local)?;
                            local.safety(self.location)?;
                            elements.push(AggregateElementIr {
                                family,
                                key: AggregateKey::Atom(atom),
                                condition,
                                variables: local.count,
                            });
                        }
                    }
                }
                AggregateFunction::Count
            }
        };
        let id = self.next_aggregate;
        self.next_aggregate += 1;
        Ok(AggregateIr {
            id,
            binding,
            negation,
            function,
            guards,
            elements,
        })
    }

    fn aggregate_tuple_element(
        &mut self,
        terms: &[Term],
        source_condition: &Condition,
        function: AggregateFunction,
        variables: &Variables,
        family: LocalFamily,
    ) -> Result<AggregateElementIr, FormulaFailure> {
        let mut local = variables.clone();
        let mut condition = self.condition(source_condition, &mut local)?;
        let tuple = terms
            .iter()
            .map(|term| {
                if matches!(term, Term::Variable(_) | Term::Symbolic(_)) {
                    self.aggregate_term(term, &mut local)
                } else {
                    self.generated_term(term, &mut local, &mut condition)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        if matches!(function, AggregateFunction::Min | AggregateFunction::Max) && tuple.is_empty() {
            return Err(unsupported(ProfileFeature::Aggregate, self.location).into());
        }
        self.bindings(&mut condition, &mut local)?;
        local.safety(self.location)?;
        Ok(AggregateElementIr {
            family,
            key: AggregateKey::Tuple(tuple),
            condition,
            variables: local.count,
        })
    }
}
fn sentinel(term: &Term) -> Option<Value> {
    match term {
        Term::Symbolic(Symbol::Infimum) => Some(Value::Infimum),
        Term::Symbolic(Symbol::Supremum) => Some(Value::Supremum),
        _ => None,
    }
}
fn reverse(relation: Relation) -> Relation {
    match relation {
        Relation::Eq => Relation::Eq,
        Relation::Neq => Relation::Neq,
        Relation::Lt => Relation::Gt,
        Relation::Le => Relation::Ge,
        Relation::Gt => Relation::Lt,
        Relation::Ge => Relation::Le,
    }
}
