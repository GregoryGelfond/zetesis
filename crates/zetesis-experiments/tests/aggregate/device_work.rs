use super::super::{Function, Options};
use super::*;
use std::num::NonZeroUsize;
use zetesis_wgpu::{AggregateGpuActivity, AggregateGpuBatchStats};

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

fn observation(prepared: &Prepared<'_>, case: Case) -> Observation {
    let occurrences = case.occurrences.get() as u64;
    let masks = occurrences * (1 + 2 * case.tuples.div_ceil(32)) as u64 * 4;
    let work = 2 * (case.tuples as u64 + 1 + 63) * occurrences;
    let transport = 32 + masks + 80 * occurrences;
    let numeric = prepared.numeric.bytes();
    let batch = AggregateGpuBatchStats {
        group_uploaded: false,
        transport_allocated: false,
        incoming_resident_bytes: 2 * numeric + transport,
        resident_group_bytes: numeric,
        resident_transport_bytes: transport,
        accounted_bytes: 2 * numeric
            + transport
            + masks
            + 40 * occurrences
            + occurrences * size_of::<zetesis_wgpu::AggregateGpuReduction>() as u64,
        host_work: masks / 4 + occurrences * (case.tuples as u64 + 4),
        occurrences,
        device_work: work,
    };
    Observation {
        case_index: 0,
        phase: Phase::Timed,
        iteration: 0,
        position: 0,
        route: Route::DeviceResident,
        elapsed_ns: 1,
        activity: Activity {
            device: Some(DeviceWork {
                activity: AggregateGpuActivity {
                    submissions: 1,
                    submitted_occurrences: occurrences,
                    scheduled_work: work,
                    completed_occurrences: occurrences,
                    completed_work: work,
                    uploaded_bytes: 32 + masks,
                    downloaded_bytes: 40 * occurrences,
                },
                batch: Some(batch),
            }),
            ..Default::default()
        },
    }
}

#[test]
fn inconsistent_device_accounting_cannot_form_a_sample() {
    let configuration = Options {
        backend: crate::Backend::Cpu,
        tuples: vec![33],
        batches: vec![nonzero(3)],
        functions: vec![Function::Sum],
        warmups: 0,
        repetitions: nonzero(1),
        workers: nonzero(1),
        max_work: 100_000_000,
    }
    .configuration()
    .unwrap();
    let case = configuration.cases[0];
    let fixture = fixture::build(case, &Cancellation::default()).unwrap();
    let prepared = Prepared::new(&fixture, 0, &configuration, &Cancellation::default()).unwrap();
    let valid = observation(&prepared, case);
    assert!(validate_activity(case, &prepared, &valid).is_ok());
    let faults: &[fn(&mut DeviceWork)] = &[
        |value| value.activity.submissions = 0,
        |value| value.activity.submitted_occurrences -= 1,
        |value| value.activity.scheduled_work -= 1,
        |value| value.activity.completed_occurrences -= 1,
        |value| value.activity.completed_work -= 1,
        |value| value.activity.uploaded_bytes -= 1,
        |value| value.activity.downloaded_bytes -= 1,
        |value| value.batch = None,
        |value| value.batch.as_mut().unwrap().group_uploaded = true,
        |value| value.batch.as_mut().unwrap().transport_allocated = true,
        |value| value.batch.as_mut().unwrap().occurrences -= 1,
        |value| value.batch.as_mut().unwrap().device_work -= 1,
        |value| value.batch.as_mut().unwrap().resident_group_bytes -= 1,
        |value| value.batch.as_mut().unwrap().resident_transport_bytes -= 1,
        |value| value.batch.as_mut().unwrap().accounted_bytes -= 1,
        |value| value.batch.as_mut().unwrap().incoming_resident_bytes -= 1,
        |value| value.batch.as_mut().unwrap().host_work -= 1,
    ];
    for fault in faults {
        let mut invalid = valid;
        fault(invalid.activity.device.as_mut().unwrap());
        assert!(matches!(
            validate_activity(case, &prepared, &invalid),
            Err(Error::Accounting)
        ));
    }
    let mut no_device = valid;
    no_device.activity.device = None;
    assert!(matches!(
        validate_activity(case, &prepared, &no_device),
        Err(Error::Accounting)
    ));
    let mut wrong_route = valid;
    wrong_route.route = Route::Scalar;
    assert!(matches!(
        validate_activity(case, &prepared, &wrong_route),
        Err(Error::Accounting)
    ));
}
