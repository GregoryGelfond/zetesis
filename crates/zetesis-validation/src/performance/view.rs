//! Serialized limits retain their distinct units and measurement boundaries.
use super::{Limits, Report};
use serde::{Serialize, Serializer};
use serde_json::json;
#[derive(Serialize)]
pub(super) struct Published<'a> {
    pub passed: bool,
    #[serde(flatten)]
    pub report: &'a Report,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<Vec<super::Summary>>,
}
impl Serialize for Limits {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        json!({ "corpus": { "manifest_bytes":self.corpus.manifest_bytes,
            "source_bytes":self.corpus.source_bytes,"total_source_bytes":self.corpus.total_source_bytes,
            "files":self.corpus.files,"cases":self.corpus.cases,"contract_symbols":self.corpus.contract_symbols },
            "process": { "timeout":self.process.timeout,"cleanup_timeout":self.process.cleanup_timeout,"max_output_bytes":self.process.max_output_bytes },
            "campaign_timeout":self.campaign_timeout,"max_total_capture_bytes":self.max_total_capture_bytes,
            "max_executable_bytes":self.max_executable_bytes,"max_report_bytes":self.max_report_bytes,
            "answers":{"max_input_bytes":self.answers.max_input_bytes,"max_witnesses":self.answers.max_witnesses,
            "max_symbols":self.answers.max_symbols,"max_cost_dimensions":self.answers.max_cost_dimensions} }).serialize(serializer)
    }
}
