//! World-view questions folded over answer sets agree with the complete family,
//! stop at absorption, and never turn an unfinished enumeration into a value.

use std::{collections::BTreeSet, num::NonZeroUsize};

use zetesis_algebra::Cancellation;
use zetesis_core::{Atom, Model, Predicate, Sign};
use zetesis_reference_support::{canonical, formula, prepared};
use zetesis_solve::{
    AnswerSelection, Backend, Completion, Grounder, Oracle, PreparedInput, ProjectionLimits,
    QueryAnswer, Session, SessionBuilder, SolveConfig, WorldView, WorldViewDecision,
    WorldViewDecisionError, WorldViewLimits, WorldViewVerdict,
};
use zetesis_themelios::{AdmittedFormula, FormulaMaterialization, TerminalFormula};

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        workers: NonZeroUsize::MIN,
        ..Default::default()
    }
}

fn request(owner: &AdmittedFormula) -> SessionBuilder<'_> {
    Session::builder(
        PreparedInput::formula(owner),
        config(),
        Cancellation::default(),
    )
}

fn literal(name: &str, sign: Sign) -> Atom {
    Atom::new(Predicate::with_sign(name, 0, sign).unwrap(), vec![]).unwrap()
}

fn positive(name: &str) -> Atom {
    literal(name, Sign::Positive)
}

/// Atoms as clingo spells them, so that signs keep full identity.
fn spelled(model: &Model) -> BTreeSet<String> {
    model.atoms().iter().map(canonical).collect()
}

fn names<const N: usize>(atoms: [&str; N]) -> BTreeSet<String> {
    atoms.into_iter().map(str::to_owned).collect()
}

/// The complete family, as the native collection returns it.
fn family(owner: &AdmittedFormula) -> Vec<BTreeSet<String>> {
    WorldView::collect(
        PreparedInput::formula(owner),
        config(),
        WorldViewLimits::default(),
        Cancellation::default(),
    )
    .unwrap()
    .answer_sets()
    .iter()
    .map(|answer| spelled(answer.interpretation()))
    .collect()
}

/// Programs with nonempty and empty families, with and without strong
/// negation, over the atoms the questions below name. Under its objective the
/// last program's default request would select the optimum {a} alone.
const PROGRAMS: [&str; 9] = [
    "{a;b}.",
    "a. {b}.",
    "-c. {b}.",
    "a :- not b. b :- not a.",
    "a :- not b. b :- not a. -a :- b.",
    "1 {a;b} 1.",
    "",
    ":-.",
    "1 {a;b} 1. #minimize {1@2,a:a; 2@2,b:b}.",
];

/// The value a decision established, after checking that a decided verdict
/// retains no more witnesses than its reduction allows.
fn value<V>(decision: &WorldViewDecision<V>, witness_bound: usize) -> Option<&V> {
    if let WorldViewVerdict::Decided { witnesses, .. } = decision.verdict() {
        assert!(witnesses.len() <= witness_bound, "{witnesses:?}");
    }
    decision.verdict().value()
}

#[test]
fn known_agrees_with_the_complete_family() {
    for source in PROGRAMS {
        let owner = formula(source);
        let members = family(&owner);
        for name in ["a", "b", "c"] {
            let decision = request(&owner).known(&positive(name)).unwrap();
            let expected = (!members.is_empty()).then(|| members.iter().all(|m| m.contains(name)));
            assert_eq!(value(&decision, 1).copied(), expected, "{source}: {name}");
        }
    }
}

#[test]
fn possible_agrees_with_the_complete_family() {
    for source in PROGRAMS {
        let owner = formula(source);
        let members = family(&owner);
        for name in ["a", "b", "c"] {
            let decision = request(&owner).possible(&positive(name)).unwrap();
            let expected = (!members.is_empty()).then(|| members.iter().any(|m| m.contains(name)));
            assert_eq!(value(&decision, 1).copied(), expected, "{source}: {name}");
        }
    }
}

