//! Fixed-domain answer identity, applied after exact membership and selection.

use std::{fmt, mem::size_of};

mod index;

use zetesis_core::Model;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_themelios::PreparedProjection;

/// Independent ceilings for distinct projected answer identities.
/// These do not replace candidate exclusion, membership or objective budgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectionLimits {
    /// Maximum distinct projection keys; duplicate answers require no new key.
    pub max_keys: usize,
    /// History header, index capacity and packed key words, including conservative
    /// old/new growth overlap and the reusable current-key buffer. Excludes the
    /// borrowed domain, answer, allocator metadata and search; this is not RSS.
    pub max_bytes: usize,
    /// Cumulative key-word resets, domain probes, hashing/comparison/copy words,
    /// and index initialization/probe steps, including failed attempts. A domain
    /// probe includes the model's typed lookup, not one machine instruction.
    /// Hash collisions consume this same finite allowance.
    pub max_work: u64,
}

impl Default for ProjectionLimits {
    fn default() -> Self {
        Self {
            max_keys: 1_000_000,
            max_bytes: 128 * 1024 * 1024,
            max_work: 100_000_000,
        }
    }
}

/// A projected-enumeration limit, independent of original answer identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionResource {
    /// Distinct completed keys.
    Keys,
    /// Named history and construction capacity.
    Bytes,
    /// Cumulative key construction and lookup work.
    Work,
}

/// Projection failed without publishing a partial or unexamined key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionError {
    /// A projected door requires completed source projection declarations.
    MissingDeclaration,
    /// An inclusive projection ceiling was exceeded.
    Limit {
        /// Resource whose admission failed.
        resource: ProjectionResource,
        /// Required amount, before publication.
        observed: u128,
        /// Inclusive configured ceiling.
        limit: u128,
    },
    /// Checked capacity or counter arithmetic overflowed.
    Overflow,
    /// A fallible history reservation failed.
    Allocation,
    /// Cooperative cancellation or deadline reached during projection.
    Control(Stop),
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDeclaration => {
                f.write_str("projected enumeration requires a completed #project declaration")
            }
            Self::Limit {
                resource,
                observed,
                limit,
            } => write!(
                f,
                "answer projection {resource:?} limit: {observed} exceeds {limit}"
            ),
            Self::Overflow => f.write_str("answer projection accounting overflow"),
            Self::Allocation => f.write_str("answer projection storage could not be reserved"),
            Self::Control(stop) => stop.fmt(f),
        }
    }
}

impl std::error::Error for ProjectionError {}

/// Evidence from projected enumeration, separate from full-model search counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProjectionStatistics {
    /// Distinct keys retained and returned as full answer-set representatives.
    pub representatives: usize,
    /// Fully keyed answers equal to a previously returned representative.
    pub duplicates: u64,
    /// Charged work, including attempts that failed before key publication.
    pub work: u64,
    /// Current named retained history capacity, excluding borrowed owners.
    pub retained_bytes: u128,
    /// Greatest observed named capacity, including conservative old/new growth
    /// overlap. Refused proposals that allocated nothing do not increase it.
    pub peak_bytes: u128,
    /// Every selected answer was consumed and original search was exhaustive.
    /// Requested representative limits and failures leave this false even when
    /// optimization independently established the best score.
    pub complete: bool,
}

pub(crate) struct Projection<'a> {
    domain: &'a PreparedProjection,
    limits: ProjectionLimits,
    history: index::History,
    current: Vec<u64>,
    stats: ProjectionStatistics,
}

