//! Relational operators Bind, Filter, Project, Union, and Least.
//!
//! These are the executable counterparts of the Lean relations in
//! `Zetesis.Lifted` and `Core.Union` / `ComponentClosure`. They stay generic
//! over integer identities (numeric values or caller-agreed atom keys) and
//! take no `zetesis-themelios` or `zetesis-core` types. Effects go through
//! the existing charge callback, which callers connect to [`crate::Meter`]
//! and [`crate::Cancellation`].
//!
//! Columnar relations are the default representation. CPU kernels are written
//! as simple loops over column slices so the compiler can auto-vectorize;
//! hand-written SIMD is reserved for a later comparison that measurably beats
//! that baseline. A GPU realization of the same relations is designed in
//! [`gpu`], and ships later behind this API; the CPU path remains.

mod arithmetic;
mod enumerate;
mod filter;
mod gpu;
mod least;
mod project;
mod relation;
mod template;
mod union;

#[cfg(test)]
mod tests;

pub use arithmetic::EnumerationStats;
pub use filter::filter;
pub use gpu::ColumnLayout;
pub use least::least;
pub use project::project;
pub use relation::{Atoms, Bindings, LayoutError};
pub use template::{Schedule, Template, TemplateError};
pub use union::union;

/// Named Bind / Filter / Project / Union / Least over columnar relations.
///
/// The CPU implementation is [`Cpu`]. A later GPU backend implements the
/// same trait over the layout in [`gpu`]; it is not a second semantics.
pub trait RelationalOps {
    /// Positive-body Bind, including the empty-body binding and any
    /// generating arithmetic scheduled on `template`.
    ///
    /// # Errors
    /// Returns the first account refusal. No partial bindings are provided.
    fn bind<E>(
        &self,
        template: &Template,
        support: &Atoms,
        account: &mut impl FnMut(u64) -> Result<(), E>,
    ) -> Result<Bindings, E>;

    /// Residual Filter: `R b ∧ t.filter b`.
    ///
    /// # Errors
    /// Returns the first account refusal.
    fn filter<E>(
        &self,
        template: &Template,
        bindings: Bindings,
        account: &mut impl FnMut(u64) -> Result<(), E>,
    ) -> Result<Bindings, E>;

    /// Existential Project, coalescing duplicate heads.
    ///
    /// # Errors
    /// Returns the first account refusal, or [`TemplateError::MissingHead`].
    fn project<E>(
        &self,
        template: &Template,
        bindings: &Bindings,
        account: &mut impl FnMut(u64) -> Result<(), E>,
    ) -> Result<Result<Atoms, TemplateError>, E>;

    /// Set union of two atom collections of equal width.
    ///
    /// # Errors
    /// Returns the first account refusal.
    ///
    /// # Panics
    /// Panics if the widths differ.
    fn union<E>(
        &self,
        left: &Atoms,
        right: &Atoms,
        account: &mut impl FnMut(u64) -> Result<(), E>,
    ) -> Result<Atoms, E>;

    /// Least pre-fixpoint of `transform`, starting from the empty set of
    /// `width` attributes.
    ///
    /// # Errors
    /// Returns the first account or transform refusal.
    fn least<E, A, F>(&self, width: usize, transform: F, account: &mut A) -> Result<Atoms, E>
    where
        A: FnMut(u64) -> Result<(), E>,
        F: FnMut(&Atoms, &mut A) -> Result<Atoms, E>;
}

/// CPU columnar realization of [`RelationalOps`].
#[derive(Clone, Copy, Debug, Default)]
pub struct Cpu;

impl RelationalOps for Cpu {
    fn bind<E>(
        &self,
        template: &Template,
        support: &Atoms,
        account: &mut impl FnMut(u64) -> Result<(), E>,
    ) -> Result<Bindings, E> {
        let _ = support;
        let mut stats = EnumerationStats::default();
        enumerate::rows(template, account, &mut stats)
    }

    fn filter<E>(
        &self,
        template: &Template,
        bindings: Bindings,
        account: &mut impl FnMut(u64) -> Result<(), E>,
    ) -> Result<Bindings, E> {
        let mut stats = EnumerationStats::default();
        filter(template, bindings, account, &mut stats)
    }

    fn project<E>(
        &self,
        template: &Template,
        bindings: &Bindings,
        account: &mut impl FnMut(u64) -> Result<(), E>,
    ) -> Result<Result<Atoms, TemplateError>, E> {
        project(template, bindings, account)
    }

    fn union<E>(
        &self,
        left: &Atoms,
        right: &Atoms,
        account: &mut impl FnMut(u64) -> Result<(), E>,
    ) -> Result<Atoms, E> {
        union(left, right, account)
    }

    fn least<E, A, F>(&self, width: usize, transform: F, account: &mut A) -> Result<Atoms, E>
    where
        A: FnMut(u64) -> Result<(), E>,
        F: FnMut(&Atoms, &mut A) -> Result<Atoms, E>,
    {
        least(width, transform, account)
    }
}

/// Filter ∘ Bind: retained bindings only, under the template's schedule.
///
/// Failed pairs are visited in the filter-only schedule (and counted in
/// [`EnumerationStats::candidates`]) but are never retained. The generating
/// schedule does not walk those pairs. `support` is reserved for a later
/// fact-from-support Bind; range and fact domains currently live on the
/// template.
///
/// # Errors
/// Returns the first account refusal.
pub fn rows<E>(
    template: &Template,
    support: &Atoms,
    account: &mut impl FnMut(u64) -> Result<(), E>,
    stats: &mut EnumerationStats,
) -> Result<Bindings, E> {
    let _ = support;
    enumerate::rows(template, account, stats)
}

/// Bind under the template's schedule, recording enumeration statistics.
///
/// This is the same fused retention as [`rows`]: even the filter-only
/// schedule drops failed residual tests before they become rows, so Project
/// never sees them.
///
/// # Errors
/// Returns the first account refusal.
pub fn bind<E>(
    template: &Template,
    support: &Atoms,
    account: &mut impl FnMut(u64) -> Result<(), E>,
    stats: &mut EnumerationStats,
) -> Result<Bindings, E> {
    rows(template, support, account, stats)
}