#[test]
fn cautious_consequences_agree_with_the_complete_family() {
    for source in PROGRAMS {
        let owner = formula(source);
        let members = family(&owner);
        let decision = request(&owner).cautious().unwrap();
        let expected = members
            .split_first()
            .map(|(first, rest)| rest.iter().fold(first.clone(), |both, next| &both & next));
        assert_eq!(value(&decision, 0).map(spelled), expected, "{source}");
    }
}

#[test]
fn brave_consequences_agree_with_the_complete_family() {
    for source in PROGRAMS {
        let owner = formula(source);
        let members = family(&owner);
        let decision = request(&owner).brave().unwrap();
        let expected = (!members.is_empty()).then(|| {
            members
                .iter()
                .fold(BTreeSet::new(), |either, next| &either | next)
        });
        assert_eq!(value(&decision, 0).map(spelled), expected, "{source}");
    }
}

#[test]
fn three_valued_answers_agree_with_the_complete_family() {
    let queries = [
        ("a", Sign::Positive),
        ("a", Sign::Negative),
        ("c", Sign::Positive),
        ("c", Sign::Negative),
    ];
    for source in PROGRAMS {
        let owner = formula(source);
        let members = family(&owner);
        for (name, sign) in queries {
            let query = literal(name, sign);
            let (present, contrary) = (canonical(&query), canonical(&query.contrary()));
            let decision = request(&owner).answer(&query).unwrap();
            let expected = (!members.is_empty()).then(|| {
                if members.iter().all(|m| m.contains(&present)) {
                    QueryAnswer::Yes
                } else if members.iter().all(|m| m.contains(&contrary)) {
                    QueryAnswer::No
                } else {
                    QueryAnswer::Unknown
                }
            });
            assert_eq!(
                value(&decision, 2).copied(),
                expected,
                "{source}: {present}"
            );
        }
    }
}

#[test]
fn membership_witnesses_are_answer_sets_that_settle_the_literal() {
    // A known literal is refuted by an answer set without it, and a possible
    // literal is established by one with it.
    let mut witnessed = 0;
    for source in PROGRAMS {
        let owner = formula(source);
        for name in ["a", "b", "c"] {
            let atom = positive(name);
            for (decision, holds) in [
                (request(&owner).known(&atom).unwrap(), false),
                (request(&owner).possible(&atom).unwrap(), true),
            ] {
                if let WorldViewVerdict::Decided { witnesses, .. } = decision.verdict() {
                    let [witness] = witnesses.as_slice() else {
                        panic!("{source}: {name}: {witnesses:?}");
                    };
                    assert_eq!(witness.interpretation().contains(&atom), holds);
                    assert!(witness.original().same_instance(decision.original()));
                    witnessed += 1;
                }
            }
        }
    }
    assert!(witnessed > 0);
}

#[test]
fn an_absorbed_fold_stops_the_enumeration_early() {
    // Every one of the 1,024 answer sets contains x, so the first decides it.
    let owner = formula("x. {a;b;c;d;e;f;g;h;i;j}.");
    let decision = request(&owner).possible(&positive("x")).unwrap();
    assert!(matches!(
        decision.verdict(),
        WorldViewVerdict::Decided { value: true, .. }
    ));
    assert!(decision.outcome().verified_models() < 1_024);
    assert_eq!(decision.outcome().completion(), None);
}

#[test]
fn one_answer_set_where_the_literal_is_unknown_decides_unknown() {
    let owner = formula("{a}.");
    let (b, contrary) = (positive("b"), literal("b", Sign::Negative));
    let decision = request(&owner).answer(&b).unwrap();
    let WorldViewVerdict::Decided {
        value: QueryAnswer::Unknown,
        witnesses,
    } = decision.verdict()
    else {
        panic!("{:?}", decision.verdict());
    };
    let [witness] = witnesses.as_slice() else {
        panic!("{witnesses:?}");
    };
    assert!(!witness.interpretation().contains(&b));
    assert!(!witness.interpretation().contains(&contrary));
}

