//! Original consequences compose with closure, never frozen membership.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use zetesis_cpu::regions::Region;
use zetesis_ferraris::{Node, Theory};
use zetesis_sat::{
    BatchError, Cancellation, Incomplete, Limits, RegionConsequence, RegionFeasibility,
    RegionFilter, RegionFilterWorker, RegionPass, SearchLimits, StableModels,
};
use zetesis_theory_support::theories::theory;

use crate::support::batching::{batch, residual};
use crate::support::interpretations::stable_models;
use crate::support::region_filters::{ROUTES, Route, choices};

type Callback = fn(&Region, RegionPass, &Cancellation) -> Result<RegionConsequence, Incomplete>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Observation {
    pass: RegionPass,
    decisions: Vec<Option<bool>>,
}

#[derive(Debug)]
struct Filter {
    subject: Theory,
    callback: Callback,
    observations: Mutex<Vec<Observation>>,
    feasibility: AtomicUsize,
    live: AtomicUsize,
}

impl Filter {
    fn new(subject: &Theory, callback: Callback) -> Arc<Self> {
        Arc::new(Self {
            subject: subject.clone(),
            callback,
            observations: Mutex::new(Vec::new()),
            feasibility: AtomicUsize::new(0),
            live: AtomicUsize::new(0),
        })
    }

    fn observations(&self) -> Vec<Observation> {
        self.observations.lock().unwrap().clone()
    }
}

struct Worker<'a>(&'a Filter);

impl Drop for Worker<'_> {
    fn drop(&mut self) {
        self.0.live.fetch_sub(1, Ordering::SeqCst);
    }
}

impl RegionFilter for Filter {
    fn worker(
        &self,
        subject: &Theory,
        cancellation: &Cancellation,
    ) -> Result<Box<dyn RegionFilterWorker + '_>, Incomplete> {
        cancellation.poll()?;
        if !subject.same_instance(&self.subject) {
            return Err(Incomplete::WrongTheory);
        }
        self.live.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(Worker(self)))
    }
}

impl RegionFilterWorker for Worker<'_> {
    fn check(
        &mut self,
        subject: &Theory,
        _: &Region,
        cancellation: &Cancellation,
    ) -> Result<RegionFeasibility, Incomplete> {
        assert!(subject.same_instance(&self.0.subject));
        cancellation.poll()?;
        self.0.feasibility.fetch_add(1, Ordering::SeqCst);
        Ok(RegionFeasibility::NotRefuted)
    }

    fn consequence(
        &mut self,
        subject: &Theory,
        region: &Region,
        cancellation: &Cancellation,
        pass: RegionPass,
    ) -> Result<RegionConsequence, Incomplete> {
        assert!(subject.same_instance(&self.0.subject));
        cancellation.poll()?;
        self.0.observations.lock().unwrap().push(Observation {
            pass,
            decisions: (0..region.len())
                .map(|atom| region.decision(atom))
                .collect(),
        });
        (self.0.callback)(region, pass, cancellation)
    }
}

fn failure(route: Route, search: &mut StableModels) -> Incomplete {
    if matches!(route, Route::Producers) {
        match search.next_batch(batch(8), residual).unwrap_err() {
            BatchError::Search(cause) => cause,
            other => panic!("expected search refusal, got {other:?}"),
        }
    } else {
        search
            .next()
            .expect("the unresolved region must report its failure")
            .unwrap_err()
    }
}

fn limited(route: Route, subject: &Theory, max_work: u64) -> StableModels {
    let limits = Limits {
        search: SearchLimits {
            max_work,
            ..SearchLimits::default()
        },
        ..Limits::default()
    };
    let cancellation = Cancellation::default();
    let workers = NonZeroUsize::new(4).unwrap();
    match route {
        Route::Scalar => StableModels::new(subject, limits, cancellation),
        Route::Producers => {
            StableModels::with_region_producers(subject, workers, limits, cancellation)
        }
        Route::Native => StableModels::with_region_workers(subject, workers, limits, cancellation),
    }
    .unwrap()
}

fn require_first(
    region: &Region,
    _: RegionPass,
    cancellation: &Cancellation,
) -> Result<RegionConsequence, Incomplete> {
    cancellation.poll()?;
    Ok(match region.decision(0) {
        None => RegionConsequence::Hold(0),
        Some(false) => RegionConsequence::Refuted,
        Some(true) => RegionConsequence::Unchanged,
    })
}

