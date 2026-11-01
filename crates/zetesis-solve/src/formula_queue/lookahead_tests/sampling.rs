//! Exercise sampling at the real queue's commit boundary. The device double
//! supplies either terminal verdicts or the full proper-subset query region.
//! Stable search, ticket handling and accounting run on the host.

use super::*;
mod evidence;
use crate::residual_handovers::decoded::DecodeJournal;
use zetesis_algebra::region::Region;
use zetesis_wgpu::ResidualReason;

struct RegionalDevice {
    transport: Double,
    journal: DecodeJournal,
    roots: Vec<Region>,
    count: usize,
    journal_bytes: u64,
    refuse_once: bool,
}

impl RegionalDevice {
    fn new(control: &Cancellation) -> Self {
        let mut transport = Double::new(usize::MAX, control);
        transport.handovers = Some(crate::ResidualHandoverStatistics::default());
        Self {
            transport,
            journal: DecodeJournal::default(),
            roots: Vec::new(),
            count: 0,
            journal_bytes: u64::MAX,
            refuse_once: false,
        }
    }
}

impl SlateDevice for RegionalDevice {
    const REGIONAL: bool = true;
    type Prepared<'d> = Prepared<'d>;
    type Checked = Result<Receipt, Failure>;

    fn handovers(&mut self) -> Option<&mut crate::ResidualHandoverStatistics> {
        self.transport.handovers.as_mut()
    }

    fn take_decode_journal(&mut self) -> DecodeJournal {
        std::mem::take(&mut self.journal)
    }

    fn prepare<'d>(
        &'d mut self,
        slate: SlateRef<'_>,
        offset: usize,
        max_bytes: u64,
        control: &Cancellation,
    ) -> Result<Self::Prepared<'d>, Failure> {
        self.count = slate.candidates().len();
        let atoms = slate.theory().atom_count();
        self.roots = slate.candidates()[offset..]
            .iter()
            .map(|candidate| {
                let mut root = Region::all_open(atoms);
                for a in 0..atoms {
                    if !candidate.contains(a) {
                        assert!(root.cut(a));
                    }
                }
                root
            })
            .collect();
        let mut prepared = self.transport.prepare(slate, offset, max_bytes, control)?;
        if std::mem::take(&mut self.refuse_once) {
            prepared.receipt.offset = usize::MAX;
        }
        Ok(prepared)
    }

    fn decode(
        &mut self,
        receipt: Self::Checked,
        _theory: &Theory,
        slate: SlateId,
        offset: usize,
        offered: usize,
        rows: &mut super::super::regional::CheckedRows,
    ) -> Result<Decoded, Failure> {
        let (verdicts, timing) = self.transport.unpack(receipt?, slate, offset, offered)?;
        for (i, (verdict, root)) in verdicts.into_iter().zip(self.roots.drain(..)).enumerate() {
            if verdict == BatchVerdict::Residual {
                let open = crate::residual_handovers::open_atoms(&root);
                let reason = ResidualReason::RoundLimit;
                self.transport
                    .handovers
                    .as_mut()
                    .unwrap()
                    .decoded(reason, open);
                self.journal
                    .record(offset + i, self.count, reason, open, self.journal_bytes);
                rows.projected(root)?;
            } else {
                rows.classified(verdict)?;
            }
        }
        Ok(timing)
    }
}

fn exhaust(
    queue: &mut BatchQueue,
    gpu: &mut RegionalDevice,
    models: &mut StableModels,
    options: &SolveConfig,
    control: &Cancellation,
) -> BTreeSet<Vec<usize>> {
    let mut answers = BTreeSet::new();
    while let Some(row) = queue.next_device(models, options, control, &Recorder::new(false), gpu) {
        let answer = row.unwrap_or_else(|_| panic!("valid slate must commit"));
        assert!(answers.insert(answer.atoms().collect()));
    }
    assert!(models.exhausted());
    answers
}

