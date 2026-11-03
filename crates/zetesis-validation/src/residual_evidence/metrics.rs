//! Preserve full-population coverage separately from ranks of retained records.
use super::{Artifact, invalid};
use crate::performance::matrix::{
    ResidualQuantiles, ResidualSampleGroup, ResidualSampleGroups, ResidualSampleMetric,
};
use std::io;
use zetesis_backend::residual_evidence::{Batch, Query, Reason, World};

impl ResidualSampleGroups {
    fn ordered(&self) -> [&ResidualSampleGroup; 4] {
        [
            &self.all,
            &self.fixed_point,
            &self.round_limit,
            &self.work_limit,
        ]
    }
}

impl ResidualSampleGroup {
    fn ordered(&self) -> [&ResidualSampleMetric; 10] {
        [
            &self.open_atoms,
            &self.visited,
            &self.attempted_splits,
            &self.published_splits,
            &self.depth,
            &self.work,
            &self.query_ns,
            &self.residuals,
            &self.width,
            &self.batch_ns,
        ]
    }
}

fn reason_index(r: Reason) -> usize {
    match r {
        Reason::FixedPoint => 1,
        Reason::RoundLimit => 2,
        Reason::WorkLimit => 3,
    }
}

fn world_value(w: &World, group: usize, metric: usize) -> Option<u128> {
    if group != 0 && w.reason.map(reason_index) != Some(group) {
        return None;
    }
    if metric == 0 {
        return w.open.map(u128::from);
    }
    let Query::Entered {
        visited,
        attempted_splits,
        published_splits,
        depth,
        work,
        host_ns,
    } = w.query
    else {
        return None;
    };
    match metric {
        1 => Some(u128::from(visited)),
        2 => Some(u128::from(attempted_splits)),
        3 => Some(u128::from(published_splits)),
        4 => depth.map(u128::from),
        5 => Some(u128::from(work)),
        6 => host_ns,
        _ => unreachable!("world measurement"),
    }
}

fn batch_value(b: &Batch, group: usize, metric: usize) -> Option<u128> {
    match metric {
        7 => b.residuals[group].map(u128::from),
        8 if b.residuals[group] == Some(0) || b.width == Some(0) => Some(0),
        8 if group == 0 || b.residuals[group].is_some() && b.residuals[0] == b.residuals[group] => {
            b.width.map(u128::from)
        }
        8 => None,
        9 => b.host_ns[group],
        _ => unreachable!("batch measurement"),
    }
}

/// A single bounded rank workspace is reused for all metrics. The two raw
/// reservoirs stay in their serialized order for reproducible subsequent draws.
struct Ranks(Vec<u128>);
impl Ranks {
    fn new(max: usize) -> io::Result<Self> {
        let mut v = Vec::new();
        v.try_reserve_exact(max).map_err(io::Error::other)?;
        Ok(Self(v))
    }
    fn measure(&mut self, worlds: &[World], batches: &[Batch], group: usize, metric: usize) {
        self.0.clear();
        if metric < 7 {
            self.0
                .extend(worlds.iter().filter_map(|w| world_value(w, group, metric)));
        } else {
            self.0
                .extend(batches.iter().filter_map(|b| batch_value(b, group, metric)));
        }
        self.0.sort_unstable();
    }
    fn quantiles(&self) -> (Option<u128>, Option<u128>) {
        let n = self.0.len();
        if n == 0 {
            return (None, None);
        }
        (
            Some(self.0[(n - 1) / 2].midpoint(self.0[n / 2])),
            Some(self.0[n - n / 10 - 1]),
        )
    }
    fn check(&self, metric: &ResidualSampleMetric) -> io::Result<()> {
        let count = self.0.len();
        let sum = self.0.iter().try_fold(0u128, |a, &b| a.checked_add(b));
        let maximum = self.0.last().copied();
        if count != metric.retained
            || metric
                .sum
                .is_some_and(|bound| sum.is_none_or(|v| v > bound))
            || metric
                .maximum
                .is_some_and(|bound| maximum.is_some_and(|v| v > bound))
        {
            return Err(invalid(
                "residual records disagree with the native measurement",
            ));
        }
        let complete = metric.eligible.is_some() && metric.eligible == metric.known;
        let census = complete && metric.known == Some(count as u64);
        if census && (metric.sum != sum || metric.maximum != maximum) {
            return Err(invalid(
                "complete residual observations disagree with native reductions",
            ));
        }
        let expected = if !complete || count == 0 {
            ResidualQuantiles::Unavailable
        } else if census {
            ResidualQuantiles::Exact
        } else {
            ResidualQuantiles::Estimated
        };
        let ranks = if expected == ResidualQuantiles::Unavailable {
            (None, None)
        } else {
            self.quantiles()
        };
        if expected != metric.quantiles || ranks != (metric.median, metric.p90) {
            return Err(invalid(
                "residual records disagree with published quantiles",
            ));
        }
        Ok(())
    }
}

/// When every world is present, availability is evidence too. Missing reason
/// joins leave an interval of possible eligible counts for that reason.
fn eligible_world(w: &World, group: usize, field: usize) -> Option<bool> {
    let belongs = if group == 0 {
        Some(true)
    } else {
        w.reason.map(|r| reason_index(r) == group)
    };
    let entered = if field == 0 {
        Some(true)
    } else {
        match w.query {
            Query::Certified => Some(false),
            Query::Unknown => None,
            Query::Entered { visited, .. } => Some(field != 4 || visited != 0),
        }
    };
    match (belongs, entered) {
        (Some(false), _) | (_, Some(false)) => Some(false),
        (Some(true), Some(true)) => Some(true),
        _ => None,
    }
}

