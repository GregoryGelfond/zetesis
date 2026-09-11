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
            let bound = self.template(&left.get().term, 1)?;
            guards.push(Guard {
                relation,
                bound: self.lift(bound)?,
            });
        }
        if let Some(right) = guarded.right_guard() {
            let bound = self.template(&right.get().term, 1)?;
            guards.push(Guard {
                relation: right.get().relation.unwrap_or(Relation::Le),
                bound: self.lift(bound)?,
            });
        }
        Ok(guards)
    }
    fn local<T>(&mut self, action: impl FnOnce(&mut Self) -> Result<T, Error>) -> Result<T, Error> {
        let variables = self.variables.clone();
        let safe = self.safe.clone();
        let slots = self.slots;
        let generated = std::mem::take(&mut self.generated);
        let used = std::mem::take(&mut self.used);
        let scope_outer = self.scope_outer;
        self.scope_outer = self.slots;
        self.safe.extend(0..self.scope_outer);
        let result = action(self);
        self.variables = variables;
        self.safe = safe;
        self.slots = slots;
        self.generated = generated;
        self.used = used;
        self.scope_outer = scope_outer;
        result
    }
    fn literals(
        &mut self,
        source: &themelios_program::program::Condition,
        positive: &mut Vec<Vec<Pattern>>,
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
        let result = self.local(|compiler| {
            let consequent = compiler.test(&source.literal)?;
            let mut positive = Vec::new();
            let mut conditions = Vec::new();
            compiler.literals(&source.condition, &mut positive, &mut conditions)?;
            let query = compiler.finish(positive, conditions)?;
            Ok(Condition::Conditional(query, Box::new(consequent)))
        })?;
        let Condition::Conditional(query, _) = &result else {
            unreachable!()
        };
        if query.inputs.iter().any(|slot| !self.safe.contains(slot)) {
            return Err(self.unsupported(Feature::UnsafeVariable));
        }
        Ok(result)
    }
    fn set_alternative(
        &mut self,
        atom: &themelios_program::program::Atom,
        arguments: &[themelios_program::term::Term],
        negation: DefaultNegation,
        condition: Option<&themelios_program::program::Condition>,
    ) -> Result<AggregateElement, Error> {
        self.local(|compiler| {
            let tag = match negation {
                DefaultNegation::None => 0,
                DefaultNegation::Not => 1,
                DefaultNegation::NotNot => 2,
            };
            let mut positive = Vec::new();
            let mut conditions = Vec::new();
            if let Some(condition) = condition {
                compiler.literals(condition, &mut positive, &mut conditions)?;
            }
            let slot = if negation == DefaultNegation::None {
                // The matched original atom supplies complete key identity,
                // including anonymous arguments and the selected pool branch.
                let mut patterns = compiler.pattern_variants(atom, arguments)?;
                let slot = compiler.slot()?;
                compiler.used.insert(slot);
                for pattern in &mut patterns {
                    pattern.key = Some(slot);
                }
                positive.push(patterns);
                slot
            } else {
                // An absent atom cannot supply a row. Generate its exact source
                // value and apply default negation to that completed alternative.
                // function() charges its children; its caller owns the root.
                compiler.node(1)?;
                let key = compiler.function(&atom.name, arguments, 1, atom.sign)?;
                let slot = compiler.generate(key)?;
                compiler.node(1)?;
                conditions.push(Condition::AtomValue(negation, slot));
                slot
            };
            compiler.node(1)?;
            let key = Template::Variable(slot);
            let query = compiler.finish(positive, conditions)?;
            // The tag has both a template node and a ground-symbol node. The
            // enclosing source set element was charged separately; each expanded
            // alternative owns exactly one key tuple root, charged here.
            compiler.node(1)?;
            let tag = Template::Value(compiler.symbol(&Symbol::Number(tag), 1, false)?);
            compiler.node(1)?;
            Ok(AggregateElement {
                tuple: Template::Tuple(vec![tag, key]),
                query,
            })
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
                        if tuple.is_empty()
                            && matches!(
                                source.function(),
                                AggregateFunction::Min | AggregateFunction::Max
                            )
                        {
                            return Err(compiler.unsupported(Feature::AggregateMeasure));
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
                    let (literal, condition) = match element.get() {
                        SetElement::Literal(literal) => (literal, None),
                        SetElement::ConditionalLiteral(value) => {
                            (&value.literal, Some(&value.condition))
                        }
                    };
                    let LiteralInner::Atom(atom) = &literal.inner else {
                        return Err(self.unsupported(Feature::Atom));
                    };
                    for arguments in atom.get().alternatives() {
                        elements.push(self.set_alternative(
                            atom.get(),
                            arguments,
                            literal.negation,
                            condition,
                        )?);
                    }
                }
                AggregateFunction::Count
            }
        };
        Ok(AggregateQuery { function, elements })
    }
}
