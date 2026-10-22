use super::*;
use crate::tight::packing::{Packing, Plan, RESULT_MAGIC, StateLayout, decode};
use crate::tight::{TightGpuLimits, TightSupport};
use zetesis_ferraris::{
    AdmissionLimits, FormulaNodes, FormulaParts, Node, Theory, TightPlanLimits, TightVerdict,
};

fn packing(device: &wgpu::Limits, support: TightSupport) -> Packing<'_> {
    Packing {
        device,
        support,
        workgroup_sizing: crate::GpuWorkgroupSizing::Fixed(crate::GpuWorkgroupSize::Threads64),
    }
}

// A pair, a wide conjunction and disjunction, and one numeric row whose first
// guard is constant: its owner is the first nonconstant guard. Rules include
// duplicate producers, a fact (a bodiless producer) and a repeated root.
fn certificate() -> TightPlan {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..6).map(Node::atom).collect(), vec![]).unwrap());
    let mut transaction = nodes.transaction();
    // One row with three guards: bound 4 exceeds the total 3, so the first is
    // constant false and the second guard owns the row.
    let family = transaction
        .push_threshold_family(&[0, 1, 2], &[1, 1, 1], &[4, 2, 3], usize::MAX, usize::MAX)
        .unwrap();
    let (constant, owner, follower) = (family[0], family[1], family[2]);
    let mut push = |view| transaction.push(view, usize::MAX, usize::MAX).unwrap();
    let pair = push(NodeView::Implies(0, 1));
    let wide = push(NodeView::And(&[1, 2, 3]));
    let operands = [wide, 4];
    let any = push(NodeView::Or(&operands));
    let first = push(NodeView::Implies(owner, 3));
    let second = push(NodeView::Implies(follower, 4));
    let duplicate = push(NodeView::Implies(follower, 4));
    let disjunction = push(NodeView::Implies(any, 5));
    let impossible = push(NodeView::Implies(constant, 2));
    transaction.commit();
    let theory = Theory::new(
        6,
        nodes.into_parts(),
        vec![
            first,
            second,
            duplicate,
            disjunction,
            impossible,
            0,
            first,
            pair,
        ],
        AdmissionLimits::default(),
    )
    .unwrap();
    TightPlan::compile(
        &theory,
        TightPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap()
}

fn graph(certificate: &TightPlan, support: TightSupport) -> Graph {
    Graph::new(certificate, packing(&wgpu::Limits::default(), support)).unwrap()
}

fn built(certificate: &TightPlan, graph: &Graph) -> Built {
    let cancellation = Cancellation::default();
    let bounds = Bounds::new(graph, &wgpu::Limits::default()).unwrap();
    let owners = graph.rows.owners(&cancellation).unwrap();
    let records = graph.pack_producers(certificate, &cancellation).unwrap();
    build(graph, certificate, &owners, bounds, &records, &cancellation).unwrap()
}

/// Per-node readers entries and per-atom atom-node entries, each sorted.
type Lists = (Vec<Vec<u32>>, Vec<Vec<u32>>);

fn decoded(built: &Built, graph: &Graph) -> Lists {
    let words = &built.words;
    let list = |offsets: usize, count: usize| -> Vec<Vec<u32>> {
        let entries = offsets + count + 1;
        (0..count)
            .map(|index| {
                let mut list = words[entries + words[offsets + index] as usize
                    ..entries + words[offsets + index + 1] as usize]
                    .to_vec();
                list.sort_unstable();
                list
            })
            .collect()
    };
    (
        list(built.index.readers as usize, graph.nodes as usize),
        list(built.index.atom_nodes as usize, graph.atoms as usize),
    )
}

