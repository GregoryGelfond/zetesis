use super::*;
use zetesis_cpu::Stop;
use zetesis_ferraris::AdmissionLimits;

fn theory(atoms: usize) -> Theory {
    Theory::new(
        atoms,
        zetesis_ferraris::FormulaParts::new(vec![], Vec::new()).unwrap(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn complete_packing_keeps_repeated_candidate_rows() {
    let theory = theory(65);
    let first = Interpretation::new(&theory, [0, 33, 64]).unwrap();
    let second = Interpretation::new(&theory, [31, 63]).unwrap();
    let mut output = [u32::MAX; 9];
    pack(
        &theory,
        &[first.clone(), second, first],
        &mut output,
        || Ok(()),
    )
    .unwrap();
    assert_eq!(output, [1, 2, 1, 0x8000_0000, 0x8000_0000, 0, 1, 2, 1]);
}

#[test]
fn packing_refuses_every_incomplete_output_shape() {
    let theory = theory(33);
    let candidate = Interpretation::new(&theory, [32]).unwrap();
    for length in [0, 1, 3, 4] {
        let mut output = vec![7; length];
        let error = pack(
            &theory,
            std::slice::from_ref(&candidate),
            &mut output,
            || Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.kind(), GpuErrorKind::Capacity);
        assert!(output.iter().all(|word| *word == 7));
    }
}

#[test]
fn empty_carrier_padding_does_not_erase_owner_checks() {
    let owner = theory(0);
    let equal = theory(0);
    let candidate = Interpretation::new(&owner, []).unwrap();
    let foreign = Interpretation::new(&equal, []).unwrap();
    let mut output = [7];
    let error = pack(&owner, &[foreign], &mut output, || Ok(())).unwrap_err();
    assert_eq!(error.kind(), GpuErrorKind::Seed);
    assert_eq!(output, [7]);
    pack(&owner, &[candidate.clone(), candidate], &mut output, || {
        Ok(())
    })
    .unwrap();
    assert_eq!(output, [0]);
}

#[test]
fn interrupted_export_retains_only_its_completed_word_prefix() {
    let theory = theory(65);
    let candidate = Interpretation::new(&theory, [0, 32, 64]).unwrap();
    for completed in 0..3 {
        let mut output = [7; 3];
        let mut polls = 0;
        let error = pack(
            &theory,
            std::slice::from_ref(&candidate),
            &mut output,
            || {
                // Admission and occurrence control precede every word's own poll.
                polls += 1;
                if polls == completed + 3 {
                    Err(GpuError::interrupted(Stop::Cancelled))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert_eq!(error.interruption(), Some(Stop::Cancelled));
        assert!(output[..completed].iter().all(|word| *word == 1));
        assert!(output[completed..].iter().all(|word| *word == 7));
    }
}

#[test]
fn cancellation_precedes_shape_refusal() {
    let theory = theory(65);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let error = pack(&theory, &[], &mut [], || {
        cancellation.poll().map_err(GpuError::interrupted)
    })
    .unwrap_err();
    assert_eq!(error.interruption(), Some(Stop::Cancelled));
}

#[test]
fn equal_shape_reuses_initialized_storage_and_overwrites_every_word() {
    let theory = theory(65);
    let first = Interpretation::new(&theory, [0, 33, 64]).unwrap();
    let second = Interpretation::new(&theory, [31, 63]).unwrap();
    let mut staging = Staging::default();
    let mut reservations = 0;
    for candidates in [
        [first.clone(), second.clone(), first],
        [second.clone(), second.clone(), second],
    ] {
        staging
            .reserve_with(36, 100, u64::MAX, |length| {
                reservations += 1;
                reserve(length)
            })
            .unwrap();
        let initialized = staging.words().to_vec();
        staging
            .reserve_with(36, 100, u64::MAX, |_| panic!("warm reservation"))
            .unwrap();
        assert_eq!(staging.words(), initialized);
        let charge = staging
            .pack(
                &theory,
                &candidates,
                36,
                100,
                u64::MAX,
                &Cancellation::default(),
            )
            .unwrap();
        let mut fresh = vec![u32::MAX; 9];
        pack(&theory, &candidates, &mut fresh, || Ok(())).unwrap();
        assert_eq!(staging.words(), fresh);
        assert_eq!(
            charge,
            capacity_charge(64, staging.words.capacity()).unwrap()
        );
    }
    assert_eq!(reservations, 1);
}

#[test]
fn smaller_allowance_replaces_optional_capacity() {
    let mut staging = Staging::default();
    let charge = staging
        .reserve_with(4, 20, u64::MAX, |_| reserve(128))
        .unwrap();
    assert!(charge > 20);
    let mut replacements = 0;
    let reduced = staging
        .reserve_with(4, 20, 20, |length| {
            replacements += 1;
            reserve(length)
        })
        .unwrap();
    assert_eq!(reduced, 20);
    assert_eq!(replacements, 1);
    assert_eq!(staging.words(), [0]);
}

#[test]
fn staging_checks_complete_actual_capacity_before_initialization() {
    let mut staging = Staging::default();
    for (ceiling, admitted) in [(31, false), (32, true)] {
        let result = staging.reserve_with(16, 32, ceiling, reserve);
        assert_eq!(result.is_ok(), admitted);
        if let Err(error) = result {
            assert_eq!(error.kind(), GpuErrorKind::Capacity);
            assert!(staging.words().is_empty());
        }
    }
    staging.release();
    let error = staging
        .reserve_with(16, 32, 32, |_| reserve(128))
        .unwrap_err();
    assert_eq!(error.kind(), GpuErrorKind::Capacity);
    assert_eq!(staging.words.capacity(), 0);
    for (requested, accounted) in [(0, 0), (1, 16), (16, 8)] {
        assert_eq!(
            staging
                .reserve_with(requested, accounted, u64::MAX, |_| panic!(
                    "invalid envelope"
                ))
                .unwrap_err()
                .kind(),
            GpuErrorKind::Capacity
        );
    }
    assert_eq!(
        capacity_charge(u64::MAX, 1).unwrap_err().kind(),
        GpuErrorKind::Capacity
    );
    if usize::BITS == 64 {
        assert_eq!(
            capacity_charge(0, usize::MAX).unwrap_err().kind(),
            GpuErrorKind::Capacity
        );
    }
}

#[test]
fn failed_replacement_releases_the_previous_allocation() {
    let mut staging = Staging::default();
    staging
        .reserve_with(4, 4, u64::MAX, |_| reserve(128))
        .unwrap();
    let error = staging
        .reserve_with(4, 4, 4, |_| {
            Err(GpuError::new(
                GpuErrorKind::Allocation,
                "injected reservation refusal",
            ))
        })
        .unwrap_err();
    assert_eq!(error.kind(), GpuErrorKind::Allocation);
    assert_eq!(staging.words.capacity(), 0);
    assert!(staging.words().is_empty());
    staging.reserve_with(4, 4, 4, reserve).unwrap();
    assert_eq!(staging.words(), [0]);
}

#[test]
fn failed_export_exposes_no_prefix_and_allows_complete_retry() {
    let owner = theory(65);
    let foreign = theory(65);
    let candidate = Interpretation::new(&owner, [0, 32, 64]).unwrap();
    let other = Interpretation::new(&foreign, []).unwrap();
    let mut staging = Staging::default();
    let error = staging
        .pack(
            &owner,
            &[candidate.clone(), other],
            24,
            24,
            u64::MAX,
            &Cancellation::default(),
        )
        .unwrap_err();
    assert_eq!(error.kind(), GpuErrorKind::Seed);
    assert!(staging.words().is_empty());
    assert!(staging.words.capacity() >= 6);
    for completed in 0..3 {
        let mut polls = 0;
        let error = staging
            .pack_with(
                &owner,
                std::slice::from_ref(&candidate),
                12,
                12,
                u64::MAX,
                || {
                    polls += 1;
                    // Staging admission, exporter admission and candidate admission.
                    if polls == completed + 4 {
                        Err(GpuError::interrupted(Stop::Cancelled))
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
        assert_eq!(error.interruption(), Some(Stop::Cancelled));
        assert!(staging.words().is_empty());
        staging
            .pack(
                &owner,
                std::slice::from_ref(&candidate),
                12,
                12,
                u64::MAX,
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(staging.words(), [1, 1, 1]);
    }
}

#[test]
fn reused_empty_carrier_staging_still_checks_every_owner() {
    let owner = theory(0);
    let foreign = theory(0);
    let candidate = Interpretation::new(&owner, []).unwrap();
    let other = Interpretation::new(&foreign, []).unwrap();
    let mut staging = Staging::default();
    staging
        .pack(
            &owner,
            &[candidate.clone(), candidate.clone()],
            4,
            4,
            u64::MAX,
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(staging.words(), [0]);
    let error = staging
        .pack(
            &owner,
            &[candidate.clone(), other],
            4,
            4,
            u64::MAX,
            &Cancellation::default(),
        )
        .unwrap_err();
    assert_eq!(error.kind(), GpuErrorKind::Seed);
    assert!(staging.words().is_empty());
    staging
        .pack(
            &owner,
            &[candidate.clone(), candidate],
            4,
            4,
            u64::MAX,
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(staging.words(), [0]);
}

#[test]
fn prefix_selection_preserves_noncapacity_failures() {
    for kind in [
        GpuErrorKind::Allocation,
        GpuErrorKind::Device,
        GpuErrorKind::Seed,
    ] {
        let mut calls = 0;
        let error = BatchSize::Prefix(16)
            .select(16, &Cancellation::default(), |count| {
                calls += 1;
                if count == 1 {
                    Ok(count)
                } else {
                    Err(GpuError::new(kind, "sentinel failure"))
                }
            })
            .unwrap_err();
        assert_eq!(error.kind(), kind);
        assert_eq!(calls, 2);
    }
}

#[test]
fn stopped_prefix_selection_never_evaluates_layout() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let error = BatchSize::Prefix(16)
        .select(16, &cancellation, |_| -> Result<(), GpuError> {
            panic!("stopped layout")
        })
        .unwrap_err();
    assert_eq!(error.interruption(), Some(Stop::Cancelled));
}

#[test]
fn prefix_selection_is_bounded_at_host_count_maximum() {
    let mut calls = 0_u32;
    let count = BatchSize::Prefix(usize::MAX)
        .select(usize::MAX, &Cancellation::default(), |count| {
            calls += 1;
            if count <= 7 {
                Ok(count)
            } else {
                Err(capacity("fixture ceiling"))
            }
        })
        .unwrap();
    assert_eq!(count, 7);
    assert!(calls <= usize::BITS + 2);
}
