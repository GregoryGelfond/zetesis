//! A region of candidates over a formula theory is narrowed by the theory's
//! readings: an impossible root refutes it, a sure body forces its head or
//! refutes a constraint, a unit root decides its one open atom, and an atom
//! no producer can support is cut. Every stable model of the region survives.

use std::collections::BTreeSet;

use zetesis_cpu::{Control, Stop};
use zetesis_ferraris::{
    AdmissionLimits, Interpretation, Limits, Narrower, Narrowing, NarrowingStatistics, Node,
    Producers, Region, RegionLimits, Theory, check, producers,
};

/// One narrowing of a region with a fresh index of the theory and knowledge
/// of nothing: the root's narrowing, for one region.
fn narrow_fresh(
    theory: &Theory,
    producers: Option<&Producers>,
    region: &mut Region,
    limits: RegionLimits,
    control: &Control,
) -> Result<(Narrowing, NarrowingStatistics), Stop> {
    let narrower = Narrower::new(theory);
    let mut knowledge = narrower.knowledge();
    narrower.narrow_known(theory, producers, region, &mut knowledge, limits, control)
}

fn theory(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(atoms, nodes, roots, AdmissionLimits::default()).unwrap()
}

/// The stable models of a small theory, as masks over its atoms.
fn stable_models(theory: &Theory) -> BTreeSet<usize> {
    let atoms = theory.atom_count();
    assert!(atoms <= 8);
    (0..1usize << atoms)
        .filter(|mask| {
            let candidate =
                Interpretation::new(theory, (0..atoms).filter(|a| mask & (1 << a) != 0)).unwrap();
            check(theory, &candidate, Limits::default(), &Control::default())
                .unwrap()
                .accepted()
        })
        .collect()
}

fn region(theory: &Theory, held: &[usize], cut: &[usize]) -> Region {
    let mut region = Region::undecided(theory.atom_count());
    for &atom in held {
        assert!(region.hold(atom));
    }
    for &atom in cut {
        assert!(region.cut(atom));
    }
    region
}

fn narrowed(theory: &Theory, region: &mut Region) -> Narrowing {
    let extracted = producers(theory, RegionLimits::default(), &Control::default()).unwrap();
    narrow_fresh(
        theory,
        extracted.producers.as_ref(),
        region,
        RegionLimits::default(),
        &Control::default(),
    )
    .unwrap()
    .0
}

/// Every stable model inside a region lies inside its narrowing, and a
/// refuted region holds none: checked over every region of a small theory.
fn narrowing_keeps_every_stable_model(theory: &Theory) {
    let atoms = theory.atom_count();
    let models = stable_models(theory);
    let regions = 3usize.pow(u32::try_from(atoms).unwrap());
    for code in 0..regions {
        let mut region = Region::undecided(theory.atom_count());
        let mut digits = code;
        for atom in 0..atoms {
            match digits % 3 {
                1 => assert!(region.hold(atom)),
                2 => assert!(region.cut(atom)),
                _ => {}
            }
            digits /= 3;
        }
        let inside: Vec<usize> = models
            .iter()
            .copied()
            .filter(|mask| {
                (0..atoms).all(|a| {
                    let present = mask & (1 << a) != 0;
                    (present || !region.is_held(a)) && (!present || !region.is_cut(a))
                })
            })
            .collect();
        match narrowed(theory, &mut region) {
            Narrowing::Refuted => {
                assert!(inside.is_empty(), "region {code} refuted with {inside:?}");
            }
            Narrowing::Fixed { .. } => {
                for mask in inside {
                    for a in 0..atoms {
                        let present = mask & (1 << a) != 0;
                        assert!(
                            present || !region.is_held(a),
                            "region {code} holds {a} against {mask:b}"
                        );
                        assert!(
                            !present || !region.is_cut(a),
                            "region {code} cuts {a} against {mask:b}"
                        );
                    }
                }
            }
        }
    }
}