// The reverse of the packed device graph, read from the packed node buffer by
// tag: pair and wide operands, the conditions of each live row under the owner
// its descriptor names, producer bodies under heads and roots under positions.
fn reversed(certificate: &TightPlan, graph: &Graph) -> Lists {
    let packed = graph.pack(certificate, &Cancellation::default()).unwrap();
    let words = &packed.nodes;
    let nodes = graph.nodes as usize;
    let tail = nodes * 4;
    let mut readers = vec![Vec::new(); nodes];
    let mut atoms = vec![Vec::new(); graph.atoms as usize];
    for index in 0..nodes {
        let header = &words[index * 4..index * 4 + 4];
        let reader = u32::try_from(index).unwrap();
        match header[0] {
            1 => atoms[header[1] as usize].push(reader),
            2..=4 => {
                readers[header[1] as usize].push(reader);
                readers[header[2] as usize].push(reader);
            }
            5 | 6 => {
                for operand in 0..header[2] as usize {
                    readers[words[tail + header[1] as usize + operand] as usize].push(reader);
                }
            }
            7..=9 => {
                let row = tail + words[tail + header[1] as usize] as usize + 3;
                if words[row - 3] == reader {
                    for condition in 0..header[2] as usize {
                        readers[words[row + condition] as usize].push(reader);
                    }
                }
            }
            _ => {}
        }
    }
    for producer in certificate.producers() {
        if let Some(body) = producer.body() {
            readers[body].push(READER_PRODUCER | u32::try_from(producer.head()).unwrap());
        }
    }
    for (position, &root) in graph.theory.roots().iter().enumerate() {
        readers[root].push(READER_ROOT | u32::try_from(position).unwrap());
    }
    for list in readers.iter_mut().chain(atoms.iter_mut()) {
        list.sort_unstable();
    }
    (readers, atoms)
}

#[test]
fn readers_index_reverses_the_packed_device_graph() {
    let certificate = certificate();
    for support in [TightSupport::Atomic, TightSupport::Grouped] {
        let graph = graph(&certificate, support);
        assert_eq!(
            decoded(&built(&certificate, &graph), &graph),
            reversed(&certificate, &graph)
        );
    }
}

#[test]
fn live_row_conditions_name_their_first_nonconstant_guard() {
    let certificate = certificate();
    let graph = graph(&certificate, TightSupport::Atomic);
    let (readers, _) = decoded(&built(&certificate, &graph), &graph);
    // Nodes 6, 7 and 8 are the row's guards; 6 is constant.
    for condition in &readers[0..3] {
        assert!(condition.contains(&7));
        assert!(!condition.contains(&6));
        assert!(!condition.contains(&8));
    }
}

#[test]
fn producer_entries_name_heads_under_either_record_order() {
    let certificate = certificate();
    let tables: Vec<_> = [TightSupport::Atomic, TightSupport::Grouped]
        .into_iter()
        .map(|support| {
            let graph = graph(&certificate, support);
            decoded(&built(&certificate, &graph), &graph)
        })
        .collect();
    assert_eq!(tables[0], tables[1]);
    // The follower guard, node 8, is read by two rules, nodes 13 and 14, and is
    // the body of their two producers, both of atom 4.
    assert_eq!(
        tables[0].0[8],
        [13, 14, READER_PRODUCER | 4, READER_PRODUCER | 4]
    );
}

#[test]
fn entry_indices_hold_thirty_bits() {
    assert!(addressable(INDEX_LIMIT));
    assert!(!addressable(INDEX_LIMIT + 1));
    let certificate = certificate();
    let mut graph = graph(&certificate, TightSupport::Atomic);
    assert!(Bounds::new(&graph, &wgpu::Limits::default()).is_some());
    graph.roots = INDEX_LIMIT + 1;
    assert_eq!(Bounds::new(&graph, &wgpu::Limits::default()), None);
}

#[test]
fn count_width_moves_at_packed_boundaries() {
    for (largest, bits) in [(0, 8), (255, 8), (256, 16), (65_535, 16), (65_536, 32)] {
        assert_eq!(count_bits(largest), bits, "{largest}");
    }
}

#[test]
fn an_intrinsic_refusal_is_never_decided_again() {
    let mut stage = Stage::Pending;
    assert_eq!(stage.bounds(|| None), None);
    assert_eq!(stage, Stage::Refused);
    assert_eq!(
        stage.bounds(|| unreachable!("a refused theory decides no bounds")),
        None
    );
}

#[test]
fn slot_state_follows_its_layout() {
    let certificate = certificate();
    let graph = graph(&certificate, TightSupport::Atomic);
    // Summary 3; retained, touched and unsupported one word each; false roots
    // over 8 positions and queued over 17 nodes one word each; counts of 6
    // atoms take 2 words at 8 bits and 6 at 32.
    assert_eq!(graph.nodes, 17);
    assert_eq!(slot_words(&graph, 8), Some(3 + 3 + 1 + 1 + 2));
    assert_eq!(slot_words(&graph, 32), Some(3 + 3 + 1 + 1 + 6));
    let mut counted = graph;
    // Atom 4 has the most producers: two.
    counted.largest_producers = 2;
    let bounds = Bounds::new(&counted, &wgpu::Limits::default()).unwrap();
    assert_eq!(bounds.count_bits, 8);
    assert_eq!(bounds.slot_words, 10);
}

