//! Sound source restrictions skip impossible seed intervals without grounding the carrier.
use std::collections::BTreeSet;
use zetesis_core::{AdmissionLimits, Atom, AtomPattern, Predicate, Program, Template, Term, Value};
use zetesis_cpu::{
    CandidateLimits, CandidateRestrictionLimits, CandidateTermination, Candidates, Control, Limits,
    Stop, check,
};

fn atom(name: &str, terms: Vec<Term>) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, terms.len()).unwrap(), terms).unwrap()
}
fn number(n: i32) -> Term {
    Term::Constant(Value::Number(n))
}
fn fact(name: &str, terms: Vec<Term>) -> Template {
    Template::new(Some(atom(name, terms)), vec![], vec![], vec![], vec![])
}
fn choice(name: &str, terms: Vec<Term>) -> Template {
    let head = atom(name, terms);
    Template::new(Some(head.clone()), vec![], vec![head], vec![], vec![])
}
fn program(templates: Vec<Template>) -> Program {
    Program::new(templates, AdmissionLimits::default()).unwrap()
}
fn restricted(program: &Program) -> Candidates<'_> {
    Candidates::restricted(
        program,
        CandidateLimits::default(),
        CandidateRestrictionLimits::default(),
        Control::default(),
    )
}
fn path(size: i32) -> Program {
    let mut templates: Vec<_> = (0..size).map(|n| choice("in", vec![number(n)])).collect();
    templates.extend((1..size).map(|n| fact("edge", vec![number(n - 1), number(n)])));
    templates.push(Template::new(
        None,
        vec![
            atom("edge", vec![Term::Variable(0), Term::Variable(1)]),
            atom("in", vec![Term::Variable(0)]),
            atom("in", vec![Term::Variable(1)]),
        ],
        vec![],
        vec![],
        vec![],
    ));
    program(templates)
}
fn answer_sets(program: &Program, candidates: Candidates<'_>) -> BTreeSet<Vec<Atom>> {
    candidates
        .map(|seed| {
            check(
                program,
                &seed.unwrap(),
                Limits::default(),
                &Control::default(),
            )
            .unwrap()
        })
        .filter(zetesis_cpu::Check::accepted)
        .map(|checked| checked.closure().atoms().iter().cloned().collect())
        .collect()
}

#[test]
fn path_constraints_reduce_eight_node_seeds_to_fifty_five() {
    let source = path(8);
    let mut candidates = restricted(&source);
    let models: Vec<_> = candidates
        .by_ref()
        .map(|seed| {
            let checked = check(
                &source,
                &seed.unwrap(),
                Limits::default(),
                &Control::default(),
            )
            .unwrap();
            assert!(checked.accepted());
            checked.closure().clone()
        })
        .collect();
    assert_eq!(models.len(), 55);
    assert_eq!(
        candidates.termination(),
        Some(CandidateTermination::Exhausted)
    );
    assert!(candidates.statistics().conflicts > 0);
    assert_eq!(
        Candidates::new(&source, CandidateLimits::default(), Control::default()).count(),
        256
    );
}

#[test]
fn restricted_path_enumeration_preserves_every_answer_set() {
    for size in 1..9 {
        let source = path(size);
        let complete = answer_sets(
            &source,
            Candidates::new(&source, CandidateLimits::default(), Control::default()),
        );
        assert_eq!(
            answer_sets(&source, restricted(&source)),
            complete,
            "path size {size}"
        );
    }
}

#[test]
fn candidate_bound_counts_only_returned_seeds() {
    let source = path(8);
    let mut candidates = Candidates::restricted(
        &source,
        CandidateLimits {
            max_candidates: 55,
            ..CandidateLimits::default()
        },
        CandidateRestrictionLimits::default(),
        Control::default(),
    );
    assert_eq!(candidates.by_ref().map(Result::unwrap).count(), 55);
    assert_eq!(
        candidates.termination(),
        Some(CandidateTermination::Exhausted)
    );
}

#[test]
fn restriction_work_stop_does_not_claim_exhaustion() {
    let source = path(3);
    let mut candidates = Candidates::restricted(
        &source,
        CandidateLimits::default(),
        CandidateRestrictionLimits {
            max_work: 1,
            ..CandidateRestrictionLimits::default()
        },
        Control::default(),
    );
    assert!(matches!(candidates.next(), Some(Err(Stop::WorkLimit))));
    assert_eq!(candidates.statistics().restriction_work, 1);
    assert_eq!(
        candidates.termination(),
        Some(CandidateTermination::Stopped(Stop::WorkLimit))
    );
    assert!(candidates.next().is_none());
}

