use super::*;
use zetesis_ferraris::{AdmissionLimits, TightPlanLimits};

fn certificate(atoms: usize, heads: &[usize]) -> TightPlan {
    let mut nodes = vec![Node::False];
    nodes.extend((0..atoms).map(Node::Atom));
    let roots = heads
        .iter()
        .enumerate()
        .map(|(ordinal, &head)| {
            if ordinal % 3 == 0 {
                head + 1
            } else {
                let body = usize::from(ordinal % 3 == 2 && head != 0);
                let root = nodes.len();
                nodes.push(Node::Implies(body, head + 1));
                root
            }
        })
        .collect();
    let theory = Theory::new(atoms, nodes, roots, AdmissionLimits::default()).unwrap();
    TightPlan::compile(&theory, TightPlanLimits::default(), &Control::default()).unwrap()
}

fn graph(certificate: &TightPlan, support: TightSupport) -> Graph {
    Graph::new(
        certificate,
        Packing {
            device: &wgpu::Limits::default(),
            support,
        },
    )
    .unwrap()
}

#[test]
fn groups_preserve_every_original_producer_occurrence() {
    for atoms in [0, 1, 31, 32, 33, 63, 64, 65] {
        let heads: Vec<_> = (0..atoms * 3)
            .map(|ordinal| (ordinal * 7) % atoms)
            .collect();
        let certificate = certificate(atoms, &heads);
        let grouped = graph(&certificate, TightSupport::Grouped);
        let packed = grouped.pack(&certificate, &Control::default()).unwrap();
        let offset = certificate.producers().len() * 4;
        let bounds = &packed.producers[offset..=offset + atoms.div_ceil(32)];
        assert_eq!(bounds[0], 0);
        assert_eq!(
            *bounds.last().unwrap() as usize,
            certificate.producers().len()
        );
        for (word, range) in bounds.windows(2).enumerate() {
            let actual: Vec<_> = packed.producers[range[0] as usize * 4..range[1] as usize * 4]
                .chunks_exact(4)
                .map(|row| (row[0] as usize, (row[2] != 0).then_some(row[1] as usize)))
                .collect();
            let expected: Vec<_> = certificate
                .producers()
                .iter()
                .filter(|producer| producer.head() / 32 == word)
                .map(|producer| (producer.head(), producer.body()))
                .collect();
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn grouped_masks_equal_original_enabled_support() {
    let heads: Vec<_> = std::iter::repeat_n(64, 129)
        .chain([0, 31, 32, 63, 1])
        .collect();
    let certificate = certificate(65, &heads);
    let grouped = graph(&certificate, TightSupport::Grouped);
    let packed = grouped.pack(&certificate, &Control::default()).unwrap();
    let offset = certificate.producers().len() * 4;
    for gate in [false, true] {
        let truth = |node| node == 1 && gate;
        for word in 0..3 {
            let start = packed.producers[offset + word] as usize;
            let end = packed.producers[offset + word + 1] as usize;
            let mut actual = 0u32;
            for producer in packed.producers[start * 4..end * 4].chunks_exact(4) {
                if producer[2] == 0 || truth(producer[1] as usize) {
                    actual |= 1 << (producer[0] % 32);
                }
            }
            // Independently ask for support of each semantic atom against all
            // original producers; do not reuse the CSR's offsets or grouping.
            let expected =
                (word * 32..((word + 1) * 32).min(65)).fold(0u32, |mask, atom| {
                    if certificate.producers().iter().any(|producer| {
                        producer.head() == atom && producer.body().is_none_or(truth)
                    }) {
                        mask | (1 << (atom % 32))
                    } else {
                        mask
                    }
                });
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn empty_groups_retain_repeated_offsets() {
    let certificate = certificate(65, &[64, 64, 0]);
    let grouped = graph(&certificate, TightSupport::Grouped);
    let packed = grouped.pack(&certificate, &Control::default()).unwrap();
    assert_eq!(&packed.producers[12..], &[0, 1, 1, 3]);
}

#[test]
fn grouping_storage_includes_temporary_write_cursors() {
    let certificate = certificate(65, &[64, 64, 0]);
    let atomic = graph(&certificate, TightSupport::Atomic);
    let grouped = graph(&certificate, TightSupport::Grouped);
    assert_eq!(grouped.bytes - atomic.bytes, 4 * (3 + 1));
    assert_eq!(grouped.packing_bytes - grouped.bytes, 4 * 3);
    assert_eq!(grouped.work, atomic.work);
    for fresh in [false, true] {
        let plan = |graph: &Graph, limit| {
            Plan::new(
                graph,
                3,
                TightGpuLimits {
                    max_batch_bytes: limit,
                    ..Default::default()
                },
                &wgpu::Limits::default(),
                fresh,
                1,
            )
        };
        let grouped_plan = plan(&grouped, u64::MAX).unwrap();
        let atomic_plan = plan(&atomic, u64::MAX).unwrap();
        assert_eq!(
            grouped_plan.accounted - atomic_plan.accounted,
            if fresh { 16 + 16 + 12 } else { 16 }
        );
        assert!(plan(&grouped, grouped_plan.accounted).is_ok());
        assert_eq!(
            plan(&grouped, grouped_plan.accounted - 1)
                .err()
                .unwrap()
                .kind(),
            GpuErrorKind::Capacity
        );
    }
}

#[test]
fn grouped_indices_obey_device_storage_limits() {
    let certificate = certificate(65, &vec![64; 513]);
    let grouped = graph(&certificate, TightSupport::Grouped);
    let device = wgpu::Limits {
        max_storage_buffer_binding_size: grouped.producer_bytes - 1,
        ..Default::default()
    };
    assert!(
        Graph::new(
            &certificate,
            Packing {
                device: &device,
                support: TightSupport::Atomic
            }
        )
        .is_ok()
    );
    assert_eq!(
        Graph::new(
            &certificate,
            Packing {
                device: &device,
                support: TightSupport::Grouped
            }
        )
        .err()
        .unwrap()
        .kind(),
        GpuErrorKind::Capacity
    );
}

#[test]
fn cancelled_group_packing_has_no_partial_output() {
    let certificate = certificate(65, &[64, 0]);
    let grouped = graph(&certificate, TightSupport::Grouped);
    let control = Control::default();
    control.cancel();
    assert_eq!(
        grouped
            .pack_producers(&certificate, &control)
            .err()
            .unwrap()
            .interruption,
        Some(zetesis_cpu::Stop::Cancelled)
    );
}

#[test]
fn producer_offsets_respect_the_u32_address_boundary() {
    let producers = u32::MAX / 4;
    assert_eq!(
        producer_storage(producers, 2, TightSupport::Grouped).unwrap(),
        u64::from(u32::MAX) * 4
    );
    assert_eq!(
        producer_storage(producers, 3, TightSupport::Grouped)
            .unwrap_err()
            .kind(),
        GpuErrorKind::Capacity
    );
    assert_eq!(
        producer_storage(producers + 1, 0, TightSupport::Atomic)
            .unwrap_err()
            .kind(),
        GpuErrorKind::Capacity
    );
}

#[test]
fn readback_requires_the_selected_support_policy() {
    let certificate = certificate(1, &[0]);
    for (policy, marker, other_marker) in [
        (TightSupport::Atomic, RESULT_MAGIC, RESULT_GROUPED_MAGIC),
        (TightSupport::Grouped, RESULT_GROUPED_MAGIC, RESULT_MAGIC),
    ] {
        let graph = graph(&certificate, policy);
        let plan = Plan::new(
            &graph,
            1,
            TightGpuLimits::default(),
            &wgpu::Limits::default(),
            true,
            1,
        )
        .unwrap();
        let record = |marker| [1, 0, STATUS_STABLE, 0, plan.work, marker];
        assert_eq!(
            decode(&record(marker), &graph, &plan, &[1], &Control::default()).unwrap()[0].verdict(),
            TightVerdict::Stable
        );
        assert_eq!(
            decode(
                &record(other_marker),
                &graph,
                &plan,
                &[1],
                &Control::default()
            )
            .unwrap_err()
            .kind(),
            GpuErrorKind::Readback
        );
    }
}