fn cascade(
    region: &Region,
    _: RegionPass,
    cancellation: &Cancellation,
) -> Result<RegionConsequence, Incomplete> {
    cancellation.poll()?;
    // The external original constraints are :- not a. and :- b,c.
    Ok(
        if region.is_cut(0) || (region.is_held(1) && region.is_held(2)) {
            RegionConsequence::Refuted
        } else if region.is_open(0) {
            RegionConsequence::Hold(0)
        } else if region.is_held(1) && region.is_open(2) {
            RegionConsequence::Cut(2)
        } else {
            RegionConsequence::Unchanged
        },
    )
}

fn cascade_theory(original_constraints: bool) -> Theory {
    let mut nodes = vec![
        Node::falsum(),
        Node::atom(0),
        Node::atom(1),
        Node::atom(2),
        Node::implies(1, 0),
        Node::or_pair([1, 4]),
        Node::implies(2, 0),
        Node::or_pair([2, 6]),
        Node::implies(3, 0),
        Node::or_pair([3, 8]),
        Node::implies(1, 2),
    ];
    let mut roots = vec![5, 7, 9, 10];
    if original_constraints {
        nodes.extend([
            Node::implies(4, 0),
            Node::and_pair([2, 3]),
            Node::implies(12, 0),
        ]);
        roots.extend([11, 13]);
    }
    theory(3, nodes, roots)
}

#[test]
fn source_decisions_reenter_original_closure() {
    let subject = cascade_theory(false);
    let expected = stable_models(&cascade_theory(true));
    assert_eq!(expected, BTreeSet::from([vec![0, 1]]));
    for route in ROUTES {
        let filter = Filter::new(&subject, cascade);
        let mut search = route.search(&subject, Cancellation::default());
        search.enable_phase_timing();
        search.set_region_filter(filter.clone()).unwrap();
        assert_eq!(route.collect(&mut search), expected, "{route:?}");
        assert_eq!(
            filter.observations(),
            vec![
                Observation {
                    pass: RegionPass::First,
                    decisions: vec![None, None, None]
                },
                Observation {
                    pass: RegionPass::Continue,
                    decisions: vec![Some(true), Some(true), None]
                },
                Observation {
                    pass: RegionPass::Continue,
                    decisions: vec![Some(true), Some(true), Some(false)]
                },
            ]
        );
        assert_eq!(filter.feasibility.load(Ordering::SeqCst), 0);
        let receipt = search.statistics();
        assert_eq!(receipt.search.decisions, 0);
        let regions = receipt.regions.unwrap().counts;
        assert_eq!(
            (regions.regions, regions.leaves, regions.held, regions.cut),
            (1, 1, 2, 1)
        );
        let source = receipt.region_filter.unwrap();
        assert_eq!(
            (
                source.preparations,
                source.checks,
                source.refuted,
                source.failed
            ),
            (1, 3, 0, 0)
        );
        assert!(!source.overflowed);
        // The shared phase also includes ordinary candidate truth evaluation
        // for membership (and batched proposal validation).
        assert!(receipt.phase_timings.unwrap().original_validation.calls >= source.checks);
    }
}

#[test]
fn child_regions_begin_fresh_source_passes() {
    let subject = choices(3);
    let expected: BTreeSet<Vec<usize>> = stable_models(&subject)
        .into_iter()
        .filter(|answer| answer.contains(&0))
        .collect();
    for route in ROUTES {
        let filter = Filter::new(&subject, require_first);
        let mut search = route.search(&subject, Cancellation::default());
        search.set_region_filter(filter.clone()).unwrap();
        assert_eq!(route.collect(&mut search), expected, "{route:?}");
        let observations = filter.observations();
        assert_eq!(observations.len(), 8);
        assert_eq!(
            observations
                .iter()
                .filter(|row| row.pass == RegionPass::First)
                .count(),
            7
        );
        let continued: Vec<_> = observations
            .iter()
            .filter(|row| row.pass == RegionPass::Continue)
            .collect();
        assert_eq!(continued.len(), 1);
        assert_eq!(continued[0].decisions, vec![Some(true), None, None]);
    }
}

