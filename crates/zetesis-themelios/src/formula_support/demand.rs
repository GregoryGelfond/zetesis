//! The columns of each support predicate that a join can bind.
//!
//! Support postings serve two readers: a probe chooses the shortest posting
//! among the columns its pattern binds, and totality reads a column's keys as
//! its distinct values. Both read only columns a join can bind, so a column is
//! demanded iff some join occurrence of its predicate holds there a constant,
//! or a variable a join can have bound: one that occurs again in the same rule,
//! in its body or its head, since factorization binds head variables before it
//! probes the body. A predicate no join occurrence names demands nothing.
//!
//! Rules with an ordinary head and a flat body of atoms, comparisons and
//! bindings are read exactly. Choice and conditional heads, negated
//! atoms, projections and every nested frame (aggregate elements, choice and
//! conditional conditions, objectives, witnesses) demand every column holding a
//! constant or a variable: a superset, since a column indexed without need only
//! costs memory, while one left unindexed only costs the probe its posting.

use std::collections::{BTreeMap, HashMap};
use std::mem::size_of;

use zetesis_core::catalog::PredicateRef;
use zetesis_core::{Predicate, TemplateComponentsRef, TemplateTerm};

use super::Counters;
use crate::formula_conditional_ir::{Consequent, ConsequentOperand};
use crate::formula_ir::{AggregateKey, HeadIr, LiteralIr, Prepared, Projection, RuleIr};
use crate::{FormulaFailure, FormulaLimits, ProgramSite};
use themelios_program::program::DefaultNegation;

/// Demanded columns by signed predicate, computed once before support grows.
#[derive(Debug, Default)]
pub(crate) struct Demand {
    columns: HashMap<Predicate, Vec<bool>>,
}

impl Demand {
    /// Named capacity: one entry per predicate a join names, with its owned
    /// signature and column flags. Hash-table control bytes are excluded.
    pub(crate) fn retained_bytes(&self) -> usize {
        self.columns.capacity() * size_of::<(Predicate, Vec<bool>)>()
            + self
                .columns
                .iter()
                .map(|(predicate, columns)| predicate.name().len() + columns.capacity())
                .sum::<usize>()
    }

    /// The demanded columns of `predicate`; `None` when no join names it.
    pub(crate) fn columns(
        &self,
        predicate: PredicateRef<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<&[bool]>, FormulaFailure> {
        counters.charge_work(predicate.name().len() as u128 + 1, limits, location)?;
        Ok(self.columns.get(&owned(predicate)).map(Vec::as_slice))
    }

    /// Walk every rule, projection and objective of `prepared` once.
    pub(crate) fn of(
        prepared: &Prepared,
        view: TemplateComponentsRef<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let mut walk = Walk {
            view,
            limits,
            counters,
            location,
            found: HashMap::new(),
        };
        for rule in prepared.rules.iter().chain(&prepared.projection) {
            walk.rule(rule)?;
        }
        for objective in &prepared.objectives {
            for &atom in &objective.positive {
                walk.every_column(atom)?;
            }
            walk.nested(objective.condition.literals())?;
        }
        Ok(Self {
            columns: walk.found,
        })
    }
}

struct Walk<'a, 'c> {
    view: TemplateComponentsRef<'a>,
    limits: &'c FormulaLimits,
    counters: &'c mut Counters,
    location: ProgramSite,
    found: HashMap<Predicate, Vec<bool>>,
}

impl Walk<'_, '_> {
    fn work(&mut self) -> Result<(), FormulaFailure> {
        self.counters.work(self.limits, self.location)
    }

    fn rule(&mut self, rule: &RuleIr) -> Result<(), FormulaFailure> {
        match &rule.head {
            HeadIr::Choice(group) => {
                self.nested(&rule.body)?;
                for element in &group.elements {
                    self.nested(&element.condition)?;
                }
            }
            HeadIr::ConditionalDisjunction { elements, .. } => {
                self.nested(&rule.body)?;
                for element in elements {
                    self.nested(&element.condition)?;
                }
            }
            HeadIr::Normal(head) => {
                let heads: Vec<_> = head.iter().copied().collect();
                self.body(&rule.body, &heads)?;
            }
            HeadIr::Disjunction(heads) => {
                let heads: Vec<_> = heads
                    .iter()
                    .filter_map(|head| head.atom().copied())
                    .collect();
                self.body(&rule.body, &heads)?;
            }
        }
        Ok(())
    }

    /// A rule's body under an ordinary head: exact when flat, else as a
    /// nested frame. Head occurrences count, since factorization binds them.
    fn body(
        &mut self,
        literals: &[LiteralIr],
        heads: &[super::components::Pattern],
    ) -> Result<(), FormulaFailure> {
        if !literals.iter().all(flat) {
            return self.nested(literals);
        }
        let mut occurrences: BTreeMap<usize, usize> = BTreeMap::new();
        for &head in heads {
            self.work()?;
            for variable in self.pattern_variables(head)? {
                *occurrences.entry(variable).or_default() += 1;
            }
        }
        for literal in literals {
            self.work()?;
            for variable in self.variables(literal)? {
                *occurrences.entry(variable).or_default() += 1;
            }
        }
        for literal in literals {
            self.work()?;
            match literal {
                LiteralIr::Atom(DefaultNegation::None, atom) => {
                    let pattern = atom.get(self.view, self.limits, self.counters, self.location)?;
                    let terms = pattern.terms();
                    let demanded = (0..terms.len())
                        .map(|column| match terms.at(column) {
                            Some(TemplateTerm::Constant(_)) => true,
                            Some(TemplateTerm::Variable(variable)) => {
                                occurrences.get(&variable).is_some_and(|&count| count > 1)
                            }
                            None => false,
                        })
                        .collect();
                    self.record(pattern.predicate(), demanded);
                }
                LiteralIr::Atom(_, atom) => self.every_column(*atom)?,
                _ => {}
            }
        }
        Ok(())
    }

