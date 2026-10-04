//! Static components borrow the committed prefix, not the mutable append lane.

use super::{
    Computation, Counters, FormulaFailure, FormulaLimits, ProgramSite, TermKey, TermRef,
    ValueNodeRef,
};
use crate::formula_support::components::{
    self, Constructor, Filter, Pattern, Predicate, Scalar, Term,
};
use zetesis_core::catalog::PredicateRef;
use zetesis_core::{FilterRef, PatternRef, TemplateComponentsRef, TemplateTerm};

impl<'source> Computation<'_, 'source> {
    pub(crate) fn static_predicate(
        &self,
        predicate: Predicate,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<PredicateRef<'source>, FormulaFailure> {
        predicate.get(
            self.static_components(location)?,
            limits,
            counters,
            location,
        )
    }

    pub(crate) fn static_pattern(
        &self,
        pattern: Pattern,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<PatternRef<'source>, FormulaFailure> {
        pattern.get(
            self.static_components(location)?,
            limits,
            counters,
            location,
        )
    }

    pub(crate) fn static_filter(
        &self,
        filter: Filter,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<FilterRef<'source>, FormulaFailure> {
        filter.get(
            self.static_components(location)?,
            limits,
            counters,
            location,
        )
    }

    pub(crate) fn static_term(
        &self,
        term: Term,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<TemplateTerm<'source>, FormulaFailure> {
        term.get(
            self.static_components(location)?,
            limits,
            counters,
            location,
        )
    }

    pub(crate) fn static_components(
        &self,
        location: ProgramSite,
    ) -> Result<TemplateComponentsRef<'source>, FormulaFailure> {
        self.support
            .components()
            .ok_or_else(|| components::missing(location))
    }

    pub(crate) fn static_scalar(
        &self,
        scalar: Scalar,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<TermRef<'source>, FormulaFailure> {
        scalar.get(
            self.static_components(location)?,
            limits,
            counters,
            location,
        )
    }

    pub(crate) fn static_key(
        &self,
        scalar: Scalar,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<TermKey, FormulaFailure> {
        let value = self.static_scalar(scalar, limits, counters, location)?;
        counters.work(limits, location)?;
        self.read()
            .term_key(value)
            .map_err(|error| crate::formula_binding::assignment(error.into(), location))
    }

    pub(crate) fn static_constructor(
        &self,
        constructor: Constructor,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<ValueNodeRef<'source>, FormulaFailure> {
        constructor.get(
            self.static_components(location)?,
            limits,
            counters,
            location,
        )
    }
}
