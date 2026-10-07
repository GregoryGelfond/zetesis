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
        &Cancellation::default(),
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
            &Cancellation::default(),
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
    let error = pack(&owner, &[foreign], &mut output, &Cancellation::default()).unwrap_err();
    assert_eq!(error.kind(), GpuErrorKind::Seed);
    assert_eq!(output, [7]);
    pack(
        &owner,
        &[candidate.clone(), candidate],
        &mut output,
        &Cancellation::default(),
    )
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
        let error = pack_with(
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
    let error = pack(&theory, &[], &mut [], &cancellation).unwrap_err();
    assert_eq!(error.interruption(), Some(Stop::Cancelled));
}
