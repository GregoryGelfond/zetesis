//! Temporary template topology names canonical terms; it owns no ASP payload.

use crate::formula_support::Context;

use crate::ProgramSite;
use crate::formula_support::components::{Filter, Pattern as AtomPattern, Term};
use std::ops::Range;
use zetesis_core::catalog::{AssignmentError, DeclaredPredicate, TermKey};
use zetesis_core::{FilterRef, PatternRef, TemplateTerm};

use crate::formula_binding::Binding;
use crate::formula_support::{Buffer, Computation, Counters};
use crate::{FormulaFailure, FormulaLimits};

#[derive(Clone, Copy)]
enum Field {
    Variable(usize),
    Constant(usize),
}
struct Pattern {
    predicate: DeclaredPredicate,
    fields: Range<usize>,
}
struct Comparison {
    equal: bool,
    left: usize,
    right: usize,
}

pub(super) struct Components {
    constants: Binding<'static>,
    fields: Buffer<Field>,
    patterns: Buffer<Pattern>,
    comparisons: Buffer<Comparison>,
    scalars: usize,
}
impl Components {
    pub(super) fn new(
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        Ok(Self {
            constants: Binding::new(computation, limits, counters, location)?,
            fields: Buffer::new(computation, limits, counters, location)?,
            patterns: Buffer::new(computation, limits, counters, location)?,
            comparisons: Buffer::new(computation, limits, counters, location)?,
            scalars: 0,
        })
    }
    pub(super) fn scalar(
        &mut self,
        key: &TermKey,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        self.constant(key, computation, limits, counters, location)?;
        self.scalars += 1;
        Ok(())
    }
    pub(super) fn lifted_scalar(
        &mut self,
        term: &Term,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let term = computation.static_term(*term, limits, counters, location)?;
        self.term(term, None, computation, limits, counters, location)?;
        self.scalars += 1;
        Ok(())
    }
    pub(super) fn pattern(
        &mut self,
        pattern: AtomPattern,
        binding: Option<&Binding>,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let pattern = computation.static_pattern(pattern, limits, counters, location)?;
        counters.work(limits, location)?;
        let predicate = computation
            .read()
            .declare_existing(pattern.predicate())
            .map_err(|error| crate::formula_binding::assignment(error.into(), location))?;
        let start = self.fields.len();
        let terms = pattern.terms();
        for index in 0..terms.len() {
            counters.work(limits, location)?;
            let term = terms.at(index).expect("checked pattern arity");
            self.term(term, binding, computation, limits, counters, location)?;
        }
        self.patterns.push(
            Pattern {
                predicate,
                fields: start..self.fields.len(),
            },
            computation,
            limits,
            counters,
            location,
        )
    }
    pub(super) fn filter(
        &mut self,
        filter: Filter,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let filter = computation.static_filter(filter, limits, counters, location)?;
        let (left, right) = filter.terms();
        let start = self.fields.len();
        self.term(left, None, computation, limits, counters, location)?;
        self.term(right, None, computation, limits, counters, location)?;
        self.comparisons.push(
            Comparison {
                equal: filter.is_equality(),
                left: start,
                right: start + 1,
            },
            computation,
            limits,
            counters,
            location,
        )
    }
    fn term(
        &mut self,
        term: TemplateTerm<'_>,
        binding: Option<&Binding>,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let key = match term {
            TemplateTerm::Variable(variable) => match binding {
                Some(binding) => binding.key(variable, location)?,
                None => {
                    return self.fields.push(
                        Field::Variable(variable),
                        computation,
                        limits,
                        counters,
                        location,
                    );
                }
            },
            TemplateTerm::Constant(value) => {
                counters.work(limits, location)?;
                computation
                    .read()
                    .term_key(value)
                    .map_err(|error| crate::formula_binding::assignment(error.into(), location))?
            }
        };
        self.constant(&key, computation, limits, counters, location)
    }
    fn constant(
        &mut self,
        key: &TermKey,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let slot = self.constants.len();
        self.constants
            .extend_scope(slot + 1, computation, limits, counters, location)?;
        self.constants.set(slot, key, limits, counters, location)?;
        self.fields.push(
            Field::Constant(slot),
            computation,
            limits,
            counters,
            location,
        )
    }
    pub(super) fn append(
        self,
        rows: &mut super::Rows,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        let read = computation.read();
        let mut fields = Buffer::new(computation, limits, counters, location)?;
        for field in self.fields.iter() {
            counters.work(limits, location)?;
            let field = match *field {
                Field::Variable(slot) => TemplateTerm::Variable(slot),
                Field::Constant(slot) => {
                    TemplateTerm::Constant(self.constants.read(slot, read, location)?)
                }
            };
            fields.push(field, computation, limits, counters, location)?;
        }
        let mut patterns = Buffer::new(computation, limits, counters, location)?;
        for pattern in self.patterns.iter() {
            counters.work(limits, location)?;
            let predicate = read.predicate(&pattern.predicate).map_err(|error| {
                crate::formula_binding::assignment(AssignmentError::Read(error), location)
            })?;
            let pattern =
                PatternRef::from_parts(predicate, &fields.slice()[pattern.fields.clone()])
                    .expect("canonical fields preserve the admitted pattern arity");
            patterns.push(pattern, computation, limits, counters, location)?;
        }
        let filters = self.comparisons.iter().map(|filter| {
            FilterRef::from_parts(
                filter.equal,
                fields.slice()[filter.left],
                fields.slice()[filter.right],
            )
        });
        rows.append(
            read,
            fields.slice()[..self.scalars].iter().copied(),
            patterns.iter().copied(),
            filters,
            Context::new(computation, limits, counters, location),
        )
    }
}
