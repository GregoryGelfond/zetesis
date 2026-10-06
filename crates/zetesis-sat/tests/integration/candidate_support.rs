//! Whole-family and accounting contracts for necessary outer support.

use std::collections::BTreeSet;

use crate::support::clause_search::by_clauses;
use zetesis_ferraris::{Interpretation, Node, Theory};
use zetesis_sat::{Cancellation, Incomplete, Limits, StableModels, SupportStatus};
use zetesis_theory_support::theories::theory;

fn disjunctions(pairs: usize) -> Theory {
    let mut nodes: Vec<_> = (0..pairs * 2).map(Node::Atom).collect();
    let roots = (0..pairs)
        .map(|pair| {
            let root = nodes.len();
            nodes.push(Node::Or(pair * 2, pair * 2 + 1));
            root
        })
        .collect();
    theory(pairs * 2, nodes, roots)
}

fn collect(models: &mut StableModels) -> BTreeSet<Vec<usize>> {
    let original = models.theory().clone();
    let mut family = BTreeSet::new();
    for result in models.by_ref() {
        let model = result.unwrap();
        assert!(original.same_instance(model.theory()));
        assert!(
            zetesis_ferraris::check(
                &original,
                &model,
                zetesis_ferraris::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .accepted()
        );
        assert!(family.insert(model.atoms().collect()));
    }
    assert!(models.exhausted());
    family
}

#[test]
fn independent_disjunctions_generate_only_two_choices_per_pair() {
    for pairs in 1..=4 {
        let input = disjunctions(pairs);
        let expected = (0..1 << pairs)
            .map(|mask| {
                (0..pairs)
                    .map(|pair| 2 * pair + ((mask >> pair) & 1))
                    .collect()
            })
            .collect();
        let classical = (0..1 << (2 * pairs))
            .filter(|mask| {
                let candidate = Interpretation::new(
                    &input,
                    (0..2 * pairs).filter(|atom| mask & (1 << atom) != 0),
                )
                .unwrap();
                zetesis_ferraris::models(
                    &input,
                    &candidate,
                    zetesis_ferraris::Limits::default(),
                    &Cancellation::default(),
                )
                .unwrap()
            })
            .count();
        assert_eq!(classical, 3_usize.pow(u32::try_from(pairs).unwrap()));
        let mut models = by_clauses(&input, Limits::default(), Cancellation::default()).unwrap();
        assert!(models.theory().same_instance(&input));
        assert_eq!(
            models.statistics().support.unwrap().status,
            SupportStatus::Applied
        );
        assert_eq!(collect(&mut models), expected);
        let statistics = models.statistics();
        assert_eq!(statistics.candidates, 1 << pairs);
        assert_eq!(statistics.countermodel_queries, 1 << pairs);
        assert_eq!(statistics.countermodels, 0);
        assert_eq!(statistics.candidate_queries, (1 << pairs) + 1);
        assert_eq!(statistics.candidate_restrictions, 0);
    }
}

#[test]
fn independent_facts_can_support_both_disjuncts() {
    let input = theory(
        2,
        vec![Node::Atom(0), Node::Atom(1), Node::Or(0, 1)],
        vec![0, 1, 2],
    );
    let mut models = by_clauses(&input, Limits::default(), Cancellation::default()).unwrap();
    assert_eq!(
        models.statistics().support.unwrap().status,
        SupportStatus::Applied
    );
    assert_eq!(collect(&mut models), BTreeSet::from([vec![0, 1]]));
}

#[test]
fn mixed_choices_generate_only_supported_candidates() {
    // (a or b) and (c or not c): ordinary sole-head support and exact choice
    // permission compose, without declaring the theory tight.
    let input = theory(
        3,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::Atom(2),
            Node::False,
            Node::Or(0, 1),
            Node::Implies(2, 3),
            Node::Or(2, 5),
        ],
        vec![4, 6],
    );
    let mut models = by_clauses(&input, Limits::default(), Cancellation::default()).unwrap();
    let support = models.statistics().support.unwrap();
    assert_eq!(support.status, SupportStatus::Applied);
    assert!(support.construction_work > 0);
    assert!(support.encoding_work > 0);
    assert_eq!(
        collect(&mut models),
        BTreeSet::from([vec![0], vec![1], vec![0, 2], vec![1, 2]])
    );
    assert_eq!(models.statistics().candidates, 4);
    assert_eq!(models.statistics().countermodels, 0);
    assert_eq!(models.statistics().countermodel_queries, 4);
}

#[test]
fn opaque_choice_heads_retain_general_candidate_search() {
    // The conjunction is classically the same choice, but deliberately outside
    // the exact head grammar. No recognized-prefix support restriction escapes.
    let input = theory(
        3,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::Atom(2),
            Node::False,
            Node::Or(0, 1),
            Node::Implies(2, 3),
            Node::Or(2, 5),
            Node::And(6, 6),
        ],
        vec![4, 7],
    );
    let mut models = by_clauses(&input, Limits::default(), Cancellation::default()).unwrap();
    let support = models.statistics().support.unwrap();
    assert_eq!(support.status, SupportStatus::NotApplicable);
    assert_eq!(support.construction_work, 10); // Eight nodes, two roots.
    assert_eq!(support.encoding_work, 0);
    assert_eq!(
        collect(&mut models),
        BTreeSet::from([vec![0], vec![1], vec![0, 2], vec![1, 2]])
    );
    assert_eq!(models.statistics().candidates, 6);
    assert_eq!(models.statistics().countermodels, 2);
}

