//! Filter-valid instances over one immutable relation snapshot.
//!
//! The public scan exhausts its supplied relation snapshot without deciding
//! per-world truth or frozen gates. A union can offer an instance, but a consumer
//! must check every antecedent against its own world. The private masked scan
//! narrows that offering to positive bindings with some current-world witness.
//! A callback error or source stop establishes neither coverage contract.

use std::fmt;
use zetesis_core::{Atom, AtomPattern, Model, Program, Value};

use super::{Relations, Work, visit, worlds};
use crate::{Control, Stop};

/// Bounds on a source scan, including a single emitted instance's copied data.
#[derive(Clone, Copy, Debug)]
pub struct ScanLimits {
    /// Shared scan work, using the existing lazy join operation units.
    pub max_work: u64,
    /// Maximum atoms in one instance, counting repeated antecedent occurrences.
    pub max_instance_atoms: usize,
    /// Maximum atom metadata and referenced payload copied for one instance.
    /// Allocator rounding, tree nodes and caller-owned input are excluded.
    pub max_instance_bytes: usize,
}

impl Default for ScanLimits {
    fn default() -> Self {
        Self {
            max_work: 10_000_000,
            max_instance_atoms: 4096,
            max_instance_bytes: 1024 * 1024,
        }
    }
}

/// One ground source instance; gates have deliberately not been evaluated.
/// Construction is confined to a successful binding of an admitted template.
#[derive(Debug)]
pub struct Instance {
    head: Option<Atom>,
    positive: Vec<Atom>,
    gate_true: Vec<Atom>,
    gate_false: Vec<Atom>,
}

impl Instance {
    /// Head atom, or absence for a constraint. Constant-time borrow.
    #[must_use]
    pub const fn head(&self) -> Option<&Atom> {
        self.head.as_ref()
    }

    /// Positive antecedents whose truth must be checked per world.
    #[must_use]
    pub fn positive(&self) -> &[Atom] {
        &self.positive
    }

    /// Atoms required true in the immutable seed.
    #[must_use]
    pub fn gate_true(&self) -> &[Atom] {
        &self.gate_true
    }

    /// Atoms required false in the immutable seed.
    #[must_use]
    pub fn gate_false(&self) -> &[Atom] {
        &self.gate_false
    }
}

/// Progress retained for both complete and interrupted scans.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScanStatistics {
    /// Charged template, tuple, filter and instance-copy work, including any
    /// private world-membership construction/intersections and the lazy batch's
    /// checked identity preparation and callback interning charges.
    pub work: u64,
    /// Fully matched bindings offered to the consumer, including its failure.
    pub bindings: u64,
    /// Membership probes, initialization and intersections charged within work.
    pub mask_words: u64,
    /// Matched positive prefixes with no possible current world.
    /// Their unvisited extensions are not counted as offered instances.
    pub pruned_prefixes: u64,
    /// Peak retained ordered-ID capacity plus requested membership/frame payload
    /// reached by round preparation. Interner storage is bounded separately;
    /// symbolic atoms and other allocator overhead are excluded. This is not RSS.
    pub mask_bytes: usize,
}

/// An incomplete scan, preserving the cause and already charged source work.
#[derive(Debug)]
pub struct ScanFailure<E> {
    /// Source interruption or consumer failure.
    pub cause: ScanCause<E>,
    /// Source work through the interruption.
    pub statistics: ScanStatistics,
}

/// Source and execution failures remain separate from logical rejection.
#[derive(Debug)]
pub enum ScanCause<E> {
    /// A source limit, cancellation, allocation or admitted invariant failed.
    Source(Stop),
    /// The injected consumer failed; snapshot coverage is incomplete.
    Consumer(E),
}

impl<E> From<Stop> for ScanCause<E> {
    fn from(stop: Stop) -> Self {
        Self::Source(stop)
    }
}

impl<E: fmt::Display> fmt::Display for ScanCause<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(stop) => stop.fmt(f),
            Self::Consumer(error) => write!(f, "source instance consumer failed: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ScanCause<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(stop) => Some(stop),
            Self::Consumer(error) => Some(error),
        }
    }
}

impl<E: fmt::Display> fmt::Display for ScanFailure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(f)
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ScanFailure<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

/// Visit all filter-valid instances over `snapshot`, retaining only a relation
/// index, the current join frame and one owned instance. Gates remain symbolic.
/// The snapshot is borrowed immutably for the whole scan. Repeated source
/// instances remain repeated; a consumer may coalesce consequences.
///
/// A successful return proves exhaustion only of this supplied snapshot, not
/// closure or candidate-stream exhaustion. Source work is bounded by `limits`;
/// consumer work and retained chunk storage need their own bounds. Relations
/// borrow atoms in canonical storage order and share the CPU prefix-window join.
///
/// # Errors
/// Returns the source or callback cause and charged progress. No completion
/// receipt is produced after any failed callback, including the final binding.
pub fn scan<E>(
    program: &Program,
    snapshot: &Model,
    limits: ScanLimits,
    control: &Control,
    mut consume: impl FnMut(Instance) -> Result<(), E>,
) -> Result<ScanStatistics, ScanFailure<E>> {
    let mut work = Work::source(control, limits.max_work);
    scan_rows(
        program,
        snapshot.atoms().iter(),
        limits,
        &mut work,
        |instance, _| consume(instance),
    )
}

