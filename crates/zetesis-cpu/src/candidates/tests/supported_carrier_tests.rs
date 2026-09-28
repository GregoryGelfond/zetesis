//! Completed support chooses identities, never a new semantic gate carrier.

use super::*;
use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, GroundProgram, Predicate, StaticLimits, Template, Term,
    Value,
};

fn pattern(name: &str, values: &[i32]) -> AtomPattern {
    AtomPattern::new(
        Predicate::new(name, values.len()).unwrap(),
        values
            .iter()
            .map(|&value| Term::Constant(Value::Number(value)))
            .collect(),
    )
    .unwrap()
}

/// Three domain values and two supported four-place gates: only two of the
/// 162 Cartesian gate tuples may hold. Their positions are not the first two.
fn sparse() -> Program {
    let p = pattern("p", &[2, 1, 2, 1]);
    let q = pattern("q", &[1, 2, 1, 2]);
    let mut templates = vec![
        Template::new(Some(p.clone()), vec![], vec![], vec![q.clone()], vec![]),
        Template::new(Some(q), vec![], vec![], vec![p], vec![]),
    ];
    for value in 0..3 {
        templates.push(Template::new(
            Some(pattern("d", &[value])),
            vec![],
            vec![],
            vec![],
            vec![],
        ));
    }
    Program::new(templates, AdmissionLimits::default()).unwrap()
}

/// Independent p(value) :- not not p(value) choices leave the supported
/// root fully open. Domain facts can also introduce unsupported carrier tuples.
fn independent_choices(values: &[i32], domain: std::ops::Range<i32>) -> Program {
    let mut templates: Vec<_> = values
        .iter()
        .map(|&value| {
            let p = pattern("p", &[value]);
            Template::new(Some(p.clone()), vec![], vec![p], vec![], vec![])
        })
        .collect();
    templates.extend(
        domain.map(|value| {
            Template::new(Some(pattern("d", &[value])), vec![], vec![], vec![], vec![])
        }),
    );
    Program::new(templates, AdmissionLimits::default()).unwrap()
}

fn expected_choices(values: &[i32]) -> Vec<Vec<Atom>> {
    let atoms: Vec<_> = values
        .iter()
        .map(|&value| {
            Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(value)]).unwrap()
        })
        .collect();
    (0..1_usize << atoms.len())
        .map(|mask| {
            atoms
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1 << index) != 0)
                .map(|(_, atom)| atom.clone())
                .collect()
        })
        .collect()
}

#[test]
fn counted_regions_cover_each_open_root_seed_once() {
    let values = [0, 1, 2, 3, 4, 5];
    let program = independent_choices(&values, 0..6);
    let mut candidates = Candidates::new(
        &program,
        CandidateLimits::default(),
        Cancellation::default(),
    );
    candidates.bounded(Limits::default());
    let actual: Vec<_> = candidates
        .by_ref()
        .map(|seed| {
            seed.unwrap()
                .atoms()
                .iter()
                .map(|atom| atom.to_atom(zetesis_core::ValueLimits::default()).unwrap())
                .collect::<Vec<_>>()
        })
        .collect();
    // Compare the sequence, not a set: deduplication would hide replayed seeds.
    assert_eq!(actual, expected_choices(&values));
    assert_eq!(candidates.discovered_atoms(), 6);
    assert!(candidates.statistics().regions_counted > 0);
    assert_eq!(candidates.statistics().narrowing_stop, None);
    assert_eq!(
        candidates.termination(),
        Some(CandidateTermination::Exhausted)
    );
}

