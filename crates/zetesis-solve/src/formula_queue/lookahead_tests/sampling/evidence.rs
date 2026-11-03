//! Stream the actual two populations of the general GPU coordinator using a
//! device double; candidate search, host completion and ticketing are real.
use super::*;
mod observations;
use std::fs::{File, OpenOptions};
use std::hash::BuildHasher;
use std::io::{Read, Seek};
use std::path::PathBuf;
use zetesis_backend::residual_evidence::{self as wire, Header};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> (Self, File) {
        let nonce = std::collections::hash_map::RandomState::new().hash_one(std::process::id());
        let path =
            std::env::temp_dir().join(format!("zetesis-residual-{}-{nonce}", std::process::id()));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        (Self(path), file)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn exported_records_use_actual_committed_world_and_batch_populations() {
    let control = Cancellation::default();
    let options = fixed(4);
    let mut queue = BatchQueue::for_joint_oracle(&options).unwrap();
    let mut gpu = RegionalDevice::new(&control);
    let (theory, mut stream0) = stream("{a;b}.", &control);
    let (scratch, file) = Scratch::new();
    let nonce = [9; 16];
    queue
        .run_samples_mut()
        .unwrap()
        .offer(file, nonce, wire::MAX_BYTES)
        .unwrap();
    let first = exhaust(&mut queue, &mut gpu, &mut stream0, &options, &control);
    assert_eq!(first.len(), 4);
    gpu.transport.decides = Decides::Residuals;
    let mut second =
        StableModels::new(theory.theory(), Limits::default(), control.clone()).unwrap();
    assert_eq!(
        exhaust(&mut queue, &mut gpu, &mut second, &options, &control),
        first
    );
    queue.settle_samples();
    let samples = queue.run_samples_mut().unwrap();
    let staged = samples.take_export().unwrap().unwrap();
    assert!(samples.take_export().is_none());
    let mut file = File::open(&scratch.0).unwrap();
    let mut initial_header = [0; 128];
    file.read_exact(&mut initial_header).unwrap();
    assert!(Header::decode(&initial_header).is_err());
    let header = staged.seal(true).unwrap();
    assert_eq!(
        (header.nonce, header.worlds, header.batches),
        (nonce, Some(4), Some(2))
    );
    assert_eq!(header.seed, samples.snapshot().seed);
    assert_eq!(header.byte_len(), Some(file.metadata().unwrap().len()));
    for _ in 0..header.retained_worlds {
        let mut raw = [0; 72];
        file.read_exact(&mut raw).unwrap();
        let world = wire::World::decode(&raw).unwrap();
        assert_eq!(world.reason, Some(wire::Reason::RoundLimit));
        assert!(world.open.is_some());
    }
    for counts in [[0, 0, 0, 0], [4, 0, 4, 0]] {
        let mut raw = [0; 112];
        file.read_exact(&mut raw).unwrap();
        assert_eq!(
            wire::Batch::decode(&raw).unwrap().residuals,
            counts.map(Some)
        );
    }
    assert_eq!(file.stream_position().unwrap(), header.byte_len().unwrap());
    let summary = observations::observed(&samples.snapshot());
    let request = zetesis_validation::residual_evidence::Request {
        version: 1,
        nonce,
        max_bytes: wire::MAX_BYTES,
    };
    let checked =
        zetesis_validation::residual_evidence::read(&scratch.0, &request, &summary, true).unwrap();
    assert_eq!(Some(checked.byte_len()), header.byte_len());
    assert!(
        zetesis_validation::residual_evidence::read(&scratch.0, &request, &summary, false).is_err()
    );
}

#[test]
fn refusing_the_artifact_leaves_search_and_the_destination_intact() {
    let control = Cancellation::default();
    let options = fixed(4);
    let mut queue = BatchQueue::for_joint_oracle(&options).unwrap();
    let mut gpu = RegionalDevice::new(&control);
    let (_theory, mut models) = stream("{a;b}.", &control);
    let (scratch, file) = Scratch::new();
    queue
        .run_samples_mut()
        .unwrap()
        .offer(file, [3; 16], 127)
        .unwrap();
    assert_eq!(
        exhaust(&mut queue, &mut gpu, &mut models, &options, &control).len(),
        4
    );
    queue.settle_samples();
    let error = queue
        .run_samples_mut()
        .unwrap()
        .take_export()
        .unwrap()
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(scratch.0.metadata().unwrap().len(), 0);
}

#[test]
fn a_failed_file_write_preserves_the_complete_answer_family() {
    let control = Cancellation::default();
    let options = fixed(4);
    let mut queue = BatchQueue::for_joint_oracle(&options).unwrap();
    let mut gpu = RegionalDevice::new(&control);
    let (_theory, mut models) = stream("{a;b}.", &control);
    let (scratch, file) = Scratch::new();
    drop(file);
    let read_only = File::open(&scratch.0).unwrap();
    queue
        .run_samples_mut()
        .unwrap()
        .offer(read_only, [1; 16], wire::MAX_BYTES)
        .unwrap();
    assert_eq!(
        exhaust(&mut queue, &mut gpu, &mut models, &options, &control).len(),
        4
    );
    queue.settle_samples();
    assert!(
        queue
            .run_samples_mut()
            .unwrap()
            .take_export()
            .unwrap()
            .is_err()
    );
    assert_eq!(scratch.0.metadata().unwrap().len(), 0);
}

#[test]
fn an_append_handle_is_refused_within_the_offer() {
    let options = fixed(4);
    let mut queue = BatchQueue::for_joint_oracle(&options).unwrap();
    let (scratch, file) = Scratch::new();
    drop(file);
    let append = OpenOptions::new().append(true).open(&scratch.0).unwrap();
    let samples = queue.run_samples_mut().unwrap();
    samples.offer(append, [2; 16], 128).unwrap();
    // A terminal prefix with no committed batches requires exactly 128 bytes.
    samples.settle();
    let failure = samples.take_export().unwrap().unwrap_err();
    assert_eq!(failure.kind(), std::io::ErrorKind::InvalidInput);
    assert!(scratch.0.metadata().unwrap().len() <= 128);
}
