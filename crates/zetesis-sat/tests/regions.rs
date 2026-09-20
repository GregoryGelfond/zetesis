//! Stable models proposed by regions: the coverage tree over the theory's
//! atoms, narrowed by its readings, with every leaf a classical model that
//! the reduct then decides. The regions and the clauses proposers return
//! the same stable models; the regions proposer never asks a classical
//! query, never proposes a nonmodel, and takes a restriction without
//! restarting.

#[path = "support/choice_theories.rs"]
mod choice_theories;
#[path = "support/formula_theories.rs"]
mod theories;

use std::collections::BTreeSet;

use zetesis_ferraris::{Node, Theory, TightPlanLimits};
use zetesis_sat::{
    BatchLimits, BatchVerdict, Control, Incomplete, Limits, SearchLimits, SearchMethod,
    StableModels,
};

use choice_theories::{choices, theory_over};
use theories::mixed;

fn regions(theory: &Theory, limits: Limits) -> StableModels {
    StableModels::with_method(theory, SearchMethod::Regions, limits, Control::default()).unwrap()
}

fn models(search: &mut StableModels) -> Vec<Vec<usize>> {
    search
        .by_ref()
        .map(|model| model.unwrap().atoms().collect())
        .collect()
}

#[test]
fn regions_and_clauses_return_the_same_stable_models() {
    for theory in [mixed(), choices(3)] {
        let mut clauses = StableModels::with_method(
            &theory,
            SearchMethod::Clauses,
            Limits::default(),
            Control::default(),
        )
        .unwrap();
        let expected: BTreeSet<Vec<usize>> = models(&mut clauses).into_iter().collect();
        assert!(clauses.exhausted());
        assert!(clauses.statistics().candidate_queries > 0);
        assert!(clauses.statistics().regions.is_none());
        let mut by_regions = regions(&theory, Limits::default());
        let found: BTreeSet<Vec<usize>> = models(&mut by_regions).into_iter().collect();
        assert!(by_regions.exhausted());
        assert!(by_regions.statistics().regions.is_some());
        assert_eq!(found, expected);
    }
}

/// The mixed theory's models by regions, with the search's receipts.
fn mixed_by_regions() -> (Vec<Vec<usize>>, zetesis_sat::Statistics) {
    let mut search = regions(&mixed(), Limits::default());
    let found = models(&mut search);
    (found, search.statistics())
}

#[test]
fn regions_ask_no_classical_query() {
    let (_, statistics) = mixed_by_regions();
    assert_eq!(statistics.candidate_queries, 0);
    let receipts = statistics.regions.expect("regions receipts");
    assert_eq!(
        u64::try_from(receipts.counts.leaves).unwrap(),
        statistics.candidates,
        "every leaf is a candidate"
    );
    assert!(
        receipts.counts.refuted > 0,
        "the constraint refutes a region"
    );
    assert!(receipts.counts.regions > receipts.counts.leaves);
}

#[test]
fn regions_propose_no_nonmodel() {
    // Every leaf was a classical model: the only rejections are reduct ones.
    let (found, statistics) = mixed_by_regions();
    assert_eq!(
        statistics.candidates,
        statistics.stable_models + statistics.countermodels,
    );
    assert_eq!(
        u64::try_from(found.len()).unwrap(),
        statistics.stable_models
    );
}

#[test]
fn regions_build_no_support_certificate() {
    let (_, statistics) = mixed_by_regions();
    assert!(statistics.support.is_none());
}

#[test]
fn regions_visit_the_cut_branch_before_the_held_one() {
    // The split atom is the narrowing's choice, so the order is not the
    // counter's; the cut branch first means the empty model comes first
    // and the full one last, on independent choices.
    let mut search = regions(&choices(2), Limits::default());
    let found = models(&mut search);
    assert_eq!(found.len(), 4);
    assert_eq!(found.first(), Some(&vec![]));
    assert_eq!(found.last(), Some(&vec![0, 1]));
}

#[test]
fn a_restriction_narrows_the_remaining_regions_without_restarting() {
    // s | not s over three atoms; after the first two models, require atom 2.
    let theory = choices(3);
    let mut search = regions(&theory, Limits::default());
    let first: Vec<Vec<usize>> = (0..2)
        .map(|_| search.next().unwrap().unwrap().atoms().collect())
        .collect();
    let restriction = theory_over(&theory, vec![Node::Atom(2)], vec![0]);
    search.restrict_candidates(&restriction).unwrap();
    let rest = models(&mut search);
    assert!(search.exhausted());
    assert!(rest.iter().all(|model| model.contains(&2)));
    // Neither earlier model returns, and every model with atom 2 not yet
    // visited is returned: the four with atom 2, less those already seen.
    assert!(rest.iter().all(|model| !first.contains(model)));
    let seen_with_2 = first.iter().filter(|model| model.contains(&2)).count();
    assert_eq!(rest.len(), 4 - seen_with_2);
    assert_eq!(search.statistics().candidate_restrictions, 1);
}

#[test]
fn a_restriction_charges_its_indexing_to_the_search_work() {
    // Indexing the theory is charged when the search is built; indexing a
    // restriction is charged when it is added, and the region receipts
    // count the same figure.
    let theory = choices(3);
    let mut search = regions(&theory, Limits::default());
    let before = search.statistics();
    let restriction = theory_over(&theory, vec![Node::Atom(2)], vec![0]);
    search.restrict_candidates(&restriction).unwrap();
    let after = search.statistics();
    let indexed = after.regions.unwrap().counts.work - before.regions.unwrap().counts.work;
    assert!(indexed > 0);
    assert_eq!(after.search.work - before.search.work, indexed);
}

