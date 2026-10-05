//! A statistics report reaches the diagnostics writer whole, in one write.

use std::io::{self, Write};

use crate::support::options::serial;
use zetesis_cli::{Options, StatisticsView, run_with_diagnostics};
use zetesis_cpu::Cancellation;

const SOURCE: &str = "a :- not b. b :- not a.";

/// Records every `write` call as its own chunk.
#[derive(Default)]
struct Chunks(Vec<Vec<u8>>);

impl Write for Chunks {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.push(bytes.to_vec());
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn options(json: bool, view: StatisticsView) -> Options {
    let mut options = serial(if json { &["--json"] } else { &[] });
    options.stats = true;
    options.statistics_view = view;
    options
}

/// The index of the one chunk containing `marker`.
fn chunk_of(chunks: &Chunks, marker: &str) -> usize {
    let found: Vec<usize> = chunks
        .0
        .iter()
        .enumerate()
        .filter(|(_, chunk)| String::from_utf8_lossy(chunk).contains(marker))
        .map(|(index, _)| index)
        .collect();
    assert_eq!(found.len(), 1, "{marker:?} in chunks {found:?}");
    found[0]
}

fn report_chunks(options: &Options, first: &str, last: &str) -> (usize, usize) {
    let mut diagnostics = Chunks::default();
    run_with_diagnostics(
        SOURCE.into(),
        options,
        &mut io::sink(),
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    (chunk_of(&diagnostics, first), chunk_of(&diagnostics, last))
}

#[test]
fn record_statistics_arrive_in_one_write() {
    // `--json --stats`: the records view, from its header to its scope line.
    let (first, last) = report_chunks(
        &options(true, StatisticsView::Records),
        "Statistics: zetesis",
        "grounding scope:",
    );
    assert_eq!(first, last);
}

#[test]
fn human_statistics_arrive_in_one_write() {
    // `--stats`: the human tables, from their title to their closing note.
    let (first, last) = report_chunks(
        &options(false, StatisticsView::Human),
        "Stage ",
        "subtotals must not be added",
    );
    assert_eq!(first, last);
}

/// Accepts `remaining` bytes, then fails every write.
struct FailsAfter {
    remaining: usize,
}

impl Write for FailsAfter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::other("diagnostics closed"));
        }
        let taken = bytes.len().min(self.remaining);
        self.remaining -= taken;
        Ok(taken)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_failing_statistics_write_is_an_output_failure() {
    // Fail a few bytes into the report's header: the diagnostics before it
    // carry no timings, so their length is the same in both runs.
    for (view, header) in [
        (StatisticsView::Records, "Statistics: zetesis"),
        (StatisticsView::Human, "Statistics\n"),
    ] {
        let options = options(matches!(view, StatisticsView::Records), view);
        let mut diagnostics = Chunks::default();
        run_with_diagnostics(
            SOURCE.into(),
            &options,
            &mut io::sink(),
            &mut diagnostics,
            &Cancellation::default(),
        )
        .unwrap();
        let written = diagnostics.0.concat();
        let before = String::from_utf8_lossy(&written)
            .find(header)
            .expect("the report has its header")
            + 4;
        let error = run_with_diagnostics(
            SOURCE.into(),
            &options,
            &mut io::sink(),
            &mut FailsAfter { remaining: before },
            &Cancellation::default(),
        )
        .unwrap_err();
        assert!(
            matches!(error, zetesis_cli::RunError::Output(ref cause) if cause.to_string() == "diagnostics closed"),
            "{error:?}"
        );
    }
}
