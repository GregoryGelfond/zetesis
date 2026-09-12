//! Checked borrowed packing owns no per-seed word scratch.

use std::sync::Arc;

use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, GroundProgram, Predicate, Program, Seed, SeedError,
    SeedSelection, StaticLimits, Template,
};

fn program(count: usize) -> Program {
    Program::new(
        (0..count)
            .map(|index| {
                let p =
                    AtomPattern::new(Predicate::new(format!("p{index:03}"), 0).unwrap(), vec![])
                        .unwrap();
                Template::new(Some(p.clone()), vec![], vec![p], vec![], vec![])
            })
            .collect(),
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn selection(program: &Program) -> SeedSelection {
    SeedSelection::new(
        program,
        [0, 31, 32, 63, 64, 69].map(|index| {
            Arc::new(Atom::new(Predicate::new(format!("p{index:03}"), 0).unwrap(), vec![]).unwrap())
        }),
    )
    .unwrap()
}

#[test]
fn borrowed_packing_replaces_dirty_words_with_exact_bits() {
    let program = program(70);
    let graph = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    let selected = selection(&program);
    let owned = selected.to_seed();
    for view in [owned.view(), selected.view()] {
        let mut words = [u32::MAX; 3];
        let storage = words.as_ptr();
        graph.seed_words_into(view, &mut words).unwrap();
        assert_eq!(words, [0x8000_0001, 0x8000_0001, 0x21]);
        assert_eq!(words.as_ptr(), storage);
        assert_eq!(graph.seed_words(&owned).unwrap(), words);
        assert_eq!(
            graph
                .model_from_words(&words)
                .unwrap()
                .atoms()
                .iter()
                .collect::<Vec<_>>(),
            owned.atoms().iter().collect::<Vec<_>>()
        );
    }
}

#[test]
fn borrowed_packing_clears_a_previous_candidate() {
    let program = program(70);
    let graph = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    let selected = selection(&program);
    let empty = SeedSelection::new(&program, []).unwrap();
    let mut words = [0; 3];
    graph.seed_words_into(selected.view(), &mut words).unwrap();
    graph.seed_words_into(empty.view(), &mut words).unwrap();
    assert_eq!(words, [0; 3]);
}

#[test]
fn wrong_word_count_preserves_the_output() {
    let program = program(70);
    let graph = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    let selected = selection(&program);
    for count in [0, 2, 4] {
        let mut words = vec![u32::MAX; count];
        assert_eq!(
            graph.seed_words_into(selected.view(), &mut words),
            Err(SeedError::WordCount {
                expected: 3,
                actual: count
            })
        );
        assert_eq!(words, vec![u32::MAX; count]);
    }
    assert_eq!(
        SeedError::WordCount {
            expected: 3,
            actual: 2
        }
        .to_string(),
        "candidate requires 3 words, received 2"
    );
}

#[test]
fn foreign_identity_precedes_shape_validation() {
    let first = program(70);
    let second = program(70);
    let graph = GroundProgram::compile(&first, StaticLimits::default()).unwrap();
    let selected = selection(&second);
    let mut words = [u32::MAX; 2];
    assert_eq!(
        graph.seed_words_into(selected.view(), &mut words),
        Err(SeedError::WrongProgram)
    );
    assert_eq!(words, [u32::MAX; 2]);
}

#[test]
fn empty_graph_accepts_only_empty_storage() {
    let program = program(0);
    let graph = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    let seed = Seed::new(&program, []).unwrap();
    assert_eq!(graph.seed_words_into(seed.view(), &mut []), Ok(()));
    assert_eq!(graph.seed_words(&seed), Ok(vec![]));
    assert_eq!(
        graph.seed_words_into(seed.view(), &mut [0]),
        Err(SeedError::WordCount {
            expected: 0,
            actual: 1
        })
    );
}