#[test]
fn restrictions_leave_an_unexpanded_carrier_for_the_empty_seed() {
    let source = path(8);
    let mut candidates = restricted(&source);
    assert!(candidates.next().unwrap().unwrap().atoms().is_empty());
    assert_eq!(candidates.discovered_atoms(), 0);
}

#[test]
fn candidate_bound_zero_is_not_unsatisfiability() {
    let source = path(3);
    let mut candidates = Candidates::restricted(
        &source,
        CandidateLimits {
            max_candidates: 0,
            ..CandidateLimits::default()
        },
        CandidateRestrictionLimits::default(),
        Control::default(),
    );
    assert!(matches!(candidates.next(), Some(Err(Stop::CandidateLimit))));
    assert_eq!(
        candidates.termination(),
        Some(CandidateTermination::Stopped(Stop::CandidateLimit))
    );
}

#[test]
fn possible_support_is_not_an_unconditional_fact() {
    let source = program(vec![
        choice("p", vec![]),
        choice("q", vec![]),
        Template::new(
            Some(atom("edge", vec![])),
            vec![atom("p", vec![])],
            vec![],
            vec![],
            vec![],
        ),
        Template::new(
            None,
            vec![atom("edge", vec![]), atom("q", vec![])],
            vec![],
            vec![],
            vec![],
        ),
    ]);
    assert_eq!(restricted(&source).count(), 4);
    assert_eq!(
        answer_sets(&source, restricted(&source)),
        answer_sets(
            &source,
            Candidates::new(&source, CandidateLimits::default(), Control::default())
        )
    );
}

#[test]
fn negative_gates_remain_for_the_original_oracle() {
    let source = program(vec![
        choice("p", vec![]),
        choice("q", vec![]),
        Template::new(
            None,
            vec![atom("p", vec![])],
            vec![],
            vec![atom("q", vec![])],
            vec![],
        ),
    ]);
    assert_eq!(restricted(&source).count(), 4);
}

#[test]
fn unbound_fact_side_declines_the_constraint() {
    let source = program(vec![
        choice("p", vec![number(0)]),
        choice("p", vec![number(1)]),
        Template::new(
            None,
            vec![atom("p", vec![Term::Variable(0)])],
            vec![],
            vec![],
            vec![],
        ),
    ]);
    assert_eq!(restricted(&source).count(), 4);
}

#[test]
fn repeated_nullary_premises_name_one_forbidden_atom() {
    let source = program(vec![
        choice("p", vec![]),
        Template::new(
            None,
            vec![atom("p", vec![]), atom("p", vec![])],
            vec![],
            vec![],
            vec![],
        ),
    ]);
    let seeds: Vec<_> = restricted(&source).map(Result::unwrap).collect();
    assert_eq!(seeds.len(), 1);
    assert!(seeds[0].atoms().is_empty());
}

#[test]
fn every_small_positive_constraint_family_keeps_exact_seed_order() {
    for family in 0..128 {
        let mut templates: Vec<_> = (0..3).map(|n| choice("p", vec![number(n)])).collect();
        for conjunction in 1..8 {
            if family & (1 << (conjunction - 1)) != 0 {
                let positive = (0..3)
                    .filter(|n| conjunction & (1 << n) != 0)
                    .map(|n| atom("p", vec![number(n)]))
                    .collect();
                templates.push(Template::new(None, positive, vec![], vec![], vec![]));
            }
        }
        let source = program(templates);
        let actual: Vec<_> = restricted(&source)
            .map(|seed| seed.unwrap().atoms().clone())
            .collect();
        let expected: Vec<_> =
            Candidates::new(&source, CandidateLimits::default(), Control::default())
                .map(Result::unwrap)
                .filter(|seed| {
                    check(&source, seed, Limits::default(), &Control::default())
                        .unwrap()
                        .accepted()
                })
                .map(|seed| seed.atoms().clone())
                .collect();
        assert_eq!(actual, expected, "constraint family {family}");
    }
}

