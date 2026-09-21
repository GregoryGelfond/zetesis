//! Cardinality consequences conditional on explicitly stated finite premises.
//!
//! A [`Plan`] validates a partition's shape and computes local lower bounds.
//! It does **not** establish that an original theory entails the supplied total
//! or group capacities. A caller may restrict complete candidate enumeration
//! only after independently establishing those premises for that original
//! theory. Conditional aggregate groups require a common, established activation;
//! passing their unconditional capacities here does not discharge that obligation.

use std::fmt;

use zetesis_cpu::{Cancellation, Stop};

mod restriction;
pub use restriction::{Restriction, RestrictionError, RestrictionErrorKind, RestrictionLimits};

/// A stated upper cardinality bound on one group of distinct semantic atoms.
#[derive(Clone, Copy, Debug)]
pub struct Group<'a> {
    /// Atom identities in the caller's declared universe.
    pub members: &'a [usize],
    /// Stated maximum number of selected members; a loose bound is permitted.
    pub upper: usize,
}

/// Logical premises supplied by a caller, with no implicit theory certificate.
#[derive(Clone, Copy, Debug)]
pub struct Premises<'a> {
    /// Size and index meaning of the caller's semantic atom universe.
    pub atom_count: usize,
    /// Distinct atoms covered exactly once by the groups.
    pub members: &'a [usize],
    /// Stated minimum selected count over all declared members.
    pub lower: usize,
    /// Ordered groups whose upper bounds hold under the same activation.
    pub groups: &'a [Group<'a>],
}

/// Independent bounds on partition validation and retained consequences.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Maximum declared semantic atom universe, including ungrouped atoms.
    pub max_atoms: usize,
    /// Maximum declared members and total group membership occurrences.
    pub max_members: usize,
    /// Maximum groups, including empty groups.
    pub max_groups: usize,
    /// Simultaneous logical vector payload, excluding borrowed inputs, stack,
    /// allocator metadata and any extra capacity granted by the allocator.
    pub max_bytes: u64,
    /// Charged group, member and initialized coverage-cell visits.
    pub max_work: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_atoms: 65_536,
            max_members: 262_144,
            max_groups: 65_536,
            max_bytes: 64 * 1024 * 1024,
            max_work: 10_000_000,
        }
    }
}

/// An independently bounded construction resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    /// Declared semantic atom universe.
    Atoms,
    /// Declared members or supplied membership occurrences.
    Members,
    /// Supplied groups.
    Groups,
    /// Logical vector payload.
    Bytes,
    /// Charged validation operations.
    Work,
}

/// Why no shape-validated consequence plan was produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// A finite construction ceiling was exceeded.
    Limit(Resource),
    /// A declared or grouped atom is outside the universe.
    Atom(usize),
    /// The declared member set contains an atom twice.
    DuplicateMember(usize),
    /// A group contains an atom outside the declared member set.
    ForeignMember(usize),
    /// An atom occurs twice within or across groups.
    RepeatedMember(usize),
    /// A declared member is absent from every group.
    MissingMember(usize),
    /// A dimension or accounting operation cannot be represented.
    Overflow,
    /// Cancellation or fallible allocation stopped construction.
    Stopped(Stop),
}

/// Construction accounting; byte fields describe requested logical payloads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    /// Successfully charged operations, including work before a refusal.
    pub work: u64,
    /// Planned simultaneous payload for coverage scratch and retained vectors.
    /// This is not an assertion that allocation succeeded.
    pub construction_bytes: u64,
    /// Planned retained group/member vector payload after scratch is released.
    pub resident_bytes: u64,
}

/// A failed construction with its work prefix; never a logical contradiction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    kind: ErrorKind,
    statistics: Statistics,
}

impl Error {
    /// Typed refusal, separate from inconsistent stated logical premises.
    #[must_use]
    pub const fn kind(self) -> ErrorKind {
        self.kind
    }

