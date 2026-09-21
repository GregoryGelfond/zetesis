//! Streamed constraints qualify the full source before an answer is returned.

use std::{collections::BTreeSet, num::NonZeroUsize};

use zetesis_core::{Atom, Predicate, Sign, Value};
use zetesis_cpu::Control;
use zetesis_solve::{
    Backend, Completion, Grounder, Interruption, Oracle, PreparedInput, SemanticOutcome, Session,
    SolveConfig, SolveError, Subject,
};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ConstraintCheckCause, ConstraintCheckLimits,
    ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource, HybridFormula, admit_formula,
    prepare_formula,
};

const MONOTONE: &str = include_str!("fixtures/hybrid/monotone.lp");
const CORE: &str = "d(1..6). p(X)|q(X):-d(X).";
const ROOT_CEILING: usize = 32;
type Family = BTreeSet<Vec<Atom>>;

fn configuration() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        grounder: Grounder::Auto,
        oracle: Oracle::Countermodel,
        models: 0,
        workers: NonZeroUsize::MIN,
        completion_workers: NonZeroUsize::MIN,
        batch_size: NonZeroUsize::MIN,
        max_candidates: 256,
        max_search_work: 10_000_000,
        max_work: 1_000_000,
        ..Default::default()
    }
}

fn limits() -> FormulaLimits {
    FormulaLimits {
        max_work: 500_000,
        max_support_bytes: 1_048_576,
        theory: zetesis_ferraris::AdmissionLimits {
            max_atoms: 256,
            max_nodes: 4_096,
            max_roots: ROOT_CEILING,
        },
        ..Default::default()
    }
}

fn expansion() -> ExpansionLimits {
    ExpansionLimits {
        max_scalar_bytes: 1_048_576,
        ..Default::default()
    }
}

fn eager(source: &str, limits: &FormulaLimits) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        expansion(),
        *limits,
    )
}

fn hybrid(source: &str, limits: &FormulaLimits) -> HybridFormula {
    prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        expansion(),
        *limits,
    )
    .unwrap()
    .ground_hybrid()
    .unwrap()
}

fn capture(input: PreparedInput<'_>, config: SolveConfig) -> (Family, SemanticOutcome) {
    let mut session = Session::builder(input, config, Control::default())
        .start()
        .unwrap();
    let mut family = Family::new();
    for answer in session.by_ref() {
        let answer = answer.unwrap();
        let mut atoms: Vec<_> = answer.interpretation().atoms().iter().cloned().collect();
        atoms.sort();
        assert!(family.insert(atoms), "a source answer was returned twice");
    }
    assert!(session.next().is_none());
    (family, session.outcome().unwrap())
}

fn atom(name: &str, sign: Sign, values: Vec<Value>) -> Atom {
    Atom::new(
        Predicate::with_sign(name, values.len(), sign).unwrap(),
        values,
    )
    .unwrap()
}

fn number(name: &str, value: i32) -> Atom {
    atom(name, Sign::Positive, vec![Value::Number(value)])
}

fn monotone_family() -> Family {
    // Each position chooses p or q. A p before a later q violates the source
    // constraint, so the complete family is exactly the seven q-prefix cuts.
    (0..=6)
        .map(|cut| {
            let mut atoms: Vec<_> = (1..=6).map(|value| number("d", value)).collect();
            atoms.extend((1..=6).map(|value| number(if value <= cut { "q" } else { "p" }, value)));
            atoms.sort();
            atoms
        })
        .collect()
}

#[test]
fn hybrid_admits_the_retained_root_ceiling() {
    let core = eager(CORE, &limits()).unwrap();
    assert_eq!(
        core.theory().roots().len(),
        30,
        "producer and support roots remain real"
    );
    let failure = eager(MONOTONE, &limits()).unwrap_err();
    assert!(matches!(
        failure,
        FormulaFailure::Limit {
            resource: FormulaResource::Roots,
            limit: 32,
            observed: 33,
            ..
        }
    ));
    // Work, scalar payload, support bytes, atom and node ceilings are unchanged.
    // This is a named retained-root claim, not a combined memory or RSS claim.
    let admitted = hybrid(MONOTONE, &limits());
    assert_eq!(
        admitted.core_theory().roots().len(),
        core.theory().roots().len()
    );
    let (family, outcome) = capture(PreparedInput::hybrid(&admitted), configuration());
    assert_eq!(family, monotone_family());
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
}

#[test]
fn objective_free_hybrid_preserves_selection_policy() {
    let admitted = hybrid("a|b.", &limits());
    for selection in [
        zetesis_solve::AnswerSelection::All,
        zetesis_solve::AnswerSelection::Optimal,
    ] {
        let mut session = Session::builder(
            PreparedInput::hybrid(&admitted),
            configuration(),
            Control::default(),
        )
        .selection(selection)
        .start()
        .unwrap();
        let answers = session.by_ref().map(Result::unwrap).count();
        assert_eq!(answers, 2);
        // Objective-free inputs normalize either request to the complete family.
        assert_eq!(
            session.outcome().unwrap().selection(),
            Some(zetesis_solve::AnswerSelection::All)
        );
    }
}

