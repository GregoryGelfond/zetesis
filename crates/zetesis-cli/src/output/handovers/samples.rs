//! Compact summaries only. Sampled worlds and batches never enter the CLI document.
use super::{Buffer, RunError};
use zetesis_solve as solve;
use zetesis_validation::performance::matrix as wire;

pub(super) fn write(out: &mut Buffer, samples: &solve::ResidualRunSamples) -> Result<(), RunError> {
    out.text("{\"version\":1,\"algorithm\":\"splitmix64-reject-v1\"")?;
    out.number_field("seed", samples.seed)?;
    out.number_field("byte_limit", samples.byte_limit)?;
    out.number_field("retained_bytes", samples.retained_bytes)?;
    out.number_field("peak_bytes", samples.peak_bytes)?;
    out.text(",\"worlds\":")?;
    out.json(&population(&samples.worlds))?;
    out.text(",\"batches\":")?;
    out.json(&population(&samples.batches))?;
    out.text(",\"groups\":")?;
    match samples.groups.as_deref() {
        None => out.text("null")?,
        Some([all, fixed, round, work]) => {
            out.text("{\"all\":")?;
            out.json(&group(all))?;
            for (name, value) in [
                ("fixed_point", fixed),
                ("round_limit", round),
                ("work_limit", work),
            ] {
                out.text(",")?;
                out.string(name)?;
                out.text(":")?;
                out.json(&group(value))?;
            }
            out.text("}")?;
        }
        Some(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "GPU residual summary lacks its four groups",
            )
            .into());
        }
    }
    out.text("}")
}

fn population(value: &solve::ResidualSamplePopulation) -> wire::ResidualSamplePopulation {
    wire::ResidualSamplePopulation {
        seen: value.seen,
        capacity: value.capacity,
        retained: value.retained,
    }
}

fn group(value: &solve::ResidualSampleGroup) -> wire::ResidualSampleGroup {
    wire::ResidualSampleGroup {
        open_atoms: metric(&value.open_atoms),
        visited: metric(&value.visited),
        attempted_splits: metric(&value.attempted_splits),
        published_splits: metric(&value.published_splits),
        depth: metric(&value.depth),
        work: metric(&value.work),
        query_ns: metric(&value.query_ns),
        residuals: metric(&value.residuals),
        width: metric(&value.width),
        batch_ns: metric(&value.batch_ns),
    }
}

fn metric(value: &solve::ResidualSampleMetric) -> wire::ResidualSampleMetric {
    wire::ResidualSampleMetric {
        eligible: value.eligible,
        known: value.known,
        retained: value.retained,
        sum: value.sum,
        maximum: value.maximum,
        median: value.median,
        p90: value.p90,
        quantiles: match value.quantiles {
            solve::ResidualQuantiles::Unavailable => wire::ResidualQuantiles::Unavailable,
            solve::ResidualQuantiles::Exact => wire::ResidualQuantiles::Exact,
            solve::ResidualQuantiles::Estimated => wire::ResidualQuantiles::Estimated,
        },
    }
}