/// d. p | q :- d.  r :- not p.  :- q, r.
fn disjunctive() -> Theory {
    let (d, p, q, r) = (0, 1, 2, 3);
    let nodes = vec![
        Node::Atom(d),       // 0
        Node::Atom(p),       // 1
        Node::Atom(q),       // 2
        Node::Or(1, 2),      // 3: p | q
        Node::Implies(0, 3), // 4: d -> p | q
        Node::False,         // 5
        Node::Implies(1, 5), // 6: not p
        Node::Atom(r),       // 7
        Node::Implies(6, 7), // 8: not p -> r
        Node::And(2, 7),     // 9: q & r
        Node::Implies(9, 5), // 10: :- q, r
    ];
    theory(4, nodes, vec![0, 4, 8, 10])
}

#[test]
fn an_impossible_root_refutes_the_region() {
    let t = theory(1, vec![Node::Atom(0)], vec![0]);
    let mut cut = region(&t, &[], &[0]);
    assert!(matches!(narrowed(&t, &mut cut), Narrowing::Refuted));
    let mut open = region(&t, &[], &[]);
    assert!(matches!(
        narrowed(&t, &mut open),
        Narrowing::Fixed { changed: true }
    ));
    assert!(open.is_held(0), "the fact is forced");
}

/// d. p :- d. :- p, s.
fn derived_head_under_a_constraint() -> Theory {
    let nodes = vec![
        Node::Atom(0),       // d
        Node::Atom(1),       // p
        Node::Implies(0, 1), // d -> p
        Node::Atom(2),       // s
        Node::And(1, 3),     // p & s
        Node::False,
        Node::Implies(4, 5), // :- p, s
    ];
    theory(3, nodes, vec![0, 2, 6])
}

#[test]
fn a_sure_body_forces_its_head() {
    let t = derived_head_under_a_constraint();
    let mut open = region(&t, &[], &[]);
    assert!(matches!(
        narrowed(&t, &mut open),
        Narrowing::Fixed { changed: true }
    ));
    assert!(open.is_held(0) && open.is_held(1));
}

#[test]
fn a_constraint_with_one_open_atom_cuts_it() {
    // With p forced, the constraint's only open atom is s.
    let t = derived_head_under_a_constraint();
    let mut open = region(&t, &[], &[]);
    narrowed(&t, &mut open);
    assert!(open.is_cut(2));
}

#[test]
fn a_constraint_whose_body_is_sure_refutes_the_region() {
    // With s held, p is forced and the constraint fires.
    let t = derived_head_under_a_constraint();
    let mut held = region(&t, &[2], &[]);
    assert!(matches!(narrowed(&t, &mut held), Narrowing::Refuted));
}

#[test]
fn a_unit_root_decides_its_one_open_atom() {
    // a | b with a cut forces b; a held decides nothing more about b by the
    // readings alone, so the support rule, which would cut b, is withheld.
    let nodes = vec![Node::Atom(0), Node::Atom(1), Node::Or(0, 1)];
    let t = theory(2, nodes, vec![2]);
    let by_readings = |region: &mut Region| {
        narrow_fresh(
            &t,
            None,
            region,
            RegionLimits::default(),
            &Control::default(),
        )
        .unwrap()
        .0
    };
    let mut cut = region(&t, &[], &[0]);
    assert!(matches!(
        by_readings(&mut cut),
        Narrowing::Fixed { changed: true }
    ));
    assert!(cut.is_held(1));
    let mut held = region(&t, &[0], &[]);
    assert!(matches!(
        by_readings(&mut held),
        Narrowing::Fixed { changed: false }
    ));
    assert!(held.is_open(1));
}

