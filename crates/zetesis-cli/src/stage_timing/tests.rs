//! Every summary output prefix propagates failure; modes retain typed meaning.
use std::io;
use zetesis_telemetry::{SolveStage, StageRecorder};
use zetesis_test_support::io::BoundedWriter;

#[test]
fn every_summary_prefix_preserves_writer_failure() {
    let recorder = StageRecorder::new(true);
    drop(recorder.enter(SolveStage::SourcePreparation));
    drop(recorder.enter(SolveStage::Grounding));
    drop(recorder.enter(SolveStage::Solving));
    drop(recorder.enter(SolveStage::ObservationOutput));
    let timings = recorder.snapshot().unwrap();
    let mut reference = Vec::new();
    super::write(&mut reference, &timings).unwrap();
    let text = std::str::from_utf8(&reference).unwrap();
    assert!(text.contains("Timing summary (host ms): source preparation="));
    assert!(text.contains("  stage grounding_mode: eager\n"));
    assert!(text.contains("  stage grounding: calls=1; elapsed_ns="));
    for capacity in 0..reference.len() {
        let mut output = BoundedWriter::new(capacity);
        assert_eq!(
            super::write(&mut output, &timings).unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
        assert_eq!(output.bytes(), &reference[..capacity]);
    }
    let mut complete = BoundedWriter::new(reference.len());
    super::write(&mut complete, &timings).unwrap();
    assert_eq!(complete.bytes(), reference);
}

#[test]
fn lazy_and_mixed_rendering_never_label_missing_lazy_time_as_zero() {
    let recorder = StageRecorder::new(true);
    recorder.mark_lazy_grounding();
    let mut bytes = Vec::new();
    super::write(&mut bytes, &recorder.snapshot().unwrap()).unwrap();
    let lazy = String::from_utf8(bytes).unwrap();
    assert!(lazy.contains("grounding=unavailable (lazy work interleaved with solving)"));
    assert!(lazy.contains("  stage grounding: unavailable=interleaved\n"));
    drop(recorder.enter(SolveStage::Grounding));
    let mut bytes = Vec::new();
    super::write(&mut bytes, &recorder.snapshot().unwrap()).unwrap();
    let mixed = String::from_utf8(bytes).unwrap();
    assert!(mixed.contains("  stage grounding_mode: mixed\n"));
    assert!(mixed.contains("eager only; lazy work interleaved with solving"));
    assert!(mixed.contains("  stage grounding: calls=1;"));
}

#[test]
fn terminal_rendering_names_only_base_grounding_as_eager() {
    let recorder = StageRecorder::new(true);
    drop(recorder.enter(SolveStage::Grounding));
    recorder.mark_terminal_definitions(zetesis_telemetry::TerminalBaseMark::Eager);
    let mut bytes = Vec::new();
    super::write(&mut bytes, &recorder.snapshot().unwrap()).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("stage grounding_mode: eager_base_terminal_definitions"));
    assert!(text.contains("base only; terminal definitions reconstructed during solving"));
}

#[test]
fn stage_rendering_matches_the_shared_protocol_fixture() {
    let recorder = StageRecorder::new(true);
    let stages = [
        (SolveStage::SourcePreparation, 1, 100),
        (SolveStage::Grounding, 1, 200),
        (SolveStage::Solving, 3, 500),
        (SolveStage::ObservationOutput, 2, 40),
    ];
    for (stage, calls, _) in stages {
        for _ in 0..calls {
            drop(recorder.enter(stage));
        }
    }
    let timings = recorder.snapshot().unwrap();
    assert!(timings.is_complete());
    // Substitute only clock durations; every other protocol field stays fixed.
    let mut expected = zetesis_test_support::fixtures::STAGE_STATISTICS.replace(
        "  stage driver: elapsed_ns=1000\n",
        &format!(
            "  stage driver: elapsed_ns={}\n",
            timings.driver_elapsed.as_nanos()
        ),
    );
    for (stage, calls, elapsed) in stages {
        let label = stage.label();
        let measured = timings.get(stage).unwrap().elapsed.as_nanos();
        expected = expected.replace(
            &format!("  stage {label}: calls={calls}; elapsed_ns={elapsed};"),
            &format!("  stage {label}: calls={calls}; elapsed_ns={measured};"),
        );
    }
    expected = expected.replace(
        "  stage unattributed: elapsed_ns=160\n",
        &format!(
            "  stage unattributed: elapsed_ns={}\n",
            timings.unattributed.unwrap().as_nanos()
        ),
    );
    let mut rendered = Vec::new();
    super::write(&mut rendered, &timings).unwrap();
    let text = std::str::from_utf8(&rendered).unwrap();
    // The human summary is independent of the versioned protocol below it.
    let (_, section) = text.split_once("\nStage timings:").unwrap();
    assert_eq!(format!("Stage timings:{section}"), expected);
}
