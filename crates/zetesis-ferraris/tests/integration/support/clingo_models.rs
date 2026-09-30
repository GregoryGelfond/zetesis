//! clingo's complete models of a source, for the oracle comparisons.

use zetesis_clingo_support as oracle;

pub use zetesis_test_support::records::Models;

/// clingo's models of `source`, from a decided run that enumerated them all.
pub fn clingo(source: &str) -> Models {
    let run = oracle::run(
        source,
        &["0", "--outf=2", "--warn=none"],
        oracle::Limits::default(),
    );
    let json = oracle::json(&run);
    assert!(
        matches!(
            json["Result"].as_str(),
            Some("SATISFIABLE" | "UNSATISFIABLE")
        ),
        "{json}"
    );
    assert_eq!(json["Models"]["More"].as_str(), Some("no"));
    let mut result = Models::new();
    let mut count = 0;
    for call in json["Call"].as_array().unwrap() {
        if let Some(witnesses) = call["Witnesses"].as_array() {
            for witness in witnesses {
                count += 1;
                assert!(
                    result.insert(
                        witness["Value"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|atom| atom.as_str().unwrap().to_owned())
                            .collect()
                    )
                );
            }
        }
    }
    assert_eq!(json["Models"]["Number"].as_u64(), Some(count));
    result
}