    /// Retained work and planned storage before the refusal.
    #[must_use]
    pub const fn statistics(self) -> Statistics {
        self.statistics
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ErrorKind::Limit(resource) => write!(f, "partition {resource:?} limit reached"),
            ErrorKind::Atom(atom) => write!(f, "partition atom {atom} is outside its universe"),
            ErrorKind::DuplicateMember(atom) => {
                write!(f, "partition member {atom} is declared twice")
            }
            ErrorKind::ForeignMember(atom) => write!(f, "group member {atom} was not declared"),
            ErrorKind::RepeatedMember(atom) => write!(f, "group member {atom} occurs twice"),
            ErrorKind::MissingMember(atom) => write!(f, "partition member {atom} has no group"),
            ErrorKind::Overflow => f.write_str("partition dimension or accounting overflow"),
            ErrorKind::Stopped(stop) => stop.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ErrorKind::Stopped(stop) => Some(stop),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Bound {
    start: usize,
    end: usize,
    lower: usize,
    upper: usize,
}

/// One derived interval, borrowing the original group occurrence order.
#[derive(Clone, Copy, Debug)]
pub struct Consequence<'a> {
    /// Distinct semantic atoms of this group.
    pub members: &'a [usize],
    /// Lower bound implied by the caller-stated total and other group capacities.
    pub lower: usize,
    /// Effective upper bound: the lesser of the stated cap and group size.
    pub upper: usize,
}

/// An owned shape-validated partition and conditional cardinality consequences.
///
/// For total lower bound L and effective capacities u, group i receives
/// `max(0, L - sum(u[j], j != i))`. This is a mathematical consequence of the
/// stated premises, not evidence that any particular theory satisfies them.
#[derive(Debug)]
pub struct Plan {
    atom_count: usize,
    lower: usize,
    capacity: usize,
    members: Vec<usize>,
    groups: Vec<Bound>,
    statistics: Statistics,
}

impl Plan {
    /// Validate exact finite coverage and derive each local interval.
    ///
    /// Group/member order is retained. Empty groups and inconsistent logical
    /// bounds are legal: [`Self::inconsistent`] reports the latter. Validation
    /// takes `O(atom_count + declared members + membership occurrences + groups)`
    /// work and initializes one coverage cell per semantic atom. All loops poll
    /// control and charge work. No theory is read, searched or transformed.
    ///
    /// # Errors
    /// Refuses malformed partitions, dimensions, storage, work or control.
    pub fn new(
        premises: Premises<'_>,
        limits: Limits,
        cancellation: &Cancellation,
    ) -> Result<Self, Error> {
        let mut budget = Budget {
            limits,
            cancellation,
            statistics: Statistics::default(),
        };
        let result = build(premises, &mut budget);
        result.map_err(|kind| Error {
            kind,
            statistics: budget.statistics,
        })
    }

    /// Declared semantic universe; index correspondence remains a caller premise.
    #[must_use]
    pub const fn atom_count(&self) -> usize {
        self.atom_count
    }

    /// Stated total lower bound over the declared member set.
    #[must_use]
    pub const fn lower(&self) -> usize {
        self.lower
    }

    /// Sum of effective disjoint group capacities.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// True when no set can satisfy the stated total and group capacities.
    /// This says nothing about an original theory without the entailment premise.
    #[must_use]
    pub const fn inconsistent(&self) -> bool {
        self.lower > self.capacity
    }

    /// Derived intervals in supplied group order, including empty groups.
    #[must_use]
    pub fn consequences(&self) -> impl ExactSizeIterator<Item = Consequence<'_>> {
        self.groups.iter().map(|group| Consequence {
            members: &self.members[group.start..group.end],
            lower: group.lower,
            upper: group.upper,
        })
    }

    /// Completed construction accounting.
    #[must_use]
    pub const fn statistics(&self) -> Statistics {
        self.statistics
    }
}

#[derive(Clone, Copy)]
enum Coverage {
    Outside,
    Unassigned,
    Assigned,
}

struct Budget<'a> {
    limits: Limits,
    cancellation: &'a Cancellation,
    statistics: Statistics,
}

impl Budget<'_> {
    fn poll(&self) -> Result<(), ErrorKind> {
        self.cancellation.poll().map_err(ErrorKind::Stopped)
    }

    fn tick(&mut self) -> Result<(), ErrorKind> {
        self.poll()?;
        if self.statistics.work == self.limits.max_work {
            return Err(ErrorKind::Limit(Resource::Work));
        }
        self.statistics.work += 1;
        Ok(())
    }

    fn storage<T>(&self, count: usize) -> Result<Vec<T>, ErrorKind> {
        self.poll()?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| ErrorKind::Stopped(Stop::Allocation))?;
        Ok(values)
    }
}