#[test]
fn streamed_constraints_preserve_the_complete_family() {
    let admitted = hybrid(MONOTONE, &limits());
    let eager_limits = FormulaLimits {
        theory: zetesis_ferraris::AdmissionLimits {
            max_roots: 64,
            ..limits().theory
        },
        ..limits()
    };
    let original = eager(MONOTONE, &eager_limits).unwrap();
    let (expected, reference) = capture(PreparedInput::formula(&original), configuration());
    assert_eq!(expected, monotone_family());
    assert_eq!(reference.completion(), Some(Completion::Exhausted));
    for (workers, batch, grounder, oracle) in [
        (1, 1, Grounder::Auto, Oracle::Countermodel),
        (4, 7, Grounder::Lazy, Oracle::Auto),
    ] {
        let config = SolveConfig {
            workers: NonZeroUsize::new(workers).unwrap(),
            batch_size: NonZeroUsize::new(batch).unwrap(),
            grounder,
            oracle,
            ..configuration()
        };
        let (actual, outcome) = capture(PreparedInput::hybrid(&admitted), config);
        assert_eq!(actual, expected);
        assert_eq!(outcome.completion(), Some(Completion::Exhausted));
        assert_eq!(outcome.verified_models(), 7);
    }
}

#[test]
fn hybrid_counts_the_core_answers_it_filters() {
    let admitted = hybrid(MONOTONE, &limits());
    let (_, outcome) = capture(PreparedInput::hybrid(&admitted), configuration());
    let statistics = outcome.hybrid_execution().unwrap();
    // Deferred constraints do not secretly prune the producer core. This is a
    // deliberately visible cost of composing membership and source checking.
    assert_eq!(statistics.core_answers, 64);
    assert_eq!(statistics.accepted, 7);
    assert_eq!(statistics.rejected, 57);
    assert_eq!(statistics.pending, 0);
}

#[test]
fn all_filtered_core_answers_establish_unsatisfiability() {
    let admitted = hybrid("a|b. :-not missing.", &limits());
    let (family, outcome) = capture(PreparedInput::hybrid(&admitted), configuration());
    assert!(family.is_empty());
    assert!(outcome.unsatisfiable());
    let statistics = outcome.hybrid_execution().unwrap();
    assert_eq!(statistics.core_answers, 2);
    assert_eq!(statistics.accepted, 0);
    assert_eq!(statistics.rejected, 2);
    assert_eq!(statistics.pending, 0);
}

#[test]
fn constraint_atoms_keep_their_signed_identity() {
    let source = "a|-a. :-a,not absent. :-missing.";
    let admitted = hybrid(source, &limits());
    let expected = Family::from([vec![atom("a", Sign::Negative, vec![])]]);
    let (family, outcome) = capture(PreparedInput::hybrid(&admitted), configuration());
    assert_eq!(family, expected);
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    let original = eager(source, &limits()).unwrap();
    assert_eq!(
        capture(PreparedInput::formula(&original), configuration()).0,
        expected
    );
}

#[test]
fn requested_models_count_only_source_answers() {
    let core = eager("a|b.", &limits()).unwrap();
    let config = SolveConfig {
        models: 1,
        ..configuration()
    };
    let (first, _) = capture(PreparedInput::formula(&core), config);
    assert_eq!(first.len(), 1);
    assert_eq!(first.first().unwrap().len(), 1);
    let rejected = first.first().unwrap()[0].predicate().name();
    assert!(matches!(rejected, "a" | "b"));
    let retained = if rejected == "a" { "b" } else { "a" };
    let admitted = hybrid(&format!("a|b. :-{rejected}."), &limits());
    let (family, outcome) = capture(PreparedInput::hybrid(&admitted), config);
    assert_eq!(
        family,
        Family::from([vec![atom(retained, Sign::Positive, vec![])]])
    );
    assert_eq!(outcome.completion(), Some(Completion::RequestedModels));
    let statistics = outcome.hybrid_execution().unwrap();
    assert_eq!(
        statistics.core_answers, 2,
        "the first core answer must be rejected"
    );
    assert_eq!(statistics.rejected, 1);
    assert_eq!(outcome.verified_models(), 1);
}

#[test]
fn cloned_hybrid_subjects_share_only_their_owner() {
    let original = hybrid("a|b. :-a.", &limits());
    let cloned = original.clone();
    let other = hybrid("a|b. :-a.", &limits());
    assert!(original.same_instance(&cloned));
    assert!(!original.same_instance(&other));
    let (_, first) = capture(PreparedInput::hybrid(&original), configuration());
    let (_, repeated) = capture(PreparedInput::hybrid(&cloned), configuration());
    let (_, separate) = capture(PreparedInput::hybrid(&other), configuration());
    assert!(
        first
            .subject()
            .unwrap()
            .same_instance(repeated.subject().unwrap())
    );
    assert!(
        !first
            .subject()
            .unwrap()
            .same_instance(separate.subject().unwrap())
    );
    assert!(
        !first
            .subject()
            .unwrap()
            .same_instance(&Subject::Theory(original.core_theory().clone()))
    );
}

