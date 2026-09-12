//! Explicit hardware qualification, separate from portable unit tests.
//! Select the intended API, for example with
//! `cargo test -p zetesis-wgpu --test hardware vulkan -- --ignored --nocapture`.
//! Adapter absence/refusal fails this requested qualification rather than
//! silently replacing it with a CPU simulation.
#![forbid(unsafe_code)]

#[path = "support/physical.rs"]
mod physical;
#[path = "support/seed_views.rs"]
mod seed_views;

use std::collections::BTreeSet;
use std::sync::Arc;

use zetesis_core::{
    AdmissionLimits, AtomPattern, GroundProgram, Predicate, Program, Seed, SeedSelection,
    StaticLimits, Template,
};
use zetesis_wgpu::{GpuCheck, GpuLimits, GpuOptions, GpuOracle};

// Complete powersets stay bounded at 256 candidate occurrences per fixture.
const MAX_FIXTURE_GATES: usize = 8;

fn atom(name: &str) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, 0).expect("valid signature"), vec![])
        .expect("nullary pattern")
}

fn rule(
    head: Option<&str>,
    positive: &[&str],
    gate_true: &[&str],
    gate_false: &[&str],
) -> Template {
    Template::new(
        head.map(atom),
        positive.iter().map(|name| atom(name)).collect(),
        gate_true.iter().map(|name| atom(name)).collect(),
        gate_false.iter().map(|name| atom(name)).collect(),
        vec![],
    )
}

fn compile(templates: Vec<Template>) -> GroundProgram {
    let program = Program::new(templates, AdmissionLimits::default()).expect("admitted fixture");
    GroundProgram::compile(&program, StaticLimits::default())
        .expect("small explicit static fixture")
}

fn seeds(graph: &GroundProgram) -> Vec<Seed> {
    let gates = graph.gate_atom_ids();
    assert!(
        gates.len() <= MAX_FIXTURE_GATES,
        "fixture candidate enumeration is bounded"
    );
    (0..1usize << gates.len())
        .map(|bits| {
            Seed::new(
                graph.program(),
                gates
                    .iter()
                    .enumerate()
                    .filter(|(bit, _)| bits & (1 << bit) != 0)
                    .map(|(_, id)| {
                        graph.atoms()[usize::try_from(*id).expect("dense index")].clone()
                    }),
            )
            .expect("program-bound gate seed")
        })
        .collect()
}

fn indexed_selections(graph: &GroundProgram, seeds: &[Seed]) -> Vec<SeedSelection> {
    let gate_count = graph.gate_atom_ids().len();
    assert!(gate_count <= MAX_FIXTURE_GATES);
    // Share one bounded carrier across every selected subset. Taking one extra
    // item makes an unexpected carrier extension a failure, not truncation.
    let gates: Vec<_> = graph
        .program()
        .indexed_gate_atoms()
        .take(gate_count + 1)
        .map(|atom| Arc::new(atom.expect("bounded indexed gate fixture")))
        .collect();
    assert_eq!(gates.len(), gate_count);
    seeds
        .iter()
        .map(|seed| {
            SeedSelection::from_gate_atoms(
                graph.program(),
                gates
                    .iter()
                    .filter(|gate| seed.atoms().contains(gate.atom()))
                    .cloned(),
            )
            .expect("indexed subset of the program's gate carrier")
        })
        .collect()
}

/// Independent ordered-set closure: no GPU packing or shader state is used.
fn reference(graph: &GroundProgram, seed: &Seed) -> (Vec<u32>, u32) {
    let frozen: BTreeSet<u32> = seed
        .atoms()
        .iter()
        .map(|atom| graph.atom_id(atom).expect("seed in static carrier"))
        .collect();
    let enabled = |rule: &zetesis_core::GroundRule| {
        rule.gate_true().iter().all(|atom| frozen.contains(atom))
            && rule.gate_false().iter().all(|atom| !frozen.contains(atom))
    };
    let mut known = BTreeSet::new();
    loop {
        let before = known.len();
        for support in graph.rules() {
            if let Some(head) = support.head()
                && enabled(support)
                && support.positive().iter().all(|atom| known.contains(atom))
            {
                known.insert(head);
            }
        }
        if known.len() == before {
            break;
        }
    }
    let constraint = graph.rules().iter().any(|support| {
        support.head().is_none()
            && enabled(support)
            && support.positive().iter().all(|atom| known.contains(atom))
    });
    let mismatch = graph
        .gate_atom_ids()
        .iter()
        .any(|atom| known.contains(atom) != frozen.contains(atom));
    let mut closure = vec![0u32; graph.word_count()];
    for atom in known {
        closure[usize::try_from(atom / 32).expect("word index")] |= 1 << (atom % 32);
    }
    (closure, u32::from(constraint) | (u32::from(mismatch) << 1))
}