#[test]
fn an_atom_no_producer_can_support_is_cut() {
    let t = disjunctive();
    // p held: q's only producer has another head held, so q is cut, and
    // then r is forced by nothing, since not p is impossible: r is cut too.
    let mut held = region(&t, &[1], &[]);
    assert!(matches!(
        narrowed(&t, &mut held),
        Narrowing::Fixed { changed: true }
    ));
    assert!(held.is_cut(2), "q is unsupported");
    assert!(held.is_cut(3), "r's only producer has an impossible body");
    // p cut: q is forced by the unit disjunction, r by its sure body, and
    // the constraint :- q, r then refutes the region.
    let mut cut = region(&t, &[], &[1]);
    assert!(matches!(narrowed(&t, &mut cut), Narrowing::Refuted));
}

#[test]
fn a_choice_is_supported_by_its_body_alone() {
    // b.  {a} :- b, written a | not a <- b.
    let nodes = vec![
        Node::Atom(0), // b
        Node::Atom(1), // a
        Node::False,
        Node::Implies(1, 2), // not a
        Node::Or(1, 3),      // a | not a
        Node::Implies(0, 4), // b -> (a | not a)
    ];
    let t = theory(2, nodes, vec![0, 5]);
    let mut open = region(&t, &[], &[]);
    narrowed(&t, &mut open);
    assert!(open.is_held(0) && open.is_open(1));
    let mut cut = region(&t, &[], &[0]);
    assert!(
        matches!(narrowed(&t, &mut cut), Narrowing::Refuted),
        "b is a fact"
    );
}

#[test]
fn outside_the_producer_fragment_the_readings_still_narrow() {
    // p | (q & r): no producer shape, so no support cut; with q cut the
    // root is a unit on p and forces it.
    let nodes = vec![
        Node::Atom(0),
        Node::Atom(1),
        Node::Atom(2),
        Node::And(1, 2),
        Node::Or(0, 3),
    ];
    let t = theory(3, nodes, vec![4]);
    assert!(
        producers(&t, RegionLimits::default(), &Control::default())
            .unwrap()
            .producers
            .is_none()
    );
    let mut cut = region(&t, &[], &[1]);
    narrowed(&t, &mut cut);
    assert!(cut.is_held(0));
}

#[test]
fn narrowing_keeps_every_stable_model_of_every_region() {
    narrowing_keeps_every_stable_model(&disjunctive());
    // p :- not q. q :- not p. r :- p, not s. s :- t.
    let nodes = vec![
        Node::Atom(0),        // p
        Node::Atom(1),        // q
        Node::False,          // 2
        Node::Implies(1, 2),  // 3: not q
        Node::Implies(3, 0),  // 4: not q -> p
        Node::Implies(0, 2),  // 5: not p
        Node::Implies(5, 1),  // 6: not p -> q
        Node::Atom(2),        // 7: r
        Node::Atom(3),        // 8: s
        Node::Implies(8, 2),  // 9: not s
        Node::And(0, 9),      // 10: p & not s
        Node::Implies(10, 7), // 11: -> r
        Node::Atom(4),        // 12: t
        Node::Implies(12, 8), // 13: t -> s
    ];
    narrowing_keeps_every_stable_model(&theory(5, nodes, vec![4, 6, 11, 13]));
}

#[test]
fn the_leaves_of_the_region_tree_are_the_stable_models() {
    fn leaves(theory: &Theory, mut region: Region, found: &mut Vec<usize>) {
        if matches!(narrowed(theory, &mut region), Narrowing::Refuted) {
            return;
        }
        match region.highest_open() {
            None => {
                let mask: usize = (0..theory.atom_count())
                    .filter(|&a| region.is_held(a))
                    .map(|a| 1 << a)
                    .sum();
                let candidate = Interpretation::new(
                    theory,
                    (0..theory.atom_count()).filter(|a| mask & (1 << a) != 0),
                )
                .unwrap();
                if check(theory, &candidate, Limits::default(), &Control::default())
                    .unwrap()
                    .accepted()
                {
                    found.push(mask);
                }
            }
            Some(atom) => {
                let (cut, held) = region.split(atom);
                leaves(theory, cut, found);
                leaves(theory, held, found);
            }
        }
    }
    let t = disjunctive();
    let mut found = Vec::new();
    leaves(&t, Region::undecided(t.atom_count()), &mut found);
    assert_eq!(
        found.iter().copied().collect::<BTreeSet<_>>(),
        stable_models(&t)
    );
    assert_eq!(found.len(), stable_models(&t).len(), "each answer once");
}