#[test]
fn true_and_false_answer_sets_together_decide_unknown() {
    // {a} holds the literal and {b, -a} its contrary.
    let owner = formula("a :- not b. b :- not a. -a :- b.");
    let (a, contrary) = (positive("a"), literal("a", Sign::Negative));
    let decision = request(&owner).answer(&a).unwrap();
    let WorldViewVerdict::Decided {
        value: QueryAnswer::Unknown,
        witnesses,
    } = decision.verdict()
    else {
        panic!("{:?}", decision.verdict());
    };
    assert_eq!(witnesses.len(), 2);
    assert!(witnesses.iter().any(|w| w.interpretation().contains(&a)));
    assert!(
        witnesses
            .iter()
            .any(|w| w.interpretation().contains(&contrary))
    );
}

#[test]
fn an_empty_intersection_decides_cautious_consequences_without_witnesses() {
    let owner = formula("1 {a;b} 1.");
    let decision = request(&owner).cautious().unwrap();
    let WorldViewVerdict::Decided { value, witnesses } = decision.verdict() else {
        panic!("{:?}", decision.verdict());
    };
    assert!(value.atoms().is_empty());
    assert!(witnesses.is_empty());
}

#[test]
fn brave_consequences_are_decided_only_by_exhaustion() {
    // The union reaches every atom of {a;b} at the second answer set, which
    // still does not decide it: the session bounds no atom universe.
    let owner = formula("{a;b}.");
    let decision = request(&owner).brave().unwrap();
    assert!(matches!(decision.verdict(), WorldViewVerdict::Complete(_)));
    assert_eq!(decision.outcome().completion(), Some(Completion::Exhausted));
    assert_eq!(decision.outcome().verified_models(), 4);
}

#[test]
fn an_inconsistent_program_has_no_fold_value() {
    let owner = formula(":-.");
    let a = positive("a");
    let verdicts = [
        request(&owner)
            .known(&a)
            .unwrap()
            .verdict()
            .value()
            .is_none(),
        request(&owner)
            .possible(&a)
            .unwrap()
            .verdict()
            .value()
            .is_none(),
        request(&owner)
            .cautious()
            .unwrap()
            .verdict()
            .value()
            .is_none(),
        request(&owner).brave().unwrap().verdict().value().is_none(),
        request(&owner)
            .answer(&a)
            .unwrap()
            .verdict()
            .value()
            .is_none(),
    ];
    assert_eq!(verdicts, [true; 5]);
    let decision = request(&owner).cautious().unwrap();
    assert!(matches!(decision.verdict(), WorldViewVerdict::Inconsistent));
    assert!(decision.outcome().unsatisfiable());
}

#[test]
fn an_objective_does_not_narrow_the_folded_family() {
    // The optimum is {a}; the family also holds the worse {b}.
    let owner = formula("1 {a;b} 1. #minimize {1@2,a:a; 2@2,b:b}.");
    let decision = request(&owner)
        .selection(AnswerSelection::Optimal)
        .brave()
        .unwrap();
    assert_eq!(
        decision.verdict().value().map(spelled),
        Some(names(["a", "b"]))
    );
}

#[test]
fn a_requested_model_count_does_not_narrow_the_folded_family() {
    let owner = formula("{a;b}.");
    let decision = Session::builder(
        PreparedInput::formula(&owner),
        SolveConfig {
            models: 1,
            ..config()
        },
        Cancellation::default(),
    )
    .brave()
    .unwrap();
    assert_eq!(
        decision.verdict().value().map(spelled),
        Some(names(["a", "b"]))
    );
}

