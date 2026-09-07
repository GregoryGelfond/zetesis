//! Scoped finite aggregate tuples and guards, independent of propositional lowering.

use themelios_program::program::{
    Aggregate, AggregateFunction, BodyElement, Choice, Condition, DefaultNegation, Guard,
    HasGuards, Relation, Rule, SetElement,
};
use themelios_program::provenance::WithProvenance;
use themelios_program::symbol::Symbol;
use themelios_program::term::Term;

use crate::diagnostic::unsupported;
use crate::formula_ir::{
    AggregateElementIr, AggregateGuard, AggregateIr, AggregateKey, Compiler, Expression, LiteralIr,
    Operation, Variables,
};
use crate::{AdmissionFailure, FormulaFailure, InputLimit, ProfileFeature};
use zetesis_core::{Term as CoreTerm, Value};

impl Compiler<'_> {
    pub(super) fn condition(
        &mut self,
        condition: &Condition,
        variables: &mut Variables,
    ) -> Result<Vec<LiteralIr>, FormulaFailure> {
        condition
            .literals()
            .enumerate()
            .map(|(index, literal)| {
                if index >= self.options.max_body_elements {
                    return Err(AdmissionFailure::Limit {
                        resource: InputLimit::BodyElements,
                        limit: self.options.max_body_elements,
                        observed: index + 1,
                        location: self.location,
                    }
                    .into());
                }
                self.literal(literal.get(), variables)
            })
            .collect()
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
    ) -> Result<Vec<AggregateGuard>, FormulaFailure> {
        self.guards(
            choice.left_guard().map(WithProvenance::get),
            choice.right_guard().map(WithProvenance::get),
            variables,
        )
    }
    fn guards(
        &mut self,
        left: Option<&Guard>,
        right: Option<&Guard>,
        variables: &mut Variables,
    ) -> Result<Vec<AggregateGuard>, FormulaFailure> {
        let mut guards = Vec::new();
        if let Some(left) = left {
            guards.push(AggregateGuard {
                relation: reverse(left.relation.unwrap_or(Relation::Le)),
                bound: self.aggregate_expression(&left.term, variables)?,
            });
        }
        if let Some(right) = right {
            guards.push(AggregateGuard {
                relation: right.relation.unwrap_or(Relation::Le),
                bound: self.aggregate_expression(&right.term, variables)?,
            });
        }
        Ok(guards)
    }
    fn aggregate_expression(
        &mut self,
        term: &Term,
        variables: &mut Variables,
    ) -> Result<Expression, FormulaFailure> {
        if let Some(value) = sentinel(term) {
            self.value(&value)?;
            return Ok(Expression {
                nodes: vec![Operation::Constant(value)],
            });
        }
        self.expression(term, variables)
    }
    pub(super) fn body_guards(
        &mut self,
        rule: &Rule,
        variables: &mut Variables,
    ) -> Result<Vec<Vec<AggregateGuard>>, FormulaFailure> {
        let mut result = Vec::new();
        for element in rule.body().get().elements() {
            if let BodyElement::Aggregate { aggregate, .. } = element.get() {
                let guarded: &dyn HasGuards = match aggregate {
                    Aggregate::Function(value) => value,
                    Aggregate::Set(value) => value,
                };
                result.push(self.guards(
                    guarded.left_guard().map(WithProvenance::get),
                    guarded.right_guard().map(WithProvenance::get),
                    variables,
                )?);
            }
        }
        Ok(result)
    }
    pub(super) fn body_aggregates(
        &mut self,
        rule: &Rule,
        guards: Vec<Vec<AggregateGuard>>,
        assignments: Vec<Option<usize>>,
        variables: &Variables,
        body: &mut Vec<LiteralIr>,
    ) -> Result<(), FormulaFailure> {
        let mut guards = guards.into_iter();
        let mut assignments = assignments.into_iter();
        for element in rule.body().get().elements() {
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
    fn aggregate_term(
        &mut self,
        term: &Term,
        variables: &mut Variables,
    ) -> Result<CoreTerm, FormulaFailure> {
        if !matches!(term, Term::Variable(_) | Term::Symbolic(_)) {
            return Err(unsupported(ProfileFeature::Aggregate, self.location).into());
        }
        let term = if let Some(value) = sentinel(term) {
            CoreTerm::Constant(value)
        } else {
            self.objective_term(term, variables)?
        };
        if let CoreTerm::Constant(value) = &term {
            self.value(value)?;
        }
        Ok(term)
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
                for element in aggregate.elements() {
                    let mut local = variables.clone();
                    let mut condition = self.condition(element.get().condition(), &mut local)?;
                    let tuple = element
                        .get()
                        .terms()
                        .map(|term| self.aggregate_term(term, &mut local))
                        .collect::<Result<Vec<_>, _>>()?;
                    if matches!(
                        aggregate.function(),
                        AggregateFunction::Min | AggregateFunction::Max
                    ) && tuple.is_empty()
                    {
                        return Err(unsupported(ProfileFeature::Aggregate, self.location).into());
                    }
                    self.bindings(&mut condition, &mut local)?;
                    local.safety(self.location)?;
                    elements.push(AggregateElementIr {
                        key: AggregateKey::Tuple(tuple),
                        condition,
                        variables: local.count,
                    });
                }
                aggregate.function()
            }
            Aggregate::Set(aggregate) => {
                for element in aggregate.elements() {
                    let mut local = variables.clone();
                    let (literal, mut condition) = match element.get() {
                        SetElement::Literal(literal) => (literal, Vec::new()),
                        SetElement::ConditionalLiteral(value) => (
                            &value.literal,
                            self.condition(&value.condition, &mut local)?,
                        ),
                    };
                    let LiteralIr::Atom(DefaultNegation::None, atom) =
                        self.literal(literal, &mut local)?
                    else {
                        return Err(unsupported(ProfileFeature::Aggregate, self.location).into());
                    };
                    condition.push(LiteralIr::Atom(DefaultNegation::None, atom.clone()));
                    self.bindings(&mut condition, &mut local)?;
                    local.safety(self.location)?;
                    elements.push(AggregateElementIr {
                        key: AggregateKey::Atom(atom),
                        condition,
                        variables: local.count,
                    });
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