#[test]
fn the_state_is_part_of_each_slots_footprint() {
    let certificate = certificate();
    let graph = graph(&certificate, TightSupport::Atomic);
    let device = wgpu::Limits::default();
    let limits = TightGpuLimits::default();
    let layout = StateLayout {
        slot_words: 11,
        index_bytes: 400,
        build_bytes: 900,
    };
    let stateless = Plan::new(&graph, 3, limits, &device, false, 2).unwrap();
    let stateful = Plan::with_state(&graph, 3, limits, &device, false, 2, Some(layout)).unwrap();
    assert_eq!(stateless.state, 0);
    assert_eq!(stateful.state, 3 * 11 * 4);
    assert_eq!(
        stateful.transport,
        stateless.transport - PLACEHOLDER_BYTES + stateful.state
    );
    assert_eq!(
        stateful.accounted,
        stateless.accounted - PLACEHOLDER_BYTES + stateful.state + 400 + 900
    );
    let ceiling = TightGpuLimits {
        max_batch_bytes: stateful.accounted - 1,
        ..limits
    };
    assert!(Plan::with_state(&graph, 3, ceiling, &device, false, 2, Some(layout)).is_err());
    assert!(Plan::new(&graph, 3, ceiling, &device, false, 2).is_ok());
}

// A plan as a resident schedule of three levels lays it out, optionally
// reusing original truth.
fn scheduled(graph: &Graph, reused: bool) -> Plan {
    let device = wgpu::Limits::default();
    let mut plan = Plan::new(graph, 2, TightGpuLimits::default(), &device, false, 2).unwrap();
    plan.levels = 3;
    plan.work += 3;
    plan.allowance = plan.work;
    if reused {
        plan.reuse_original_truth(graph, u64::MAX);
    }
    plan
}

#[test]
fn allowed_routes_charge_the_states_writes() {
    let certificate = certificate();
    let graph = graph(&certificate, TightSupport::Atomic);
    let index = built(&certificate, &graph).index;
    let words = graph.words;
    // Without authority every slot is complete: the state's writes only.
    let mut complete = scheduled(&graph, false);
    let base = complete.work;
    assert!(complete.bind_state(&graph, &index, u64::MAX));
    assert_eq!(complete.work, base + words + 1);
    assert_eq!(complete.allowance, complete.work);
    // With authority a slot may also count its change, or update: the
    // reservation covers the largest of them.
    let mut reused = scheduled(&graph, true);
    let dense = reused.work;
    assert!(reused.bind_state(&graph, &index, u64::MAX));
    assert_eq!(reused.work, dense + words + 1);
    let worst = worst_case(
        &graph,
        3,
        index.count_bits,
        index.entries,
        index.atom_entries,
    )
    .and_then(|worst| u32::try_from(worst).ok())
    .unwrap();
    assert_eq!(reused.allowance, (reused.work + 2 * words).max(worst));
    assert_eq!(u64::from(reused.route.floor), floor(&graph, 3));
}

#[test]
fn a_route_not_allowed_charges_what_the_stateless_routes_do() {
    let certificate = certificate();
    let graph = graph(&certificate, TightSupport::Atomic);
    let index = built(&certificate, &graph).index;
    let mut plan = scheduled(&graph, true);
    let dense = plan.work;
    // Exactly dense reuse's allowance: the state is bound but nothing writes it.
    assert!(!plan.bind_state(&graph, &index, u64::from(dense)));
    assert_eq!((plan.work, plan.allowance), (dense, dense));
    assert!(!plan.route.allowed);
}

#[test]
fn a_rebuilt_producer_buffer_beyond_the_device_is_refused() {
    let certificate = certificate();
    let graph = graph(&certificate, TightSupport::Atomic);
    let bounds = Bounds::new(&graph, &wgpu::Limits::default()).unwrap();
    let rebuilt = u64::from(bounds.base) * 4 + bounds.index_bytes(&graph);
    for (limit, admitted) in [(rebuilt, true), (rebuilt - 4, false)] {
        for field in 0..2 {
            let mut device = wgpu::Limits::default();
            if field == 0 {
                device.max_buffer_size = limit;
            } else {
                device.max_storage_buffer_binding_size = limit;
            }
            assert_eq!(Bounds::new(&graph, &device).is_some(), admitted);
        }
    }
}