fn assert_closures(graph: &GroundProgram, candidates: &[Seed], checks: &[GpuCheck]) {
    assert_eq!(checks.len(), candidates.len());
    for (seed, check) in candidates.iter().zip(checks) {
        let (closure, status) = reference(graph, seed);
        assert_eq!(check.closure_words(), closure);
        assert_eq!(check.accepted(), status == 0);
        assert_eq!(check.constraint_violated(), status & 1 != 0);
        assert_eq!(check.seed_mismatch(), status & 2 != 0);
        graph
            .model_from_words(check.closure_words())
            .expect("GPU closure reconstructs a valid model");
    }
}

fn compare(oracle: &mut GpuOracle, graph: &GroundProgram) -> usize {
    let candidates = seeds(graph);
    let checks = oracle
        .check_batch(graph, &candidates, GpuLimits::default())
        .expect("real GPU dispatch succeeds");
    let cold = *oracle
        .last_batch_stats()
        .expect("nonempty batch statistics");
    assert!(cold.graph_uploaded && cold.transport_allocated);
    assert_closures(graph, &candidates, &checks);
    // No-dispatch calls leave both residency and the next submission identity
    // available, even with no payload/candidate budget or wait allowance.
    assert!(
        oracle
            .check_batch(
                graph,
                &[],
                GpuLimits {
                    max_candidates: 0,
                    max_batch_bytes: 0,
                    timeout: std::time::Duration::ZERO,
                },
            )
            .expect("an empty batch needs no dispatch resources")
            .is_empty()
    );
    assert!(oracle.last_batch_stats().is_none());
    // The next epoch uses reversed candidate order and the same immutable
    // program. Any leaked candidate latches or stale lane mapping is visible.
    let reversed: Vec<_> = candidates.into_iter().rev().collect();
    let selections = indexed_selections(graph, &reversed);
    let repeated = oracle
        .check_batch_views(
            graph,
            selections.iter().map(SeedSelection::view),
            GpuLimits::default(),
        )
        .expect("subsequent epoch succeeds");
    assert_closures(graph, &reversed, &repeated);
    assert!(checks.iter().rev().eq(repeated.iter()));
    let hot = *oracle
        .last_batch_stats()
        .expect("resident batch statistics");
    assert!(!hot.graph_uploaded && !hot.transport_allocated);
    assert_eq!(
        cold.accounted_batch_bytes - hot.accounted_batch_bytes,
        hot.resident_graph_bytes
    );
    if reversed.len() > 1 {
        let manual = seed_views::selections(&reversed[..1]);
        let smaller = oracle
            .check_batch_views(
                graph,
                manual.iter().map(SeedSelection::view),
                GpuLimits {
                    max_batch_bytes: hot.accounted_batch_bytes,
                    ..GpuLimits::default()
                },
            )
            .expect("smaller transport replaces larger buffers under a hot budget");
        assert_closures(graph, &reversed[..1], &smaller);
        assert_eq!(smaller, repeated[..1]);
        let resized = oracle
            .last_batch_stats()
            .expect("resized transport statistics");
        assert!(!resized.graph_uploaded && resized.transport_allocated);
        assert!(resized.resident_transport_bytes < hot.resident_transport_bytes);
        checks.len() * 2 + 1
    } else {
        checks.len() * 2
    }
}

#[test]
#[ignore = "requires an actual Metal GPU; run this hardware qualification explicitly"]
fn metal_constructor_executes_resident_batches_without_fallback() {
    qualify_constructor_executes_resident_batches_without_fallback(physical::Backend::Metal);
}

