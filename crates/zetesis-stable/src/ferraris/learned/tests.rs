//! A solve's streams share learned rows through one store, at their walks'
//! boundaries only, and sharing changes no answer set.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use super::LearnedStore;
use crate::{Cancellation, Engagement, Incomplete, Limits, SearchLimits, StableModels};
use zetesis_ferraris::{
    AdmissionLimits, FormulaParts, Interpretation, LearningLimits, Node, Theory,
};

/// The walks a stream can run: one scalar walk, workers engaged at once, a
/// coordinator that never engages its workers, and joined producers.
#[derive(Clone, Copy, Debug)]
enum Route {
    Scalar,
    Workers,
    Coordinator,
    Producers,
}

const ROUTES: [Route; 4] = [
    Route::Scalar,
    Route::Workers,
    Route::Coordinator,
    Route::Producers,
];

fn stream(theory: &Theory, route: Route, limits: Limits) -> StableModels {
    let two = NonZeroUsize::new(2).unwrap();
    match route {
        Route::Scalar => StableModels::new(theory, limits, Cancellation::default()),
        Route::Workers => StableModels::with_region_workers_engaging(
            theory,
            two,
            Engagement::Immediate,
            limits,
            Cancellation::default(),
        ),
        Route::Coordinator => StableModels::with_region_workers_engaging(
            theory,
            two,
            Engagement::After(Duration::MAX),
            limits,
            Cancellation::default(),
        ),
        Route::Producers => {
            StableModels::with_region_producers(theory, two, limits, Cancellation::default())
        }
    }
    .unwrap()
}

fn sharing(theory: &Theory, route: Route, store: &Arc<LearnedStore>) -> StableModels {
    let mut models = stream(theory, route, Limits::default());
    models.set_learned_store(Arc::clone(store)).unwrap();
    models
}

/// Every answer set, as sorted atom lists.
fn answers(models: &mut StableModels) -> Vec<Vec<usize>> {
    let mut found: Vec<Vec<usize>> = models
        .by_ref()
        .map(|model| model.unwrap().atoms().collect())
        .collect();
    found.sort();
    found
}

fn rows(store: &LearnedStore) -> usize {
    store.lock().len()
}

fn theory(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(
        atoms,
        FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap()
}

/// `a <- b`, `b <- a`: the root's possible-foundedness pass captures the
/// loop's absence, and `{}` is the only answer set.
fn cycle() -> Theory {
    theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::implies(0, 1),
            Node::implies(1, 0),
        ],
        vec![2, 3],
    )
}

/// `{x}`, `a <- b`, `b <- a`, `a <- x`, with x, a, b as atoms 0, 1, 2. The
/// walk splits on x, holding it first: `{x, a, b}`, then the region without
/// x, whose pass captures the loop's absence, then `{}`.
fn choice_feeding_a_loop() -> Theory {
    theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::falsum(),
            Node::implies(0, 3),
            Node::or_pair([0, 4]),
            Node::implies(2, 1),
            Node::implies(1, 2),
            Node::implies(0, 1),
        ],
        vec![5, 6, 7, 8],
    )
}

#[test]
fn a_closed_stream_refuses_a_store() {
    let mut models = stream(&cycle(), Route::Scalar, Limits::default());
    models.stop().unwrap();
    assert_eq!(
        models.set_learned_store(Arc::new(LearnedStore::new())),
        Err(Incomplete::ClosedEnumerator)
    );
}

#[test]
fn a_started_stream_refuses_a_store() {
    let mut models = stream(&choice_feeding_a_loop(), Route::Scalar, Limits::default());
    assert!(models.next().unwrap().is_ok());
    assert_eq!(
        models.set_learned_store(Arc::new(LearnedStore::new())),
        Err(Incomplete::LateLearnedStore)
    );
}

#[test]
fn an_installed_store_is_not_replaced() {
    for route in ROUTES {
        let mut models = stream(&cycle(), route, Limits::default());
        models
            .set_learned_store(Arc::new(LearnedStore::new()))
            .unwrap();
        assert_eq!(
            models.set_learned_store(Arc::new(LearnedStore::new())),
            Err(Incomplete::LearnedStoreAlreadySet),
            "{route:?}"
        );
    }
}

#[test]
fn every_walk_delivers_its_rows_at_its_end() {
    for route in ROUTES {
        let store = Arc::new(LearnedStore::new());
        let mut models = sharing(&cycle(), route, &store);
        assert_eq!(answers(&mut models), [Vec::<usize>::new()], "{route:?}");
        assert!(models.exhausted(), "{route:?}");
        assert_eq!(rows(&store), 1, "{route:?}");
    }
}

#[test]
fn every_walk_draws_the_rows_another_stream_delivered() {
    for route in ROUTES {
        // A class is one theory instance, as a solve's streams share.
        let theory = cycle();
        let store = Arc::new(LearnedStore::new());
        answers(&mut sharing(&theory, Route::Scalar, &store));
        let mut models = sharing(&theory, route, &store);
        assert_eq!(answers(&mut models), [Vec::<usize>::new()], "{route:?}");
        let explanations = models
            .statistics()
            .regions
            .unwrap()
            .counts
            .support_explanations;
        assert_eq!(explanations.certified, 0, "{route:?}");
        assert!(explanations.matched > 0, "{route:?}");
    }
}