#[test]
fn a_built_index_stays_charged_to_later_batches() {
    let certificate = certificate();
    let mut graph = graph(&certificate, TightSupport::Atomic);
    let device = wgpu::Limits::default();
    let limits = TightGpuLimits::default();
    let before = Plan::new(&graph, 2, limits, &device, false, 2)
        .unwrap()
        .accounted;
    let records = graph.producer_bytes;
    let rebuilt = built(&certificate, &graph).words.len() as u64 * 4;
    // What installing the index does to the resident theory's bytes.
    graph.bytes = graph.bytes - records + rebuilt;
    graph.producer_bytes = rebuilt;
    let after = Plan::new(&graph, 2, limits, &device, false, 2).unwrap();
    assert_eq!(after.accounted - before, rebuilt - records);
    let ceiling = TightGpuLimits {
        max_batch_bytes: after.accounted - 1,
        ..limits
    };
    assert_eq!(
        Plan::new(&graph, 2, ceiling, &device, false, 2)
            .err()
            .map(|error| error.kind()),
        Some(crate::GpuErrorKind::Capacity)
    );
}

#[test]
fn a_rows_followers_share_its_owners_level() {
    let certificate = certificate();
    let device = wgpu::Limits::default();
    let mut graph = graph(&certificate, TightSupport::Atomic);
    let mut plan = Plan::new(&graph, 1, TightGpuLimits::default(), &device, true, 1).unwrap();
    let packed = graph
        .prepare(
            &certificate,
            TightGpuLimits::default(),
            &device,
            &mut plan,
            &Cancellation::default(),
        )
        .unwrap();
    assert!(graph.levels > 0);
    let level = |node: usize| {
        let position = packed.nodes[node * 4 + 3];
        let offsets = &packed.roots[graph.roots as usize..=(graph.roots + graph.levels) as usize];
        offsets
            .windows(2)
            .position(|range| (range[0]..range[1]).contains(&position))
    };
    // Nodes 7 and 8 are the row's live guards: owner and follower.
    assert_eq!(level(7), level(8));
    assert!(level(7).is_some());
}

// An update record performing `work` and evaluating `nodes` nodes, in slot 0
// of epoch 2, with a stable verdict and its state seeded.
fn update_record(work: u32, nodes: u32) -> [u32; 9] {
    [2, 0, 0, 0, work, RESULT_MAGIC, ROUTE_UPDATE, nodes, 1]
}

#[test]
fn update_receipts_lie_between_the_floor_and_the_reservation() {
    let certificate = certificate();
    let graph = graph(&certificate, TightSupport::Atomic);
    let device = wgpu::Limits::default();
    let mut plan = Plan::new(&graph, 1, TightGpuLimits::default(), &device, false, 2).unwrap();
    plan.original_truth_reused = true;
    plan.route.allowed = true;
    plan.route.floor = 40;
    plan.allowance = 90;
    let seeds = [0x3f];
    let cancellation = Cancellation::default();
    let read = |record: [u32; 9]| decode(&record, &graph, &plan, &seeds, &cancellation);
    for (work, nodes, accepted) in [
        (40, 0, true),
        (90, graph.nodes, true),
        (39, 0, false),
        (91, 0, false),
        (60, graph.nodes + 1, false),
    ] {
        assert_eq!(
            read(update_record(work, nodes)).is_ok(),
            accepted,
            "{work} {nodes}"
        );
    }
    let decoded = read(update_record(55, 4)).unwrap();
    assert_eq!(decoded.checks[0].verdict(), TightVerdict::Stable);
    assert_eq!(decoded.checks[0].work(), 55);
    assert_eq!(decoded.routes.update, 1);
    assert_eq!((decoded.nodes_evaluated, decoded.work), (4, 55));
}

