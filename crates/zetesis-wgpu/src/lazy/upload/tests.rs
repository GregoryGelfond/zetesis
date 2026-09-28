//! Input freshness decisions and their exact contribution to submitted bytes.

use super::{Receipt, Uploads};
use crate::lazy::{Plan, Selection, plan::Retention};
use crate::{GpuLimits, LazyGpuStatistics};
use std::num::NonZeroU32;
use zetesis_core::{AdmissionLimits, AtomPattern, Predicate, Program, Seed, Template};
use zetesis_cpu::{Cancellation, lazy};

fn retained(snapshots: bool, seeds: bool) -> Retention {
    Retention {
        uniform: true,
        inputs: [true, true, snapshots, seeds],
        result: true,
    }
}

#[test]
fn only_matching_retained_input_prefixes_can_skip_writes() {
    let previous = Receipt {
        round: 0,
        words: 2,
        worlds: 3,
    };
    for round in [0, 1] {
        for words in [1, 2, 3] {
            for worlds in [2, 3, 4] {
                for snapshots in [false, true] {
                    for seeds in [false, true] {
                        let current = Receipt {
                            round,
                            words,
                            worlds,
                        };
                        let actual =
                            Uploads::needed(Some(previous), current, retained(snapshots, seeds));
                        assert_eq!(actual.seeds, !(seeds && words == 2 && worlds == 3));
                        assert_eq!(
                            actual.snapshots,
                            !(snapshots && round == 0 && words == 2 && worlds == 3)
                        );
                        assert_eq!(
                            Uploads::needed(None, current, retained(snapshots, seeds)),
                            Uploads::ALL
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn saved_uploads_do_not_weaken_the_retained_memory_ceiling() {
    crate::lazy::tests::inspect(|chunk| {
        let plan = Plan::new(
            NonZeroU32::MIN,
            chunk,
            GpuLimits::default(),
            &wgpu::Limits::default(),
        )
        .unwrap();
        for (snapshots, seeds, bytes) in [
            (true, true, 44),
            (true, false, 40),
            (false, true, 40),
            (false, false, 36),
        ] {
            let mut selected = Selection::new(Some(plan.capacity), &plan, u64::MAX);
            selected.uploads = Uploads { snapshots, seeds };
            assert_eq!(selected.uploads.bytes(&plan).unwrap(), bytes);
            assert_eq!(selected.capacity.accounted(&plan), Some(136));
            let statistics = LazyGpuStatistics::default()
                .submitted(&plan, selected)
                .unwrap();
            assert_eq!(statistics.uploaded_bytes, bytes);
        }
    });
}

fn pattern(name: String) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap()
}

#[test]
fn source_round_receipts_preserve_actual_input_prefixes() {
    let mut rules = Vec::new();
    for index in 0..40 {
        rules.push(Template::new(
            Some(pattern(format!("p{index}"))),
            vec![],
            vec![],
            vec![],
            vec![],
        ));
        rules.push(Template::new(
            Some(pattern(format!("q{index}"))),
            vec![pattern(format!("p{index}"))],
            vec![],
            vec![],
            vec![],
        ));
    }
    let program = Program::new(rules, AdmissionLimits::default()).unwrap();
    let seeds = [Seed::new(&program, []).unwrap()];
    for selection in [lazy::SourceSelection::Union, lazy::SourceSelection::Worlds] {
        let mut receipt = None;
        let mut capacity = None;
        let mut prior_seeds = Vec::new();
        let mut prior_snapshots = Vec::new();
        let mut chunks = [0; 3];
        let mut widths = std::collections::BTreeSet::new();
        let batch = lazy::check_with_source(
            &program,
            &seeds,
            lazy::Limits {
                max_chunk_rules: 1,
                ..Default::default()
            },
            selection,
            &Cancellation::default(),
            |chunk| {
                chunks[usize::try_from(chunk.round_index()).unwrap()] += 1;
                widths.insert(chunk.words());
                let plan = Plan::new(
                    NonZeroU32::MIN,
                    chunk,
                    GpuLimits::default(),
                    &wgpu::Limits::default(),
                )
                .unwrap();
                let selected = Selection::new(capacity, &plan, u64::MAX);
                let current = Receipt::from(chunk);
                let uploads = Uploads::needed(receipt, current, selected.retention);
                if !uploads.seeds {
                    assert_eq!(chunk.seeds(), prior_seeds);
                }
                if !uploads.snapshots {
                    assert_eq!(chunk.snapshots(), prior_snapshots);
                }
                prior_seeds = chunk.seeds().to_vec();
                prior_snapshots = chunk.snapshots().to_vec();
                receipt = Some(current);
                capacity = Some(selected.capacity);
                lazy::evaluate(chunk)
            },
        )
        .unwrap();
        assert_eq!(chunks, [40, 80, 80]);
        // The source coordinator doubles mask width at 33 and 65 atoms; this
        // is admitted logical padding, independent of Vec allocator capacity.
        assert_eq!(widths, [1, 2, 4].into_iter().collect());
        assert_eq!(batch.progress.rounds, 3);
        assert_eq!(batch.checks[0].closure().atoms().len(), 80);
    }
}
