//! Explicit physical API qualification for lazy source-owned round snapshots.
#[path = "support/physical.rs"]
mod physical;

use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, GroundProgram, Predicate, Program, Seed, StaticLimits,
    Template, Term, Value,
};
use zetesis_cpu::{Control, Limits, check, lazy};
use zetesis_wgpu::{GpuLazyOracle, GpuLimits, GpuOptions};

fn predicate(name: &str, arity: usize) -> Predicate {
    Predicate::new(name, arity).unwrap()
}
fn pattern(name: &str, terms: Vec<Term>) -> AtomPattern {
    AtomPattern::new(predicate(name, terms.len()), terms).unwrap()
}
fn oracle(backend: physical::Backend) -> GpuLazyOracle {
    let oracle = GpuLazyOracle::new_selected(GpuOptions::default(), backend.selection())
        .expect("requested physical adapter must be available for this qualification");
    backend.verify(oracle.info());
    oracle
}

fn compare(oracle: &mut GpuLazyOracle, program: &Program, seeds: &[Seed], limits: lazy::Limits) {
    let batch = oracle
        .check_batch(
            program,
            seeds,
            limits,
            GpuLimits::default(),
            &Control::default(),
        )
        .unwrap();
    assert_eq!(batch.checks.len(), seeds.len());
    for (actual, seed) in batch.checks.iter().zip(seeds) {
        let expected = check(program, seed, Limits::default(), &Control::default()).unwrap();
        assert_eq!(actual.closure(), expected.closure());
        assert_eq!(
            (
                actual.accepted(),
                actual.constraint_violated(),
                actual.seed_mismatch()
            ),
            (
                expected.accepted(),
                expected.constraint_violated(),
                expected.seed_mismatch()
            )
        );
    }
    let work = oracle.statistics();
    assert_eq!(work.dispatches, batch.progress.chunks);
    assert!(work.dispatches > 0);
    assert!(work.world_instances > 0);
    assert!(work.uploaded_bytes > 0);
    assert!(work.downloaded_bytes > 0);
    assert_eq!(
        work.dispatches,
        work.transport_allocations + work.transport_reuses
    );
    assert!(work.peak_transport_bytes > 0);
    eprintln!(
        "adapter={} rounds={} catalog={} instances={} dispatches={} world_instances={} uploaded={} downloaded={} wait_ns={}",
        oracle.info().name(),
        batch.progress.rounds,
        batch.progress.catalog_atoms,
        batch.progress.instances,
        work.dispatches,
        work.world_instances,
        work.uploaded_bytes,
        work.downloaded_bytes,
        work.host_wait.as_nanos()
    );
}

#[test]
#[ignore = "requires a physical Metal adapter"]
fn metal_lazy_worlds_match_exact_frozen_cpu_closures() {
    qualify_lazy_worlds_match_exact_frozen_cpu_closures(physical::Backend::Metal);
}

#[test]
#[ignore = "requires an actual Vulkan GPU; explicit physical qualification"]
fn vulkan_lazy_worlds_match_exact_frozen_cpu_closures() {
    qualify_lazy_worlds_match_exact_frozen_cpu_closures(physical::Backend::Vulkan);
}

fn qualify_lazy_worlds_match_exact_frozen_cpu_closures(backend: physical::Backend) {
    let nullary = |name| pattern(name, vec![]);
    let program = Program::new(
        vec![
            Template::new(
                Some(nullary("a")),
                vec![],
                vec![],
                vec![nullary("b")],
                vec![],
            ),
            Template::new(
                Some(nullary("b")),
                vec![],
                vec![],
                vec![nullary("a")],
                vec![],
            ),
            Template::new(
                Some(nullary("x")),
                vec![nullary("a")],
                vec![],
                vec![],
                vec![],
            ),
            Template::new(
                Some(nullary("y")),
                vec![nullary("b")],
                vec![],
                vec![],
                vec![],
            ),
            Template::new(
                Some(nullary("cross")),
                vec![nullary("x"), nullary("y")],
                vec![],
                vec![],
                vec![],
            ),
            Template::new(None, vec![nullary("cross")], vec![], vec![], vec![]),
        ],
        AdmissionLimits::default(),
    )
    .unwrap();
    let patterns = [vec![], vec!["a"], vec!["b"], vec!["a", "b"]];
    let seeds: Vec<_> = (0..65)
        .map(|index| {
            Seed::new(
                &program,
                patterns[index % 4]
                    .iter()
                    .map(|name| Atom::new(predicate(name, 0), vec![]).unwrap()),
            )
            .unwrap()
        })
        .collect();
    let mut oracle = oracle(backend);
    for batch in [1, 3, 31, 32, 33, 65] {
        for chunk in [1, 3, 7, 32] {
            compare(
                &mut oracle,
                &program,
                &seeds[..batch],
                lazy::Limits {
                    max_atoms: 65,
                    max_chunk_rules: chunk,
                    ..lazy::Limits::default()
                },
            );
        }
    }
}

