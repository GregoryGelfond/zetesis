//! Construct the validation reader's independently typed input from the
//! same queue's terminal observations. The CLI writer has its own wire
//! round-trip tests; this fixture never recomputes a count or rank.
use crate::{
    ResidualQuantiles, ResidualRunSamples, ResidualSampleGroup, ResidualSampleMetric,
    ResidualSamplePopulation,
};
use zetesis_validation::performance::matrix as receiver;

pub(super) fn observed(s: &ResidualRunSamples) -> receiver::ResidualRunSamples {
    let [all, fixed, round, work] = s.groups.as_deref().unwrap() else {
        panic!("four measured groups")
    };
    receiver::ResidualRunSamples {
        version: 1,
        algorithm: "splitmix64-reject-v1".into(),
        seed: s.seed,
        byte_limit: s.byte_limit,
        retained_bytes: s.retained_bytes,
        peak_bytes: s.peak_bytes,
        worlds: population(&s.worlds),
        batches: population(&s.batches),
        groups: Some(Box::new(receiver::ResidualSampleGroups {
            all: group(all),
            fixed_point: group(fixed),
            round_limit: group(round),
            work_limit: group(work),
        })),
    }
}

fn population(p: &ResidualSamplePopulation) -> receiver::ResidualSamplePopulation {
    receiver::ResidualSamplePopulation {
        seen: p.seen,
        capacity: p.capacity,
        retained: p.retained,
    }
}
fn group(g: &ResidualSampleGroup) -> receiver::ResidualSampleGroup {
    receiver::ResidualSampleGroup {
        open_atoms: metric(&g.open_atoms),
        visited: metric(&g.visited),
        attempted_splits: metric(&g.attempted_splits),
        published_splits: metric(&g.published_splits),
        depth: metric(&g.depth),
        work: metric(&g.work),
        query_ns: metric(&g.query_ns),
        residuals: metric(&g.residuals),
        width: metric(&g.width),
        batch_ns: metric(&g.batch_ns),
    }
}
fn metric(m: &ResidualSampleMetric) -> receiver::ResidualSampleMetric {
    receiver::ResidualSampleMetric {
        eligible: m.eligible,
        known: m.known,
        retained: m.retained,
        sum: m.sum,
        maximum: m.maximum,
        median: m.median,
        p90: m.p90,
        quantiles: match m.quantiles {
            ResidualQuantiles::Unavailable => receiver::ResidualQuantiles::Unavailable,
            ResidualQuantiles::Exact => receiver::ResidualQuantiles::Exact,
            ResidualQuantiles::Estimated => receiver::ResidualQuantiles::Estimated,
        },
    }
}
