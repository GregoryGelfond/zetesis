//! Explicit physical Metal qualification for lazy source-owned round snapshots.
use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, GroundProgram, Predicate, Program, Seed, StaticLimits,
    Template, Term, Value,
};
use zetesis_cpu::{Control, Limits, check, lazy};
use zetesis_wgpu::{GpuBackendPreference, GpuLazyOracle, GpuLimits, GpuOptions, GpuSelection};

fn predicate(name: &str, arity: usize) -> Predicate {
    Predicate::new(name, arity).unwrap()
}
fn pattern(name: &str, terms: Vec<Term>) -> AtomPattern {
    AtomPattern::new(predicate(name, terms.len()), terms).unwrap()
}
fn metal() -> GpuLazyOracle {
    GpuLazyOracle::new_selected(
        GpuOptions::default(),
        GpuSelection {
            backend: GpuBackendPreference::Metal,
            vendor_id: None,
        },
    )
    .expect("physical Metal must be available for this explicit qualification")
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
    let mut oracle = metal();
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
    let mut oracle = metal();
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
    let mut oracle = metal();
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
