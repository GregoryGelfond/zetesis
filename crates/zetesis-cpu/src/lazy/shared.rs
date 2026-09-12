//! Shared source traversal with independent, ordered CPU world evaluation.
//!
//! This is the concrete CPU implementation of the immutable-round protocol.
//! Source work is shared; each world has its own evaluation ceiling. A stopped
//! world invalidates the entire batch, including other worlds that completed a
//! chunk. There is no retry, fallback, or partial closure publication.

use std::fmt;

use rayon::prelude::*;
use zetesis_core::{Program, SeedView};

use super::{Chunk, EvaluationStep, Progress, SourceSelection};
use crate::{BatchError, Control, Stop};

/// Shared source limits and a separate per-world evaluation limit.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Collective catalog, source work, chunks, rounds and requested host bytes.
    /// The allowance also includes the retained per-world progress vector.
    pub source: super::Limits,
    /// Cumulative record visits plus antecedent tests per world, across chunks
    /// and rounds. This excludes shared source work and differs from the
    /// independent relational oracle's join/copy operation units.
    pub max_world_work: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            source: super::Limits::default(),
            max_world_work: 10_000_000,
        }
    }
}

/// Attempted evaluation for one input occurrence, including duplicate seeds.
/// Successful chunk work is not a completed closure or membership result.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorldProgress {
    /// Charged record visits plus antecedent tests.
    pub work: u64,
    /// Record visits; a subset of `work`, not an additional charge.
    pub instances: u64,
    /// A local evaluation stop, if observed. Absence does not prove completion
    /// when another world or the shared source interrupted the batch.
    pub interruption: Option<Stop>,
}

/// Retained source and world accounting, with separate operation units.
#[derive(Debug)]
pub struct Statistics {
    /// Selected source traversal; both retain the same per-world semantics.
    pub selection: SourceSelection,
    /// Submitted occurrences, including those refused before progress allocation.
    pub submitted_candidates: usize,
    /// Completed source rounds and attempted source work.
    pub source: Progress,
    /// Input-ordered evaluation records. Empty if allocation or preflight failed
    /// before this vector was reserved; otherwise one per submitted occurrence.
    pub worlds: Vec<WorldProgress>,
}

/// Output record for concrete shared CPU checking. Unmodified successful
/// `BatchOracle::check_shared` results contain complete ordered checks.
/// Public fields permit caller changes; this record is not a proof receipt.
#[derive(Debug)]
pub struct Batch {
    /// Complete closures and verdicts in exactly the submitted seed order.
    pub checks: Vec<super::Check>,
    /// Shared source and separate per-world execution accounting.
    pub statistics: Statistics,
}

/// The reason no complete results can be published for this batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    /// Shared source, collective storage, control or input failure.
    Source(Stop),
    /// Evaluation failed for this zero-based input occurrence. When several
    /// worlds stop, the first in input order is reported; all retain their work.
    World {
        /// Zero-based input occurrence whose evaluation stopped.
        index: usize,
        /// Exact world evaluation or control stop.
        stop: Stop,
    },
    /// The concrete evaluator violated the immutable-round output contract.
    InvalidOutput,
}

/// Incomplete shared rounds; no candidate in this value has a complete check.
#[derive(Debug)]
pub struct Failure {
    /// Exact shared or indexed world cause.
    pub cause: Cause,
    /// Progress retained before the failure, without completed-check claims.
    pub statistics: Statistics,
}

/// Admission failure is distinct from incomplete execution of an admitted batch.
#[derive(Debug)]
pub enum Error {
    /// Pool occupancy, poison or submission capacity prevented execution.
    Admission(BatchError),
    /// The batch started but did not establish any complete checks.
    Incomplete(Failure),
}

pub(crate) fn check<'seed>(
    pool: &rayon::ThreadPool,
    program: &Program,
    seeds: impl ExactSizeIterator<Item = SeedView<'seed>> + Clone,
    limits: Limits,
    selection: SourceSelection,
    control: &Control,
) -> Result<Batch, Error> {
    let mut statistics = Statistics {
        selection,
        submitted_candidates: seeds.len(),
        source: Progress::default(),
        worlds: Vec::new(),
    };
    match run(pool, program, seeds, limits, control, &mut statistics) {
        Ok(checks) => Ok(Batch { checks, statistics }),
        Err(cause) => Err(Error::Incomplete(Failure { cause, statistics })),
    }
}