#[test]
#[ignore = "requires an actual Vulkan GPU; explicit physical qualification"]
fn vulkan_constructor_executes_resident_batches_without_fallback() {
    qualify_constructor_executes_resident_batches_without_fallback(physical::Backend::Vulkan);
}

fn qualify_constructor_executes_resident_batches_without_fallback(backend: physical::Backend) {
    let mut oracle = GpuOracle::new_selected(GpuOptions::default(), backend.selection())
        .expect("requested physical GPU must be available");
    backend.verify(oracle.info());
    let graph = compile(vec![
        rule(Some("a"), &[], &["a"], &[]),
        rule(Some("b"), &["a"], &[], &[]),
    ]);
    let compared = compare(&mut oracle, &graph);
    oracle.clear_residency();
    assert!(oracle.last_batch_stats().is_none());
    assert_eq!(compare(&mut oracle, &graph), compared);
}

#[test]
#[ignore = "requires an actual GPU; run this hardware qualification explicitly"]
fn exact_static_oracle_matches_independent_cpu_closures() {
    let oracle = GpuOracle::new(GpuOptions::default()).expect("physical GPU adapter is available");
    qualify_static(oracle);
}

#[test]
#[ignore = "requires an actual Metal GPU; explicit physical qualification"]
fn metal_static_oracle_matches_independent_closures() {
    let backend = physical::Backend::Metal;
    let oracle = GpuOracle::new_selected(GpuOptions::default(), backend.selection())
        .expect("requested physical Metal adapter must be available");
    backend.verify(oracle.info());
    qualify_static(oracle);
}

#[test]
#[ignore = "requires an actual Vulkan GPU; explicit physical qualification"]
fn vulkan_static_oracle_matches_independent_closures() {
    let backend = physical::Backend::Vulkan;
    let oracle = GpuOracle::new_selected(GpuOptions::default(), backend.selection())
        .expect("requested physical Vulkan adapter must be available");
    backend.verify(oracle.info());
    qualify_static(oracle);
}

fn qualify_static(mut oracle: GpuOracle) {
    assert!(oracle.info().is_hardware_gpu());
    println!(
        "adapter={} backend={} type={}",
        oracle.info().name(),
        oracle.info().backend(),
        oracle.info().device_type()
    );
    let fixtures = [
        compile(vec![]),
        compile(vec![rule(None, &[], &[], &[])]),
        compile(vec![
            rule(Some("a"), &[], &[], &["b"]),
            rule(Some("b"), &[], &[], &["a"]),
            rule(Some("c"), &["a"], &[], &[]),
        ]),
        compile(vec![
            rule(Some("a"), &[], &["a"], &[]),
            rule(Some("c"), &[], &["c"], &[]),
            rule(Some("b"), &["a"], &[], &[]),
            rule(None, &["b"], &[], &["c"]),
        ]),
        compile(vec![
            rule(Some("a"), &["b"], &[], &[]),
            rule(Some("b"), &["a"], &[], &[]),
        ]),
        compile(vec![
            rule(Some("a"), &[], &["a"], &[]),
            rule(None, &[], &[], &["a"]),
        ]),
        compile(vec![rule(Some("a"), &[], &[], &["a"])]),
    ];
    let mut compared = fixtures
        .iter()
        .map(|graph| compare(&mut oracle, graph))
        .sum::<usize>();
    // Fixed-width names keep numeric positions in canonical predicate order.
    let mut chain = vec![rule(Some("p000"), &[], &["p000"], &[])];
    for index in (1..130).rev() {
        chain.push(rule(
            Some(&format!("p{index:03}")),
            &[&format!("p{:03}", index - 1)],
            &[],
            &[],
        ));
    }
    // Gate ordinals 0/1/2 must resolve to dense bits in three distinct words.
    chain.push(rule(None, &[], &["p064"], &["p129"]));
    let chain = compile(chain);
    assert_eq!(chain.gate_atom_ids(), [0, 64, 129]);
    compared += compare(&mut oracle, &chain);
    let boundary = (0..zetesis_wgpu::MAX_ATOMS)
        .map(|index| rule(Some(&format!("b{index}")), &[], &[], &[]))
        .collect();
    compared += compare(&mut oracle, &compile(boundary));
    println!(
        "compared {compared} candidate executions, including reused epochs and the 4096-atom boundary"
    );
}
