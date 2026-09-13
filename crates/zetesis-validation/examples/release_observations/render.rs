//! Fixed three-sample medians and independently sampled RSS table views.

use std::fmt::Write;

use zetesis_validation::performance::{Phase, Producer};

use super::data::{self, Block, Observation, Observations};
use super::{Result, require};

pub(super) fn tables(data: &Observations) -> Result<String> {
    data::validate(data)?;
    let mut output = String::new();
    for (producer, caption) in [
        (
            Producer::Native,
            "Native wall time, ms: median [minimum, maximum] of three timed samples per block.",
        ),
        (
            Producer::Reference,
            "Matched clingo wall time, ms, with the same three-sample notation.",
        ),
    ] {
        header(&mut output, caption, data)?;
        for case in &data.cases {
            write!(output, "| {} |", case.label)?;
            for block in &data.blocks {
                let mut observations = samples(block, &case.path, producer, Phase::Timed);
                let mut values = [0; 3];
                for value in &mut values {
                    *value = observations
                        .next()
                        .ok_or("missing timed sample")?
                        .capture
                        .elapsed_ns;
                }
                require(observations.next().is_none(), "extra timed sample")?;
                values.sort_unstable();
                write!(
                    output,
                    " {} [{}, {}] |",
                    decimal(values[1], 1_000_000),
                    decimal(values[0], 1_000_000),
                    decimal(values[2], 1_000_000)
                )?;
            }
            output.push('\n');
        }
        output.push('\n');
    }
    header(
        &mut output,
        "Separate child RSS, MiB. Each cell is **native / clingo**, one fresh-helper observation of each per block.",
        data,
    )?;
    for case in &data.cases {
        write!(output, "| {} |", case.label)?;
        for block in &data.blocks {
            let native = rss(block, &case.path, Producer::Native)?;
            let reference = rss(block, &case.path, Producer::Reference)?;
            write!(
                output,
                " {} / {} |",
                decimal(native, 1_048_576),
                decimal(reference, 1_048_576)
            )?;
        }
        output.push('\n');
    }
    Ok(output)
}

fn header(output: &mut String, caption: &str, data: &Observations) -> Result<()> {
    writeln!(output, "{caption}\n")?;
    output.push_str("| Case |");
    for block in &data.blocks {
        write!(output, " {} |", block.label)?;
    }
    output.push_str("\n|---|---:|---:|---:|---:|\n");
    Ok(())
}

fn samples<'a>(
    block: &'a Block,
    case: &'a str,
    producer: Producer,
    phase: Phase,
) -> impl Iterator<Item = &'a Observation> {
    block.observations.iter().filter(move |sample| {
        sample.case == case && sample.producer == producer && sample.phase == phase
    })
}

fn rss(block: &Block, case: &str, producer: Producer) -> Result<u64> {
    let mut observations = samples(block, case, producer, Phase::Memory);
    let memory = observations
        .next()
        .and_then(|sample| sample.memory.as_ref())
        .ok_or("missing child-RSS sample")?;
    require(observations.next().is_none(), "extra child-RSS sample")?;
    Ok(memory.peak_rss_bytes)
}

/// Round an integer-unit observation to three places, nearest with ties to even.
/// The u64 input times 1,000 fits u128; neither nanoseconds nor bytes use f64.
fn decimal(value: u64, scale: u64) -> String {
    let numerator = u128::from(value) * 1_000;
    let denominator = u128::from(scale);
    let whole = numerator / denominator;
    let twice_remainder = (numerator % denominator) * 2;
    let rounded = whole
        + u128::from(
            twice_remainder > denominator
                || (twice_remainder == denominator && !whole.is_multiple_of(2)),
        );
    format!("{}.{:03}", rounded / 1_000, rounded % 1_000)
}

#[cfg(test)]
mod tests {
    use super::decimal;

    #[test]
    fn exact_time_midpoints_round_to_the_even_final_digit() {
        for (nanoseconds, below, at, above) in [
            (32_871_500, "32.871", "32.872", "32.872"),
            (9_117_500, "9.117", "9.118", "9.118"),
            (8_958_500, "8.958", "8.958", "8.959"),
        ] {
            assert_eq!(decimal(nanoseconds - 1, 1_000_000), below);
            assert_eq!(decimal(nanoseconds, 1_000_000), at);
            assert_eq!(decimal(nanoseconds + 1, 1_000_000), above);
        }
    }

    #[test]
    fn exact_memory_midpoints_round_to_the_even_final_digit() {
        // Both exact fractional MiB values end in .8125. Neither conversion
        // through f64 nor a half-away rule supplies this table's rounding law.
        for (bytes, below, above) in [
            (6_094_848, "5.812", "5.813"),
            (13_434_880, "12.812", "12.813"),
        ] {
            assert_eq!(decimal(bytes - 1, 1_048_576), below);
            assert_eq!(decimal(bytes, 1_048_576), below);
            assert_eq!(decimal(bytes + 1, 1_048_576), above);
        }
    }
}
