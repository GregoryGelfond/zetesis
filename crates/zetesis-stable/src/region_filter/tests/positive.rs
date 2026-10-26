//! A positive candidate's single region still answers to the region check.

use zetesis_ferraris::{Node, Theory};
use zetesis_theory_support::theories::theory;

use super::support::{Condition, Filter, ROUTES, search};
use crate::{Cancellation, CertificateLimits, CertificateOrder, Incomplete, Limits, StableModels};

fn enable(stream: &mut StableModels) {
    assert!(
        stream
            .enable_class_checking(
                CertificateLimits::default(),
                CertificateOrder::PositiveFirst
            )
            .unwrap()
    );
}

fn cycle(seed: bool) -> Theory {
    let mut roots = vec![2, 3];
    if seed {
        roots.push(0);
    }
    theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::implies(0, 1),
            Node::implies(1, 0),
        ],
        roots,
    )
}

#[test]
fn singleton_filter_refutation_establishes_coverage() {
    let original = cycle(true);
    for route in ROUTES {
        let mut stream = search(route, &original, Limits::default(), Cancellation::default());
        enable(&mut stream);
        stream
            .set_region_filter(Filter::new(&original, Condition::ForbidHeld(0)))
            .unwrap();
        assert!(stream.next().is_none());
        assert!(stream.exhausted());
        assert_eq!(stream.statistics().candidates, 0);
        let stats = stream.statistics().region_filter.unwrap();
        assert_eq!(stats.preparations, 1);
        assert_eq!(stats.refuted, 1);
        assert_eq!(stats.failed, 0);
    }
}

#[test]
fn singleton_filter_stops_leave_coverage_open() {
    let original = cycle(true);
    for route in ROUTES {
        for (condition, expected) in [
            (Condition::FailPreparation, Incomplete::RegionFilter),
            (Condition::FailCheck, Incomplete::RegionFilter),
            (Condition::Cancel, Incomplete::Cancelled),
        ] {
            let mut stream = search(route, &original, Limits::default(), Cancellation::default());
            enable(&mut stream);
            stream
                .set_region_filter(Filter::new(&original, condition))
                .unwrap();
            assert!(matches!(stream.next(), Some(Err(found)) if found == expected));
            assert!(!stream.exhausted());
            assert_eq!(stream.statistics().candidates, 0);
            let stats = stream.statistics().region_filter.unwrap();
            assert_eq!(stats.preparations, 1);
            assert_eq!(stats.failed, 1);
            assert_eq!(stats.refuted, 0);
        }
    }
}
