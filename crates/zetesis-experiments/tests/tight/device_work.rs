use super::*;

fn observation() -> Observation {
    Observation {
        case_index: 0,
        phase: Phase::Initial,
        iteration: 0,
        position: 0,
        route: Route::MetalFresh,
        elapsed_ns: 0,
        classification_ns: 0,
        completion_ns: None,
        activity: Activity {
            device: Some(DeviceWork {
                submissions: 1,
                submitted_candidates: 3,
                scheduled_work: 60,
                completed_candidates: 3,
                completed_work: 60,
                uploaded_bytes: 16,
                downloaded_bytes: 16,
                residency: Some(Residency {
                    theory_uploaded: true,
                    transport_allocated: true,
                    theory_bytes: 16,
                    transport_bytes: 16,
                    accounted_bytes: 64,
                    dispatches: 1,
                    candidates: 3,
                    work: 60,
                }),
            }),
            ..Activity::default()
        },
    }
}

#[test]
fn missing_device_activity_refuses_a_metal_sample() {
    let mut observation = observation();
    observation.activity.device = None;
    assert!(matches!(
        validate_device(&observation, 3, 20),
        Err(Error::DeviceWork)
    ));
}

#[test]
fn completed_shader_work_must_match_the_exact_dimensions() {
    assert!(validate_device(&observation(), 3, 20).is_ok());
    assert!(matches!(
        validate_device(&observation(), 3, 21),
        Err(Error::DeviceWork)
    ));
    assert!(matches!(
        validate_device(&observation(), 4, 20),
        Err(Error::DeviceWork)
    ));
}

#[test]
fn fresh_route_cannot_reuse_residency() {
    for theory in [true, false] {
        let mut observation = observation();
        let residency = observation
            .activity
            .device
            .as_mut()
            .unwrap()
            .residency
            .as_mut()
            .unwrap();
        if theory {
            residency.theory_uploaded = false;
        } else {
            residency.transport_allocated = false;
        }
        assert!(matches!(
            validate_device(&observation, 3, 20),
            Err(Error::DeviceWork)
        ));
    }
}

#[test]
fn resident_reuse_is_required_after_priming() {
    let mut observation = observation();
    observation.route = Route::MetalResident;
    assert!(validate_device(&observation, 3, 20).is_ok());
    observation.phase = Phase::Warmup;
    assert!(matches!(
        validate_device(&observation, 3, 20),
        Err(Error::DeviceWork)
    ));
    let residency = observation
        .activity
        .device
        .as_mut()
        .unwrap()
        .residency
        .as_mut()
        .unwrap();
    residency.theory_uploaded = false;
    residency.transport_allocated = false;
    assert!(validate_device(&observation, 3, 20).is_ok());
}

#[test]
fn unsuccessful_or_partial_work_cannot_qualify() {
    let base = observation();
    for defect in 0..8 {
        let mut observation = base;
        let device = observation.activity.device.as_mut().unwrap();
        match defect {
            0 => device.submissions = 0,
            1 => device.completed_candidates = 2,
            2 => device.scheduled_work = 80,
            3 => device.uploaded_bytes = 0,
            4 => device.downloaded_bytes = 0,
            5 => device.residency = None,
            6 => device.residency.as_mut().unwrap().dispatches = 2,
            _ => device.residency.as_mut().unwrap().work = 0,
        }
        assert!(matches!(
            validate_device(&observation, 3, 20),
            Err(Error::DeviceWork)
        ));
    }
}

#[test]
fn cpu_route_cannot_claim_device_activity() {
    let mut observation = observation();
    observation.route = Route::Scalar;
    assert!(matches!(
        validate_device(&observation, 3, 20),
        Err(Error::DeviceWork)
    ));
}
