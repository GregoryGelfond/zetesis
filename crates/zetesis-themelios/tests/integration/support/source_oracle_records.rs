//! Complete clingo JSON records with occurrence and completion reconciliation.

use serde_json::Value as Json;

use crate::support::source_records::{Records, atoms, costs};

/// Require complete enumeration and reconcile every full-model occurrence.
pub(crate) fn model_records(json: &Json) -> Records {
    assert_eq!(json["Models"]["More"].as_str(), Some("no"));
    assert!(matches!(
        json["Result"].as_str(),
        Some("SATISFIABLE" | "UNSATISFIABLE" | "OPTIMUM FOUND")
    ));
    let mut records = Records::new();
    let mut count = 0;
    for call in json["Call"].as_array().expect("oracle calls") {
        if let Some(witnesses) = call["Witnesses"].as_array() {
            for witness in witnesses {
                count += 1;
                assert!(
                    records.insert((atoms(&witness["Value"]), costs(&witness["Costs"]))),
                    "duplicate full model"
                );
            }
        }
    }
    assert_eq!(json["Models"]["Number"].as_u64(), Some(count));
    records
}