#[test]
fn counted_regions_never_resume_the_symbolic_carrier() {
    let values = [0, 2, 4];
    let program = independent_choices(&values, 0..6);
    let mut candidates = Candidates::new(
        &program,
        CandidateLimits::default(),
        Cancellation::default(),
    );
    candidates.bounded(Limits::default());
    let actual: Vec<_> = candidates
        .by_ref()
        .map(|seed| {
            seed.unwrap()
                .atoms()
                .iter()
                .map(|atom| atom.to_atom(zetesis_core::ValueLimits::default()).unwrap())
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(actual, expected_choices(&values));
    assert_eq!(candidates.statistics().cut_gate_atoms, 3);
    assert!(candidates.statistics().regions_counted > 0);
    // Even the final carry left the original carrier untouched. Unsupported
    // tuples cannot enter the counted region or consume additional work.
    let first = candidates.carrier.next().unwrap().unwrap();
    let expected = program.indexed_gate_atoms().next().unwrap().unwrap();
    assert_eq!(first.atom(), expected.atom());
}

#[test]
fn completed_support_does_not_walk_the_cartesian_carrier() {
    let program = sparse();
    let mut candidates = Candidates::new(
        &program,
        CandidateLimits::default(),
        Cancellation::default(),
    );
    candidates.bounded(Limits::default());
    assert!(candidates.next_selection().unwrap().is_ok());
    assert_eq!(candidates.discovered_atoms(), 2);
    assert_eq!(candidates.statistics().cut_gate_atoms, 160);
    // Inspect the actual production cursor: the supported-root path has not
    // requested even its first tuple. This detects a hidden full scan without
    // relying on timing or making the test itself enumerate a huge universe.
    let first = candidates.carrier.next().unwrap().unwrap();
    let expected = program.indexed_gate_atoms().next().unwrap().unwrap();
    assert_eq!(first.atom(), expected.atom());
}

#[test]
fn supported_root_tokens_pack_at_their_original_positions() {
    let program = sparse();
    let graph = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    let mut candidates = Candidates::new(
        &program,
        CandidateLimits::default(),
        Cancellation::default(),
    );
    candidates.bounded(Limits::default());
    let mut count = 0;
    while let Some(selection) = candidates.next_selection() {
        let selection = selection.unwrap();
        let mut words = vec![u32::MAX; graph.word_count()];
        graph.seed_words_into(selection.view(), &mut words).unwrap();
        assert_eq!(words, graph.seed_words(&selection.to_seed()).unwrap());
        for entry in selection.view().entries() {
            assert_eq!(
                graph
                    .atoms()
                    .at(entry.resolve_in(&graph).unwrap() as usize)
                    .unwrap(),
                entry.atom()
            );
        }
        count += 1;
    }
    assert_eq!(count, 2);
}

#[test]
fn supported_root_still_enforces_the_open_atom_limit() {
    let program = sparse();
    let mut candidates = Candidates::new(
        &program,
        CandidateLimits {
            max_carrier_atoms: 1,
            ..CandidateLimits::default()
        },
        Cancellation::default(),
    );
    candidates.bounded(Limits::default());
    assert!(matches!(candidates.next(), Some(Err(Stop::CarrierLimit))));
    assert_eq!(
        candidates.termination(),
        Some(CandidateTermination::Stopped(Stop::CarrierLimit))
    );
    assert!(candidates.next_selection().is_none());
}

#[test]
fn a_failed_later_pass_keeps_only_completed_bounds() {
    let program = sparse();
    let mut closures = Closures::new(&program, Limits::default(), Cancellation::default()).unwrap();
    let mut cube = Cube::all_open();
    assert!(matches!(closures.narrow(&mut cube), Ok(Pass::Changed)));
    let completed_may = cube.may.clone();
    let completed_must = cube.must.clone();
    assert_eq!(completed_may.as_ref().unwrap().len(), 2);
    closures.limits.max_work = 0;
    assert!(matches!(closures.narrow(&mut cube), Err(Stop::WorkLimit)));
    assert_eq!(cube.may, completed_may);
    assert_eq!(cube.must, completed_must);
}

#[test]
fn a_failed_first_pass_has_no_completed_upper_bound() {
    let program = sparse();
    let mut closures = Closures::new(&program, Limits::default(), Cancellation::default()).unwrap();
    closures.limits.max_work = 0;
    let mut cube = Cube::all_open();
    assert!(matches!(closures.narrow(&mut cube), Err(Stop::WorkLimit)));
    assert!(cube.may.is_none());
    assert!(cube.must.is_empty());
}