fn check_census(
    a: &Artifact,
    group: usize,
    field: usize,
    metric: &ResidualSampleMetric,
) -> io::Result<()> {
    let (seen, retained) = if field < 7 {
        (a.header.worlds, a.worlds.len())
    } else {
        (a.header.batches, a.batches.len())
    };
    if seen != Some(retained as u64) {
        return Ok(());
    }
    if metric.known != Some(metric.retained as u64) {
        return Err(invalid(
            "fully captured residual frame lacks reported measurements",
        ));
    }
    if field < 7 {
        let (lower, unknown) =
            a.worlds.iter().fold(
                (0u64, 0u64),
                |(lower, unknown), world| match eligible_world(world, group, field) {
                    Some(true) => (lower + 1, unknown),
                    Some(false) => (lower, unknown),
                    None => (lower, unknown + 1),
                },
            );
        if metric
            .eligible
            .is_some_and(|n| n < lower || n > lower + unknown)
            || group == 0 && metric.eligible != ((unknown == 0).then_some(lower))
        {
            return Err(invalid("world states disagree with native eligibility"));
        }
    }
    Ok(())
}

pub(super) fn check(a: &Artifact, groups: &ResidualSampleGroups) -> io::Result<()> {
    let mut scratch = Ranks::new(a.worlds.len().max(a.batches.len()))?;
    for (index, group) in groups.ordered().iter().enumerate() {
        for (field, metric) in group.ordered().into_iter().enumerate() {
            scratch.measure(&a.worlds, &a.batches, index, field);
            scratch.check(metric)?;
            check_census(a, index, field, metric)?;
        }
    }
    Ok(())
}

/// Compact additive population reductions are copied from the authenticated
/// native document. An empty population has no maximum and is neutral in a join.
#[derive(Clone, Copy, Debug)]
struct Total {
    eligible: Option<u64>,
    known: Option<u64>,
    sum: Option<u128>,
    max: Option<u128>,
    missing_max: bool,
}

impl Default for Total {
    fn default() -> Self {
        Self {
            eligible: Some(0),
            known: Some(0),
            sum: Some(0),
            max: None,
            missing_max: false,
        }
    }
}

impl Total {
    fn add(self, other: Self) -> Self {
        Self {
            eligible: self
                .eligible
                .zip(other.eligible)
                .and_then(|(a, b)| a.checked_add(b)),
            known: self
                .known
                .zip(other.known)
                .and_then(|(a, b)| a.checked_add(b)),
            sum: self.sum.zip(other.sum).and_then(|(a, b)| a.checked_add(b)),
            max: self.max.max(other.max),
            missing_max: self.missing_max || other.missing_max,
        }
    }
    fn result(self, scratch: &Ranks) -> ResidualSampleMetric {
        let retained = scratch.0.len();
        let complete = self.eligible.is_some() && self.known == self.eligible;
        let quantiles = if !complete || retained == 0 {
            ResidualQuantiles::Unavailable
        } else if self.known == Some(retained as u64) {
            ResidualQuantiles::Exact
        } else {
            ResidualQuantiles::Estimated
        };
        let (median, p90) = if quantiles == ResidualQuantiles::Unavailable {
            (None, None)
        } else {
            scratch.quantiles()
        };
        ResidualSampleMetric {
            eligible: self.eligible,
            known: self.known,
            retained,
            sum: if complete { self.sum } else { None },
            maximum: if self.missing_max { None } else { self.max },
            median,
            p90,
            quantiles,
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct Totals([[Total; 10]; 4]);
impl Totals {
    pub(super) fn from_groups(groups: &ResidualSampleGroups) -> Self {
        Self(groups.ordered().map(|g| {
            g.ordered().map(|m| Total {
                eligible: m.eligible,
                known: m.known,
                sum: m.sum,
                max: m.maximum,
                missing_max: m.maximum.is_none() && m.eligible != Some(0),
            })
        }))
    }
    pub(super) fn join(&mut self, other: &Self) {
        for (left, right) in self.0.iter_mut().zip(other.0) {
            for (l, r) in left.iter_mut().zip(right) {
                *l = l.add(r);
            }
        }
    }
    pub(super) fn finish(
        self,
        worlds: &[World],
        batches: &[Batch],
    ) -> io::Result<ResidualSampleGroups> {
        let mut scratch = Ranks::new(worlds.len().max(batches.len()))?;
        let [all, fixed_point, round_limit, work_limit] = std::array::from_fn(|group| {
            let [
                open_atoms,
                visited,
                attempted_splits,
                published_splits,
                depth,
                work,
                query_ns,
                residuals,
                width,
                batch_ns,
            ] = std::array::from_fn(|metric| {
                scratch.measure(worlds, batches, group, metric);
                self.0[group][metric].result(&scratch)
            });
            ResidualSampleGroup {
                open_atoms,
                visited,
                attempted_splits,
                published_splits,
                depth,
                work,
                query_ns,
                residuals,
                width,
                batch_ns,
            }
        });
        Ok(ResidualSampleGroups {
            all,
            fixed_point,
            round_limit,
            work_limit,
        })
    }
}