#[test]
fn source_consequences_never_enter_frozen_queries() {
    // a <- b; b <- a. :- not a. has no answer. Forcing a may narrow
    // original candidates, but must leave the empty reduct witness intact.
    let nodes = vec![
        Node::atom(0),
        Node::atom(1),
        Node::implies(0, 1),
        Node::implies(1, 0),
    ];
    let subject = theory(2, nodes.clone(), vec![2, 3]);
    let mut original_nodes = nodes;
    original_nodes.extend([Node::falsum(), Node::implies(0, 4), Node::implies(5, 4)]);
    let original = theory(2, original_nodes, vec![2, 3, 6]);
    assert!(stable_models(&original).is_empty());
    for route in ROUTES {
        let filter = Filter::new(&subject, require_first);
        let mut search = route.search(&subject, Cancellation::default());
        search.set_region_filter(filter.clone()).unwrap();
        assert!(route.collect(&mut search).is_empty(), "{route:?}");
        assert_eq!(
            filter.observations(),
            vec![
                Observation {
                    pass: RegionPass::First,
                    decisions: vec![None, None]
                },
                Observation {
                    pass: RegionPass::Continue,
                    decisions: vec![Some(true), Some(true)]
                },
            ]
        );
        assert_eq!(filter.feasibility.load(Ordering::SeqCst), 0);
        let receipt = search.statistics();
        assert!(receipt.candidates > 0);
        assert!(receipt.countermodel_queries > 0);
        assert!(
            receipt.countermodels > 0,
            "the empty frozen witness must survive"
        );
    }
}

fn out_of_range(
    _: &Region,
    _: RegionPass,
    cancellation: &Cancellation,
) -> Result<RegionConsequence, Incomplete> {
    cancellation.poll()?;
    Ok(RegionConsequence::Hold(usize::MAX))
}

#[test]
fn out_of_range_consequences_are_typed_failures() {
    let subject = choices(1);
    for route in ROUTES {
        let mut search = route.search(&subject, Cancellation::default());
        search
            .set_region_filter(Filter::new(&subject, out_of_range))
            .unwrap();
        assert_eq!(
            failure(route, &mut search),
            Incomplete::InvalidRegionConsequence
        );
        let _ = search.stop();
        assert!(!search.exhausted());
        let receipt = search.statistics();
        assert_eq!(receipt.candidates, 0);
        let source = receipt.region_filter.unwrap();
        assert_eq!((source.checks, source.failed, source.refuted), (1, 1, 0));
        assert_eq!(receipt.regions.unwrap().counts.held, 0);
    }
}

fn stale_hold(
    _: &Region,
    _: RegionPass,
    cancellation: &Cancellation,
) -> Result<RegionConsequence, Incomplete> {
    cancellation.poll()?;
    Ok(RegionConsequence::Hold(0))
}

fn stale_cut(
    _: &Region,
    pass: RegionPass,
    cancellation: &Cancellation,
) -> Result<RegionConsequence, Incomplete> {
    cancellation.poll()?;
    Ok(if pass == RegionPass::First {
        RegionConsequence::Hold(0)
    } else {
        RegionConsequence::Cut(0)
    })
}

#[test]
fn decided_consequences_are_typed_failures() {
    let subject = choices(1);
    for callback in [stale_hold as Callback, stale_cut] {
        for route in ROUTES {
            let filter = Filter::new(&subject, callback);
            let mut search = route.search(&subject, Cancellation::default());
            search.set_region_filter(filter.clone()).unwrap();
            assert_eq!(
                failure(route, &mut search),
                Incomplete::InvalidRegionConsequence
            );
            let _ = search.stop();
            assert!(!search.exhausted());
            let receipt = search.statistics();
            assert_eq!(receipt.candidates, 0);
            assert_eq!(receipt.regions.unwrap().counts.held, 1);
            let source = receipt.region_filter.unwrap();
            assert_eq!((source.checks, source.failed, source.refuted), (2, 1, 0));
            assert_eq!(
                filter.observations()[1],
                Observation {
                    pass: RegionPass::Continue,
                    decisions: vec![Some(true)],
                }
            );
        }
    }
}

fn cancelled_force(
    _: &Region,
    _: RegionPass,
    cancellation: &Cancellation,
) -> Result<RegionConsequence, Incomplete> {
    cancellation.poll()?;
    cancellation.cancel();
    Ok(RegionConsequence::Hold(0))
}