#[test]
fn routes_follow_the_batch_authority() {
    let certificate = certificate();
    let graph = graph(&certificate, TightSupport::Atomic);
    let device = wgpu::Limits::default();
    let seeds = [0x3f];
    let cancellation = Cancellation::default();
    let mut plan = Plan::new(&graph, 1, TightGpuLimits::default(), &device, false, 2).unwrap();
    plan.route.floor = 1;
    plan.allowance = u32::MAX;
    let work = plan.work;
    let charge = |route| match route {
        ROUTE_BEYOND_THRESHOLD => work + 2 * graph.words,
        _ => work,
    };
    let accepts = |plan: &Plan, route| {
        let seeded = u32::from(plan.route.allowed);
        let record = [2, 0, 0, 0, charge(route), RESULT_MAGIC, route, 0, seeded];
        decode(&record, &graph, plan, &seeds, &cancellation).is_ok()
    };
    let all = [
        ROUTE_COMPLETE,
        ROUTE_DENSE,
        ROUTE_UPDATE,
        ROUTE_BEYOND_THRESHOLD,
        4,
    ];
    let permitted = |plan: &Plan| {
        all.iter()
            .map(|&route| accepts(plan, route))
            .collect::<Vec<_>>()
    };
    // Without reuse only the complete route, allowed or not.
    assert_eq!(permitted(&plan), [true, false, false, false, false]);
    plan.route.allowed = true;
    assert_eq!(permitted(&plan), [true, false, false, false, false]);
    // Reuse without the update route: dense only.
    plan.original_truth_reused = true;
    plan.route.allowed = false;
    assert_eq!(permitted(&plan), [false, true, false, false, false]);
    // Reuse with the route allowed: dense, an update or dense beyond the threshold.
    plan.route.allowed = true;
    assert_eq!(permitted(&plan), [false, true, true, true, false]);
}

#[test]
fn records_are_seeded_exactly_when_the_route_is_allowed() {
    let certificate = certificate();
    let graph = graph(&certificate, TightSupport::Atomic);
    let device = wgpu::Limits::default();
    let seeds = [0x3f];
    let cancellation = Cancellation::default();
    let mut plan = Plan::new(&graph, 1, TightGpuLimits::default(), &device, false, 2).unwrap();
    for allowed in [false, true] {
        plan.route.allowed = allowed;
        for seeded in [0, 1] {
            let record = [
                2,
                0,
                0,
                0,
                plan.work,
                RESULT_MAGIC,
                ROUTE_COMPLETE,
                0,
                seeded,
            ];
            assert_eq!(
                decode(&record, &graph, &plan, &seeds, &cancellation).is_ok(),
                seeded == u32::from(allowed)
            );
        }
    }
}

#[test]
fn dense_receipts_evaluate_nothing_by_update_and_charge_their_route() {
    let certificate = certificate();
    let graph = graph(&certificate, TightSupport::Atomic);
    let device = wgpu::Limits::default();
    let seeds = [0x3f];
    let cancellation = Cancellation::default();
    let mut plan = Plan::new(&graph, 1, TightGpuLimits::default(), &device, false, 2).unwrap();
    plan.original_truth_reused = true;
    plan.route.allowed = true;
    plan.allowance = u32::MAX;
    let work = plan.work;
    let counted = work + 2 * graph.words;
    for (route, performed, evaluated, accepted) in [
        (ROUTE_DENSE, work, 0, true),
        (ROUTE_DENSE, work, 1, false),
        (ROUTE_DENSE, work + 1, 0, false),
        (ROUTE_BEYOND_THRESHOLD, counted, 0, true),
        (ROUTE_BEYOND_THRESHOLD, work, 0, false),
        (ROUTE_BEYOND_THRESHOLD, counted, 1, false),
    ] {
        let record = [2, 0, 0, 0, performed, RESULT_MAGIC, route, evaluated, 1];
        assert_eq!(
            decode(&record, &graph, &plan, &seeds, &cancellation).is_ok(),
            accepted,
            "{route} {performed} {evaluated}"
        );
    }
    let record = [
        2,
        0,
        0,
        0,
        counted,
        RESULT_MAGIC,
        ROUTE_BEYOND_THRESHOLD,
        0,
        1,
    ];
    let decoded = decode(&record, &graph, &plan, &seeds, &cancellation).unwrap();
    assert_eq!(
        (decoded.routes.dense_reuse, decoded.routes.beyond_threshold),
        (1, 1)
    );
}

#[test]
fn the_threshold_starts_at_the_cpu_rule() {
    for (atoms, threshold) in [(0, 64), (512, 64), (520, 65), (262_286, 32_785)] {
        assert_eq!(super::threshold(atoms), threshold, "{atoms}");
    }
}

#[test]
fn the_route_engages_from_the_factor_times_the_levels() {
    for levels in [1, 4, 18] {
        let threshold = ENGAGEMENT_STRIDES * u64::from(levels);
        assert!(engages(threshold, levels), "{levels} levels");
        assert!(!engages(threshold - 1, levels), "{levels} levels");
    }
    assert!(!engages(u64::MAX, 0));
}

