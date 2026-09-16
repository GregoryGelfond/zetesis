//! An incremental round visits its new rows first, so unchanged relations are
//! entered only through bound-prefix windows and never scanned in full.

use zetesis_core::{AdmissionLimits, AtomPattern, Predicate, Program, Seed, Template, Term, Value};
use zetesis_cpu::{Control, Limits, check};

/// `e(0,1). … e(n-1,n). r(0). r(Y) :- r(X), e(X,Y).`: one derivation per round
/// for `n` rounds, with `e` unchanged after the first.
fn chain(length: i32) -> Program {
    let edge = Predicate::new("e", 2).unwrap();
    let reach = Predicate::new("r", 1).unwrap();
    let fact = |atom: AtomPattern| Template::new(Some(atom), vec![], vec![], vec![], vec![]);
    let number = |value| Term::Constant(Value::Number(value));
    let mut templates: Vec<_> = (0..length)
        .map(|index| {
            fact(AtomPattern::new(edge.clone(), vec![number(index), number(index + 1)]).unwrap())
        })
        .collect();
    templates.push(fact(
        AtomPattern::new(reach.clone(), vec![number(0)]).unwrap(),
    ));
    templates.push(Template::new(
        Some(AtomPattern::new(reach.clone(), vec![Term::Variable(1)]).unwrap()),
        vec![
            AtomPattern::new(reach, vec![Term::Variable(0)]).unwrap(),
            AtomPattern::new(edge, vec![Term::Variable(0), Term::Variable(1)]).unwrap(),
        ],
        vec![],
        vec![],
        vec![],
    ));
    Program::new(templates, AdmissionLimits::default()).unwrap()
}

#[test]
fn a_chain_round_probes_the_new_row_and_its_window_only() {
    let length = 256;
    let program = chain(length);
    let seed = Seed::new(&program, []).unwrap();
    let check = check(&program, &seed, Limits::default(), &Control::default()).unwrap();
    assert!(check.accepted());
    let statistics = check.statistics();
    assert_eq!(
        statistics.derived_atoms,
        usize::try_from(2 * length + 1).unwrap()
    );
    // Each round offers the round's one new `r` row and the one `e` row its
    // window selects; the first round offers every `e` row once.
    let rounds = statistics.rounds;
    assert!(rounds >= u64::try_from(length).unwrap());
    let ceiling = u64::try_from(length).unwrap() + 3 * rounds;
    assert!(
        statistics.tuple_probes <= ceiling,
        "probes {} exceed {ceiling}: an unchanged relation was scanned in full",
        statistics.tuple_probes
    );
}
