//! Lower every compiled producer into one conservative argument analysis.
//!
//! A local choice/conditional head gets a separate rule frame. Negative body
//! atoms bind nothing. Unsupported term transfers widen; they never erase a
//! producer. Fact intervals supply bounds without expanding their products.

use std::cmp::Ordering;

use numeric::{ArgumentId, Binding, ExpressionId, Guard, Head, Input};
use themelios_program::program::DefaultNegation;
use zetesis_core::catalog::{DeclaredPredicate, PredicateRef, ReadError, TermRef};
use zetesis_core::{PatternRef, TemplateComponentsRef, TemplateTerm};
use zetesis_domain::{Widening, numeric};

use crate::ProgramSite;
use crate::formula_facts::ColumnSegment;
use crate::formula_ir::{Expression, HeadIr, LiteralIr, Operation, Prepared, Projection, RuleIr};
use crate::formula_support::components::{Pattern, Scalar, Term};

use super::census::{Occurrence, Query, Requests};
use super::reader::Reader;

type Error = numeric::Error<ReadError>;
type Engine<'a, 'b> = Input<'a, Reader<'b>, ProgramSite>;

pub(super) struct Predicate {
    pub(super) key: DeclaredPredicate,
    pub(super) arguments: Vec<ArgumentId>,
}

pub(super) struct Lowering<'a, 'prepared> {
    pub(super) reader: Reader<'a>,
    pub(super) components: TemplateComponentsRef<'a>,
    pub(super) predicates: Vec<Predicate>,
    pub(super) finite_width: usize,
    pub(super) diagnostics: bool,
    pub(super) occurrences: Vec<Occurrence<'prepared>>,
}