// `rules` separate rules `1 :- 0` beside the two guards of one numeric row over
// atoms 0 and 2, each the body of a rule for atom 3: levels of 4 atom leaves,
// `rules + 2` gates and 2 rules.
fn fan(rules: usize) -> TightPlan {
    let mut leaves: Vec<Node> = (0..4).map(Node::atom).collect();
    leaves.extend(std::iter::repeat_n(Node::implies(0, 1), rules));
    let mut nodes = FormulaNodes::new(FormulaParts::new(leaves, vec![]).unwrap());
    let mut transaction = nodes.transaction();
    let guards = transaction
        .push_threshold_family(&[0, 2], &[1, 1], &[1, 2], usize::MAX, usize::MAX)
        .unwrap();
    let guarded: Vec<usize> = guards
        .iter()
        .map(|&guard| {
            transaction
                .push(NodeView::Implies(guard, 3), usize::MAX, usize::MAX)
                .unwrap()
        })
        .collect();
    transaction.commit();
    let theory = Theory::new(
        4,
        nodes.into_parts(),
        (4..rules + 4).chain(guarded).collect(),
        AdmissionLimits::default(),
    )
    .unwrap();
    TightPlan::compile(
        &theory,
        TightPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap()
}

/// Rules of a [`fan`] whose gate level takes `3 * ENGAGEMENT_STRIDES - 2`
/// strides of 32 invocations, so that with one stride for each other level the
/// schedule engages at 32 invocations and, taking about half as many, not at
/// 64.
fn engaging_at_32() -> usize {
    let gates = 32 * (3 * usize::try_from(ENGAGEMENT_STRIDES).unwrap() - 2);
    gates - 2
}

/// The graph and cold plan of one candidate under a fixed `size`.
fn prepared(certificate: &TightPlan, size: crate::GpuWorkgroupSize) -> (Graph, Plan) {
    let device = wgpu::Limits::default();
    let packing = Packing {
        device: &device,
        support: TightSupport::Atomic,
        workgroup_sizing: crate::GpuWorkgroupSizing::Fixed(size),
    };
    let mut graph = Graph::new(certificate, packing).unwrap();
    let mut plan = Plan::new(&graph, 1, TightGpuLimits::default(), &device, true, 1).unwrap();
    graph
        .prepare(
            certificate,
            TightGpuLimits::default(),
            &device,
            &mut plan,
            &Cancellation::default(),
        )
        .unwrap();
    (graph, plan)
}

#[test]
fn a_prepared_schedule_records_its_strided_depth() {
    // Levels of 4 leaves, the gates and 2 rules: one stride each for the leaves
    // and the rules at either size.
    let rules = engaging_at_32();
    let certificate = fan(rules);
    for size in [
        crate::GpuWorkgroupSize::Threads32,
        crate::GpuWorkgroupSize::Threads64,
    ] {
        let (graph, _) = prepared(&certificate, size);
        let gates = u64::try_from(rules + 2).unwrap();
        assert_eq!(graph.levels, 3);
        assert_eq!(
            graph.strided_depth,
            1 + gates.div_ceil(u64::from(size.invocations())) + 1,
            "{size:?}"
        );
    }
}

#[test]
fn only_an_engaging_schedule_retains_the_update_inputs() {
    let rules = engaging_at_32();
    let certificate = fan(rules);
    let (wide, _) = prepared(&certificate, crate::GpuWorkgroupSize::Threads32);
    assert!(engages(wide.strided_depth, wide.levels));
    assert_eq!(wide.largest_producers, u32::try_from(rules).unwrap());
    assert_eq!(wide.owners.len(), 1);
    let (narrow, _) = prepared(&certificate, crate::GpuWorkgroupSize::Threads64);
    assert!(!engages(narrow.strided_depth, narrow.levels));
    assert_eq!(narrow.largest_producers, 0);
    assert!(narrow.owners.is_empty());
    assert_eq!(narrow.owner_bytes, 0);
}

#[test]
fn a_narrow_schedule_accounts_none_of_the_update_inputs() {
    // Admission reserves the owner column, eight bytes for the one live row,
    // and a transient counter word per atom before the level widths are known.
    let certificate = fan(engaging_at_32());
    let (wide, wide_plan) = prepared(&certificate, crate::GpuWorkgroupSize::Threads32);
    let (narrow, narrow_plan) = prepared(&certificate, crate::GpuWorkgroupSize::Threads64);
    assert_eq!(wide.owner_bytes, 8);
    assert_eq!(wide.packing_bytes - narrow.packing_bytes, 4 * 4);
    assert_eq!(wide_plan.accounted - narrow_plan.accounted, 8 + 4 * 4);
}