    /// A frame read conservatively, with every frame nested in it.
    fn nested(&mut self, literals: &[LiteralIr]) -> Result<(), FormulaFailure> {
        let mut pending = vec![literals];
        while let Some(literals) = pending.pop() {
            for literal in literals {
                self.work()?;
                match literal {
                    LiteralIr::Atom(_, atom) => self.every_column(*atom)?,
                    LiteralIr::PatternAtom(atom) => self.every_column(atom.atom)?,
                    LiteralIr::ProjectedAtom(_, projection) => {
                        self.projection(projection, &mut pending)?;
                    }
                    LiteralIr::Conditional(value) => {
                        pending.push(&value.condition);
                        match &value.consequent {
                            Consequent::Atoms(_, alternatives) => {
                                for alternative in alternatives {
                                    pending.push(&alternative.bindings);
                                    match &alternative.operand {
                                        ConsequentOperand::Atom(atom) => {
                                            self.every_column(*atom)?;
                                        }
                                        ConsequentOperand::Projection(projection) => {
                                            self.projection(projection, &mut pending)?;
                                        }
                                    }
                                }
                            }
                            Consequent::Guards(alternatives) => {
                                for alternative in alternatives {
                                    pending.push(&alternative.bindings);
                                }
                            }
                            Consequent::Guard(_) => {}
                        }
                    }
                    LiteralIr::Aggregate(value) => {
                        for element in &value.elements {
                            pending.push(&element.condition);
                            if let AggregateKey::Atom(atom) = &element.key {
                                self.every_column(*atom)?;
                            }
                        }
                    }
                    LiteralIr::Compare(..)
                    | LiteralIr::ArgumentCheck { .. }
                    | LiteralIr::TupleCompare(..)
                    | LiteralIr::Guard(_)
                    | LiteralIr::HeadGuard(_)
                    | LiteralIr::Bind { .. }
                    | LiteralIr::Range { .. } => {}
                }
            }
        }
        Ok(())
    }

    fn projection<'ir>(
        &mut self,
        projection: &'ir Projection,
        pending: &mut Vec<&'ir [LiteralIr]>,
    ) -> Result<(), FormulaFailure> {
        match projection {
            Projection::Arguments { predicate, .. } => {
                let predicate =
                    predicate.get(self.view, self.limits, self.counters, self.location)?;
                self.record(predicate, vec![true; predicate.arity()]);
            }
            Projection::Witnesses { atom, bindings, .. } => {
                self.every_column(*atom)?;
                pending.push(bindings);
            }
        }
        Ok(())
    }

    /// Demand every column of an occurrence that holds a constant or variable.
    fn every_column(&mut self, atom: super::components::Pattern) -> Result<(), FormulaFailure> {
        let pattern = atom.get(self.view, self.limits, self.counters, self.location)?;
        let demanded = vec![true; pattern.terms().len()];
        self.record(pattern.predicate(), demanded);
        Ok(())
    }

    fn record(&mut self, predicate: PredicateRef<'_>, demanded: Vec<bool>) {
        let known = self
            .found
            .entry(owned(predicate))
            .or_insert_with(|| vec![false; demanded.len()]);
        for (known, demanded) in known.iter_mut().zip(demanded) {
            *known |= demanded;
        }
    }

    fn pattern_variables(
        &mut self,
        atom: super::components::Pattern,
    ) -> Result<Vec<usize>, FormulaFailure> {
        let pattern = atom.get(self.view, self.limits, self.counters, self.location)?;
        let terms = pattern.terms();
        Ok((0..terms.len())
            .filter_map(|column| match terms.at(column) {
                Some(TemplateTerm::Variable(variable)) => Some(variable),
                _ => None,
            })
            .collect())
    }

    /// The variables a flat literal reads or binds, once per occurrence.
    fn variables(&mut self, literal: &LiteralIr) -> Result<Vec<usize>, FormulaFailure> {
        Ok(match literal {
            LiteralIr::Atom(_, atom) => self.pattern_variables(*atom)?,
            LiteralIr::Compare(left, _, right)
            | LiteralIr::ArgumentCheck {
                captured: left,
                value: right,
            } => left.inputs().chain(right.inputs()).collect(),
            LiteralIr::TupleCompare(left, _, right) => left
                .iter()
                .chain(right)
                .flat_map(crate::formula_ir::Expression::inputs)
                .collect(),
            LiteralIr::Bind { target, value } => {
                std::iter::once(*target).chain(value.inputs()).collect()
            }
            LiteralIr::Range {
                target,
                lower,
                upper,
                ..
            } => std::iter::once(*target)
                .chain(lower.inputs())
                .chain(upper.inputs())
                .collect(),
            _ => unreachable!("only flat literals are counted"),
        })
    }
}

/// A literal whose variables the exact reading counts.
fn flat(literal: &LiteralIr) -> bool {
    matches!(
        literal,
        LiteralIr::Atom(..)
            | LiteralIr::Compare(..)
            | LiteralIr::ArgumentCheck { .. }
            | LiteralIr::TupleCompare(..)
            | LiteralIr::Bind { .. }
            | LiteralIr::Range { .. }
    )
}

/// The owned key of a signed predicate.
fn owned(predicate: PredicateRef<'_>) -> Predicate {
    Predicate::with_sign(predicate.name(), predicate.arity(), predicate.sign())
        .expect("an admitted predicate is a valid signature")
}
