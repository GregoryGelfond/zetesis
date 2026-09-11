//! Local observation scopes inherit outer bindings without exporting local names.

use themelios_program::program::{
    Aggregate, AggregateFunction, ConditionalLiteral, HasGuards, LiteralInner, SetElement,
};

use super::{
    Compiler, Condition, DefaultNegation, Error, Feature, Pattern, Relation, Resource, Template,
};
use crate::observation::{AggregateElement, AggregateQuery, Guard, Symbol};

impl Compiler<'_> {
    pub(super) fn guards(&mut self, aggregate: &Aggregate) -> Result<Vec<Guard>, Error> {
        let guarded: &dyn HasGuards = match aggregate {
            Aggregate::Function(value) => value,
            Aggregate::Set(value) => value,
        };
        let mut guards = Vec::new();
        if let Some(left) = guarded.left_guard() {
            let relation = match left.get().relation.unwrap_or(Relation::Le) {
                Relation::Lt => Relation::Gt,
                Relation::Le => Relation::Ge,
                Relation::Gt => Relation::Lt,
                Relation::Ge => Relation::Le,
                other => other,
            };
            guards.push(Guard {
                relation,
                bound: self.template(&left.get().term, 1)?,
            });
        }
        if let Some(right) = guarded.right_guard() {
            guards.push(Guard {
                relation: right.get().relation.unwrap_or(Relation::Le),
                bound: self.template(&right.get().term, 1)?,
            });
        }
        Ok(guards)
    }
    fn local<T>(&mut self, action: impl FnOnce(&mut Self) -> Result<T, Error>) -> Result<T, Error> {
        let variables = self.variables.clone();
        let safe = self.safe.clone();
        let result = action(self);
        self.variables = variables;
        self.safe = safe;
        result
    }
    fn literals(
        &mut self,
        source: &themelios_program::program::Condition,
        positive: &mut Vec<Pattern>,
        conditions: &mut Vec<Condition>,
    ) -> Result<(), Error> {
        for (index, literal) in source.literals().enumerate() {
            self.check(
                Resource::BodyElements,
                index.saturating_add(1),
                self.limits.max_body_elements as usize,
            )?;
            self.node(1)?;
            self.literal(literal.get(), positive, conditions)?;
        }
        Ok(())
    }
    pub(super) fn conditional(&mut self, source: &ConditionalLiteral) -> Result<Condition, Error> {
        self.local(|compiler| {
            let consequent = compiler.test(&source.literal)?;
            let mut positive = Vec::new();
            let mut conditions = Vec::new();
            compiler.literals(&source.condition, &mut positive, &mut conditions)?;
            let query = compiler.finish(positive, conditions)?;
            Ok(Condition::Conditional(query, Box::new(consequent)))
        })
    }
    pub(super) fn aggregate(&mut self, aggregate: &Aggregate) -> Result<AggregateQuery, Error> {
        let mut elements = Vec::new();
        let function = match aggregate {
            Aggregate::Function(source) => {
                for (index, element) in source.elements().enumerate() {
                    self.check(
                        Resource::BodyElements,
                        index.saturating_add(1),
                        self.limits.max_body_elements as usize,
                    )?;
                    self.node(1)?;
                    elements.push(self.local(|compiler| {
                        let mut tuple = Vec::new();
                        for term in element.get().terms() {
                            tuple.push(compiler.template(term, 1)?);
                        }
                        compiler.arity(tuple.len())?;
                        let mut positive = Vec::new();
                        let mut conditions = Vec::new();
                        compiler.literals(
                            element.get().condition(),
                            &mut positive,
                            &mut conditions,
                        )?;
                        let query = compiler.finish(positive, conditions)?;
                        Ok(AggregateElement {
                            tuple: Template::Tuple(tuple),
                            query,
                        })
                    })?);
                }
                source.function()
            }
            Aggregate::Set(source) => {
                for (index, element) in source.elements().enumerate() {
                    self.check(
                        Resource::BodyElements,
                        index.saturating_add(1),
                        self.limits.max_body_elements as usize,
                    )?;
                    self.node(1)?;
                    elements.push(self.local(|compiler| {
                        let (literal, condition) = match element.get() {
                            SetElement::Literal(literal) => (literal, None),
                            SetElement::ConditionalLiteral(value) => {
                                (&value.literal, Some(&value.condition))
                            }
                        };
                        let LiteralInner::Atom(atom) = &literal.inner else {
                            return Err(compiler.unsupported(Feature::Atom));
                        };
                        let themelios_program::program::Arguments::Single(arguments) =
                            &atom.get().arguments
                        else {
                            return Err(compiler.unsupported(Feature::Atom));
                        };
                        let key =
                            compiler.function(&atom.get().name, arguments, 1, atom.get().sign)?;
                        let negation = match literal.negation {
                            DefaultNegation::None => 0,
                            DefaultNegation::Not => 1,
                            DefaultNegation::NotNot => 2,
                        };
                        let mut positive = Vec::new();
                        let mut conditions = Vec::new();
                        if let Some(condition) = condition {
                            compiler.literals(condition, &mut positive, &mut conditions)?;
                        }
                        compiler.literal(literal, &mut positive, &mut conditions)?;
                        let query = compiler.finish(positive, conditions)?;
                        Ok(AggregateElement {
                            tuple: Template::Tuple(vec![
                                Template::Value(Symbol::Number(negation)),
                                key,
                            ]),
                            query,
                        })
                    })?);
                }
                AggregateFunction::Count
            }
        };
        Ok(AggregateQuery { function, elements })
    }
}
