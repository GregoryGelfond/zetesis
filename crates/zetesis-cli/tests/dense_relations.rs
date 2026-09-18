//! A closure over dense relations is the closure over trees, atom for atom,
//! on the families the closure route serves: the bounds decide only how a
//! relation is stored, never which atoms it holds.
use std::fmt::Write as _;

use zetesis_core::{Atom, Model, Seed};
use zetesis_cpu::{
    ClosureWorkspace, Control, Limits, PreparationLimits, PreparedQueries, upper_closure,
};

fn edges(name: &str, first: u32, last: u32) -> String {
    let mut text = String::new();
    for i in first..last {
        let _ = write!(text, "{name}({i},{}). ", i + 1);
    }
    text
}

/// Small members of the families the series measures on the closure route,
/// with a program whose gate carrier is wide and one over values of every
/// kind.
fn families() -> Vec<(&'static str, String)> {
    vec![
        (
            "independent-negation",
            format!(
                "node(1..6).\n{}in(X) :- node(X), not out(X).\nout(X) :- node(X), not in(X).\n:- edge(X,Y), in(X), in(Y).\n",
                edges("edge", 1, 6)
            ),
        ),
        (
            "transitive-path",
            format!(
                "{}reach(X,Y) :- e(X,Y).\nreach(X,Z) :- reach(X,Y), e(Y,Z).\n",
                edges("e", 1, 8)
            ),
        ),
        (
            "chain",
            format!("{}r(0).\nr(Y) :- r(X), e(X,Y).\n", edges("e", 0, 10)),
        ),
        (
            "producer-chain",
            "p1. p2. p3. p4. p5.\nq1 :- p1, p2.\nq2 :- p2, p3.\nq3 :- p3, p4.\nq4 :- p4, p5.\n".to_string(),
        ),
        (
            "stratified",
            format!(
                "node(1..8).\n{}e(1,3). e(2,4). e(3,5). e(4,6). e(5,7). e(6,8).\n{}bad(3).\nblocked(Y) :- bad(X), next(X,Y).\nreach(1).\nreach(Y) :- reach(X), e(X,Y), not blocked(Y).\n:- not reach(8).\n",
                edges("e", 1, 8),
                edges("next", 1, 8)
            ),
        ),
        (
            "wide-carrier",
            "n(1..3).\ncell(R,C,V) :- n(R), n(C), n(V), not other(R,C,V).\nother(R,C,V) :- n(R), n(C), n(V), not cell(R,C,V).\n:- cell(R,C,V), cell(R,C,W), V != W.\n".to_string(),
        ),
        (
            "mixed-values",
            "name(alice). name(\"bob\"). name(f(1)). name(3).\nknows(X,Y) :- name(X), name(Y), X != Y.\nreach(X) :- name(X), not blocked(X).\nblocked(f(1)).\n".to_string(),
        ),
    ]
}

/// The seeds a family is checked under: nothing, every gate atom the upper
/// closure allows, and every other one of them.
fn seeds(program: &zetesis_core::Program, control: &Control) -> Vec<Seed> {
    let upper = upper_closure(program, Limits::default(), control).unwrap();
    let gates: Vec<Atom> = upper
        .atoms()
        .iter()
        .filter(|atom| program.gate_predicates().contains(atom.predicate()))
        .cloned()
        .collect();
    let alternate: Vec<Atom> = gates.iter().step_by(2).cloned().collect();
    [Vec::new(), gates, alternate]
        .into_iter()
        .map(|atoms| Seed::new(program, atoms).unwrap())
        .collect()
}

fn closures(
    program: &zetesis_core::Program,
    limits: PreparationLimits,
    seeds: &[Seed],
    control: &Control,
) -> (usize, Vec<(Model, bool, bool)>) {
    let prepared = PreparedQueries::new(program, limits, control).unwrap();
    let mut workspace = ClosureWorkspace::default();
    let checks = seeds
        .iter()
        .map(|seed| {
            let check = prepared
                .check_view(seed.view(), &mut workspace, Limits::default(), control)
                .unwrap();
            (
                check.closure().clone(),
                check.accepted(),
                check.constraint_violated(),
            )
        })
        .collect();
    (prepared.statistics().dense_predicates, checks)
}

#[test]
fn dense_and_tree_closures_agree_atom_for_atom_on_every_family() {
    let control = Control::default();
    let dense = PreparationLimits::default();
    let tree = PreparationLimits {
        max_dense_atoms: 0,
        ..PreparationLimits::default()
    };
    for (name, source) in families() {
        let owner = zetesis_themelios::admit_extended(
            source.clone(),
            zetesis_themelios::AdmissionOptions::default(),
            zetesis_themelios::ExpansionLimits::default(),
        )
        .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        let program = owner.program();
        let seeds = seeds(program, &control);
        let (dense_predicates, with_dense) = closures(program, dense, &seeds, &control);
        let (tree_predicates, with_trees) = closures(program, tree, &seeds, &control);
        assert!(dense_predicates > 0, "{name}: no predicate was laid out");
        assert_eq!(tree_predicates, 0, "{name}");
        for (seed, (dense, tree)) in seeds.iter().zip(with_dense.iter().zip(&with_trees)) {
            assert_eq!(
                dense,
                tree,
                "{name} under {} seed atoms",
                seed.atoms().len()
            );
        }
    }
}

