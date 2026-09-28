//! Original sources and complete records decoded from campaign fixtures.
use serde_json::Value as Json;

use super::source_records::{Records, atoms, costs};

pub struct Case {
    pub name: String,
    pub source: String,
    pub records: Records,
}
pub fn cases(fixture: &str) -> Vec<Case> {
    fixture
        .lines()
        .map(|line| {
            let row: Json = serde_json::from_str(line).unwrap();
            Case {
                name: row["name"].as_str().unwrap().into(),
                source: row["source"].as_str().unwrap().into(),
                records: row["records"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|record| (atoms(&record[0]), costs(&record[1])))
                    .collect(),
            }
        })
        .collect()
}
