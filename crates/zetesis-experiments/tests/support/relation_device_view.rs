//! Supplied accounting views do not constitute physical execution evidence.

use super::DeviceWork;
use zetesis_wgpu::{RelationGpuActivity, RelationGpuStats};

#[test]
fn relation_view_preserves_the_supplied_accounting_fields() {
    // Distinct field values detect misattribution between attempt accounting
    // and successful storage statistics. No device receipt is certified here.
    let activity = RelationGpuActivity {
        submissions: 1,
        submitted_queries: 2,
        submitted_workgroups: 3,
        scheduled_work: 4,
        completed_queries: 5,
        completed_work: 6,
        uploaded_bytes: 7,
        downloaded_bytes: 8,
    };
    let stats = RelationGpuStats {
        rows: 9,
        queries: 10,
        column_bytes: 11,
        transport_bytes: 12,
        accounted_bytes: 13,
        workgroups: [14, 15, 16],
    };
    assert_eq!(
        serde_json::to_value(DeviceWork::new(activity, stats)).unwrap(),
        serde_json::json!({
            "submissions": 1,
            "submitted_queries": 2,
            "completed_queries": 5,
            "workgroups": 3,
            "scheduled_work": 4,
            "completed_work": 6,
            "uploaded_bytes": 7,
            "downloaded_bytes": 8,
            "accounted_bytes": 13,
            "transport_bytes": 12
        })
    );
}