#[test]
fn mixed_support_keeps_positive_cycle_minimality_checks() {
    // d supports itself, alongside a-or-b and a choice of c. Necessary support
    // retains candidates containing d; exact reduct checking must reject them.
    let input = theory(
        4,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::Atom(2),
            Node::Atom(3),
            Node::False,
            Node::Or(0, 1),
            Node::Implies(2, 4),
            Node::Or(2, 6),
            Node::Implies(3, 3),
        ],
        vec![5, 7, 8],
    );
    let mut models = by_clauses(&input, Limits::default(), Cancellation::default()).unwrap();
    assert_eq!(
        models.statistics().support.unwrap().status,
        SupportStatus::Applied
    );
    assert_eq!(
        collect(&mut models),
        BTreeSet::from([vec![0], vec![1], vec![0, 2], vec![1, 2]])
    );
    assert_eq!(models.statistics().candidates, 8);
    assert_eq!(models.statistics().countermodel_queries, 8);
    assert_eq!(models.statistics().countermodels, 4);
}

#[test]
fn optional_formula_limit_preserves_the_original_query() {
    let input = disjunctions(1);
    let mut limits = Limits::default();
    // Eight occurrences admit the original a-or-b encoding. They do not admit
    // the separately bounded support formula. Reduct-query admission and
    // exclusion history are independent of this original candidate limit.
    limits.admission.max_literals = 8;
    let mut models = by_clauses(&input, limits, Cancellation::default()).unwrap();
    let support = models.statistics().support.unwrap();
    assert_eq!(support.status, SupportStatus::FormulaLimit);
    assert!(support.construction_work > 0);
    assert_eq!(support.encoding_work, 0);
    assert_eq!(collect(&mut models), BTreeSet::from([vec![0], vec![1]]));
    assert_eq!(models.statistics().candidates, 3);
    assert_eq!(models.statistics().countermodels, 1);
}

#[test]
fn cold_reduct_admission_refusal_cannot_publish_an_answer() {
    let input = disjunctions(1);
    let mut limits = Limits::default();
    // The same eight-literal candidate owner completes its original encoding,
    // with optional support declined. The separately requested immutable reduct
    // cannot admit its next ten-literal prefix and never enters native search.
    limits.admission.max_literals = 8;
    limits.reduct_admission.max_literals = 8;
    let mut models = by_clauses(&input, limits, Cancellation::default()).unwrap();
    assert_eq!(
        models.statistics().support.unwrap().status,
        SupportStatus::FormulaLimit
    );
    assert!(matches!(
        models.next().unwrap(),
        Err(Incomplete::Admission(zetesis_sat::AdmissionError::Limit {
            resource: zetesis_sat::Resource::Literals,
            observed: 10,
            limit: 8,
        }))
    ));
    let statistics = models.statistics();
    assert_eq!(statistics.candidates, 1);
    assert_eq!(statistics.countermodel_queries, 0);
    assert_eq!(statistics.stable_models, 0);
    let preparation = statistics.reduct.preparation.unwrap();
    assert_eq!(preparation.retained_bytes, 0);
    assert!(preparation.work > 0);
    assert!(preparation.work <= statistics.search.work);
    assert!(models.next().is_none());
    assert!(!models.exhausted());
}

#[test]
fn optional_encoding_limit_rolls_back_to_general_search() {
    let input = disjunctions(1);
    let mut limits = Limits::default();
    limits.admission.max_variables = 3; // The original a-or-b gate fits exactly.
    let mut models = by_clauses(&input, limits, Cancellation::default()).unwrap();
    let support = models.statistics().support.unwrap();
    assert!(matches!(support.status, SupportStatus::EncodingLimit(_)));
    assert!(support.construction_work > 0);
    assert!(support.encoding_work > 0);
    assert_eq!(collect(&mut models), BTreeSet::from([vec![0], vec![1]]));
    assert_eq!(models.statistics().candidates, 3);
    assert_eq!(models.statistics().countermodels, 1);
}

#[test]
fn setup_work_ceiling_is_inclusive() {
    // Under the default regions method construction charges only the
    // producer extraction: the original index is charged when the walk
    // starts, so construction no longer fails for want of index work, and
    // the exact ceiling stops at the walk's start.
    let input = disjunctions(2);
    let complete = StableModels::new(&input, Limits::default(), Cancellation::default()).unwrap();
    let work = complete.statistics().search.work;
    for max_work in [0, 1, work - 1] {
        let mut limits = Limits::default();
        limits.search.max_work = max_work;
        assert!(matches!(
            StableModels::new(&input, limits, Cancellation::default()),
            Err(Incomplete::WorkLimit)
        ));
    }
    let mut limits = Limits::default();
    limits.search.max_work = work;
    let mut exact = StableModels::new(&input, limits, Cancellation::default()).unwrap();
    assert_eq!(exact.statistics().search.work, work);
    assert_eq!(exact.next().unwrap().unwrap_err(), Incomplete::WorkLimit);
    assert!(!exact.exhausted());
}
