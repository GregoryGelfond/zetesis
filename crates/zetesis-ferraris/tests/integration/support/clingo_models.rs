//! clingo's complete models of a source, for the oracle comparisons.

use zetesis_clingo_support as oracle;

pub(crate) use zetesis_test_support::records::Models;

/// clingo's models of `source`, from a decided run that enumerated them all.
pub(crate) fn clingo(source: &str) -> Models {
    let run = oracle::run(
        source,
        &["0", "--outf=2", "--warn=none"],
        oracle::Limits::default(),
    );
    oracle::objective_free_models(&oracle::json(&run))
}