#[test]
fn unconditional_constraints_exhaust_without_a_candidate() {
    let source = program(vec![
        choice("p", vec![]),
        fact("present", vec![]),
        Template::new(None, vec![atom("present", vec![])], vec![], vec![], vec![]),
    ]);
    let mut candidates = restricted(&source);
    assert!(candidates.next().is_none());
    assert_eq!(
        candidates.termination(),
        Some(CandidateTermination::Exhausted)
    );
}

#[test]
fn signed_gate_predicates_remain_distinct() {
    use zetesis_core::Sign;
    let signed = AtomPattern::new(
        Predicate::with_sign("p", 0, Sign::Negative).unwrap(),
        vec![],
    )
    .unwrap();
    let source = program(vec![
        choice("p", vec![]),
        Template::new(
            Some(signed.clone()),
            vec![],
            vec![signed.clone()],
            vec![],
            vec![],
        ),
        Template::new(None, vec![signed], vec![], vec![], vec![]),
    ]);
    let seeds: Vec<_> = restricted(&source).map(Result::unwrap).collect();
    assert_eq!(seeds.len(), 2);
    let nonempty = seeds[1].atoms().iter().next().unwrap();
    assert_eq!(nonempty.predicate().sign(), Sign::Positive);
}

#[test]
fn typed_gate_values_do_not_coerce() {
    let numeric = number(1);
    let textual = Term::Constant(Value::String("1".into()));
    let source = program(vec![
        choice("p", vec![numeric.clone()]),
        choice("p", vec![textual]),
        Template::new(None, vec![atom("p", vec![numeric])], vec![], vec![], vec![]),
    ]);
    let seeds: Vec<_> = restricted(&source).map(Result::unwrap).collect();
    assert_eq!(seeds.len(), 2);
    assert_eq!(
        seeds[1].atoms().iter().next().unwrap().values(),
        [Value::String("1".into())]
    );
}

#[test]
fn facts_of_gate_predicates_do_not_change_seed_identity() {
    let source = program(vec![
        fact("p", vec![]),
        choice("p", vec![]),
        Template::new(None, vec![atom("p", vec![])], vec![], vec![], vec![]),
    ]);
    let seeds: Vec<_> = restricted(&source).map(Result::unwrap).collect();
    assert_eq!(seeds.len(), 1);
    assert!(seeds[0].atoms().is_empty());
    assert!(
        !check(&source, &seeds[0], Limits::default(), &Control::default())
            .unwrap()
            .accepted()
    );
}

#[test]
fn cancellation_after_preparation_retains_incomplete_coverage() {
    let source = path(4);
    let control = Control::default();
    let mut candidates = Candidates::restricted(
        &source,
        CandidateLimits::default(),
        CandidateRestrictionLimits::default(),
        control.clone(),
    );
    candidates.next().unwrap().unwrap();
    let work = candidates.statistics().restriction_work;
    control.cancel();
    assert!(matches!(candidates.next(), Some(Err(Stop::Cancelled))));
    assert_eq!(candidates.statistics().restriction_work, work);
    assert_eq!(
        candidates.termination(),
        Some(CandidateTermination::Stopped(Stop::Cancelled))
    );
}

#[test]
fn restriction_payload_limits_are_real_ceilings() {
    let source = path(3);
    for limits in [
        CandidateRestrictionLimits {
            max_atoms: 0,
            ..CandidateRestrictionLimits::default()
        },
        CandidateRestrictionLimits {
            max_bytes: 0,
            ..CandidateRestrictionLimits::default()
        },
    ] {
        let mut candidates = Candidates::restricted(
            &source,
            CandidateLimits::default(),
            limits,
            Control::default(),
        );
        assert!(candidates.next().unwrap().is_err());
        assert!(matches!(
            candidates.termination(),
            Some(CandidateTermination::Stopped(_))
        ));
    }
}

#[test]
fn twenty_node_path_completes_with_the_default_restriction_budget() {
    let source = path(20);
    let mut candidates = restricted(&source);
    let count = candidates.by_ref().map(Result::unwrap).count();
    assert_eq!(count, 17_711);
    assert_eq!(
        candidates.termination(),
        Some(CandidateTermination::Exhausted)
    );
    eprintln!(
        "path20 candidates={count} restrictions={:?}",
        candidates.statistics()
    );
}