impl<'prepared> Lowering<'_, 'prepared> {
    pub(super) fn program(
        &mut self,
        prepared: &'prepared Prepared,
        input: &mut Engine<'_, '_>,
    ) -> Result<(), Error> {
        self.seed_facts(prepared, input)?;
        for source in prepared.rules_without_facts() {
            input.context(source.location);
            let mut requests = None;
            match &source.head {
                HeadIr::Normal(_) => requests = self.normal(source, input)?,
                HeadIr::Disjunction(heads) => {
                    let mut rule = self.body(
                        source.variables,
                        source.variables,
                        &source.body,
                        input,
                        None,
                    )?;
                    for head in heads {
                        input.work(1)?;
                        if let Some(atom) = head.positive_atom() {
                            self.head(*atom, &mut rule, input)?;
                        }
                    }
                    input.rule(rule)?;
                }
                HeadIr::ConditionalDisjunction { ordinary, elements } => {
                    let mut rule = self.body(
                        source.variables,
                        source.variables,
                        &source.body,
                        input,
                        None,
                    )?;
                    for head in ordinary {
                        input.work(1)?;
                        if let Some(atom) = head.positive_atom() {
                            self.head(*atom, &mut rule, input)?;
                        }
                    }
                    input.rule(rule)?;
                    for element in elements {
                        let mut rule = self.body(
                            element.variables,
                            source.body_variables,
                            &source.body,
                            input,
                            None,
                        )?;
                        self.body_into(
                            &element.condition,
                            element.variables,
                            &mut rule,
                            input,
                            None,
                        )?;
                        if let Some(atom) = element.head.positive_atom() {
                            self.head(*atom, &mut rule, input)?;
                        }
                        input.rule(rule)?;
                    }
                }
                HeadIr::Choice(choice) => {
                    for element in &choice.elements {
                        input.work(1)?;
                        let mut rule = self.body(
                            element.variables,
                            source.body_variables,
                            &source.body,
                            input,
                            None,
                        )?;
                        self.body_into(
                            &element.condition,
                            element.variables,
                            &mut rule,
                            input,
                            None,
                        )?;
                        if let Some(atom) = element.head.positive_atom() {
                            self.head(*atom, &mut rule, input)?;
                        }
                        input.rule(rule)?;
                    }
                }
            }
            if self.diagnostics {
                input.work(1)?;
                self.occurrences.try_reserve(1)?;
                self.occurrences.push(Occurrence::new(source, requests));
            }
        }
        Ok(())
    }

    fn normal(
        &mut self,
        source: &RuleIr,
        input: &mut Engine<'_, '_>,
    ) -> Result<Option<Requests>, Error> {
        let mut comparisons = Vec::new();
        let mut rule = self.body(
            source.variables,
            source.variables,
            &source.body,
            input,
            self.diagnostics.then_some(&mut comparisons),
        )?;
        if let HeadIr::Normal(Some(head)) = &source.head {
            self.head(*head, &mut rule, input)?;
        }
        let rule = input.rule(rule)?;
        Ok(self.diagnostics.then_some(Requests { rule, comparisons }))
    }

    fn seed_facts(&mut self, prepared: &Prepared, input: &mut Engine<'_, '_>) -> Result<(), Error> {
        for block in prepared.items().blocks() {
            input.context(block.location());
            input.work(1)?;
            let predicate = block
                .predicate()
                .resolve(self.components)
                .ok_or(Error::InvalidInput)?;
            let index = self.predicate(predicate, input)?;
            for (column, values) in block.columns().enumerate() {
                let argument = self.predicates[index].arguments[column];
                for segment in values.segments() {
                    input.work(1)?;
                    match segment {
                        ColumnSegment::Scalar(scalar) => {
                            let value = self.scalar(scalar, input)?;
                            input.seed_value(argument, value)?;
                        }
                        ColumnSegment::Integers { first, scalars } => {
                            if scalars.len() <= self.finite_width {
                                for offset in 0..scalars.len() {
                                    let value = self.scalar(scalars.get(offset), input)?;
                                    input.seed_value(argument, value)?;
                                }
                            } else {
                                let high = i64::from(first)
                                    + i64::try_from(scalars.len() - 1)
                                        .map_err(|_| Error::InvalidInput)?;
                                input.seed_interval(
                                    argument,
                                    first,
                                    i32::try_from(high).map_err(|_| Error::InvalidInput)?,
                                )?;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn predicate(
        &mut self,
        predicate: PredicateRef<'_>,
        input: &mut Engine<'_, '_>,
    ) -> Result<usize, Error> {
        let mut low = 0;
        let mut high = self.predicates.len();
        while low < high {
            let middle = low + (high - low) / 2;
            input.work(1)?;
            let retained = self
                .reader
                .0
                .predicate(&self.predicates[middle].key)
                .map_err(Error::Read)?;
            input.bytes((predicate.name().len() + retained.name().len()) as u64)?;
            match predicate.compare_ref_with(retained, || input.work(1))? {
                Ordering::Less => high = middle,
                Ordering::Greater => low = middle + 1,
                Ordering::Equal => return Ok(middle),
            }
        }
        input.work(1)?;
        let key = self
            .reader
            .0
            .declare_existing(predicate)
            .map_err(Error::Read)?;
        let id = input.predicate(predicate.arity())?;
        let mut arguments = Vec::new();
        arguments.try_reserve_exact(predicate.arity())?;
        for column in 0..predicate.arity() {
            input.work(1)?;
            arguments.push(input.argument(id, column)?);
        }
        self.predicates.try_reserve(1)?;
        input.work((self.predicates.len() - low + 1) as u64)?;
        self.predicates.insert(low, Predicate { key, arguments });
        Ok(low)
    }

    fn constant(
        &self,
        value: TermRef<'_>,
        input: &mut Engine<'_, '_>,
    ) -> Result<numeric::ConstantId, Error> {
        input.work(1)?;
        input.constant(self.reader.0.term_key(value).map_err(Error::Read)?)
    }

    fn scalar(
        &self,
        scalar: Scalar,
        input: &mut Engine<'_, '_>,
    ) -> Result<numeric::ConstantId, Error> {
        input.work(1)?;
        self.constant(
            scalar.resolve(self.components).ok_or(Error::InvalidInput)?,
            input,
        )
    }

    fn head(
        &mut self,
        pattern: Pattern,
        rule: &mut numeric::Rule,
        input: &mut Engine<'_, '_>,
    ) -> Result<(), Error> {
        // The component view borrows the source, independently of metadata growth.
        input.work(1)?;
        let pattern = pattern
            .resolve(self.components)
            .ok_or(Error::InvalidInput)?;
        let predicate = self.predicate(pattern.predicate(), input)?;
        for column in 0..pattern.terms().len() {
            input.work(1)?;
            let term = pattern.terms().at(column).ok_or(Error::InvalidInput)?;
            let value = self.term(term, rule.slots, input)?;
            rule.heads.try_reserve(1)?;
            rule.heads.push(Head {
                argument: self.predicates[predicate].arguments[column],
                value,
            });
        }
        Ok(())
    }

    fn term(
        &self,
        term: TemplateTerm<'_>,
        slots: usize,
        input: &mut Engine<'_, '_>,
    ) -> Result<ExpressionId, Error> {
        let term = match term {
            TemplateTerm::Variable(slot) if slot < slots => numeric::Expression::Slot(slot),
            TemplateTerm::Variable(_) => numeric::Expression::Unknown {
                integer: false,
                reason: Widening::UnboundVariable,
            },
            TemplateTerm::Constant(value) => {
                numeric::Expression::Constant(self.constant(value, input)?)
            }
        };
        input.expression(term)
    }

    fn body(
        &mut self,
        slots: usize,
        visible: usize,
        body: &[LiteralIr],
        input: &mut Engine<'_, '_>,
        comparisons: Option<&mut Vec<Query>>,
    ) -> Result<numeric::Rule, Error> {
        let mut rule = numeric::Rule {
            slots,
            ..numeric::Rule::default()
        };
        self.body_into(body, visible, &mut rule, input, comparisons)?;
        Ok(rule)
    }

    fn positive(
        &mut self,
        pattern: PatternRef<'_>,
        slots: usize,
        bindings: &mut Vec<Binding>,
        input: &mut Engine<'_, '_>,
    ) -> Result<(), Error> {
        let predicate = self.predicate(pattern.predicate(), input)?;
        for column in 0..pattern.terms().len() {
            input.work(1)?;
            if let Some(TemplateTerm::Variable(slot)) = pattern.terms().at(column)
                && slot < slots
            {
                bindings.try_reserve(1)?;
                bindings.push(Binding::Positive {
                    slot,
                    argument: self.predicates[predicate].arguments[column],
                });
            }
        }
        Ok(())
    }

    fn body_into(
        &mut self,
        body: &[LiteralIr],
        visible: usize,
        rule: &mut numeric::Rule,
        input: &mut Engine<'_, '_>,
        mut comparisons: Option<&mut Vec<Query>>,
    ) -> Result<(), Error> {
        let visible = visible.min(rule.slots);
        // Only the enclosing body prefix belongs to a local element. An outer
        // head's generated suffix cannot alias this element's private slots.
        // Relational inputs precede generated assignments. An unsupported or
        // not-yet-bound expression remains Top, never an invented empty domain.
        self.relational_bindings(body, visible, rule, input)?;
        for (literal_index, literal) in body.iter().enumerate() {
            input.work(1)?;
            match literal {
                LiteralIr::Bind { target, value } if *target < visible => {
                    let value = self.expression(value, visible, input)?;
                    rule.bindings.try_reserve(1)?;
                    rule.bindings.push(Binding::Assign {
                        slot: *target,
                        value,
                    });
                }
                LiteralIr::Range {
                    target,
                    lower,
                    upper,
                    binder,
                } if *target < visible => {
                    let low = self.expression(lower, visible, input)?;
                    let high = self.expression(upper, visible, input)?;
                    if *binder {
                        rule.bindings.try_reserve(1)?;
                        rule.bindings.push(Binding::Range {
                            slot: *target,
                            low,
                            high,
                        });
                    } else {
                        rule.guards.try_reserve(1)?;
                        rule.guards.push(Guard::Membership {
                            slot: *target,
                            low,
                            high,
                        });
                    }
                }
                LiteralIr::Aggregate(aggregate) => {
                    if let Some(slot) = aggregate.binding
                        && slot < visible
                    {
                        rule.bindings.try_reserve(1)?;
                        rule.bindings.push(Binding::Aggregate {
                            slot,
                            function: aggregate.function,
                        });
                    }
                }
                LiteralIr::Compare(comparison) | LiteralIr::ArgumentCheck(comparison) => {
                    let left = self.expression(comparison.left(), visible, input)?;
                    let right = self.expression(comparison.right(), visible, input)?;
                    rule.guards.try_reserve(1)?;
                    rule.guards.push(Guard::Compare {
                        left,
                        relation: comparison.relation(),
                        right,
                    });
                    if let Some(comparisons) = comparisons.as_deref_mut() {
                        input.work(3)?;
                        rule.integer_queries.try_reserve(2)?;
                        comparisons.try_reserve(1)?;
                        let first = rule.integer_queries.len();
                        rule.integer_queries.extend([left, right]);
                        comparisons.push(Query::new(literal_index, first, [left, right]));
                    }
                }
                LiteralIr::Atom(..)
                | LiteralIr::PatternAtom(_)
                | LiteralIr::ProjectedAtom(..)
                | LiteralIr::TupleCompare(..)
                | LiteralIr::Guard(_)
                | LiteralIr::HeadGuard(_)
                | LiteralIr::Conditional(_)
                | LiteralIr::Bind { .. }
                | LiteralIr::Range { .. } => {}
            }
        }
        Ok(())
    }

    fn relational_bindings(
        &mut self,
        body: &[LiteralIr],
        visible: usize,
        rule: &mut numeric::Rule,
        input: &mut Engine<'_, '_>,
    ) -> Result<(), Error> {
        for literal in body {
            input.work(1)?;
            match literal {
                LiteralIr::Atom(DefaultNegation::None, atom) => {
                    let atom = atom.resolve(self.components).ok_or(Error::InvalidInput)?;
                    self.positive(atom, visible, &mut rule.bindings, input)?;
                }
                LiteralIr::PatternAtom(pattern) => {
                    let atom = pattern
                        .atom
                        .resolve(self.components)
                        .ok_or(Error::InvalidInput)?;
                    self.positive(atom, visible, &mut rule.bindings, input)?;
                }
                LiteralIr::ProjectedAtom(DefaultNegation::None, projection) => match projection {
                    Projection::Arguments { predicate, terms } => {
                        let predicate = predicate
                            .resolve(self.components)
                            .ok_or(Error::InvalidInput)?;
                        let predicate = self.predicate(predicate, input)?;
                        for (column, term) in terms.iter().enumerate() {
                            input.work(1)?;
                            if let Some(Term::Variable(slot)) = term
                                && *slot < visible
                            {
                                rule.bindings.try_reserve(1)?;
                                rule.bindings.push(Binding::Positive {
                                    slot: *slot,
                                    argument: self.predicates[predicate].arguments[column],
                                });
                            }
                        }
                    }
                    Projection::Witnesses { atom, inputs, .. } => {
                        let atom = atom.resolve(self.components).ok_or(Error::InvalidInput)?;
                        self.positive(atom, visible.min(*inputs), &mut rule.bindings, input)?;
                    }
                },
                LiteralIr::Atom(DefaultNegation::Not | DefaultNegation::NotNot, _)
                | LiteralIr::ProjectedAtom(DefaultNegation::Not | DefaultNegation::NotNot, _)
                | LiteralIr::Compare(_)
                | LiteralIr::ArgumentCheck(_)
                | LiteralIr::TupleCompare(..)
                | LiteralIr::Guard(_)
                | LiteralIr::HeadGuard(_)
                | LiteralIr::Conditional(_)
                | LiteralIr::Aggregate(_)
                | LiteralIr::Bind { .. }
                | LiteralIr::Range { .. } => {}
            }
        }
        Ok(())
    }

    fn expression(
        &self,
        source: &Expression,
        slots: usize,
        input: &mut Engine<'_, '_>,
    ) -> Result<ExpressionId, Error> {
        let mut nodes = Vec::new();
        nodes.try_reserve_exact(source.nodes.len())?;
        for operation in &source.nodes {
            input.work(1)?;
            let earlier = |index: usize| nodes.get(index).copied().ok_or(Error::InvalidInput);
            let node = match operation {
                Operation::Constant(scalar) => {
                    numeric::Expression::Constant(self.scalar(*scalar, input)?)
                }
                Operation::Variable(slot) if *slot < slots => numeric::Expression::Slot(*slot),
                Operation::Variable(_) => numeric::Expression::Unknown {
                    integer: false,
                    reason: Widening::UnboundVariable,
                },
                Operation::Unary(operator, value) => {
                    numeric::Expression::Unary(*operator, earlier(*value)?)
                }
                Operation::Binary(operator, left, right) => {
                    numeric::Expression::Binary(*operator, earlier(*left)?, earlier(*right)?)
                }
                Operation::Absolute(value) => numeric::Expression::Absolute(earlier(*value)?),
                Operation::Constructor(_) => numeric::Expression::Unknown {
                    integer: false,
                    reason: Widening::HeadTerm,
                },
            };
            nodes.push(input.expression(node)?);
        }
        nodes.last().copied().ok_or(Error::InvalidInput)
    }
}