#[test]
fn a_held_atom_no_producer_can_support_refutes_the_region() {
    // d.  p | q <- d.  r <- not p.  :- q, r.  With r held: q is cut by the
    // constraint, p is forced by the disjunction, and r's only producer then
    // has an impossible body, so the held r refutes the region.
    let nodes = vec![
        Node::Atom(0),       // d
        Node::Atom(1),       // p
        Node::Atom(2),       // q
        Node::Atom(3),       // r
        Node::Or(1, 2),      // p | q
        Node::Implies(0, 4), // d -> p | q
        Node::False,         // 6
        Node::Implies(1, 6), // not p
        Node::Implies(7, 3), // not p -> r
        Node::And(2, 3),     // q & r
        Node::Implies(9, 6), // :- q, r
    ];
    let t = theory(4, nodes, vec![0, 5, 8, 10]);
    let mut held = region(&t, &[3], &[]);
    assert!(matches!(narrowed(&t, &mut held), Narrowing::Refuted));
    narrowing_keeps_every_stable_model(&t);
}

#[test]
fn a_disjunction_whose_one_side_is_impossible_holds_the_other_sides_atoms() {
    // (a & b) | (c & d) with a cut: the left side is impossible, so the
    // disjunction known to hold forces the right side, a conjunction known
    // to hold, which holds both its atoms.
    let nodes = vec![
        Node::Atom(0),
        Node::Atom(1),
        Node::Atom(2),
        Node::Atom(3),
        Node::And(0, 1),
        Node::And(2, 3),
        Node::Or(4, 5),
    ];
    let t = theory(4, nodes, vec![6]);
    let mut cut = region(&t, &[], &[0]);
    assert!(matches!(
        narrowed(&t, &mut cut),
        Narrowing::Fixed { changed: true }
    ));
    assert!(cut.is_held(2) && cut.is_held(3));
    assert!(
        cut.is_open(1),
        "the impossible side decides nothing about its other atom"
    );
    narrowing_keeps_every_stable_model(&t);
}

#[test]
fn a_failing_consequent_teaches_the_antecedent_to_fail() {
    // (a & b) -> c with c cut and a held: b is cut. Producers are withheld,
    // since a has none and the support cut would refute the region first.
    let nodes = vec![
        Node::Atom(0),
        Node::Atom(1),
        Node::Atom(2),
        Node::And(0, 1),
        Node::Implies(3, 2),
    ];
    let t = theory(3, nodes, vec![4]);
    let mut region = region(&t, &[0], &[2]);
    let (outcome, _) = narrow_fresh(
        &t,
        None,
        &mut region,
        RegionLimits::default(),
        &Control::default(),
    )
    .unwrap();
    assert!(matches!(outcome, Narrowing::Fixed { changed: true }));
    assert!(region.is_cut(1));
    narrowing_keeps_every_stable_model(&t);
}

