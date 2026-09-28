//! Every summary output prefix propagates failure; modes retain typed meaning.
use crate::test_writer::BoundedWriter;
use std::io;
use zetesis_telemetry::{SolveStage, StageRecorder};

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
    recorder.mark_terminal_definitions();
    let mut bytes = Vec::new();
    super::write(&mut bytes, &recorder.snapshot().unwrap()).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("stage grounding_mode: eager_base_terminal_definitions"));
    assert!(text.contains("base only; terminal definitions reconstructed during solving"));
}
