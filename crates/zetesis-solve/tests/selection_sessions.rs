//! Ordinary relational sessions preserve typed families across candidate ownership.

use std::{collections::BTreeSet, convert::Infallible, num::NonZeroUsize};

use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, Predicate, Program, Sign, Template, Term, Value,
    ValueLimits, ValueNode,
};
use zetesis_cpu::{Control, Stop};
use zetesis_solve::{
    Backend, Completion, ExecutionObservation, ExecutionObserver, Grounder, Interruption,
    PreparedInput, SearchState, SemanticOutcome, Session, SolveConfig,
};

const PERMITTED_CANDIDATES: u64 = 192;
const ANSWERS: usize = 24;
const CARRIER_ATOMS: usize = 8;

type Family = BTreeSet<Vec<Atom>>;

fn predicate(name: &str, arity: usize, sign: Sign) -> Predicate {
    Predicate::with_sign(name, arity, sign).unwrap()
}

fn pattern(name: &str, sign: Sign, terms: Vec<Term>) -> AtomPattern {
    AtomPattern::new(predicate(name, terms.len(), sign), terms).unwrap()
}

fn atom(name: &str, sign: Sign, values: Vec<Value>) -> Atom {
    Atom::new(predicate(name, values.len(), sign), values).unwrap()
}

struct Fixture {
    program: Program,
    values: Vec<Value>,
}

impl Fixture {
    fn new() -> Self {
        let text = "item".repeat(128);
        let values = vec![
            Value::Number(1),
            Value::String(text.clone()),
            Value::Symbol(text),
            Value::from_nodes(
                vec![
                    ValueNode::Function {
                        name: "f".into(),
                        arity: 1,
                        sign: Sign::Positive,
                    },
                    ValueNode::Number(1),
                ],
                ValueLimits::default(),
            )
            .unwrap(),
        ];
        let positive =
            |value: &Value| pattern("p", Sign::Positive, vec![Term::Constant(value.clone())]);
        let negative = pattern("p", Sign::Negative, vec![Term::Constant(values[0].clone())]);
        let mut rules: Vec<_> = values
            .iter()
            .map(positive)
            .chain([negative.clone()])
            .map(|head| Template::new(Some(head.clone()), vec![], vec![head], vec![], vec![]))
            .collect();
        rules.push(Template::new(
            Some(pattern("anchor", Sign::Positive, vec![])),
            vec![],
            vec![],
            vec![],
            vec![],
        ));
        rules.push(Template::new(
            None,
            vec![positive(&values[0]), negative],
            vec![],
            vec![],
            vec![],
        ));
        for sign in [Sign::Positive, Sign::Negative] {
            for (head, body) in [("echo", "p"), ("derived", "echo"), ("echo", "derived")] {
                rules.push(Template::new(
                    Some(pattern(head, sign, vec![Term::Variable(0)])),
                    vec![pattern(body, sign, vec![Term::Variable(0)])],
                    vec![],
                    vec![],
                    vec![],
                ));
            }
        }
        let program = Program::new(rules, AdmissionLimits::default()).unwrap();
        Self { program, values }
    }

    fn interpretation(&self, selected: &[(Sign, usize)]) -> Vec<Atom> {
        let mut atoms = BTreeSet::from([atom("anchor", Sign::Positive, vec![])]);
        for &(sign, index) in selected {
            for name in ["p", "echo", "derived"] {
                atoms.insert(atom(name, sign, vec![self.values[index].clone()]));
            }
        }
        atoms.into_iter().collect()
    }

    fn family(&self) -> Family {
        // p(1) and -p(1) have three coherent states. The remaining three
        // positive typed atoms are independent choices: 3 * 2^3 = 24 answers.
        let mut family = Family::new();
        for number in [None, Some(Sign::Positive), Some(Sign::Negative)] {
            for mask in 0..8 {
                let mut selected: Vec<_> = number.into_iter().map(|sign| (sign, 0)).collect();
                for index in 1..4 {
                    if mask & (1 << (index - 1)) != 0 {
                        selected.push((Sign::Positive, index));
                    }
                }
                family.insert(self.interpretation(&selected));
            }
        }
        family
    }
}

#[derive(Default)]
struct Route(Vec<(Grounder, NonZeroUsize)>);

impl ExecutionObserver for Route {
    type Error = Infallible;

    fn observe(&mut self, event: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        if let ExecutionObservation::CpuClosure {
            grounder, workers, ..
        } = event
        {
            self.0.push((grounder, workers));
        }
        Ok(())
    }
}