#[test]
fn a_held_atom_with_one_producer_left_demands_its_body() {
    // {a}. {b}. c :- a. c :- b, d.  With c held and a cut, the only producer
    // left for c is the second rule, so b and d are held.
    let nodes = vec![
        Node::Atom(0),        // a
        Node::Atom(1),        // b
        Node::Atom(2),        // c
        Node::Atom(3),        // d
        Node::False,          // 4
        Node::Implies(0, 4),  // not a
        Node::Or(0, 5),       // a | not a
        Node::Implies(1, 4),  // not b
        Node::Or(1, 7),       // b | not b
        Node::Implies(0, 2),  // a -> c
        Node::And(1, 3),      // b & d
        Node::Implies(10, 2), // b & d -> c
        Node::Implies(3, 4),  // not d
        Node::Or(3, 12),      // d | not d
    ];
    let t = theory(4, nodes, vec![6, 8, 9, 11, 13]);
    let mut region = region(&t, &[2], &[0]);
    assert!(matches!(
        narrowed(&t, &mut region),
        Narrowing::Fixed { changed: true }
    ));
    assert!(region.is_held(1) && region.is_held(3));
    narrowing_keeps_every_stable_model(&t);
}

/// Walking the tree with knowledge carried from parent to child decides
/// every region exactly as a fresh narrowing of that region does, and
/// reaches the same leaves: the knowledge of a region holds in every
/// region inside it.
#[test]
fn a_clause_of_three_literals_forces_its_last_open_one() {
    // a | b | c, admitted as a chain of two disjunctions; with a and b cut
    // the chain's one open operand is forced, and with all three cut the
    // region is refuted, through the chain rules (`disj_chain_unit`,
    // `disj_chain_never`) rather than a walk of the chain.
    let nodes = vec![
        Node::Atom(0),
        Node::Atom(1),
        Node::Atom(2),
        Node::Or(1, 2),
        Node::Or(0, 3),
    ];
    let t = theory(3, nodes, vec![4]);
    let by_readings = |region: &mut Region| {
        narrow_fresh(
            &t,
            None,
            region,
            RegionLimits::default(),
            &Control::default(),
        )
        .unwrap()
        .0
    };
    let mut two_cut = region(&t, &[], &[0, 1]);
    assert!(matches!(
        by_readings(&mut two_cut),
        Narrowing::Fixed { changed: true }
    ));
    assert!(two_cut.is_held(2));
    let mut one_cut = region(&t, &[], &[1]);
    assert!(matches!(
        by_readings(&mut one_cut),
        Narrowing::Fixed { changed: false }
    ));
    assert!(one_cut.is_open(0) && one_cut.is_open(2));
    let mut all_cut = region(&t, &[], &[0, 1, 2]);
    assert!(matches!(by_readings(&mut all_cut), Narrowing::Refuted));
}

#[test]
fn a_node_reached_on_both_sides_of_a_chain_is_one_operand() {
    // a | (a | b): a is an operand of the outer disjunction and of the inner
    // one it absorbs, one node of the DAG, so the chain has the operands
    // {a, b} and fails when both fail; counted twice it never would.
    let nodes = vec![Node::Atom(0), Node::Atom(1), Node::Or(0, 1), Node::Or(0, 2)];
    let t = theory(2, nodes, vec![3]);
    let mut both_cut = region(&t, &[], &[0, 1]);
    assert!(matches!(
        narrow_fresh(
            &t,
            None,
            &mut both_cut,
            RegionLimits::default(),
            &Control::default()
        )
        .unwrap()
        .0,
        Narrowing::Refuted
    ));
    let mut a_cut = region(&t, &[], &[0]);
    assert!(matches!(
        narrow_fresh(
            &t,
            None,
            &mut a_cut,
            RegionLimits::default(),
            &Control::default()
        )
        .unwrap()
        .0,
        Narrowing::Fixed { changed: true }
    ));
    assert!(a_cut.is_held(1));
}

