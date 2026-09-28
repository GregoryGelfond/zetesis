//! A record's atoms, spelled. The document's atom table is every record's
//! spelled atoms in document order, and a record refers to its atoms by index
//! into it, so the table read up to the last record resolves any record.

use serde_json::Value;

/// The document's atom table.
pub fn table(document: &Value) -> Vec<Value> {
    document["models"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|record| record["model"]["atoms"].as_array().unwrap().iter().cloned())
        .collect()
}

/// The record's full model as spelled atoms, in the record's order.
pub fn spelled(document: &Value, record: &Value) -> Vec<Value> {
    let table = table(document);
    record["model"]["full_model"]
        .as_array()
        .unwrap()
        .iter()
        .map(|index| table[usize::try_from(index.as_u64().unwrap()).unwrap()].clone())
        .collect()
}