#[test]
fn a_restriction_beyond_the_remaining_search_work_is_refused() {
    // The ceiling is exactly what construction charged, so no work remains
    // for the restriction's indexing.
    let theory = choices(3);
    let construction = regions(&theory, Limits::default()).statistics().search.work;
    let limits = Limits {
        search: SearchLimits {
            max_work: construction,
            ..SearchLimits::default()
        },
        ..Limits::default()
    };
    let mut search = regions(&theory, limits);
    let restriction = theory_over(&theory, vec![Node::Atom(2)], vec![0]);
    assert!(matches!(
        search.restrict_candidates(&restriction),
        Err(Incomplete::WorkLimit)
    ));
    assert_eq!(search.statistics().candidate_restrictions, 0);
}

#[test]
fn the_candidate_limit_stops_regions_without_exhaustion() {
    let limits = Limits {
        max_candidates: 2,
        ..Limits::default()
    };
    let mut search = regions(&choices(3), limits);
    let outcomes: Vec<_> = search.by_ref().collect();
    assert_eq!(outcomes.len(), 3);
    assert!(outcomes[..2].iter().all(Result::is_ok));
    assert!(matches!(outcomes[2], Err(Incomplete::CandidateLimit)));
    assert!(!search.exhausted());
}

#[test]
fn the_work_limit_stops_regions_without_exhaustion() {
    // Indexing the theory and extracting its producers are charged at
    // construction; the ceiling leaves a little search beyond them.
    let theory = choices(4);
    let construction = regions(&theory, Limits::default()).statistics().search.work;
    let max_work = construction + 10;
    let limits = Limits {
        search: SearchLimits {
            max_work,
            ..SearchLimits::default()
        },
        ..Limits::default()
    };
    let mut search = regions(&theory, limits);
    let outcomes: Vec<_> = search.by_ref().collect();
    assert!(matches!(outcomes.last(), Some(Err(Incomplete::WorkLimit))));
    assert!(!search.exhausted());
    assert!(search.statistics().search.work >= max_work);
}

#[test]
fn each_split_is_a_decision() {
    let limits = Limits {
        search: SearchLimits {
            max_decisions: 1,
            ..SearchLimits::default()
        },
        ..Limits::default()
    };
    let mut search = regions(&choices(3), limits);
    let outcomes: Vec<_> = search.by_ref().collect();
    assert!(matches!(
        outcomes.last(),
        Some(Err(Incomplete::DecisionLimit))
    ));
}

#[test]
fn the_batch_protocol_proposes_from_regions() {
    let theory = mixed();
    let mut expected = regions(&theory, Limits::default());
    let expected: BTreeSet<Vec<usize>> = models(&mut expected).into_iter().collect();
    let mut search = regions(&theory, Limits::default());
    let limits = BatchLimits {
        max_candidates: std::num::NonZeroUsize::new(2).unwrap(),
        max_pending_bytes: 1 << 20,
    };
    let mut found = BTreeSet::new();
    loop {
        let batch = search
            .next_batch(limits, |_, candidates| {
                Ok::<_, Incomplete>(vec![BatchVerdict::Residual; candidates.len()])
            })
            .unwrap();
        for model in batch {
            assert!(found.insert(model.atoms().collect::<Vec<_>>()));
        }
        if search.exhausted() {
            break;
        }
    }
    assert_eq!(found, expected);
}

#[test]
fn a_tight_certificate_decides_region_leaves_without_a_countermodel_query() {
    let theory = choices(3);
    let mut search = regions(&theory, Limits::default());
    assert!(
        search
            .enable_certified_checking(TightPlanLimits::default())
            .unwrap()
    );
    let found = models(&mut search);
    assert_eq!(found.len(), 8);
    assert!(search.exhausted());
    assert_eq!(search.statistics().countermodel_queries, 0);
}

#[test]
fn under_regions_the_reduct_is_never_encoded() {
    let (found, statistics) = mixed_by_regions();
    assert!(!found.is_empty());
    assert!(
        statistics.reduct.preparation.is_none(),
        "no reduct encoding"
    );
    assert_eq!(statistics.reduct.parameter_work, 0);
}

#[test]
fn under_regions_the_reduct_is_queried_by_regions() {
    let (_, statistics) = mixed_by_regions();
    assert!(statistics.countermodel_queries > 0);
    assert!(statistics.reduct.regions.regions > 0);
    assert_eq!(
        u64::try_from(statistics.reduct.regions.leaves).unwrap(),
        statistics.countermodels + statistics.stable_models,
        "each query ends at a countermodel leaf or at the candidate's own leaf"
    );
}

#[test]
fn the_search_methods_have_stable_spellings() {
    assert_eq!(SearchMethod::Regions.label(), "regions");
    assert_eq!(SearchMethod::Clauses.label(), "clauses");
}

#[test]
fn the_default_search_method_is_regions() {
    assert_eq!(SearchMethod::default(), SearchMethod::Regions);
}

#[test]
fn the_default_entry_enumerates_by_regions() {
    // `new` follows the type's default: the regions proposer's counts are
    // present and no clause form was built.
    let theory = mixed();
    let search = StableModels::new(&theory, Limits::default(), Control::default()).unwrap();
    assert!(search.statistics().regions.is_some());
    assert!(search.statistics().support.is_none());
}