#[test]
fn a_subformula_shared_by_two_parents_serves_both_as_one_operand() {
    // (a | b) is an operand of both a | b | c and of the constraint
    // :- (a | b), d, so it is a chain of its own rather than absorbed: with
    // c cut the first root forces a | b, which then cuts d.
    let nodes = vec![
        Node::Atom(0),       // a
        Node::Atom(1),       // b
        Node::Or(0, 1),      // 2: a | b, shared
        Node::Atom(2),       // c
        Node::Or(2, 3),      // 4: (a | b) | c
        Node::Atom(3),       // d
        Node::And(2, 5),     // 6: (a | b) & d
        Node::False,         // 7
        Node::Implies(6, 7), // 8: :- (a | b), d
    ];
    let t = theory(4, nodes, vec![4, 8]);
    let mut cut_c = region(&t, &[], &[2]);
    assert!(matches!(
        narrow_fresh(
            &t,
            None,
            &mut cut_c,
            RegionLimits::default(),
            &Control::default()
        )
        .unwrap()
        .0,
        Narrowing::Fixed { changed: true }
    ));
    assert!(cut_c.is_cut(3), "the shared disjunction holds, so d is cut");
    assert!(cut_c.is_open(0) && cut_c.is_open(1));
    narrowing_keeps_every_stable_model(&t);
}

#[test]
fn a_frozen_mask_on_a_chain_node_reads_as_its_operands_masks() {
    // Under the candidate {c}, the inner disjunction b | c of a | (b | c)
    // holds, and under {a} it fails together with both its operands: the
    // frozen reading of the chain is the reading of its operands' masks,
    // so the subsets of {a} keep a forced and those of {c} keep c forced.
    let nodes = vec![
        Node::Atom(0),
        Node::Atom(1),
        Node::Atom(2),
        Node::Or(1, 2),
        Node::Or(0, 3),
    ];
    let t = theory(3, nodes, vec![4]);
    let narrower = Narrower::new(&t);
    for (candidate, forced) in [(vec![0], 0), (vec![2], 2)] {
        let interpretation = Interpretation::new(&t, candidate.iter().copied()).unwrap();
        let mut workspace = zetesis_ferraris::EvaluationWorkspace::default();
        let attempt = workspace.evaluate(
            &interpretation,
            zetesis_ferraris::EvaluationLimits::default(),
            &Control::default(),
        );
        let evaluation = attempt.result.unwrap();
        let truth = evaluation.truth();
        let mut subsets = Region::undecided(3);
        for atom in (0..3).filter(|atom| !candidate.contains(atom)) {
            subsets.cut(atom);
        }
        let (narrowing, _) = narrower
            .narrow_frozen_known(
                &t,
                truth,
                &mut subsets,
                &mut narrower.knowledge(),
                RegionLimits::default(),
                &Control::default(),
            )
            .unwrap();
        assert!(matches!(narrowing, Narrowing::Fixed { changed: true }));
        assert!(subsets.is_held(forced), "candidate {candidate:?}");
    }
}

#[test]
fn carried_knowledge_narrows_every_region_as_a_fresh_narrowing_does() {
    for t in [disjunctive(), {
        // p :- not q. q :- not p. r :- p, not s. s :- t. {t}.
        let nodes = vec![
            Node::Atom(0),        // p
            Node::Atom(1),        // q
            Node::False,          // 2
            Node::Implies(1, 2),  // not q
            Node::Implies(3, 0),  // not q -> p
            Node::Implies(0, 2),  // not p
            Node::Implies(5, 1),  // not p -> q
            Node::Atom(2),        // r
            Node::Atom(3),        // s
            Node::Implies(8, 2),  // not s
            Node::And(0, 9),      // p & not s
            Node::Implies(10, 7), // -> r
            Node::Atom(4),        // t
            Node::Implies(12, 8), // t -> s
            Node::Implies(12, 2), // not t
            Node::Or(12, 14),     // t | not t
        ];
        theory(5, nodes, vec![4, 6, 11, 13, 15])
    }] {
        let narrower = Narrower::new(&t);
        let extracted = producers(&t, RegionLimits::default(), &Control::default()).unwrap();
        let producers = extracted.producers.as_ref();
        let mut stack = vec![(Region::undecided(t.atom_count()), narrower.knowledge())];
        let mut leaves = 0;
        while let Some((mut carried, mut knowledge)) = stack.pop() {
            let mut fresh = carried.clone();
            let (from_fresh, _) = narrower
                .narrow_known(
                    &t,
                    producers,
                    &mut fresh,
                    &mut narrower.knowledge(),
                    RegionLimits::default(),
                    &Control::default(),
                )
                .unwrap();
            let (from_carried, _) = narrower
                .narrow_known(
                    &t,
                    producers,
                    &mut carried,
                    &mut knowledge,
                    RegionLimits::default(),
                    &Control::default(),
                )
                .unwrap();
            assert_eq!(
                matches!(from_fresh, Narrowing::Refuted),
                matches!(from_carried, Narrowing::Refuted)
            );
            if matches!(from_fresh, Narrowing::Refuted) {
                continue;
            }
            for atom in 0..t.atom_count() {
                assert_eq!(fresh.decision(atom), carried.decision(atom), "atom {atom}");
            }
            match carried.highest_open() {
                None => leaves += 1,
                Some(atom) => {
                    let (cut, held) = carried.split(atom);
                    stack.push((held, knowledge.clone()));
                    stack.push((cut, knowledge));
                }
            }
        }
        assert_eq!(leaves, stable_models(&t).len());
    }
}