#[test]
#[ignore = "requires a physical Metal adapter"]
fn metal_lazy_growth_preserves_previous_round_truth() {
    qualify_lazy_growth_preserves_previous_round_truth(physical::Backend::Metal);
}

#[test]
#[ignore = "requires an actual Vulkan GPU; explicit physical qualification"]
fn vulkan_lazy_growth_preserves_previous_round_truth() {
    qualify_lazy_growth_preserves_previous_round_truth(physical::Backend::Vulkan);
}

fn qualify_lazy_growth_preserves_previous_round_truth(backend: physical::Backend) {
    let mut oracle = oracle(backend);
    for count in [31, 32, 33, 63, 64, 65] {
        let program = Program::new(
            (0..count)
                .map(|n| {
                    Template::new(
                        Some(pattern("p", vec![Term::Constant(Value::Number(n))])),
                        if n == 0 {
                            vec![]
                        } else {
                            vec![pattern("p", vec![Term::Constant(Value::Number(n - 1))])]
                        },
                        vec![],
                        vec![],
                        vec![],
                    )
                })
                .collect(),
            AdmissionLimits::default(),
        )
        .unwrap();
        let seeds = vec![Seed::new(&program, []).unwrap(); 3];
        compare(&mut oracle, &program, &seeds, lazy::Limits::default());
    }
}

#[test]
#[ignore = "requires a physical Metal adapter"]
fn metal_lazy_catalog_fits_when_static_carrier_refuses() {
    qualify_lazy_catalog_fits_when_static_carrier_refuses(physical::Backend::Metal);
}

#[test]
#[ignore = "requires an actual Vulkan GPU; explicit physical qualification"]
fn vulkan_lazy_catalog_fits_when_static_carrier_refuses() {
    qualify_lazy_catalog_fits_when_static_carrier_refuses(physical::Backend::Vulkan);
}

fn qualify_lazy_catalog_fits_when_static_carrier_refuses(backend: physical::Backend) {
    let mut templates = (0..21)
        .map(|n| {
            Template::new(
                Some(pattern("p", vec![Term::Constant(Value::Number(n))])),
                vec![],
                vec![],
                vec![],
                vec![],
            )
        })
        .collect::<Vec<_>>();
    templates.push(Template::new(
        Some(pattern("h", vec![Term::Variable(0)])),
        vec![pattern("p", vec![Term::Variable(0)])],
        vec![],
        vec![pattern("g", vec![Term::Variable(0); 3])],
        vec![],
    ));
    let program = Program::new(templates, AdmissionLimits::default()).unwrap();
    assert!(
        GroundProgram::compile(
            &program,
            StaticLimits {
                max_atoms: 128,
                ..StaticLimits::default()
            }
        )
        .is_err()
    );
    let seeds = [
        Seed::new(&program, []).unwrap(),
        Seed::new(
            &program,
            [Atom::new(predicate("g", 3), vec![Value::Number(0); 3]).unwrap()],
        )
        .unwrap(),
    ];
    let mut oracle = oracle(backend);
    compare(
        &mut oracle,
        &program,
        &seeds,
        lazy::Limits {
            max_atoms: 128,
            max_chunk_rules: 7,
            ..lazy::Limits::default()
        },
    );
}

