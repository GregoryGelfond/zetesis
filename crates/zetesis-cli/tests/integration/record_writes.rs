//! Each execution-detail record reaches the diagnostics writer whole, in one
//! write.

use std::io;

use crate::support::options::serial;
use zetesis_cli::{ColorMode, Options, RunError, StatisticsView, run_with_diagnostics};
use zetesis_cpu::Cancellation;
use zetesis_test_support::io::{Chunks, ChunksFailingAt, FAILED};

/// An optimization program whose search installs two objective bounds at one
/// worker, so its records include every kind the tests cover.
const SOURCE: &str =
    "{a;b;c;d}. :- not a, not b. :- not c, not d. #minimize { 1:a; 4:b; 1:c; 4:d }.";

/// `--json --stats`: the records in the plain style.
fn plain() -> Options {
    let mut options = serial(&["--json"]);
    options.stats = true;
    options.statistics_view = StatisticsView::Records;
    options
}

/// `--stats` with the records view and color: the records in the styled style.
fn styled() -> Options {
    let mut options = serial(&[]);
    options.stats = true;
    options.statistics_view = StatisticsView::Records;
    options.color = ColorMode::Always;
    options
}

fn contains(chunk: &[u8], text: &str) -> bool {
    String::from_utf8_lossy(chunk).contains(text)
}

/// The chunks written before the statistics report.
fn record_chunks(options: &Options) -> Vec<Vec<u8>> {
    let mut diagnostics = Chunks::new();
    run_with_diagnostics(
        SOURCE.into(),
        options,
        &mut io::sink(),
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    let chunks = diagnostics.chunks();
    let report = chunks
        .iter()
        .position(|chunk| contains(chunk, "Statistics:"))
        .expect("the statistics report follows the records");
    chunks[..report].to_vec()
}

#[test]
fn execution_detail_records_arrive_one_chunk_each() {
    for options in [plain(), styled()] {
        let records = record_chunks(&options);
        let count = |label: &str| {
            records
                .iter()
                .filter(|chunk| contains(chunk, label))
                .count()
        };
        // The run wrote each kind of record the claim covers: metadata, an
        // observation, information and several objective bounds.
        assert!(count("Source:") + count("Comparisons:") + count("Backend:") >= 1);
        assert_eq!(count("Formula:"), 1);
        assert_eq!(count("Reduct search:"), 1);
        assert!(count("Objective pruning:") >= 2, "{records:?}");
        // Each chunk is one whole line: it ends at its only newline.
        for chunk in &records {
            let line = chunk.strip_suffix(b"\n");
            assert!(line.is_some_and(|line| !line.contains(&b'\n')), "{chunk:?}");
        }
    }
}

#[test]
fn a_call_boundary_writer_fails_at_the_whole_record() {
    // The marker falls inside the first objective bound's record, after the
    // fragments that fragment-by-fragment writes would have delivered.
    let options = plain();
    let records = record_chunks(&options);
    let first_bound = records
        .iter()
        .position(|chunk| contains(chunk, "Objective pruning:"))
        .expect("the run installs an objective bound");
    let mut diagnostics = ChunksFailingAt::new(b"construction work");
    let error = run_with_diagnostics(
        SOURCE.into(),
        &options,
        &mut io::sink(),
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap_err();
    assert_eq!(diagnostics.chunks(), &records[..first_bound]);
    assert!(
        matches!(error, RunError::Output(ref cause) if cause.to_string() == FAILED),
        "{error:?}"
    );
}
