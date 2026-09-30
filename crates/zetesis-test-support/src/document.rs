//! A JSON document's answers, read back as spelled atoms.
//!
//! The document's atom table is every record's spelled atoms in document
//! order, and a record refers to its atoms by index into it, so the table
//! read up to the last record resolves any record.

use serde_json::Value;

/// The record's full model as spelled atoms, in the record's order.
///
/// # Panics
/// Panics if `document` or `record` does not have the JSON document's shape.
#[must_use]
pub fn spelled(document: &Value, record: &Value) -> Vec<Value> {
    let table = table(document);
    record["model"]["full_model"]
        .as_array()
        .unwrap()
        .iter()
        .map(|index| table[usize::try_from(index.as_u64().unwrap()).unwrap()].clone())
        .collect()
}

/// The document's atom table.
fn table(document: &Value) -> Vec<Value> {
    document["models"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|record| record["model"]["atoms"].as_array().unwrap().iter().cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_record_resolves_atoms_an_earlier_record_spelled() {
        let document = json!({"models": [
            {"model": {"atoms": ["a", "b"], "full_model": [0, 1]}},
            {"model": {"atoms": ["c"], "full_model": [2, 0]}},
        ]});
        let second = &document["models"][1];
        assert_eq!(spelled(&document, second), [json!("c"), json!("a")]);
    }
}