#[test]
fn a_new_candidate_stream_keeps_the_run_population() {
    let control = Cancellation::default();
    let options = fixed(4);
    let mut queue = BatchQueue::for_joint_oracle(&options).unwrap();
    let mut gpu = RegionalDevice::new(&control);
    let (theory, mut first) = stream("{a;b}.", &control);
    let expected = exhaust(&mut queue, &mut gpu, &mut first, &options, &control);
    assert_eq!(expected.len(), 4);
    let before = gpu.transport.handovers.as_ref().unwrap().committed_slates;
    assert_eq!(before, 1);

    // A new tie stream can issue the same local slate sequence. The run
    // owner survives as it does when FormulaSession replaces its stream.
    let mut second =
        StableModels::new(theory.theory(), Limits::default(), control.clone()).unwrap();
    gpu.transport.decides = Decides::Residuals;
    assert_eq!(
        exhaust(&mut queue, &mut gpu, &mut second, &options, &control),
        expected
    );
    let mut stats = gpu.transport.handovers.unwrap();
    assert_eq!((stats.committed_slates, stats.zero_residual_slates), (2, 1));
    queue.observe_samples(&mut stats);
    let live = stats.samples.take().unwrap();
    assert_eq!((live.batches.seen, live.worlds.seen), (Some(2), Some(4)));
    assert!(live.groups.is_none());

    queue.settle_samples();
    queue.observe_samples(&mut stats);
    let settled = stats.samples.take().unwrap();
    let all = &settled.groups.as_ref().unwrap()[0];
    assert_eq!(all.residuals.median, Some(2));
    assert_eq!(all.open_atoms.known, Some(4));
    // Polling or stopping a terminal stream again does not add samples.
    queue.settle_samples();
    queue.observe_samples(&mut stats);
    assert_eq!(stats.samples.as_ref(), Some(&settled));
}

#[test]
fn a_failed_decode_and_retry_publish_one_batch() {
    let control = Cancellation::default();
    let options = fixed(4);
    let mut queue = BatchQueue::for_joint_oracle(&options).unwrap();
    let (_, mut models) = stream("{a;b}.", &control);
    let mut gpu = RegionalDevice::new(&control);
    gpu.transport.decides = Decides::Residuals;
    gpu.refuse_once = true;
    gpu.journal_bytes = 0;
    let first = queue.next_device(
        &mut models,
        &options,
        &control,
        &Recorder::new(false),
        &mut gpu,
    );
    assert!(matches!(
        first,
        Some(Err(Failure::Run(SolveError::FormulaBatchShape { .. })))
    ));
    let mut prefix = gpu.transport.handovers.as_ref().unwrap().clone();
    queue.observe_samples(&mut prefix);
    assert_eq!(prefix.samples.unwrap().batches.seen, Some(0));
    assert_eq!(prefix.incomplete_attempts, 1);

    assert_eq!(
        exhaust(&mut queue, &mut gpu, &mut models, &options, &control).len(),
        4
    );
    let mut stats = gpu.transport.handovers.unwrap();
    assert!(stats.last_attempt.as_ref().unwrap().retry);
    assert_eq!((stats.attempts, stats.committed_slates), (2, 1));
    queue.settle_samples();
    queue.observe_samples(&mut stats);
    let sample = stats.samples.unwrap();
    assert_eq!(
        (sample.batches.seen, sample.worlds.seen),
        (Some(1), Some(4))
    );
    let all = &sample.groups.unwrap()[0];
    assert_eq!(all.residuals.maximum, Some(4));
    assert_eq!(all.open_atoms.known, Some(0));
    assert_eq!(all.open_atoms.maximum, None);
}

#[test]
fn sampling_preserves_slate_selection_and_search_work() {
    for ceiling in [65_536, 1_048_576] {
        let run = |observe| {
            let control = Cancellation::default();
            let options = SolveConfig {
                max_batch_bytes: ceiling,
                ..fixed(2)
            };
            let mut queue = BatchQueue::for_joint_oracle(&options).unwrap();
            if observe {
                assert!(queue.samples.is_some());
            } else {
                queue.samples = None;
            }
            let mut device = RegionalDevice::new(&control);
            device.transport.decides = Decides::FirstSlateWholly;
            let (_, mut models) = stream("{a;b}.", &control);
            let family = exhaust(&mut queue, &mut device, &mut models, &options, &control);
            assert_eq!(family.len(), 4);
            (family, models.statistics(), device.transport.slates)
        };
        assert_eq!(run(true), run(false));
    }
}