#[test]
fn checker_setup_work_is_visible_before_search() {
    let admitted = hybrid(MONOTONE, &limits());
    let prepared = admitted
        .checker(ConstraintCheckLimits::default())
        .unwrap()
        .statistics();
    assert!(
        prepared.work > 0,
        "this fixture prepares completed relation descriptors"
    );
    let session = Session::builder(
        PreparedInput::hybrid(&admitted),
        configuration(),
        Control::default(),
    )
    .start()
    .unwrap();
    let outcome = session.progress();
    let statistics = outcome.hybrid_execution().unwrap();
    assert_eq!(statistics.constraints, prepared);
    assert_eq!(statistics.core_answers, 0);
    assert_eq!(outcome.completion(), None);
}

#[test]
fn checker_setup_refusal_retains_the_source_subject() {
    let admitted = hybrid(MONOTONE, &limits());
    let config = SolveConfig {
        constraints: ConstraintCheckLimits {
            max_work: 0,
            ..Default::default()
        },
        ..configuration()
    };
    let failure = Session::builder(PreparedInput::hybrid(&admitted), config, Control::default())
        .start()
        .err()
        .expect("zero work refuses the nonempty checker snapshot");
    let SolveError::Constraint(error) = failure.cause.as_ref() else {
        panic!("wrong setup failure: {failure:?}");
    };
    assert!(
        matches!(&error.cause, ConstraintCheckCause::Source(error) if matches!(error.as_ref(), FormulaFailure::Limit {
            resource: FormulaResource::Work, limit: 0, ..
        }))
    );
    assert_eq!(error.statistics.work, 0);
    assert!(
        failure
            .subject()
            .unwrap()
            .same_instance(&Subject::Hybrid(admitted))
    );
    assert!(failure.semantic().is_none());
}

#[test]
fn unfinished_constraint_check_never_yields_a_core_answer() {
    let admitted = hybrid(MONOTONE, &limits());
    // Admit exactly the public preparation receipt. The next charged operation
    // belongs to a real core answer's source check and must refuse before yield.
    let prepared = admitted
        .checker(ConstraintCheckLimits::default())
        .unwrap()
        .statistics();
    let config = SolveConfig {
        constraints: ConstraintCheckLimits {
            max_work: prepared.work,
            ..Default::default()
        },
        ..configuration()
    };
    let mut session =
        Session::builder(PreparedInput::hybrid(&admitted), config, Control::default())
            .start()
            .unwrap();
    let failure = session.next().unwrap().unwrap_err();
    let SolveError::Constraint(error) = failure.cause.as_ref() else {
        panic!("wrong runtime failure: {failure:?}");
    };
    assert!(
        matches!(&error.cause, ConstraintCheckCause::Source(error) if matches!(error.as_ref(), FormulaFailure::Limit {
        resource: FormulaResource::Work, limit, observed, ..
    } if *limit == u128::from(prepared.work) && *observed > *limit))
    );
    let outcome = failure.semantic().unwrap();
    let statistics = outcome.hybrid_execution().unwrap();
    assert_eq!(statistics.core_answers, 1);
    assert_eq!(statistics.accepted, 0);
    assert_eq!(statistics.rejected, 0);
    assert_eq!(statistics.pending, 1);
    assert_eq!(statistics.constraints, error.statistics);
    assert_eq!(statistics.constraints.work, prepared.work);
    assert_eq!(outcome.verified_models(), 0);
    assert_eq!(outcome.completion(), None);
    assert!(!outcome.unsatisfiable());
    assert!(session.next().is_none());
    assert_eq!(
        session.outcome().unwrap().hybrid_execution().unwrap(),
        statistics
    );
}

#[test]
fn cancellation_preserves_only_the_verified_prefix() {
    let admitted = hybrid("a|b. :-a,b.", &limits());
    let control = Control::default();
    let mut session = Session::builder(
        PreparedInput::hybrid(&admitted),
        configuration(),
        control.clone(),
    )
    .start()
    .unwrap();
    let answer = session.next().unwrap().unwrap();
    assert!(
        answer
            .subject()
            .same_instance(&Subject::Hybrid(admitted.clone()))
    );
    assert_eq!(answer.interpretation().atoms().len(), 1);
    control.cancel();
    assert!(session.next().is_none());
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert_eq!(
        outcome.interruption(),
        Some(Interruption::Countermodel(
            zetesis_sat::Incomplete::Cancelled
        ))
    );
    assert_eq!(outcome.verified_models(), 1);
    let statistics = outcome.hybrid_execution().unwrap();
    assert_eq!(statistics.core_answers, 1);
    assert_eq!(statistics.accepted, 1);
    assert_eq!(statistics.rejected, 0);
    assert_eq!(statistics.pending, 0);
    assert!(!outcome.unsatisfiable());
    assert!(session.next().is_none());
}
