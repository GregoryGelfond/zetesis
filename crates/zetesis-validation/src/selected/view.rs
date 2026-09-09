//! Explicit serialized resource units without adding serialization to the core.
use serde::{Serialize, Serializer};
use serde_json::{Value, json};

use super::{Limits, Report};

#[derive(Serialize)]
pub(super) struct Published<'a> {
    pub passed: bool,
    #[serde(flatten)]
    pub report: &'a Report,
}

fn answers(limits: crate::answers::Limits) -> Value {
    json!({
        "max_input_bytes": limits.max_input_bytes,
        "max_witnesses": limits.max_witnesses,
        "max_symbols": limits.max_symbols,
        "max_cost_dimensions": limits.max_cost_dimensions,
    })
}

impl Serialize for Limits {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        json!({
            "corpus": {
                "manifest_bytes": self.corpus.manifest_bytes,
                "source_bytes": self.corpus.source_bytes,
                "total_source_bytes": self.corpus.total_source_bytes,
                "license_bytes": self.corpus.license_bytes,
                "cases": self.corpus.cases,
                "models": self.corpus.models,
                "atoms": self.corpus.atoms,
            },
            "process": {
                "timeout": self.process.timeout,
                "cleanup_timeout": self.process.cleanup_timeout,
                "max_output_bytes": self.process.max_output_bytes,
            },
            "max_total_capture_bytes": self.max_total_capture_bytes,
            "max_executable_bytes": self.max_executable_bytes,
            "reference": answers(self.reference),
            "native": {
                "report": answers(self.native.report),
                "max_atoms": self.native.max_atoms,
                "max_value_nodes": self.native.max_value_nodes,
                "value": {
                    "max_nodes": self.native.value.max_nodes,
                    "max_depth": self.native.value.max_depth,
                    "max_bytes": self.native.value.max_bytes,
                },
            },
            "max_spelling_bytes": self.max_spelling_bytes,
            "max_report_bytes": self.max_report_bytes,
        })
        .serialize(serializer)
    }
}
