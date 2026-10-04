//! Checked compiler metadata over already admitted scalar occurrences.

use super::{
    Admission, CatalogRead, Counters, Event, Filter, FilterRef, FormulaFailure, FormulaLimits,
    FormulaResource, Pattern, PatternRef, Predicate, PredicateRef, ProgramSite, StorageLease,
    TemplateCatalogFailure, TemplateComponentsRef, TemplateTerm, Term, atom_failure, ceiling,
    failure, missing, owner_limits, read_failure, remaining_u128, size_of,
};

impl Admission<'_> {
    pub(crate) fn predicate(
        &mut self,
        predicate: PredicateRef<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Predicate, FormulaFailure> {
        let outer = self.total(location)? as u128 - self.catalog.owner.storage_bytes();
        let checked = owner_limits(limits, outer, location)?;
        self.catalog.owner.restart_storage_peak();
        let result = self
            .catalog
            .owner
            .declare_predicate_with(predicate, checked, || counters.work(limits, location));
        let observed = self.owner_observed(outer, limits, counters, location);
        let declaration = result.map_err(|error| atom_failure(error, limits, outer, location))?;
        observed?;
        self.append(
            limits,
            counters,
            location,
            |components, read, allowance, counters| {
                components
                    .append_predicate_with(read, &declaration, allowance, || {
                        counters.work(limits, location)
                    })
                    .map(Predicate)
            },
        )
    }

    pub(crate) fn pattern(
        &mut self,
        predicate: Predicate,
        arguments: &[Term],
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Pattern, FormulaFailure> {
        let mut terms = Vec::new();
        let mut lease = self.lease();
        let header = size_of::<Vec<TemplateTerm<'_>>>() + size_of::<StorageLease>();
        lease.observe(header, location)?;
        self.storage_observed(&lease, 0, header, limits, counters, location)?;
        let required =
            header as u128 + arguments.len() as u128 * size_of::<TemplateTerm<'_>>() as u128;
        let outer_scratch = (self.total(location)? - lease.bytes()) as u128;
        ceiling(
            FormulaResource::SupportBytes,
            outer_scratch + required,
            limits.max_support_bytes as u128,
            location,
        )?;
        counters.work(limits, location)?;
        let reserved = terms.try_reserve_exact(arguments.len());
        lease.observe(
            header + terms.capacity() * size_of::<TemplateTerm<'_>>(),
            location,
        )?;
        let observed = self.storage_observed(&lease, header, header, limits, counters, location);
        reserved.map_err(|error| FormulaFailure::AtomAllocation { error, location })?;
        observed?;

        let components = self
            .catalog
            .components
            .as_ref()
            .ok_or_else(|| missing(location))?;
        let outer = self.total(location)? as u128 - components.storage_bytes();
        let allowance = remaining_u128(outer, limits, location)?;
        let read = self.catalog.owner.read();
        let bound = components
            .bind_with(read, || counters.work(limits, location))
            .map_err(|error| failure(error, limits, outer, location))?;
        let predicate = predicate.get(bound, limits, counters, location)?;
        counters.work(limits, location)?;
        let declaration = read
            .declare_existing(predicate)
            .map_err(|error| read_failure(error, location))?;
        counters.work(limits, location)?;
        let predicate = read
            .predicate(&declaration)
            .map_err(|error| read_failure(error, location))?;
        for argument in arguments {
            counters.work(limits, location)?;
            let value = detached(*argument, bound, read, limits, counters, location)?;
            counters.work(limits, location)?;
            terms.push(value);
        }
        let pattern = PatternRef::from_parts(predicate, &terms)
            .map_err(|error| crate::AdmissionFailure::Construction { error, location })?;
        let components = self
            .catalog
            .components
            .as_mut()
            .ok_or_else(|| missing(location))?;
        components.restart_storage_peak();
        let result = components
            .append_pattern_with(read, pattern, allowance, || counters.work(limits, location));
        let peak = outer + components.storage_peak_bytes();
        counters.record(Event::SupportPeakBytes(peak));
        let observed = ceiling(
            FormulaResource::SupportBytes,
            peak,
            limits.max_support_bytes as u128,
            location,
        );
        let position = result.map_err(|error| failure(error, limits, outer, location))?;
        observed?;
        Ok(Pattern(position))
    }

    pub(crate) fn filter(
        &mut self,
        equal: bool,
        left: Term,
        right: Term,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Filter, FormulaFailure> {
        self.append(
            limits,
            counters,
            location,
            |components, read, allowance, counters| {
                let bound = components.bind_with(read, || counters.work(limits, location))?;
                let left = detached(left, bound, read, limits, counters, location)
                    .map_err(TemplateCatalogFailure::Stopped)?;
                let right = detached(right, bound, read, limits, counters, location)
                    .map_err(TemplateCatalogFailure::Stopped)?;
                let filter = FilterRef::from_parts(equal, left, right);
                components
                    .append_filter_with(read, filter, allowance, || counters.work(limits, location))
                    .map(Filter)
            },
        )
    }

    pub(crate) fn pattern_ref(
        &self,
        pattern: Pattern,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<PatternRef<'_>, FormulaFailure> {
        pattern.get(
            self.bound(limits, counters, location)?,
            limits,
            counters,
            location,
        )
    }
}

// Detach the short component-metadata borrow without copying payload. Each
// transient key is immediately resolved through the independently borrowed
// canonical reader and dropped; argument scratch retains no per-cell witness.
fn detached<'read>(
    term: Term,
    components: TemplateComponentsRef<'_>,
    read: CatalogRead<'read>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<TemplateTerm<'read>, FormulaFailure> {
    match term.get(components, limits, counters, location)? {
        TemplateTerm::Variable(variable) => Ok(TemplateTerm::Variable(variable)),
        TemplateTerm::Constant(value) => {
            counters.work(limits, location)?;
            let key = read
                .term_key(value)
                .map_err(|error| read_failure(error, location))?;
            counters.work(limits, location)?;
            read.term(&key)
                .map(TemplateTerm::Constant)
                .map_err(|error| read_failure(error, location))
        }
    }
}

impl Term {
    pub(crate) fn get<'a>(
        self,
        view: TemplateComponentsRef<'a>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<TemplateTerm<'a>, FormulaFailure> {
        match self {
            Self::Variable(variable) => {
                counters.work(limits, location)?;
                Ok(TemplateTerm::Variable(variable))
            }
            Self::Constant(scalar) => scalar
                .get(view, limits, counters, location)
                .map(TemplateTerm::Constant),
        }
    }
}
impl Predicate {
    pub(crate) fn get<'a>(
        self,
        view: TemplateComponentsRef<'a>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<PredicateRef<'a>, FormulaFailure> {
        counters.work(limits, location)?;
        view.predicate(self.0).ok_or_else(|| missing(location))
    }
}
impl Pattern {
    pub(crate) fn get<'a>(
        self,
        view: TemplateComponentsRef<'a>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<PatternRef<'a>, FormulaFailure> {
        self.get_with(view, location, || counters.work(limits, location))
    }

    pub(crate) fn get_with(
        self,
        view: TemplateComponentsRef<'_>,
        location: ProgramSite,
        before: impl FnOnce() -> Result<(), FormulaFailure>,
    ) -> Result<PatternRef<'_>, FormulaFailure> {
        before()?;
        view.pattern(self.0).ok_or_else(|| missing(location))
    }
}
impl Filter {
    pub(crate) fn get<'a>(
        self,
        view: TemplateComponentsRef<'a>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<FilterRef<'a>, FormulaFailure> {
        counters.work(limits, location)?;
        view.filter(self.0).ok_or_else(|| missing(location))
    }
}