#[test]
fn cancellation_refuses_a_returned_consequence() {
    let subject = choices(1);
    for route in ROUTES {
        let filter = Filter::new(&subject, cancelled_force);
        let mut search = route.search(&subject, Cancellation::default());
        search.enable_phase_timing();
        search.set_region_filter(filter.clone()).unwrap();
        assert_eq!(failure(route, &mut search), Incomplete::Cancelled);
        let _ = search.stop();
        assert!(!search.exhausted());
        assert_eq!(filter.live.load(Ordering::SeqCst), 0);
        let receipt = search.statistics();
        assert_eq!(receipt.candidates, 0);
        assert_eq!(receipt.regions.unwrap().counts.held, 0);
        let source = receipt.region_filter.unwrap();
        assert_eq!((source.checks, source.failed), (1, 1));
        assert_eq!(receipt.phase_timings.unwrap().original_validation.calls, 1);
    }
}

fn failed_continuation(
    _: &Region,
    pass: RegionPass,
    _: &Cancellation,
) -> Result<RegionConsequence, Incomplete> {
    if pass == RegionPass::First {
        Ok(RegionConsequence::Hold(0))
    } else {
        Err(Incomplete::RegionFilter)
    }
}

#[test]
fn failed_continuation_retains_the_applied_prefix() {
    let subject = choices(1);
    for route in ROUTES {
        let filter = Filter::new(&subject, failed_continuation);
        let mut search = route.search(&subject, Cancellation::default());
        search.set_region_filter(filter.clone()).unwrap();
        assert_eq!(failure(route, &mut search), Incomplete::RegionFilter);
        let _ = search.stop();
        assert!(!search.exhausted());
        assert_eq!(filter.live.load(Ordering::SeqCst), 0);
        let receipt = search.statistics();
        assert_eq!(receipt.candidates, 0);
        assert_eq!(receipt.search.decisions, 0);
        let regions = receipt.regions.unwrap().counts;
        assert_eq!(regions.held, 1);
        assert_eq!(regions.work, receipt.search.work);
        let source = receipt.region_filter.unwrap();
        assert_eq!((source.checks, source.failed), (2, 1));
    }
}

fn refuted_continuation(
    _: &Region,
    pass: RegionPass,
    cancellation: &Cancellation,
) -> Result<RegionConsequence, Incomplete> {
    cancellation.poll()?;
    Ok(if pass == RegionPass::First {
        RegionConsequence::Hold(0)
    } else {
        RegionConsequence::Refuted
    })
}

#[test]
fn refuted_continuation_preserves_the_attempt_receipt() {
    let subject = choices(1);
    for route in ROUTES {
        let mut search = route.search(&subject, Cancellation::default());
        search
            .set_region_filter(Filter::new(&subject, refuted_continuation))
            .unwrap();
        assert!(route.collect(&mut search).is_empty());
        let receipt = search.statistics();
        assert_eq!(receipt.candidates, 0);
        assert_eq!(receipt.search.decisions, 0);
        let regions = receipt.regions.unwrap().counts;
        assert_eq!((regions.held, regions.refuted), (1, 1));
        let source = receipt.region_filter.unwrap();
        assert_eq!((source.checks, source.refuted, source.failed), (2, 1, 0));
    }
}

fn refused_pass(
    _: &Region,
    _: RegionPass,
    _: &Cancellation,
) -> Result<RegionConsequence, Incomplete> {
    Err(Incomplete::RegionFilter)
}

#[test]
fn source_publication_spends_one_search_permit() {
    let subject = choices(1);
    for route in ROUTES {
        // A deliberate callback failure measures the exact root prefix before
        // any source update, independent of this theory's indexing details.
        let mut probe = route.search(&subject, Cancellation::default());
        probe
            .set_region_filter(Filter::new(&subject, refused_pass))
            .unwrap();
        assert_eq!(failure(route, &mut probe), Incomplete::RegionFilter);
        let _ = probe.stop();
        let before = probe.statistics().search.work;
        for permitted in [0, 1] {
            let filter = Filter::new(&subject, require_first);
            let mut search = limited(route, &subject, before + permitted);
            search.set_region_filter(filter.clone()).unwrap();
            assert_eq!(failure(route, &mut search), Incomplete::WorkLimit);
            let _ = search.stop();
            assert!(!search.exhausted());
            let receipt = search.statistics();
            assert_eq!(receipt.search.work, before + permitted);
            let regions = receipt.regions.unwrap().counts;
            assert_eq!(regions.work, receipt.search.work);
            assert_eq!(regions.held, permitted);
            assert_eq!(receipt.candidates, 0);
            assert_eq!(receipt.search.decisions, 0);
            assert_eq!(filter.observations().len(), 1);
            let source = receipt.region_filter.unwrap();
            assert_eq!((source.checks, source.failed), (1, 0));
        }
    }
}