#[test]
#[ignore = "requires a physical Metal adapter"]
fn metal_source_selections_preserve_each_frozen_closure() {
    qualify_source_selections_preserve_each_frozen_closure(physical::Backend::Metal);
}

#[test]
#[ignore = "requires an actual Vulkan GPU; explicit physical qualification"]
fn vulkan_source_selections_preserve_each_frozen_closure() {
    qualify_source_selections_preserve_each_frozen_closure(physical::Backend::Vulkan);
}

fn qualify_source_selections_preserve_each_frozen_closure(backend: physical::Backend) {
    let constant = |name, value| pattern(name, vec![Term::Constant(Value::Number(value))]);
    let mut templates = Vec::new();
    for value in 0..2 {
        let gate = constant("pick", value);
        for name in ["pick", "a", "b", "c"] {
            templates.push(Template::new(
                Some(constant(name, value)),
                vec![],
                vec![gate.clone()],
                vec![],
                vec![],
            ));
        }
    }
    templates.push(Template::new(
        Some(pattern("triple", (0..3).map(Term::Variable).collect())),
        ["a", "b", "c"]
            .into_iter()
            .enumerate()
            .map(|(slot, name)| pattern(name, vec![Term::Variable(slot)]))
            .collect(),
        vec![],
        vec![],
        vec![],
    ));
    // The same shared atom gains world 1 one round after gaining world 0.
    // Reused join storage must read rebuilt membership, including that identity.
    for (head, positive) in [
        (constant("shared", 0), vec![constant("a", 0)]),
        (constant("later", 0), vec![constant("b", 1)]),
        (constant("shared", 0), vec![constant("later", 0)]),
        (
            constant("observed", 0),
            vec![constant("shared", 0), constant("b", 1)],
        ),
    ] {
        templates.push(Template::new(Some(head), positive, vec![], vec![], vec![]));
    }
    let program = Program::new(templates, AdmissionLimits::default()).unwrap();
    let seeds: Vec<_> = (0..33)
        .map(|index| {
            Seed::new(
                &program,
                [Atom::new(predicate("pick", 1), vec![Value::Number(index % 2)]).unwrap()],
            )
            .unwrap()
        })
        .collect();
    let mut oracle = oracle(backend);
    for chunk in [1, 7] {
        for selection in [lazy::SourceSelection::Union, lazy::SourceSelection::Worlds] {
            let batch = oracle
                .check_batch_with_source(
                    &program,
                    &seeds,
                    lazy::Limits {
                        max_chunk_rules: chunk,
                        ..Default::default()
                    },
                    GpuLimits::default(),
                    selection,
                    &Control::default(),
                )
                .unwrap();
            assert_eq!(batch.checks.len(), seeds.len());
            assert!(batch.progress.rounds >= 5);
            for (actual, seed) in batch.checks.iter().zip(&seeds) {
                let expected =
                    check(&program, seed, Limits::default(), &Control::default()).unwrap();
                assert_eq!(actual.closure(), expected.closure());
                assert_eq!(actual.accepted(), expected.accepted());
                assert_eq!(actual.constraint_violated(), expected.constraint_violated());
                assert_eq!(actual.seed_mismatch(), expected.seed_mismatch());
            }
            let submitted = oracle.statistics();
            assert_eq!(submitted.dispatches, batch.progress.chunks);
            assert_eq!(submitted.world_instances, batch.progress.instances * 33);
            assert!(submitted.uploaded_bytes > 0);
            assert!(submitted.downloaded_bytes > 0);
            assert!(submitted.transport_reuses > 0);
            assert_eq!(
                submitted.dispatches,
                submitted.transport_allocations + submitted.transport_reuses
            );
            assert_eq!(
                batch.progress.pruned_prefixes > 0,
                selection == lazy::SourceSelection::Worlds
            );
            eprintln!(
                "adapter={} selection={selection:?} chunk={chunk} instances={} mask_work={} mask_bytes={} pruned={} uploaded={} downloaded={}",
                oracle.info().name(),
                batch.progress.instances,
                batch.progress.mask_words,
                batch.progress.peak_mask_bytes,
                batch.progress.pruned_prefixes,
                submitted.uploaded_bytes,
                submitted.downloaded_bytes
            );
        }
    }
}