/// Visit a canonical borrowed row selection. The batch's interner and source
/// visitor share one work owner; callback charges are retained on every exit.
pub(crate) fn scan_rows<'a, E>(
    program: &Program,
    atoms: impl Iterator<Item = &'a Atom>,
    limits: ScanLimits,
    work: &mut Work<'_>,
    mut consume: impl FnMut(Instance, &mut Work<'_>) -> Result<(), E>,
) -> Result<ScanStatistics, ScanFailure<E>> {
    let mut offered = 0;
    let result = scan_inner(
        program,
        atoms,
        None,
        limits,
        work,
        &mut offered,
        &mut consume,
    );
    let statistics = work.source_statistics(offered);
    result
        .map(|()| statistics)
        .map_err(|cause| ScanFailure { cause, statistics })
}

pub(crate) fn scan_worlds<E>(
    program: &Program,
    snapshot: &mut worlds::Snapshot<'_>,
    limits: ScanLimits,
    work: &mut Work<'_>,
    mut consume: impl FnMut(Instance, &mut Work<'_>) -> Result<(), E>,
) -> Result<ScanStatistics, ScanFailure<E>> {
    let mut offered = 0;
    let (atoms, mut membership) = snapshot.parts();
    let result = scan_inner(
        program,
        atoms,
        Some(&mut membership),
        limits,
        work,
        &mut offered,
        &mut consume,
    );
    let statistics = work.source_statistics(offered);
    result
        .map(|()| statistics)
        .map_err(|cause| ScanFailure { cause, statistics })
}

fn scan_inner<'a, E>(
    program: &Program,
    atoms: impl Iterator<Item = &'a Atom>,
    mut membership: Option<&mut worlds::Join<'_>>,
    limits: ScanLimits,
    work: &mut Work<'_>,
    offered: &mut u64,
    consume: &mut impl FnMut(Instance, &mut Work<'_>) -> Result<(), E>,
) -> Result<(), ScanCause<E>> {
    work.control.poll()?;
    let mut relations = Relations::new();
    for atom in atoms {
        work.tick()?;
        relations.push(atom);
    }
    for template in program.templates() {
        work.tick()?;
        visit(
            template,
            &relations,
            None,
            membership.as_deref_mut(),
            work,
            |assignment, work| {
                let mut remaining_atoms = limits.max_instance_atoms;
                let mut remaining_bytes = limits.max_instance_bytes;
                let mut copy = |pattern: &AtomPattern| {
                    copy_atom(
                        pattern,
                        assignment,
                        &mut remaining_atoms,
                        &mut remaining_bytes,
                        work,
                    )
                };
                let head = template.head().map(&mut copy).transpose()?;
                let positive = template
                    .positive()
                    .iter()
                    .map(&mut copy)
                    .collect::<Result<_, Stop>>()?;
                let gate_true = template
                    .gate_true()
                    .iter()
                    .map(&mut copy)
                    .collect::<Result<_, Stop>>()?;
                let gate_false = template
                    .gate_false()
                    .iter()
                    .map(&mut copy)
                    .collect::<Result<_, Stop>>()?;
                *offered += 1;
                consume(
                    Instance {
                        head,
                        positive,
                        gate_true,
                        gate_false,
                    },
                    work,
                )
                .map_err(ScanCause::Consumer)
            },
        )?;
    }
    work.control.poll()?;
    Ok(())
}

pub(super) fn copy_atom(
    pattern: &AtomPattern,
    assignment: &[Option<&Value>],
    remaining_atoms: &mut usize,
    remaining_bytes: &mut usize,
    work: &mut Work<'_>,
) -> Result<Atom, Stop> {
    *remaining_atoms = remaining_atoms.checked_sub(1).ok_or(Stop::CarrierLimit)?;
    let mut bytes = size_of::<Atom>()
        .checked_add(pattern.predicate().name().len())
        .ok_or(Stop::Allocation)?;
    for term in pattern.terms() {
        let value = super::resolve(term, assignment).ok_or(Stop::InvalidProgram)?;
        bytes = bytes
            .checked_add(size_of::<Value>())
            .and_then(|n| n.checked_add(value.payload_bytes()))
            .ok_or(Stop::Allocation)?;
    }
    *remaining_bytes = remaining_bytes.checked_sub(bytes).ok_or(Stop::Allocation)?;
    work.charge(bytes)?;
    pattern
        .key(assignment)
        .map(zetesis_core::AtomKey::to_atom)
        .map_err(|_| Stop::InvalidProgram)
}