/// Seven edges and their 28 paths: 35 atoms, every predicate bounded.
fn transitive_path() -> zetesis_themelios::Admitted {
    zetesis_themelios::admit_extended(
        format!(
            "{}reach(X,Y) :- e(X,Y).\nreach(X,Z) :- reach(X,Y), e(Y,Z).\n",
            edges("e", 1, 8)
        ),
        zetesis_themelios::AdmissionOptions::default(),
        zetesis_themelios::ExpansionLimits::default(),
    )
    .unwrap()
}

fn stores() -> [PreparationLimits; 2] {
    [
        PreparationLimits::default(),
        PreparationLimits {
            max_dense_atoms: 0,
            ..PreparationLimits::default()
        },
    ]
}

/// The heads a closure of the transitive path recorded as bits, and its
/// atoms, under one store.
fn heads_recorded_as_bits(limits: PreparationLimits) -> (u64, usize) {
    let control = Control::default();
    let owner = transitive_path();
    let program = owner.program();
    let seed = Seed::new(program, []).unwrap();
    let prepared = PreparedQueries::new(program, limits, &control).unwrap();
    let check = prepared
        .check_view(
            seed.view(),
            &mut ClosureWorkspace::default(),
            Limits::default(),
            &control,
        )
        .unwrap();
    (
        check.statistics().dense_heads,
        check.closure().atoms().len(),
    )
}

#[test]
fn every_head_of_a_dense_relation_is_recorded_as_a_bit() {
    let [dense, _] = stores();
    assert_eq!(heads_recorded_as_bits(dense), (35, 35));
}

#[test]
fn no_head_of_a_tree_relation_is_recorded_as_a_bit() {
    let [_, tree] = stores();
    assert_eq!(heads_recorded_as_bits(tree), (0, 35));
}

#[test]
fn the_transitive_rule_joins_one_block_for_each_new_path_with_an_onward_edge() {
    // Each of the 28 paths is new in one round, where the recursive rule
    // visits it outermost and takes the edges leaving its end as one block.
    // The seven paths ending at node 8 have none: 8 lies outside the bound
    // of an edge's first argument, so their block is empty and not joined.
    let control = Control::default();
    let owner = transitive_path();
    let program = owner.program();
    let seed = Seed::new(program, []).unwrap();
    let [dense, tree] = stores().map(|limits| {
        let prepared = PreparedQueries::new(program, limits, &control).unwrap();
        let check = prepared
            .check_view(
                seed.view(),
                &mut ClosureWorkspace::default(),
                Limits::default(),
                &control,
            )
            .unwrap();
        let statistics = check.statistics();
        (
            statistics.row_steps,
            statistics.bindings,
            check.closure().clone(),
        )
    });
    assert_eq!((dense.0, tree.0), (21, 0));
    // A row of a block is a binding, as it is when bound singly.
    assert_eq!(dense.1, tree.1);
    assert_eq!(dense.2, tree.2);
}

#[test]
fn the_derived_atom_limit_stops_a_dense_closure_where_it_stops_a_tree() {
    let control = Control::default();
    let owner = transitive_path();
    let program = owner.program();
    let seed = Seed::new(program, []).unwrap();
    for limits in stores() {
        let prepared = PreparedQueries::new(program, limits, &control).unwrap();
        let check = |max_derived_atoms| {
            prepared
                .check_view(
                    seed.view(),
                    &mut ClosureWorkspace::default(),
                    Limits {
                        max_derived_atoms,
                        ..Limits::default()
                    },
                    &control,
                )
                .map(|check| check.closure().atoms().len())
        };
        assert_eq!(check(35), Ok(35));
        assert_eq!(check(34), Err(zetesis_cpu::Stop::DerivedAtomLimit));
    }
}

#[test]
fn a_predicate_wider_than_the_ceiling_keeps_its_tree_beside_dense_ones() {
    // `n` has four tuples inside its bounds and `e` sixteen: a ceiling of
    // four lays out `n` and keeps `e` a tree, and the mixed store agrees
    // with both pure ones.
    let control = Control::default();
    let owner = zetesis_themelios::admit_extended(
        "n(1..4).\ne(X,Y) :- n(X), n(Y), X != Y.\nreach(X,Y) :- e(X,Y).\nreach(X,Z) :- reach(X,Y), e(Y,Z).\n"
            .into(),
        zetesis_themelios::AdmissionOptions::default(),
        zetesis_themelios::ExpansionLimits::default(),
    )
    .unwrap();
    let program = owner.program();
    let seeds = seeds(program, &control);
    let mixed = PreparationLimits {
        max_dense_atoms: 4,
        ..PreparationLimits::default()
    };
    let tree = PreparationLimits {
        max_dense_atoms: 0,
        ..PreparationLimits::default()
    };
    let (laid_out, with_mixed) = closures(program, mixed, &seeds, &control);
    let (all, with_dense) = closures(program, PreparationLimits::default(), &seeds, &control);
    let (none, with_trees) = closures(program, tree, &seeds, &control);
    assert_eq!((laid_out, all, none), (1, 3, 0));
    assert_eq!(with_mixed, with_dense);
    assert_eq!(with_mixed, with_trees);
}
