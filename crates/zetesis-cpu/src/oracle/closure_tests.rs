//! Completion and gate equality are distinct obligations of normal acceptance.

use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, Model, Predicate, Program, Seed, Template, Term, Value,
};

use super::{Cancellation, Limits, Statistics, Stop, Work, gate_agreement, least_closure};

fn atom(name: &str) -> Atom {
    Atom::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap()
}

fn pattern(name: &str) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap()
}

fn work(cancellation: &Cancellation, max_work: u64) -> Work<'_> {
    Work {
        cancellation,
        limits: Limits {
            max_work,
            ..Limits::default()
        },
        statistics: Statistics::default(),
        mask_words: 0,
        pruned_prefixes: 0,
        mask_bytes: 0,
    }
}

fn choices() -> Program {
    Program::new(
        ["a", "b"]
            .map(|name| {
                let head = pattern(name);
                Template::new(Some(head.clone()), vec![], vec![head], vec![], vec![])
            })
            .into(),
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn join_bindings_borrow_their_source_values() {
    let predicate = Predicate::new("row", 1).unwrap();
    let mut rows = [
        Value::String("first".into()),
        Value::Symbol("second".into()),
    ]
    .map(|value| Atom::new(predicate.clone(), vec![value]).unwrap());
    rows.sort();
    let pattern = AtomPattern::new(predicate.clone(), vec![Term::Variable(0)]).unwrap();
    let template = Template::new(None, vec![pattern], vec![], vec![], vec![]);
    let relations = super::Relations::from([(&predicate, rows.iter().collect())]);
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation, Limits::default().max_work);
    let mut visited = 0;
    super::visit(
        &template,
        &relations,
        super::Gates::Unjudged,
        None,
        &mut work,
        |assignment, _| -> Result<(), Stop> {
            let bound = assignment[0].expect("the positive row binds its variable");
            let original = &rows[visited].values()[0];
            assert!(std::ptr::eq(bound, original));
            visited += 1;
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(visited, rows.len());
}

#[test]
fn gate_agreement_requires_every_derived_gate_atom() {
    let program = choices();
    let seed = Seed::new(&program, []).unwrap();
    let closure = Model::new([atom("a")]);
    let cancellation = Cancellation::default();
    // Two gate predicates and one derived gate row: three units.
    assert!(
        !gate_agreement(
            &program,
            seed.view(),
            closure.atoms(),
            &mut work(&cancellation, 3)
        )
        .unwrap()
    );
}

#[test]
fn gate_agreement_requires_every_seed_atom() {
    let program = choices();
    let seed = Seed::new(&program, [atom("a")]).unwrap();
    let cancellation = Cancellation::default();
    // Two gate predicates and one seed atom left unmatched: three units.
    assert!(
        !gate_agreement(
            &program,
            seed.view(),
            Model::default().atoms(),
            &mut work(&cancellation, 3)
        )
        .unwrap()
    );
}

#[test]
fn gate_agreement_ignores_positive_only_atoms() {
    let program = Program::new(
        vec![Template::new(
            Some(pattern("a")),
            vec![],
            vec![],
            vec![],
            vec![],
        )],
        AdmissionLimits::default(),
    )
    .unwrap();
    let seed = Seed::new(&program, []).unwrap();
    let closure = Model::new([atom("a")]);
    let cancellation = Cancellation::default();
    assert!(
        gate_agreement(
            &program,
            seed.view(),
            closure.atoms(),
            &mut work(&cancellation, 1)
        )
        .unwrap()
    );
}

#[test]
fn gate_mismatch_does_not_truncate_charged_scans() {
    let program = choices();
    let seed = Seed::new(&program, [atom("a")]).unwrap();
    let closure = Model::new([atom("b")]);
    let cancellation = Cancellation::default();
    assert_eq!(
        gate_agreement(
            &program,
            seed.view(),
            closure.atoms(),
            &mut work(&cancellation, 1)
        ),
        Err(Stop::WorkLimit)
    );
    // One unit per gate predicate (a, b), one for the closure's row b, and
    // one for the seed's a, which the walk passes without a match.
    let mut exact = work(&cancellation, 4);
    assert!(!gate_agreement(&program, seed.view(), closure.atoms(), &mut exact).unwrap());
    assert_eq!(exact.statistics.work, 4);
}

fn chain(constraint: Template) -> Program {
    Program::new(
        vec![
            constraint,
            Template::new(Some(pattern("a")), vec![], vec![], vec![], vec![]),
            Template::new(
                Some(pattern("b")),
                vec![pattern("a")],
                vec![],
                vec![],
                vec![],
            ),
        ],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn completed_closure_checks_final_round_constraints() {
    let program = chain(Template::new(
        None,
        vec![pattern("b")],
        vec![],
        vec![],
        vec![],
    ));
    let seed = Seed::new(&program, []).unwrap();
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation, Limits::default().max_work);
    let completed = least_closure(&program, seed.view(), &mut work).unwrap();
    // b is derived in round two; this constraint first holds in the final scan.
    assert!(completed.constraint_violated);
    assert_eq!(work.statistics.rounds, 3);
}

#[test]
fn violated_constraints_preserve_complete_closure() {
    let program = chain(Template::new(None, vec![], vec![], vec![], vec![]));
    let seed = Seed::new(&program, []).unwrap();
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation, Limits::default().max_work);
    let completed = least_closure(&program, seed.view(), &mut work).unwrap();
    assert!(completed.constraint_violated);
    assert_eq!(completed.atoms, Model::new([atom("a"), atom("b")]));
    assert_eq!(work.statistics.rounds, 3);
}

#[test]
fn growing_closure_stops_before_exceeding_atom_limit() {
    let program = chain(Template::new(None, vec![], vec![], vec![], vec![]));
    let seed = Seed::new(&program, []).unwrap();
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation, Limits::default().max_work);
    work.limits.max_derived_atoms = 1;
    assert!(matches!(
        least_closure(&program, seed.view(), &mut work),
        Err(Stop::DerivedAtomLimit)
    ));
    assert_eq!(work.statistics.rounds, 1);
}

fn capacity_program() -> (Program, Model) {
    let p = Predicate::new("p", 1).unwrap();
    let q = Predicate::new("q", 1).unwrap();
    let values = [
        Value::String("payload".into()),
        Value::Symbol("payload".into()),
    ];
    let mut templates = values
        .iter()
        .map(|value| {
            Template::new(
                Some(AtomPattern::new(p.clone(), vec![Term::Constant(value.clone())]).unwrap()),
                vec![],
                vec![],
                vec![],
                vec![],
            )
        })
        .collect::<Vec<_>>();
    templates.push(Template::new(
        Some(AtomPattern::new(q.clone(), vec![Term::Variable(0)]).unwrap()),
        vec![AtomPattern::new(p.clone(), vec![Term::Variable(0)]).unwrap()],
        vec![],
        vec![],
        vec![],
    ));
    let expected = Model::new([p, q].into_iter().flat_map(|predicate| {
        values
            .clone()
            .into_iter()
            .map(move |value| Atom::new(predicate.clone(), vec![value]).unwrap())
    }));
    (
        Program::new(templates, AdmissionLimits::default()).unwrap(),
        expected,
    )
}

#[test]
fn closure_capacity_admits_the_complete_boundary() {
    let (program, expected) = capacity_program();
    let seed = Seed::new(&program, []).unwrap();
    let cancellation = Cancellation::default();
    let reference = super::check(&program, &seed, Limits::default(), &cancellation).unwrap();
    assert_eq!(reference.closure(), &expected);
    assert!(reference.accepted());
    assert!(reference.statistics().catalog_work > 0);
    let peak = reference.statistics().peak_closure_bytes;
    assert!(peak > 0);
    let exact = super::check(
        &program,
        &seed,
        Limits {
            max_closure_bytes: peak,
            ..Limits::default()
        },
        &cancellation,
    )
    .unwrap();
    assert_eq!(exact.closure(), &expected);
    assert!(exact.accepted());
    assert_eq!(exact.statistics(), reference.statistics());
    let Err(stop) = super::check(
        &program,
        &seed,
        Limits {
            max_closure_bytes: peak - 1,
            ..Limits::default()
        },
        &cancellation,
    ) else {
        panic!("the complete capacity envelope must be admitted");
    };
    assert_eq!(stop, Stop::StorageLimit);
}

#[test]
fn refused_capacity_does_not_report_a_completed_closure() {
    let (program, _) = capacity_program();
    let seed = Seed::new(&program, []).unwrap();
    let cancellation = Cancellation::default();
    let mut bounded = work(&cancellation, Limits::default().max_work);
    bounded.limits.max_closure_bytes = 0;
    let Err(stop) = least_closure(&program, seed.view(), &mut bounded) else {
        panic!("zero capacity cannot admit the named closure owner");
    };
    assert_eq!(stop, Stop::StorageLimit);
    assert_eq!(bounded.statistics.peak_closure_bytes, 0);
    assert_eq!(bounded.statistics.derived_atoms, 0);
}

#[test]
fn tuple_probes_include_whole_row_rejections() {
    let predicate = Predicate::new("pair", 2).unwrap();
    let rows = [[1, 2], [2, 2]]
        .map(|values| Atom::new(predicate.clone(), values.map(Value::Number).to_vec()).unwrap());
    let template = Template::new(
        None,
        vec![
            AtomPattern::new(
                predicate.clone(),
                vec![Term::Variable(0), Term::Variable(0)],
            )
            .unwrap(),
        ],
        vec![],
        vec![],
        vec![],
    );
    let relations = super::Relations::from([(&predicate, rows.iter().collect())]);
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation, Limits::default().max_work);
    let mut bound = Vec::new();
    super::visit(
        &template,
        &relations,
        super::Gates::Unjudged,
        None,
        &mut work,
        |assignment, _| -> Result<(), Stop> {
            bound.push(assignment[0].unwrap().clone());
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(bound, [Value::Number(2)]);
    assert_eq!(work.statistics.tuple_probes, 2);
    assert_eq!(work.statistics.bindings, 1);
    assert!(work.statistics.tuple_probes <= work.statistics.work);
}

#[test]
fn a_join_that_judges_no_gates_charges_no_gate_work() {
    // A rule with one positive occurrence and one gate. Under the possible
    // reading of the undecided cube every gate passes, but each binding
    // still builds and tests the gate's key; unjudged, the join charges the
    // binding alone and offers the same bindings.
    let predicate = Predicate::new("row", 1).unwrap();
    let rows: Vec<Atom> = (0..4)
        .map(|value| Atom::new(predicate.clone(), vec![Value::Number(value)]).unwrap())
        .collect();
    let pattern = AtomPattern::new(predicate.clone(), vec![Term::Variable(0)]).unwrap();
    let gate =
        AtomPattern::new(Predicate::new("gate", 1).unwrap(), vec![Term::Variable(0)]).unwrap();
    let template = Template::new(None, vec![pattern], vec![gate], vec![], vec![]);
    let relations = super::Relations::from([(&predicate, rows.iter().collect())]);
    let cancellation = Cancellation::default();
    let charged = |gates: super::Gates<'_>| {
        let mut work = work(&cancellation, Limits::default().max_work);
        let mut bindings = 0;
        super::visit(
            &template,
            &relations,
            gates,
            None,
            &mut work,
            |_, _| -> Result<(), Stop> {
                bindings += 1;
                Ok(())
            },
        )
        .unwrap();
        (bindings, work.statistics.work)
    };
    let undecided = super::Cube::all_open();
    let (judged, judged_work) = charged(super::Gates::Possible((&undecided).into()));
    let (unjudged, unjudged_work) = charged(super::Gates::Unjudged);
    assert_eq!((judged, unjudged), (4, 4));
    assert!(
        unjudged_work < judged_work,
        "{unjudged_work} against {judged_work}"
    );
}