impl<'a> Projection<'a> {
    pub(crate) fn new(
        domain: &'a PreparedProjection,
        limits: ProjectionLimits,
    ) -> Result<Self, ProjectionError> {
        if !domain.is_explicit() {
            return Err(ProjectionError::MissingDeclaration);
        }
        let words = domain.atoms().len().div_ceil(64);
        let mut result = Self {
            domain,
            limits,
            history: index::History::new(words),
            current: Vec::new(),
            stats: ProjectionStatistics::default(),
        };
        ceiling(
            ProjectionResource::Bytes,
            result.base_bytes() + bytes::<u64>(words)?,
            limits.max_bytes as u128,
        )?;
        result
            .current
            .try_reserve_exact(words)
            .map_err(|_| ProjectionError::Allocation)?;
        result.stats.retained_bytes = result.base_bytes();
        result.stats.peak_bytes = result.stats.retained_bytes;
        ceiling(
            ProjectionResource::Bytes,
            result.stats.retained_bytes,
            limits.max_bytes as u128,
        )?;
        result.current.resize(words, 0);
        Ok(result)
    }

    pub(crate) fn statistics(&self) -> ProjectionStatistics {
        self.stats
    }

    pub(crate) fn finish(&mut self, complete: bool) {
        self.stats.complete = complete;
    }

    /// Complete the key before indexing it. History has one packed copy per key;
    /// hash slots contain only its local index. Neither hash nor local index is
    /// an answer identity, and collision equality always compares every word.
    pub(crate) fn insert(
        &mut self,
        model: &Model,
        cancellation: &Cancellation,
    ) -> Result<bool, ProjectionError> {
        let base = self.base_bytes();
        let mut work = Work {
            limits: self.limits,
            stats: &mut self.stats,
            cancellation,
        };
        for word in &mut self.current {
            work.charge(1)?;
            *word = 0;
        }
        for (index, atom) in self.domain.atoms().iter().enumerate() {
            work.charge(1)?;
            if model.contains(atom) {
                self.current[index / 64] |= 1_u64 << (index % 64);
            }
        }
        let inserted = self.history.insert(&self.current, base, &mut work)?;
        if inserted {
            work.stats.representatives = self.history.len();
        } else {
            work.stats.duplicates = work
                .stats
                .duplicates
                .checked_add(1)
                .ok_or(ProjectionError::Overflow)?;
        }
        Ok(inserted)
    }

    // All vector headers, including History's, are already within Self.
    fn base_bytes(&self) -> u128 {
        size_of::<Self>() as u128 + self.current.capacity() as u128 * size_of::<u64>() as u128
    }
}

struct Work<'a> {
    limits: ProjectionLimits,
    stats: &'a mut ProjectionStatistics,
    cancellation: &'a Cancellation,
}

impl Work<'_> {
    fn charge(&mut self, amount: u64) -> Result<(), ProjectionError> {
        self.cancellation.poll().map_err(ProjectionError::Control)?;
        let observed = self
            .stats
            .work
            .checked_add(amount)
            .ok_or(ProjectionError::Overflow)?;
        ceiling(
            ProjectionResource::Work,
            u128::from(observed),
            u128::from(self.limits.max_work),
        )?;
        self.stats.work = observed;
        Ok(())
    }

    fn admit_bytes(&self, required: u128) -> Result<(), ProjectionError> {
        ceiling(
            ProjectionResource::Bytes,
            required,
            self.limits.max_bytes as u128,
        )
    }

    /// Observe actual retained capacity plus a currently allocated replacement.
    /// A refused proposal calls `admit_bytes` alone. Allocator slack is observed
    /// before refusal; failed temporary construction drops that replacement.
    fn observe(&mut self, retained: u128, temporary: u128) -> Result<(), ProjectionError> {
        self.stats.retained_bytes = retained;
        let live = retained
            .checked_add(temporary)
            .ok_or(ProjectionError::Overflow)?;
        self.stats.peak_bytes = self.stats.peak_bytes.max(live);
        self.admit_bytes(live)
    }
}

fn bytes<T>(count: usize) -> Result<u128, ProjectionError> {
    (count as u128)
        .checked_mul(size_of::<T>() as u128)
        .ok_or(ProjectionError::Overflow)
}

fn ceiling(
    resource: ProjectionResource,
    observed: u128,
    limit: u128,
) -> Result<(), ProjectionError> {
    if observed > limit {
        Err(ProjectionError::Limit {
            resource,
            observed,
            limit,
        })
    } else {
        Ok(())
    }
}
