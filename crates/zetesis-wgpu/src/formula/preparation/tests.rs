use super::*;
use zetesis_ferraris::AdmissionLimits;

fn fixture() -> Theory {
    Theory::new(
        2,
        vec![
            Node::Atom(0),
            Node::And(0, 0),
            Node::Atom(1),
            Node::Or(1, 2),
            Node::False,
            Node::Implies(3, 4),
            Node::Atom(0),
            Node::And(2, 6),
        ],
        vec![5, 7],
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn preparation(theory: &Theory, limits: FormulaLimits) -> Result<Preparation, GpuError> {
    Preparation::new(theory, 2, limits, &wgpu::Limits::default(), 1)
}

#[test]
fn shared_interleaved_nodes_have_literal_stable_levels() {
    let (prepared, plan) = preparation(&fixture(), FormulaLimits::default())
        .unwrap()
        .finish(&wgpu::Limits::default(), &Cancellation::default())
        .unwrap();
    assert_eq!(prepared.graph.schedule.levels, 4);
    assert_eq!(
        prepared.roots,
        [5, 7, 0, 4, 6, 7, 8, 0, 2, 4, 6, 1, 7, 3, 5]
    );
    let outputs: Vec<_> = prepared.nodes.chunks_exact(4).map(|node| node[3]).collect();
    assert_eq!(outputs, [0, 2, 1, 3, 4, 5, 0, 6]);
    assert_eq!(plan.setup, 24); // 2*8 nodes + 2 atoms + 2 roots + 4 levels.
    assert_eq!(plan.sweep, 139); // 9*8 + 2 + 64 + 1.
}

#[test]
fn pure_chains_keep_serial_truth() {
    let mut nodes = vec![Node::Atom(0)];
    nodes.extend((1..128).map(|index| Node::And(index - 1, index - 1)));
    let chain = Theory::new(1, nodes, vec![127], AdmissionLimits::default()).unwrap();
    let (prepared, plan) = preparation(&chain, FormulaLimits::default())
        .unwrap()
        .finish(&wgpu::Limits::default(), &Cancellation::default())
        .unwrap();
    assert_eq!(prepared.graph.schedule.levels, 0);
    assert_eq!(prepared.roots, [127]);
    assert_eq!(plan.setup, 258);
}

#[test]
fn empty_graphs_keep_only_required_padding() {
    let empty = Theory::new(0, vec![], vec![], AdmissionLimits::default()).unwrap();
    let (prepared, plan) = preparation(&empty, FormulaLimits::default())
        .unwrap()
        .finish(&wgpu::Limits::default(), &Cancellation::default())
        .unwrap();
    assert_eq!(prepared.graph.schedule.levels, 0);
    assert_eq!((prepared.nodes, prepared.roots), (vec![0; 4], vec![0]));
    assert_eq!(plan.setup, 0);
}

#[test]
fn zero_roots_still_cover_every_duplicate_leaf() {
    let theory = Theory::new(
        1,
        vec![Node::Atom(0), Node::Atom(0)],
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap();
    let (prepared, _) = preparation(&theory, FormulaLimits::default())
        .unwrap()
        .finish(&wgpu::Limits::default(), &Cancellation::default())
        .unwrap();
    assert_eq!(prepared.graph.schedule.levels, 1);
    assert_eq!(prepared.roots, [0, 2, 0, 1]);
    assert_eq!(prepared.nodes, [1, 0, 0, 0, 1, 0, 0, 0]);
}

#[test]
fn retained_upload_capacities_are_admitted_in_the_cold_peak() {
    for below in [0, 1] {
        // Fixed graph: node128 + roots60; two-world transport272. Staging uses
        // real u32 vectors with deliberate slack, observed before admission.
        let nodes = vector::<u32>(32 + 17).unwrap();
        let roots = vector::<u32>(15 + 19).unwrap();
        let pointers = (nodes.as_ptr(), roots.as_ptr());
        let staging = retained_bytes(&nodes).unwrap() + retained_bytes(&roots).unwrap();
        let exact =
            188 + 272 + 48 + 8 + 2 * size_of::<super::super::FormulaCheck>() as u64 + staging;
        let mut allocations = [nodes, roots].into_iter();
        let result = preparation(
            &fixture(),
            FormulaLimits {
                max_batch_bytes: exact - below,
                ..Default::default()
            },
        )
        .unwrap()
        .finish_with(&wgpu::Limits::default(), &Cancellation::default(), |_| {
            Ok(allocations.next().unwrap())
        });
        if below == 0 {
            let (prepared, plan) = result.unwrap();
            assert_eq!(plan.accounted, exact);
            assert_eq!((prepared.nodes.as_ptr(), prepared.roots.as_ptr()), pointers);
        } else {
            assert_eq!(result.err().unwrap().kind(), GpuErrorKind::Capacity);
        }
    }
}

#[test]
fn either_upload_reservation_preserves_allocation_failure() {
    for fail_at in [0, 1] {
        let mut calls = 0;
        let result = preparation(&fixture(), FormulaLimits::default())
            .unwrap()
            .finish_with(
                &wgpu::Limits::default(),
                &Cancellation::default(),
                |length| {
                    let current = calls;
                    calls += 1;
                    if current == fail_at {
                        vector::<u32>(usize::MAX)
                    } else {
                        vector(length)
                    }
                },
            );
        assert_eq!(result.err().unwrap().kind(), GpuErrorKind::Allocation);
        assert_eq!(calls, fail_at + 1);
    }
}

#[test]
fn interruption_after_either_reservation_publishes_no_graph() {
    for stop_at in [0, 1] {
        let cancellation = Cancellation::default();
        let mut calls = 0;
        let result = preparation(&fixture(), FormulaLimits::default())
            .unwrap()
            .finish_with(&wgpu::Limits::default(), &cancellation, |length| {
                let words = vector(length)?;
                if calls == stop_at {
                    cancellation.cancel();
                }
                calls += 1;
                Ok(words)
            });
        assert_eq!(
            result.err().unwrap().interruption(),
            Some(zetesis_cpu::Stop::Cancelled)
        );
        assert_eq!(calls, stop_at + 1);
    }
}

#[test]
fn checked_schedule_offsets_refuse_overflow_and_underflow() {
    let mut shape = Shape::new(&fixture(), &wgpu::Limits::default()).unwrap();
    shape.roots = u32::MAX - 64;
    assert_eq!(
        Schedule::new(&shape, 4, &wgpu::Limits::default())
            .err()
            .unwrap()
            .kind(),
        GpuErrorKind::Capacity
    );
    for (end, nodes) in [(0, 8), (9, 8), (1, 0)] {
        let mut cursor = end;
        assert_eq!(
            insert_position(&mut cursor, nodes).unwrap_err().kind(),
            GpuErrorKind::Capacity
        );
        assert_eq!(cursor, end);
    }
    let mut cursor = 8;
    assert_eq!(insert_position(&mut cursor, 8).unwrap(), 7);
    assert_eq!(cursor, 7);
}

#[test]
fn level_setup_is_admitted_after_the_minimum_serial_envelope() {
    // Minimum setup20 succeeds; the actual four-level setup requires24.
    for (work, accepted) in [(23, false), (24, true)] {
        let result = preparation(
            &fixture(),
            FormulaLimits {
                max_work_per_candidate: work,
                ..Default::default()
            },
        )
        .unwrap()
        .finish(&wgpu::Limits::default(), &Cancellation::default());
        assert_eq!(result.is_ok(), accepted);
        if let Err(error) = result {
            assert_eq!(error.kind(), GpuErrorKind::Capacity);
        }
    }
    assert_eq!(
        preparation(
            &fixture(),
            FormulaLimits {
                max_batch_bytes: 0,
                ..Default::default()
            }
        )
        .err()
        .unwrap()
        .kind(),
        GpuErrorKind::Capacity
    );
}