#[test]
fn a_projection_request_does_not_narrow_the_folded_family() {
    // Projected on p, the family {}, {p,q}, {p,r} keeps one of the last two.
    let owner = formula("{p}. 1 {q;r} 1 :- p. #project p/0.");
    let decision = request(&owner)
        .projected(ProjectionLimits::default())
        .brave()
        .unwrap();
    assert_eq!(
        decision.verdict().value().map(spelled),
        Some(names(["p", "q", "r"]))
    );
}

#[test]
fn a_cancelled_enumeration_is_not_an_inconsistency() {
    // Cancelled before it starts, the session folds no answer set; only
    // exhaustion could make that an empty family.
    let owner = formula("a.");
    let cancellation = Cancellation::default();
    let pending = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        cancellation.clone(),
    );
    cancellation.cancel();
    let failure = pending.cautious().unwrap_err();
    assert!(matches!(
        failure.cause(),
        WorldViewDecisionError::NotExhausted
    ));
    let outcome = failure.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert!(!outcome.unsatisfiable());
}

#[test]
fn a_search_limit_before_absorption_is_not_a_verdict() {
    // c is in no answer set, so its possibility is never absorbed, and one
    // checked answer set cannot establish that it is impossible.
    let owner = formula("{a;b}.");
    let failure = Session::builder(
        PreparedInput::formula(&owner),
        SolveConfig {
            max_candidates: 1,
            ..config()
        },
        Cancellation::default(),
    )
    .possible(&positive("c"))
    .unwrap_err();
    assert!(matches!(
        failure.cause(),
        WorldViewDecisionError::NotExhausted
    ));
    assert_eq!(
        failure.outcome().unwrap().completion(),
        Some(Completion::Interrupted)
    );
}

/// Each answer set of this program extends a core answer with terminal
/// definitions that lazy grounding reconstructs for it.
const RECONSTRUCTED: &str = "{a; b}. d(1..2). t(X) :- a, d(X). u(X) :- b, d(X).";

fn terminal(source: &str) -> TerminalFormula {
    let FormulaMaterialization::Terminal(owner) = prepared(source).ground_lazy().unwrap() else {
        panic!("{source}: no certified terminal partition");
    };
    owner
}

fn lazy(owner: &TerminalFormula) -> SessionBuilder<'_> {
    Session::builder(
        PreparedInput::terminal(owner),
        SolveConfig {
            grounder: Grounder::Lazy,
            oracle: Oracle::Auto,
            completion_workers: NonZeroUsize::MIN,
            ..config()
        },
        Cancellation::default(),
    )
}

/// The complete family, after checking that no two consecutive answer sets
/// share an atom catalog, so each fold step combines distinct catalogs.
fn reconstructed_family(owner: &TerminalFormula) -> Vec<BTreeSet<String>> {
    let world_view = lazy(owner).collect(WorldViewLimits::default()).unwrap();
    let answers = world_view.answer_sets();
    assert!(answers.windows(2).all(|pair| {
        !pair[0]
            .interpretation()
            .catalog()
            .same_owner(pair[1].interpretation().catalog())
    }));
    answers
        .iter()
        .map(|answer| spelled(answer.interpretation()))
        .collect()
}

#[test]
fn cautious_consequences_agree_across_answer_catalogs() {
    let owner = terminal(RECONSTRUCTED);
    let members = reconstructed_family(&owner);
    let expected = members
        .split_first()
        .map(|(first, rest)| rest.iter().fold(first.clone(), |both, next| &both & next));
    let decision = lazy(&owner).cautious().unwrap();
    assert_eq!(decision.verdict().value().map(spelled), expected);
    assert_eq!(expected, Some(names(["d(1)", "d(2)"])));
}

#[test]
fn brave_consequences_agree_across_answer_catalogs() {
    let owner = terminal(RECONSTRUCTED);
    let members = reconstructed_family(&owner);
    let expected = members
        .iter()
        .fold(BTreeSet::new(), |either, next| &either | next);
    let decision = lazy(&owner).brave().unwrap();
    assert_eq!(decision.verdict().value().map(spelled), Some(expected));
    assert_eq!(members.len(), 4);
}