#[test]
fn sharing_reads_count_as_learning_work() {
    let learning = |models: &mut StableModels| {
        answers(models);
        models
            .statistics()
            .regions
            .unwrap()
            .counts
            .support_explanations
            .work
    };
    let theory = cycle();
    let alone = learning(&mut stream(&theory, Route::Scalar, Limits::default()));
    let store = Arc::new(LearnedStore::new());
    // Delivering the walk's one row adds reads to the same walk.
    let shared = learning(&mut sharing(&theory, Route::Scalar, &store));
    assert!(shared > alone, "{shared} with a store, {alone} without");
}

#[test]
fn a_stream_without_a_store_keeps_its_rows() {
    let theory = cycle();
    let store = Arc::new(LearnedStore::new());
    answers(&mut stream(&theory, Route::Scalar, Limits::default()));
    let mut models = sharing(&theory, Route::Scalar, &store);
    answers(&mut models);
    let explanations = models
        .statistics()
        .regions
        .unwrap()
        .counts
        .support_explanations;
    assert_eq!(explanations.certified, 1);
}

fn publishing(every: u64) -> Limits {
    Limits {
        learning: LearningLimits {
            publish_regions: every,
            ..LearningLimits::DEFAULT
        },
        ..Limits::default()
    }
}

/// The store's rows after the walk's second answer, before its end.
fn rows_before_the_end(limits: Limits) -> usize {
    let store = Arc::new(LearnedStore::new());
    let mut models = stream(&choice_feeding_a_loop(), Route::Scalar, limits);
    models.set_learned_store(Arc::clone(&store)).unwrap();
    for _ in 0..2 {
        assert!(models.next().unwrap().is_ok());
    }
    let delivered = rows(&store);
    assert!(models.next().is_none());
    assert_eq!(rows(&store), 1);
    delivered
}

#[test]
fn the_publication_trigger_delivers_before_the_walk_ends() {
    assert_eq!(rows_before_the_end(publishing(1)), 1);
}

#[test]
fn without_its_trigger_a_walk_delivers_only_at_its_end() {
    assert_eq!(rows_before_the_end(publishing(0)), 0);
}

#[test]
fn a_stop_while_delivering_ends_the_walk_incomplete() {
    let mut alone = stream(&cycle(), Route::Scalar, Limits::default());
    answers(&mut alone);
    let walked = alone.statistics().search.work;
    let limits = Limits {
        search: SearchLimits {
            max_work: walked,
            ..SearchLimits::default()
        },
        ..Limits::default()
    };
    let mut models = stream(&cycle(), Route::Scalar, limits);
    let store = Arc::new(LearnedStore::new());
    models.set_learned_store(Arc::clone(&store)).unwrap();
    // The walk itself fits the ceiling; delivering its row does not.
    let pulled: Vec<_> = models.by_ref().collect();
    assert!(
        pulled
            .iter()
            .any(|result| matches!(result, Err(Incomplete::WorkLimit))),
        "{pulled:?}"
    );
    assert!(!models.exhausted());
    assert_eq!(rows(&store), 0);
}

#[test]
fn an_exhausted_sharing_allowance_never_ends_the_walk() {
    for route in ROUTES {
        let limits = Limits {
            learning: LearningLimits {
                share_work: 1,
                ..LearningLimits::DEFAULT
            },
            ..Limits::default()
        };
        let store = Arc::new(LearnedStore::new());
        let mut models = stream(&cycle(), route, limits);
        models.set_learned_store(Arc::clone(&store)).unwrap();
        assert_eq!(answers(&mut models), [Vec::<usize>::new()], "{route:?}");
        assert!(models.exhausted(), "{route:?}");
        assert_eq!(rows(&store), 0, "{route:?}");
    }
}

#[test]
fn sharing_changes_no_answer_set() {
    for fixture in [
        cycle(),
        choice_feeding_a_loop(),
        super::super::fixtures::choice_beside_a_loop(),
    ] {
        let expected = answers(&mut stream(&fixture, Route::Scalar, Limits::default()));
        for route in ROUTES {
            let store = Arc::new(LearnedStore::new());
            // A first stream fills the store; the second draws from it.
            for _ in 0..2 {
                let mut models = sharing(&fixture, route, &store);
                assert_eq!(answers(&mut models), expected, "{route:?}");
                assert!(models.exhausted(), "{route:?}");
            }
        }
    }
}

/// A stream's candidates under a store are its candidates without one.
#[test]
fn a_shared_row_never_decides_membership() {
    // Rows prune classical candidates only: every answer a sharing stream
    // returns is an answer set the reference checker accepts.
    let fixture = choice_feeding_a_loop();
    let store = Arc::new(LearnedStore::new());
    for _ in 0..2 {
        for model in sharing(&fixture, Route::Scalar, &store) {
            let model: Interpretation = model.unwrap();
            assert!(
                zetesis_ferraris::check(
                    &fixture,
                    &model,
                    zetesis_ferraris::Limits::default(),
                    &Cancellation::default(),
                )
                .unwrap()
                .accepted()
            );
        }
    }
}
