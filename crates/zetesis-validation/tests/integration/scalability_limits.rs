//! Maintained measurement capacity remains separate from solver resources.
use serde_json::json;
use zetesis_validation::{
    answers::{Error, Resource, native_json},
    performance::{Limits, scalability},
};

#[test]
fn scalability_presets_preserve_independent_ceilings() {
    let mut base = Limits::default();
    base.process.timeout = std::time::Duration::from_secs(7);
    base.campaign_timeout = std::time::Duration::from_secs(11);
    base.max_total_capture_bytes = 123;
    base.max_report_bytes = 456;
    let limits = scalability::limits(base);
    assert_eq!(limits.process.max_output_bytes, scalability::CAPTURE_BYTES);
    assert_eq!(limits.answers.max_input_bytes, scalability::CAPTURE_BYTES);
    assert_eq!(limits.process.timeout, base.process.timeout);
    assert_eq!(limits.process.cleanup_timeout, base.process.cleanup_timeout);
    assert_eq!(limits.campaign_timeout, base.campaign_timeout);
    assert_eq!(limits.max_total_capture_bytes, 123);
    assert_eq!(limits.max_report_bytes, 456);
    assert_eq!(limits.max_executable_bytes, base.max_executable_bytes);
    assert_eq!(limits.answers.max_symbols, base.answers.max_symbols);
    assert_eq!(limits.answers.max_witnesses, base.answers.max_witnesses);
    assert_eq!(
        limits.answers.max_cost_dimensions,
        base.answers.max_cost_dimensions
    );

    let mut native = native_json::Limits::default();
    native.value.max_nodes = 17;
    native.value.max_depth = 3;
    native.value.max_bytes = 400;
    native.report.max_witnesses = 19;
    native.report.max_symbols = 23;
    native.report.max_cost_dimensions = 7;
    let normalized = scalability::native_answers(native);
    assert_eq!(normalized.value.max_nodes, 17);
    assert_eq!(normalized.value.max_depth, 3);
    assert_eq!(normalized.value.max_bytes, 400);
    assert_eq!(normalized.report.max_witnesses, 19);
    assert_eq!(normalized.report.max_symbols, 23);
    assert_eq!(normalized.report.max_cost_dimensions, 7);
}

#[test]
fn scalability_presets_keep_larger_caller_capacities() {
    let mut base = Limits::default();
    base.process.max_output_bytes = 2 * scalability::CAPTURE_BYTES;
    base.answers.max_input_bytes = 2 * scalability::CAPTURE_BYTES;
    let limits = scalability::limits(base);
    assert_eq!(
        limits.process.max_output_bytes,
        base.process.max_output_bytes
    );
    assert_eq!(limits.answers.max_input_bytes, base.answers.max_input_bytes);
    let mut native = native_json::Limits::default();
    native.report.max_input_bytes = 2 * scalability::CAPTURE_BYTES;
    native.max_atoms = 2 * scalability::NATIVE_ATOMS;
    native.max_value_nodes = 2 * scalability::NATIVE_VALUE_NODES;
    let normalized = scalability::native_answers(native);
    assert_eq!(
        normalized.report.max_input_bytes,
        native.report.max_input_bytes
    );
    assert_eq!(normalized.max_atoms, native.max_atoms);
    assert_eq!(normalized.max_value_nodes, native.max_value_nodes);
}

#[test]
fn compact_records_keep_complete_occurrence_accounting() {
    // Table reuse makes the input small, but every full-model occurrence still
    // consumes normalization capacity. Cross both former default ceilings.
    let atoms: Vec<_> = (0..512)
        .map(|index| {
            let arguments: Vec<_> = [index, 0, 1, 2]
                .map(|value| json!([{"kind":"number","value":value}]))
                .into();
            json!({"predicate":"p","sign":"positive","arguments":arguments})
        })
        .collect();
    let full: Vec<_> = (0..atoms.len()).collect();
    let records: Vec<_> = (1..=513)
        .map(|number| {
            let spelled = if number == 1 { atoms.as_slice() } else { &[] };
            json!({"number":number,"model":{"atoms":spelled,"full_model":full,
                "shown":{"atom_indices":[],"terms":[]},"costs":null}})
        })
        .collect();
    let document = serde_json::to_vec(&json!({"schema":2,"format":"zetesis",
        "models":records,"outcome":{"status":"satisfiable","completion":"exhausted",
        "coverage":"exhausted","published_models":513,"verified_models":513,
        "checked":513,"interruption":null,"optimization":null,"error":null}}))
    .unwrap();
    let base = native_json::Limits::default();
    assert!(matches!(
        native_json::parse(&document, base),
        Err(Error::Limit {
            resource: Resource::Atoms,
            ..
        })
    ));
    let atoms_only = native_json::Limits {
        max_atoms: scalability::NATIVE_ATOMS,
        ..base
    };
    assert!(matches!(
        native_json::parse(&document, atoms_only),
        Err(Error::Limit {
            resource: Resource::ValueNodes,
            ..
        })
    ));
    let normalized = native_json::parse(&document, scalability::native_answers(base)).unwrap();
    assert_eq!(normalized.records().len(), 513);
    assert!(
        normalized
            .records()
            .iter()
            .all(|record| record.full_model().len() == 512)
    );
}
