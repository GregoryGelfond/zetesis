//! View contracts only: supplied accounting is not physical execution evidence.

use std::io::{self, Write};
use zetesis_experiments::aggregate_measurement::DeviceWork;
use zetesis_wgpu::{AggregateGpuActivity, AggregateGpuBatchStats};

fn supplied_work() -> DeviceWork {
    // Distinct values make a swapped, omitted or duplicated field observable.
    // This is a formatting fixture, not an accepted GPU execution receipt.
    DeviceWork {
        activity: AggregateGpuActivity {
            submissions: 1,
            submitted_occurrences: 2,
            scheduled_work: 3,
            completed_occurrences: 4,
            completed_work: 5,
            uploaded_bytes: 6,
            downloaded_bytes: 7,
        },
        batch: Some(AggregateGpuBatchStats {
            group_uploaded: true,
            transport_allocated: false,
            incoming_resident_bytes: 8,
            resident_group_bytes: 9,
            resident_transport_bytes: 10,
            accounted_bytes: 11,
            host_work: 12,
            occurrences: 13,
            device_work: 14,
        }),
    }
}

#[test]
fn the_device_view_preserves_each_supplied_account() {
    assert_eq!(
        serde_json::to_value(supplied_work()).unwrap(),
        serde_json::json!({
            "submissions": 1,
            "submitted_occurrences": 2,
            "scheduled_work": 3,
            "completed_occurrences": 4,
            "completed_work": 5,
            "uploaded_bytes": 6,
            "downloaded_bytes": 7,
            "batch": {
                "group_uploaded": true,
                "transport_allocated": false,
                "incoming_resident_bytes": 8,
                "resident_group_bytes": 9,
                "resident_transport_bytes": 10,
                "accounted_bytes": 11,
                "host_work": 12,
                "occurrences": 13,
                "device_work": 14
            }
        })
    );
}

#[test]
fn an_absent_batch_does_not_invent_success_accounting() {
    let mut work = supplied_work();
    work.batch = None;
    let view = serde_json::to_value(work).unwrap();
    assert!(view["batch"].is_null());
    assert_eq!(view["submissions"], 1);
    assert_eq!(view["downloaded_bytes"], 7);
}

struct Prefix {
    remaining: usize,
    bytes: Vec<u8>,
}

impl Write for Prefix {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "view sink closed",
            ));
        }
        let count = self.remaining.min(bytes.len());
        self.bytes.extend_from_slice(&bytes[..count]);
        self.remaining -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn device_serialization_preserves_every_failed_prefix() {
    for batch in [None, supplied_work().batch] {
        let work = DeviceWork {
            batch,
            ..supplied_work()
        };
        let expected = serde_json::to_vec(&work).unwrap();
        for capacity in 0..expected.len() {
            let mut sink = Prefix {
                remaining: capacity,
                bytes: Vec::new(),
            };
            let error = serde_json::to_writer(&mut sink, &work).unwrap_err();
            assert_eq!(error.io_error_kind(), Some(io::ErrorKind::BrokenPipe));
            assert!(error.to_string().contains("view sink closed"));
            assert_eq!(sink.bytes, expected[..capacity]);
        }
    }
}