fn run<'seed>(
    pool: &rayon::ThreadPool,
    program: &Program,
    seeds: impl ExactSizeIterator<Item = SeedView<'seed>> + Clone,
    limits: Limits,
    control: &Control,
    statistics: &mut Statistics,
) -> Result<Vec<super::Check>, Cause> {
    control.poll().map_err(Cause::Source)?;
    super::validate_seeds(program, seeds.clone(), seeds.len()).map_err(Cause::Source)?;
    if seeds.len() > limits.source.max_candidates {
        return Err(Cause::Source(Stop::CarrierLimit));
    }
    let progress_bytes = seeds.len().checked_mul(size_of::<WorldProgress>());
    let mut source_limits = limits.source;
    source_limits.max_host_bytes = progress_bytes
        .and_then(|bytes| source_limits.max_host_bytes.checked_sub(bytes))
        .ok_or(Cause::Source(Stop::Allocation))?;
    statistics
        .worlds
        .try_reserve_exact(seeds.len())
        .map_err(|_| Cause::Source(Stop::Allocation))?;
    statistics
        .worlds
        .resize(seeds.len(), WorldProgress::default());
    let result = super::check_with_source_views(
        program,
        seeds,
        source_limits,
        statistics.selection,
        control,
        |chunk| {
            evaluate(
                pool,
                chunk,
                &mut statistics.worlds,
                limits.max_world_work,
                control,
            )
        },
    );
    match result {
        Ok(batch) => {
            statistics.source = batch.progress;
            Ok(batch.checks)
        }
        Err(failure) => {
            statistics.source = failure.progress;
            Err(match failure.cause {
                super::Cause::Source(stop) => Cause::Source(stop),
                super::Cause::Execution(cause) => cause,
                super::Cause::InvalidOutput => Cause::InvalidOutput,
            })
        }
    }
}

fn evaluate(
    pool: &rayon::ThreadPool,
    chunk: &Chunk<'_>,
    progress: &mut [WorldProgress],
    limit: u64,
    control: &Control,
) -> Result<Vec<u32>, Cause> {
    control.poll().map_err(Cause::Source)?;
    // Chunk construction already bounds this exact output payload as part of
    // source.max_host_bytes. Workers write disjoint slices without private copies.
    let mut output = super::zeros(chunk.worlds() * chunk.result_words()).map_err(Cause::Source)?;
    pool.install(|| {
        output
            .par_chunks_mut(chunk.result_words())
            .zip(progress.par_iter_mut())
            .enumerate()
            .for_each(|(world, (result, progress))| {
                let outcome = super::evaluate_world(chunk, world, result, &mut |step| {
                    control.poll()?;
                    if progress.work >= limit {
                        return Err(Stop::WorkLimit);
                    }
                    progress.work += 1;
                    if matches!(step, EvaluationStep::Instance) {
                        // Every instance is also charged work, so this counter
                        // cannot overflow before the checked work ceiling.
                        progress.instances += 1;
                    }
                    Ok(())
                });
                progress.interruption = outcome.err();
            });
    });
    if let Some((index, stop)) = progress
        .iter()
        .enumerate()
        .find_map(|(index, progress)| progress.interruption.map(|stop| (index, stop)))
    {
        return Err(Cause::World { index, stop });
    }
    control.poll().map_err(Cause::Source)?;
    Ok(output)
}

impl fmt::Display for Cause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(stop) => write!(f, "shared source: {stop}"),
            Self::World { index, stop } => write!(f, "candidate occurrence {index}: {stop}"),
            Self::InvalidOutput => f.write_str("shared CPU output violated its round contract"),
        }
    }
}

impl std::error::Error for Cause {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(stop) | Self::World { stop, .. } => Some(stop),
            Self::InvalidOutput => None,
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(f)
    }
}

impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(error) => error.fmt(f),
            Self::Incomplete(failure) => failure.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error),
            Self::Incomplete(failure) => Some(failure),
        }
    }
}
