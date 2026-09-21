use super::{Completion, Error, ViewOptions};
use clap::{Args, Subcommand};
use std::{
    io::{self, Write},
    path::PathBuf,
};
use zetesis_experiments::{
    aggregate_measurement as aggregate, lazy_measurement as lazy,
    primitives::{self, Event, Request},
    relation_measurement as relation, tight_measurement as tight,
};
use zetesis_presentation::{Layout, Row};

/// Primitive profiles that expose typed preparation and sample observations.
/// Static/formula compatibility experiments remain in `zetesis-bench` until
/// they offer the same typed presentation boundary.
#[derive(Debug, Subcommand)]
pub enum Primitive {
    /// Packed equality masks and typed row reconstruction.
    Relation(relation::Options),
    /// Exact native aggregate reductions.
    Aggregate(aggregate::Options),
    /// Tight support classification with exact residual completion.
    Tight(tight::Options),
    /// Matched scalar, Rayon and lazy round-source operations.
    Lazy(lazy::Options),
}
impl Primitive {
    /// Map arguments into the profile's validated library request.
    ///
    /// # Errors
    /// Refuses unsupported dimensions and finite schedule/operation bounds.
    pub fn request(&self) -> Result<Request, zetesis_experiments::command::Error> {
        use zetesis_experiments::command::Error as MeasurementError;
        match self {
            Self::Relation(options) => options
                .configuration()
                .map(Request::Relation)
                .map_err(MeasurementError::Relation),
            Self::Aggregate(options) => options
                .configuration()
                .map(Request::Aggregate)
                .map_err(MeasurementError::Aggregate),
            Self::Tight(options) => options
                .configuration()
                .map(Request::Tight)
                .map_err(MeasurementError::Tight),
            Self::Lazy(options) => options
                .configuration()
                .map(Request::Lazy)
                .map_err(MeasurementError::Lazy),
        }
    }
}

/// Bounded primitive event publication, separate from measurement configuration.
#[derive(Debug, Args)]
pub struct PrimitiveOptions {
    /// Profile whose complete matched outcomes must qualify every timed sample.
    #[command(subcommand)]
    pub primitive: Primitive,
    /// Optional new JSON-lines evidence file; partial prefixes survive failures.
    #[arg(long, global = true)]
    pub report: Option<PathBuf>,
    /// Maximum serialized bytes per JSON-lines sink.
    #[arg(long, global = true, default_value_t = 256 * 1024 * 1024)]
    pub report_bytes: usize,
    /// With --json, stdout is the profile's versioned JSON-lines event stream.
    #[command(flatten)]
    pub view: ViewOptions,
}

pub(super) fn execute(
    options: &PrimitiveOptions,
    layout: Layout,
    output: &mut impl Write,
) -> Result<Completion, Error> {
    let request = options.primitive.request().map_err(Error::Primitive)?;
    let file = options
        .report
        .as_ref()
        .map(|path| {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
        })
        .transpose()
        .map_err(Error::Io)?;
    let mut report = file.map(|file| Bounded {
        writer: file,
        remaining: options.report_bytes,
    });
    let mut stdout = Bounded {
        writer: output,
        remaining: options.report_bytes,
    };
    let mut rows = Vec::new();
    let result = primitives::measure(&request, |event| {
        if let Some(report) = &mut report {
            write_event(report, &event)?;
        }
        if options.view.json {
            write_event(&mut stdout, &event)?;
        } else if let Some(row) = sample(&event) {
            rows.push(row);
        }
        Ok(())
    });
    // A later view or flush error must not replace the measurement failure.
    let mut result = result.map_err(Error::Primitive);
    if let Some(report) = &mut report {
        result = retain_primary(result, report.flush().map_err(Error::Io));
    }
    if !options.view.json {
        let rendered = super::view::table(
            if result.is_ok() {
                "Primitive measurements — complete matched samples"
            } else {
                "Primitive measurements — completed prefix; command failed"
            },
            &[
                "Profile",
                "Case",
                "Route",
                "Population",
                "Elapsed ms",
                "Checked",
            ],
            rows,
        )
        .and_then(|table| table.write(&mut stdout.writer, layout).map_err(Error::Io));
        result = retain_primary(result, rendered);
    }
    result?;
    Ok(Completion::Passed)
}

fn retain_primary(primary: Result<(), Error>, secondary: Result<(), Error>) -> Result<(), Error> {
    match (primary, secondary) {
        (Err(primary), Err(secondary)) => Err(Error::Reporting {
            primary: Box::new(primary),
            secondary: Box::new(secondary),
        }),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn write_event(output: &mut impl Write, event: &Event<'_, '_>) -> io::Result<()> {
    serde_json::to_writer(&mut *output, event).map_err(io::Error::other)?;
    writeln!(output)
}

// This view only accepts completed typed sample events. Failed measured prefixes
// remain in structured evidence and never join a successful timing population.
fn sample(event: &Event<'_, '_>) -> Option<Row> {
    let (profile, case, route, phase, elapsed, checked) = match event {
        Event::Relation(relation::Event::Observation(observation)) => (
            "relation",
            0,
            format!("{:?}", observation.route),
            format!("{:?}", observation.phase),
            observation.operation_ns,
            observation.queries,
        ),
        Event::Aggregate(aggregate::Event::Sample { sample }) => {
            let observation = sample.observation;
            (
                "aggregate",
                observation.case_index,
                format!("{:?}", observation.route),
                format!("{:?}", observation.phase),
                observation.elapsed_ns,
                sample.outcomes.len(),
            )
        }
        Event::Tight(tight::Event::Sample { sample }) => {
            let observation = sample.observation;
            (
                "tight",
                observation.case_index,
                format!("{:?}", observation.route),
                format!("{:?}", observation.phase),
                observation.elapsed_ns,
                sample.outcomes.len(),
            )
        }
        Event::Lazy(lazy::Event::Sample(sample)) => (
            "lazy",
            sample.case_index,
            format!("{:?}", sample.route),
            format!("{:?}", sample.phase),
            sample.elapsed_ns,
            sample.checked,
        ),
        _ => return None,
    };
    Some(Row::new([
        profile.into(),
        case.to_string(),
        route,
        phase,
        format!("{}.{:03}", elapsed / 1_000_000, elapsed % 1_000_000 / 1000),
        checked.to_string(),
    ]))
}

struct Bounded<W> {
    writer: W,
    remaining: usize,
}
impl<W: Write> Write for Bounded<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.remaining {
            return Err(io::Error::other(
                "primitive JSON-lines byte ceiling exceeded",
            ));
        }
        let written = self.writer.write(bytes)?;
        self.remaining -= written;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
