//! Uniform mixing of independent bounded samples at original population weights.
use super::{Artifact, invalid, metrics::Totals};
use crate::performance::matrix::{ResidualSampleGroups, ResidualSamplePopulation};
use serde::Serialize;
use std::{collections::HashSet, io};
use zetesis_backend::residual_evidence::{Batch, Draw, World};

struct Frame<T> {
    seen: Option<u64>,
    records: Vec<T>,
    draw: Draw,
}

impl<T> Frame<T> {
    fn new(seed: u64, tag: u64) -> Self {
        Self {
            seen: Some(0),
            records: Vec::new(),
            draw: Draw::new(seed, tag),
        }
    }

    fn join(&mut self, seen: Option<u64>, mut incoming: Vec<T>, target: usize) -> io::Result<()> {
        let combined = self
            .seen
            .zip(seen)
            .and_then(|(left, right)| left.checked_add(right).map(|total| (left, right, total)));
        let Some((left, right, total)) = combined else {
            self.seen = None;
            self.records = Vec::new();
            return Ok(());
        };
        let mut k = total.min(target as u64);
        for (n, r) in [(left, self.records.len()), (right, incoming.len())] {
            if (r as u64) < n {
                k = k.min(r as u64);
            }
        }
        // Draw the origin counts without replacement from the original
        // populations. Then choose that many distinct retained positions in each.
        let from_left = origin_count(left, total, k, |bound| self.draw.below(bound));
        let k = usize::try_from(k).expect("bounded pool size");
        keep(&mut self.records, from_left, &mut self.draw);
        keep(&mut incoming, k - from_left, &mut self.draw);
        self.records
            .try_reserve_exact(incoming.len())
            .map_err(io::Error::other)?;
        self.records.append(&mut incoming);
        self.seen = Some(total);
        Ok(())
    }

    fn population(&self) -> ResidualSamplePopulation {
        ResidualSamplePopulation {
            seen: self.seen,
            retained: self.records.len(),
            capacity: self.records.len(),
        }
    }
}

fn keep<T>(items: &mut Vec<T>, wanted: usize, draw: &mut Draw) {
    if wanted < items.len() {
        for i in 0..wanted {
            let available = items.len() - i;
            let offset =
                usize::try_from(draw.below(available as u64)).expect("index from slice length");
            items.swap(i, i + offset);
        }
        items.truncate(wanted);
    }
}

/// The count from the first urn in a uniform draw without replacement.
fn origin_count(mut left: u64, total: u64, k: u64, mut below: impl FnMut(u64) -> u64) -> usize {
    let mut chosen = 0;
    for i in 0..k {
        if left == 0 {
            break;
        }
        let remaining = total - i;
        if remaining == left {
            chosen += usize::try_from(k - i).expect("bounded pool target");
            break;
        }
        if below(remaining) < left {
            left -= 1;
            chosen += 1;
        }
    }
    chosen
}

#[cfg(test)]
mod tests;

/// Two independently re-sampled populations for one explicit same-profile fixture cohort.
pub struct Pool {
    seed: u64,
    worlds: Frame<World>,
    batches: Frame<Batch>,
    totals: Totals,
    runs: u64,
    seeds: HashSet<u64>,
    nonces: HashSet<[u8; 16]>,
    failed: bool,
}

impl std::fmt::Debug for Pool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pool")
            .field("runs", &self.runs)
            .field("worlds", &self.worlds.population())
            .field("batches", &self.batches.population())
            .field("failed", &self.failed)
            .finish_non_exhaustive()
    }
}

/// Bounded population-weighted quantiles and full-population reductions.
#[derive(Debug, Serialize)]
pub struct PooledSamples {
    /// Pooling schema.
    pub version: u32,
    /// Seed of the two merge streams.
    pub seed: u64,
    /// Distinct completed invocations in the cohort.
    pub runs: u64,
    /// Original and retained world counts.
    pub worlds: ResidualSamplePopulation,
    /// Original and retained committed batch counts.
    pub batches: ResidualSamplePopulation,
    /// Full and per-reason coverage, reductions and ranks.
    pub groups: ResidualSampleGroups,
}

impl Pool {
    /// Start a pool with a merge seed independent of the child selections.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            worlds: Frame::new(seed, 0x504f_4f4c_5f57_3031),
            batches: Frame::new(seed, 0x504f_4f4c_5f42_3031),
            totals: Totals::default(),
            runs: 0,
            seeds: HashSet::new(),
            nonces: HashSet::new(),
            failed: false,
        }
    }
    /// Mix one validated artifact and its authenticated coverage. Any error
    /// makes this pool unavailable for publication.
    ///
    /// # Errors
    /// Returns errors on incomplete/duplicate runs, repeated sampled streams,
    /// count overflow or optional storage refusal.
    pub fn push(&mut self, a: Artifact) -> io::Result<()> {
        if self.failed {
            return Err(invalid("residual cohort was already refused"));
        }
        self.failed = true;
        if !a.header.complete {
            return Err(invalid("incomplete run in residual cohort"));
        }
        self.nonces.try_reserve(1).map_err(io::Error::other)?;
        if !self.nonces.insert(a.header.nonce) {
            return Err(invalid("duplicate residual invocation"));
        }
        let sampled = a.header.worlds.is_some_and(|n| {
            n > u64::from(a.header.retained_worlds) && a.header.retained_worlds != 0
        }) || a.header.batches.is_some_and(|n| {
            n > u64::from(a.header.retained_batches) && a.header.retained_batches != 0
        });
        if sampled {
            self.seeds.try_reserve(1).map_err(io::Error::other)?;
            if !self.seeds.insert(a.header.seed) {
                return Err(invalid("reused residual sampling seed"));
            }
        }
        self.worlds.join(a.header.worlds, a.worlds, 65_536)?;
        self.batches.join(a.header.batches, a.batches, 4_096)?;
        self.totals.join(&a.totals);
        self.runs = self
            .runs
            .checked_add(1)
            .ok_or_else(|| invalid("too many residual invocations"))?;
        self.failed = false;
        Ok(())
    }
    /// Finalize both distinct populations. An unavailable or empty population
    /// cannot yield a percentile.
    ///
    /// # Errors
    /// Refuses an empty or failed pool and reports a bounded scratch allocation failure.
    pub fn finish(self) -> io::Result<PooledSamples> {
        if self.failed || self.runs == 0 {
            return Err(invalid("incomplete or empty residual cohort"));
        }
        Ok(PooledSamples {
            version: 1,
            seed: self.seed,
            runs: self.runs,
            worlds: self.worlds.population(),
            batches: self.batches.population(),
            groups: self
                .totals
                .finish(&self.worlds.records, &self.batches.records)?,
        })
    }
}
