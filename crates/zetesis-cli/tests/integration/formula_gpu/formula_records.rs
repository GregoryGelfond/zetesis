//! Separate displayed records from complete semantic model identities.

use zetesis_core::Atom;
use zetesis_validation::answers::native_json;

type FullRecord = (Vec<Atom>, Option<Vec<(i32, i64)>>);

pub fn full_records(output: &[u8]) -> Vec<FullRecord> {
    let answers = native_json::parse(output, native_json::Limits::default()).unwrap();
    let mut rows: Vec<_> = answers
        .records()
        .iter()
        .map(|record| {
            let mut atoms = record.full_model().to_vec();
            atoms.sort();
            (atoms, record.costs().map(<[_]>::to_vec))
        })
        .collect();
    // Preserve duplicate record multiplicity rather than coalescing a bad run.
    rows.sort();
    rows
}

pub fn displayed_records(output: &[u8]) -> Vec<(Vec<String>, Option<String>)> {
    let text = std::str::from_utf8(output).unwrap();
    let mut rows = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        if line.starts_with("Answer:") {
            // These named tiny fixtures deliberately contain no whitespace in symbols.
            let mut atoms: Vec<_> = lines
                .next()
                .unwrap()
                .split_whitespace()
                .map(str::to_owned)
                .collect();
            atoms.sort();
            let score = lines
                .peek()
                .filter(|line| line.starts_with("Optimization:"))
                .map(|_| ())
                .and_then(|()| lines.next().map(str::to_owned));
            rows.push((atoms, score));
        }
    }
    rows.sort();
    rows
}
