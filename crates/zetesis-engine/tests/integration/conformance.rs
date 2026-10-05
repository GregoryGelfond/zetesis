//! The upstream corpus and obligations remain owned by the upstream suite.

use std::num::NonZeroUsize;
use themelios_solve::conformance::{self, Check, Verdict};
use zetesis_engine::{Config, Grounder, Solver};

#[test]
fn full_formula_configurations_meet_upstream_conformance() {
    // The public suite drives its corpus through both doors. Hybrid and lazy
    // have narrower profiles and are checked on their eligible programs below.
    for grounder in [Grounder::Auto, Grounder::Eager] {
        let mut solver = Solver::new(Config {
            grounder,
            workers: NonZeroUsize::new(2).unwrap(),
            ..Config::default()
        });
        let report = conformance::run(&mut solver);
        assert!(report.is_conformant(), "{grounder:?}: {report}");
        for check in [
            Check::CancellationIsNotExhaustion,
            Check::OnlyTheBaseGrounds,
        ] {
            assert_eq!(
                report.verdict(check),
                Some(&Verdict::Passed),
                "{grounder:?}: {report}"
            );
        }
    }
}