fn run(fixture: &Fixture, config: SolveConfig) -> (Vec<Vec<Atom>>, SemanticOutcome) {
    let mut route = Route::default();
    let mut session = Session::builder(
        PreparedInput::program(&fixture.program),
        config,
        Control::default(),
    )
    .start_observed(&mut route)
    .unwrap();
    let mut answers = Vec::new();
    while let Some(answer) = session.next_observed(&mut route) {
        answers.push(
            answer
                .unwrap()
                .interpretation()
                .atoms()
                .iter()
                .cloned()
                .collect(),
        );
    }
    assert_eq!(route.0, [(config.grounder, config.workers)]);
    assert!(session.next().is_none());
    (answers, session.outcome().unwrap())
}

fn configurations() -> impl Iterator<Item = SolveConfig> {
    [Grounder::Lazy, Grounder::Eager]
        .into_iter()
        .flat_map(|grounder| {
            [1, 4].into_iter().map(move |workers| SolveConfig {
                backend: Backend::Cpu,
                grounder,
                models: 0,
                workers: NonZeroUsize::new(workers).unwrap(),
                batch_size: NonZeroUsize::new(7).unwrap(),
                max_candidates: PERMITTED_CANDIDATES,
                max_carrier_atoms: CARRIER_ATOMS,
                ..Default::default()
            })
        })
}

#[test]
fn sessions_preserve_the_complete_typed_family() {
    let fixture = Fixture::new();
    let expected = fixture.family();
    assert_eq!(expected.len(), ANSWERS);
    // Four complete typed values under two signed gate predicates give eight
    // carrier atoms. Only the p(1)/-p(1) pair is forbidden, so 3 * 2^6 = 192
    // proposals remain. Unsupported negative typed atoms still require rejection.
    assert_eq!(fixture.program.domain().len(), 4);
    assert_eq!(fixture.program.gate_predicates().len(), 2);
    for config in configurations() {
        let (answers, outcome) = run(&fixture, config);
        assert_eq!(answers.len(), ANSWERS);
        assert_eq!(answers.into_iter().collect::<Family>(), expected);
        assert_eq!(outcome.completion(), Some(Completion::Exhausted));
        assert_eq!(outcome.interruption(), None);
        assert_eq!(outcome.candidate_progress(), PERMITTED_CANDIDATES);
        assert_eq!(outcome.discovered_gate_atoms(), CARRIER_ATOMS);
        assert_eq!(outcome.verified_models(), u64::try_from(ANSWERS).unwrap());
    }
}

#[test]
fn candidate_limits_preserve_the_verified_prefix() {
    let fixture = Fixture::new();
    let expected = vec![
        fixture.interpretation(&[]),
        fixture.interpretation(&[(Sign::Positive, 0)]),
        fixture.interpretation(&[(Sign::Positive, 1)]),
        fixture.interpretation(&[(Sign::Positive, 0), (Sign::Positive, 1)]),
    ];
    // Four permitted candidates stop inside an irregular seven-slot batch.
    // Their four answers must survive the deferred candidate-limit outcome.
    for config in configurations() {
        let (answers, outcome) = run(
            &fixture,
            SolveConfig {
                max_candidates: 4,
                ..config
            },
        );
        assert_eq!(answers, expected);
        assert_eq!(outcome.completion(), Some(Completion::Interrupted));
        assert_eq!(
            outcome.interruption(),
            Some(Interruption::Oracle(Stop::CandidateLimit))
        );
        assert_eq!(outcome.candidate_progress(), 4);
        assert_eq!(outcome.verified_models(), 4);
    }
}

#[test]
fn pending_candidate_stop_preserves_the_checked_answers_before_finalization() {
    let fixture = Fixture::new();
    let reason = Interruption::Oracle(Stop::CandidateLimit);
    for config in configurations() {
        let mut session = Session::builder(
            PreparedInput::program(&fixture.program),
            SolveConfig {
                max_candidates: 4,
                ..config
            },
            Control::default(),
        )
        .start()
        .unwrap();
        assert_eq!(session.progress().search_state(), None);
        assert!(session.next().unwrap().is_ok());
        assert_eq!(session.progress().search_state(), None);
        for consumed in 2..=4 {
            assert!(session.next().unwrap().is_ok());
            let progress = session.progress();
            // The second batch verified three answers and hit its candidate
            // bound. Its stop must not erase any checked answer in that batch.
            assert_eq!(progress.verified_models(), 4);
            assert_eq!(progress.candidate_progress(), consumed);
            assert_eq!(
                progress.search_state(),
                Some(SearchState::PendingInterruption(reason))
            );
            assert_eq!(progress.completion(), None);
            assert_eq!(progress.interruption(), Some(reason));
            assert!(!progress.unsatisfiable());
        }
        assert!(session.next().is_none());
        let outcome = session.outcome().unwrap();
        assert_eq!(
            outcome.search_state(),
            Some(SearchState::Interrupted(reason))
        );
        assert_eq!(outcome.completion(), Some(Completion::Interrupted));
        assert_eq!(outcome.interruption(), Some(reason));
        assert_eq!(outcome.verified_models(), 4);
    }
}