#[test]
fn holding_an_atom_without_producers_rechecks_no_support() {
    // One root atom, narrowed without producers: the root is learned and
    // revisited, which holds its atom. That is the one propagation; without
    // producers there is no support to recheck, so none is queued.
    let t = theory(1, vec![Node::Atom(0)], vec![0]);
    let mut region = Region::undecided(1);
    let (narrowing, statistics) = narrow_fresh(
        &t,
        None,
        &mut region,
        RegionLimits::default(),
        &Control::default(),
    )
    .unwrap();
    assert!(matches!(narrowing, Narrowing::Fixed { changed: true }));
    assert!(region.is_held(0));
    assert_eq!(statistics.propagations, 1);
}

#[test]
fn an_implication_from_an_atom_to_itself_is_one_parent_of_the_atom() {
    // a -> a: the implication is one parent of a's node, counted once in
    // the split ranking and taken off once when the node is revisited; the
    // root holds and decides nothing, so a stays the atom to split on.
    let t = theory(1, vec![Node::Atom(0), Node::Implies(0, 0)], vec![1]);
    let narrower = Narrower::new(&t);
    let mut knowledge = narrower.knowledge();
    let mut region = Region::undecided(1);
    let (narrowing, _) = narrower
        .narrow_known(
            &t,
            None,
            &mut region,
            &mut knowledge,
            RegionLimits::default(),
            &Control::default(),
        )
        .unwrap();
    assert!(matches!(narrowing, Narrowing::Fixed { changed: false }));
    assert_eq!(region.split_atom(), Some(0));
}

#[test]
fn every_propagation_event_is_charged_work() {
    // The disjunctive theory narrows by several propagations, each a node
    // revisited or a support rechecked, and each reads at least one node:
    // the work spent bounds the events, so a work ceiling below the events
    // stops the narrowing with the work stop. The work ceiling is the one
    // ceiling a narrowing needs.
    let t = disjunctive();
    let extracted = producers(&t, RegionLimits::default(), &Control::default()).unwrap();
    let mut region = Region::undecided(4);
    let (_, statistics) = narrow_fresh(
        &t,
        extracted.producers.as_ref(),
        &mut region,
        RegionLimits::default(),
        &Control::default(),
    )
    .unwrap();
    assert!(statistics.propagations > 1);
    assert!(statistics.propagations <= statistics.work);
    let mut region = Region::undecided(4);
    let stopped = narrow_fresh(
        &t,
        extracted.producers.as_ref(),
        &mut region,
        RegionLimits {
            max_work: statistics.propagations - 1,
        },
        &Control::default(),
    );
    assert!(matches!(stopped, Err(Stop::WorkLimit)));
}
