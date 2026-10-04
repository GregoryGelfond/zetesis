//! Private compiled occurrences over the source's canonical vocabulary.
//!
//! Coordinates are paired with their owning Preparation, never transported as
//! canonical IDs. Compilation may append components; runtime only borrows the
//! committed prefix. Components select neither explicit domain roots nor truth.

mod patterns;

use std::mem::size_of;

use crate::ProgramSite;
use zetesis_core::catalog::{
    AssignmentFailure, CatalogRead, Error, PredicateRef, ReadError, TermKey, TermRef,
};
use zetesis_core::{
    FilterRef, PatternRef, TemplateCatalogFailure, TemplateComponents, TemplateComponentsRef,
    TemplateTerm, ValueNodeRef,
};

use super::relations::{atom_failure, owner_limits};
use super::storage::{Scope, StorageLease, Workspace};
use super::{Counters, SupportCatalog};
use crate::formula::ceiling;
use crate::grounding_observer::Event;
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Scalar(usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Constructor(usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Predicate(usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Pattern(usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Filter(usize);

/// Source-variable coordinates and compiled scalar occurrences. Equality here
/// compares metadata positions; constant semantic equality requires resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Term {
    Variable(usize),
    Constant(Scalar),
}

/// The compiler borrows the retained source authority and its cumulative
/// workspace. Temporary keys and metadata may borrow this capability; no public
/// method returns a raw canonical ID or makes a second payload owner.
pub(crate) struct Admission<'a> {
    catalog: &'a mut SupportCatalog,
    workspace: Workspace,
    _header: StorageLease,
}

impl SupportCatalog {
    pub(crate) fn component_view(
        &self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<TemplateComponentsRef<'_>>, FormulaFailure> {
        self.components
            .as_ref()
            .map(|components| {
                components
                    .bind_with(self.owner.read(), || counters.work(limits, location))
                    .map_err(|error| failure(error, limits, 0, location))
            })
            .transpose()
    }

    pub(crate) fn component_admission(
        &mut self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Admission<'_>, FormulaFailure> {
        counters.work(limits, location)?;
        let workspace = counters.accounting.workspace.clone();
        let mut header = workspace.lease();
        header.observe(size_of::<Admission<'_>>(), location)?;
        let admission = Admission {
            catalog: self,
            workspace,
            _header: header,
        };
        admission.check(limits, counters, location)?;
        if admission.catalog.components.is_none() {
            let outer = admission.total(location)? - size_of::<TemplateComponents>();
            let allowance = remaining(outer, limits, location)?;
            let components = TemplateComponents::new(admission.read(), allowance)
                .map_err(|error| failure(infallible(error), limits, outer as u128, location))?;
            admission.catalog.components = Some(components);
        }
        Ok(admission)
    }
}

impl Admission<'_> {
    pub(crate) fn read(&self) -> CatalogRead<'_> {
        self.catalog.owner.read()
    }

    pub(crate) fn lease(&self) -> StorageLease {
        self.workspace.lease()
    }

    fn total(&self, location: ProgramSite) -> Result<usize, FormulaFailure> {
        self.catalog
            .bytes(location)?
            .checked_add(self.workspace.bytes())
            .ok_or_else(|| storage_overflow(location))
    }

    fn check(
        &self,
        limits: &FormulaLimits,
        counters: &Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let total = self.total(location)?;
        counters.record(Event::SupportPeakBytes(total as u128));
        ceiling(
            FormulaResource::SupportBytes,
            total as u128,
            limits.max_support_bytes as u128,
            location,
        )
    }

    /// Canonical and compiled-component bytes outside this entire workspace.
    /// Authenticate the selected lease before inspecting the owner capacities.
    pub(crate) fn external_bytes(
        &self,
        lease: &StorageLease,
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        if !self.workspace.owns(lease) {
            return Err(read_failure(ReadError::ForeignCatalog, location));
        }
        Ok(self.total(location)? - self.workspace.bytes())
    }

    fn storage<'a>(
        &'a self,
        lease: &'a StorageLease,
        location: ProgramSite,
    ) -> Result<Scope<'a>, FormulaFailure> {
        let external = self.external_bytes(lease, location)?;
        self.workspace
            .scope(lease, external)
            .map_err(|error| read_failure(error, location))
    }

    pub(crate) fn allowance(
        &self,
        lease: &StorageLease,
        limits: &FormulaLimits,
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        self.storage(lease, location)?.allowance(limits, location)
    }

    pub(crate) fn storage_observed(
        &self,
        lease: &StorageLease,
        previous: usize,
        header: usize,
        limits: &FormulaLimits,
        counters: &Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        self.storage(lease, location)?
            .observed(previous, header, limits, counters, location)
    }

    pub(crate) fn storage_result<T>(
        &self,
        result: Result<T, AssignmentFailure<FormulaFailure>>,
        lease: &StorageLease,
        limits: &FormulaLimits,
        location: ProgramSite,
    ) -> Result<T, FormulaFailure> {
        self.storage(lease, location)?
            .result(result, limits, location)
    }

    pub(crate) fn import(
        &mut self,
        value: TermRef<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<TermKey, FormulaFailure> {
        let outer = self.total(location)? as u128 - self.catalog.owner.storage_bytes();
        let checked = owner_limits(limits, outer, location)?;
        self.catalog.owner.restart_storage_peak();
        let result = self.catalog.owner.appender().import_term_with(
            value,
            zetesis_core::catalog::Limits {
                max_nodes: usize::MAX,
                max_depth: usize::MAX,
                max_bytes: usize::MAX,
            },
            checked,
            || counters.work(limits, location),
        );
        let observed = self.owner_observed(outer, limits, counters, location);
        let key = result.map_err(|error| atom_failure(error, limits, outer, location))?;
        observed?;
        Ok(key)
    }

    fn owner_observed(
        &self,
        outer: u128,
        limits: &FormulaLimits,
        counters: &Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let peak = outer + self.catalog.owner.storage_peak_bytes();
        counters.record(Event::SupportPeakBytes(peak));
        ceiling(
            FormulaResource::SupportBytes,
            peak,
            limits.max_support_bytes as u128,
            location,
        )
    }

    fn append<T>(
        &mut self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
        operation: impl FnOnce(
            &mut TemplateComponents,
            CatalogRead<'_>,
            usize,
            &mut Counters,
        ) -> Result<T, TemplateCatalogFailure<FormulaFailure>>,
    ) -> Result<T, FormulaFailure> {
        let components = self
            .catalog
            .components
            .as_ref()
            .ok_or_else(|| missing(location))?;
        let outer = self.total(location)? as u128 - components.storage_bytes();
        let allowance = remaining_u128(outer, limits, location)?;
        let components = self
            .catalog
            .components
            .as_mut()
            .ok_or_else(|| missing(location))?;
        components.restart_storage_peak();
        let result = operation(components, self.catalog.owner.read(), allowance, counters);
        let peak = outer + components.storage_peak_bytes();
        counters.record(Event::SupportPeakBytes(peak));
        let observed = ceiling(
            FormulaResource::SupportBytes,
            peak,
            limits.max_support_bytes as u128,
            location,
        );
        let value = result.map_err(|error| failure(error, limits, outer, location))?;
        observed?;
        Ok(value)
    }

    pub(crate) fn scalar_key(
        &mut self,
        key: &TermKey,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Scalar, FormulaFailure> {
        self.append(
            limits,
            counters,
            location,
            |components, read, allowance, counters| {
                counters
                    .work(limits, location)
                    .map_err(TemplateCatalogFailure::Stopped)?;
                let value = read.term(key).map_err(TemplateCatalogFailure::Read)?;
                components
                    .append_terms_with(read, [TemplateTerm::Constant(value)], allowance, || {
                        counters.work(limits, location)
                    })
                    .map(|range| Scalar(range.start))
            },
        )
    }

    pub(crate) fn scalar(
        &mut self,
        value: TermRef<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Scalar, FormulaFailure> {
        let key = self.import(value, limits, counters, location)?;
        self.scalar_key(&key, limits, counters, location)
    }

    fn bound(
        &self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<TemplateComponentsRef<'_>, FormulaFailure> {
        self.catalog
            .components
            .as_ref()
            .ok_or_else(|| missing(location))?
            .bind_with(self.read(), || counters.work(limits, location))
            .map_err(|error| failure(error, limits, 0, location))
    }

    /// Bind once for a read-only compiler pass. The resulting borrow cannot
    /// overlap another append; callers retain only coordinates between passes.
    pub(crate) fn components(
        &self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<TemplateComponentsRef<'_>, FormulaFailure> {
        self.bound(limits, counters, location)
    }

    pub(crate) fn scalar_ref(
        &self,
        scalar: Scalar,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<TermRef<'_>, FormulaFailure> {
        let view = self.bound(limits, counters, location)?;
        scalar.get(view, limits, counters, location)
    }

    pub(crate) fn constructor(
        &mut self,
        descriptor: ValueNodeRef<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Constructor, FormulaFailure> {
        let outer = self.total(location)? as u128 - self.catalog.owner.storage_bytes();
        let checked = owner_limits(limits, outer, location)?;
        self.catalog.owner.restart_storage_peak();
        let result =
            self.catalog
                .owner
                .appender()
                .declare_constructor_with(descriptor, checked, || counters.work(limits, location));
        let observed = self.owner_observed(outer, limits, counters, location);
        let declaration = result.map_err(|error| atom_failure(error, limits, outer, location))?;
        observed?;
        self.append(
            limits,
            counters,
            location,
            |components, read, allowance, counters| {
                components
                    .append_constructor_with(read, &declaration, allowance, || {
                        counters.work(limits, location)
                    })
                    .map(Constructor)
            },
        )
    }

    pub(crate) fn negate_constructor(
        &mut self,
        constructor: Constructor,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<Constructor>, FormulaFailure> {
        let bound = self.bound(limits, counters, location)?;
        counters.work(limits, location)?;
        let descriptor = bound
            .constructor(constructor.0)
            .ok_or_else(|| missing(location))?;
        let ValueNodeRef::Function { sign, .. } = descriptor else {
            return Ok(None);
        };
        counters.work(limits, location)?;
        let declaration = bound
            .declared_constructor(constructor.0)
            .ok_or_else(|| missing(location))?
            .with_sign(match sign {
                zetesis_core::Sign::Positive => zetesis_core::Sign::Negative,
                zetesis_core::Sign::Negative => zetesis_core::Sign::Positive,
            })
            .ok_or_else(|| missing(location))?;
        self.append(
            limits,
            counters,
            location,
            |components, read, allowance, counters| {
                components
                    .append_constructor_with(read, &declaration, allowance, || {
                        counters.work(limits, location)
                    })
                    .map(|position| Some(Constructor(position)))
            },
        )
    }

    /// Publication seals even a text/term-only prefix. Static metadata is bound
    /// only after this succeeds; a stopped compiler never creates a Prepared.
    pub(crate) fn finish(
        self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let outer = self.total(location)? as u128 - self.catalog.owner.storage_bytes();
        let checked = owner_limits(limits, outer, location)?;
        self.catalog.owner.restart_storage_peak();
        let result = self
            .catalog
            .owner
            .commit_with(checked, || counters.work(limits, location));
        let observed = self.owner_observed(outer, limits, counters, location);
        result.map_err(|error| atom_failure(error, limits, outer, location))?;
        observed?;
        self.catalog
            .components
            .as_ref()
            .ok_or_else(|| missing(location))?
            .bind_with(self.catalog.owner.committed().read(), || {
                counters.work(limits, location)
            })
            .map_err(|error| failure(error, limits, 0, location))?;
        Ok(())
    }
}

impl Scalar {
    pub(crate) fn get<'a>(
        self,
        view: TemplateComponentsRef<'a>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<TermRef<'a>, FormulaFailure> {
        counters.work(limits, location)?;
        match view.term(self.0) {
            Some(TemplateTerm::Constant(value)) => Ok(value),
            _ => Err(missing(location)),
        }
    }
}
impl Constructor {
    pub(crate) fn get<'a>(
        self,
        view: TemplateComponentsRef<'a>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<ValueNodeRef<'a>, FormulaFailure> {
        counters.work(limits, location)?;
        view.constructor(self.0).ok_or_else(|| missing(location))
    }
}

pub(crate) fn missing(location: ProgramSite) -> FormulaFailure {
    FormulaFailure::TemplateCatalog {
        error: TemplateCatalogFailure::Incomplete,
        location,
    }
}
fn read_failure(error: ReadError, location: ProgramSite) -> FormulaFailure {
    FormulaFailure::TemplateCatalog {
        error: TemplateCatalogFailure::Read(error),
        location,
    }
}
fn storage_overflow(location: ProgramSite) -> FormulaFailure {
    FormulaFailure::TemplateCatalog {
        error: TemplateCatalogFailure::Storage(Error::Overflow),
        location,
    }
}
fn remaining(
    outer: usize,
    limits: &FormulaLimits,
    location: ProgramSite,
) -> Result<usize, FormulaFailure> {
    remaining_u128(outer as u128, limits, location)
}
fn remaining_u128(
    outer: u128,
    limits: &FormulaLimits,
    location: ProgramSite,
) -> Result<usize, FormulaFailure> {
    ceiling(
        FormulaResource::SupportBytes,
        outer,
        limits.max_support_bytes as u128,
        location,
    )?;
    Ok(
        limits.max_support_bytes
            - usize::try_from(outer).map_err(|_| storage_overflow(location))?,
    )
}

pub(crate) fn failure(
    error: TemplateCatalogFailure<FormulaFailure>,
    limits: &FormulaLimits,
    outer: u128,
    location: ProgramSite,
) -> FormulaFailure {
    match error {
        TemplateCatalogFailure::Stopped(error) => error,
        TemplateCatalogFailure::Storage(Error::Storage { required, .. }) => FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            observed: outer + required,
            limit: limits.max_support_bytes as u128,
            location,
        },
        TemplateCatalogFailure::Storage(error) => FormulaFailure::TemplateCatalog {
            error: TemplateCatalogFailure::Storage(error),
            location,
        },
        TemplateCatalogFailure::Read(error) => read_failure(error, location),
        TemplateCatalogFailure::Incomplete => missing(location),
    }
}

fn infallible(error: TemplateCatalogFailure) -> TemplateCatalogFailure<FormulaFailure> {
    match error {
        TemplateCatalogFailure::Read(error) => TemplateCatalogFailure::Read(error),
        TemplateCatalogFailure::Storage(error) => TemplateCatalogFailure::Storage(error),
        TemplateCatalogFailure::Incomplete => TemplateCatalogFailure::Incomplete,
        TemplateCatalogFailure::Stopped(never) => match never {},
    }
}

#[cfg(test)]
mod tests;