fn build(premises: Premises<'_>, budget: &mut Budget<'_>) -> Result<Plan, ErrorKind> {
    budget.poll()?;
    limit(
        premises.atom_count,
        budget.limits.max_atoms,
        Resource::Atoms,
    )?;
    limit(
        premises.members.len(),
        budget.limits.max_members,
        Resource::Members,
    )?;
    limit(
        premises.groups.len(),
        budget.limits.max_groups,
        Resource::Groups,
    )?;
    let (occurrences, capacity) = dimensions(premises.groups, budget)?;
    let resident = bytes::<usize>(occurrences)?
        .checked_add(bytes::<Bound>(premises.groups.len())?)
        .ok_or(ErrorKind::Overflow)?;
    budget.statistics.resident_bytes = resident;
    budget.statistics.construction_bytes = resident
        .checked_add(bytes::<Coverage>(premises.atom_count)?)
        .ok_or(ErrorKind::Overflow)?;
    if budget.statistics.construction_bytes > budget.limits.max_bytes {
        return Err(ErrorKind::Limit(Resource::Bytes));
    }
    let mut coverage = budget.storage(premises.atom_count)?;
    for _ in 0..premises.atom_count {
        budget.tick()?;
        coverage.push(Coverage::Outside);
    }
    declare(premises.members, &mut coverage, budget)?;
    let mut members = budget.storage(occurrences)?;
    let mut groups = budget.storage(premises.groups.len())?;
    for group in premises.groups {
        budget.tick()?;
        let start = members.len();
        assign(group.members, &mut coverage, &mut members, budget)?;
        let upper = group.upper.min(group.members.len());
        groups.push(Bound {
            start,
            end: members.len(),
            lower: premises.lower.saturating_sub(capacity - upper),
            upper,
        });
    }
    for &atom in premises.members {
        budget.tick()?;
        if matches!(coverage[atom], Coverage::Unassigned) {
            return Err(ErrorKind::MissingMember(atom));
        }
    }
    budget.poll()?;
    Ok(Plan {
        atom_count: premises.atom_count,
        lower: premises.lower,
        capacity,
        members,
        groups,
        statistics: budget.statistics,
    })
}

fn dimensions(groups: &[Group<'_>], budget: &mut Budget<'_>) -> Result<(usize, usize), ErrorKind> {
    let (mut occurrences, mut capacity) = (0_usize, 0_usize);
    for group in groups {
        budget.tick()?;
        occurrences = occurrences
            .checked_add(group.members.len())
            .ok_or(ErrorKind::Overflow)?;
        limit(occurrences, budget.limits.max_members, Resource::Members)?;
        capacity = capacity
            .checked_add(group.upper.min(group.members.len()))
            .ok_or(ErrorKind::Overflow)?;
    }
    Ok((occurrences, capacity))
}

fn declare(
    members: &[usize],
    coverage: &mut [Coverage],
    budget: &mut Budget<'_>,
) -> Result<(), ErrorKind> {
    for &atom in members {
        budget.tick()?;
        let cell = coverage.get_mut(atom).ok_or(ErrorKind::Atom(atom))?;
        if !matches!(cell, Coverage::Outside) {
            return Err(ErrorKind::DuplicateMember(atom));
        }
        *cell = Coverage::Unassigned;
    }
    Ok(())
}

fn assign(
    input: &[usize],
    coverage: &mut [Coverage],
    members: &mut Vec<usize>,
    budget: &mut Budget<'_>,
) -> Result<(), ErrorKind> {
    for &atom in input {
        budget.tick()?;
        match coverage.get_mut(atom).ok_or(ErrorKind::Atom(atom))? {
            Coverage::Outside => return Err(ErrorKind::ForeignMember(atom)),
            Coverage::Assigned => return Err(ErrorKind::RepeatedMember(atom)),
            cell @ Coverage::Unassigned => *cell = Coverage::Assigned,
        }
        members.push(atom);
    }
    Ok(())
}

fn limit(actual: usize, maximum: usize, resource: Resource) -> Result<(), ErrorKind> {
    if actual > maximum {
        Err(ErrorKind::Limit(resource))
    } else {
        Ok(())
    }
}

fn bytes<T>(count: usize) -> Result<u64, ErrorKind> {
    let bytes = count
        .checked_mul(size_of::<T>())
        .ok_or(ErrorKind::Overflow)?;
    u64::try_from(bytes).map_err(|_| ErrorKind::Overflow)
}
