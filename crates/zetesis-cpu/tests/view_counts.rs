//! Broken iterator size claims must refuse rather than index a missing world.

use zetesis_core::{AdmissionLimits, Program, Seed, SeedView};
use zetesis_cpu::{Control, Stop, lazy};

struct Views<'a> {
    seed: SeedView<'a>,
    remaining: usize,
    claimed: usize,
    cloned: usize,
}
impl Clone for Views<'_> {
    fn clone(&self) -> Self {
        Self {
            seed: self.seed,
            remaining: self.cloned,
            claimed: self.claimed,
            cloned: self.cloned,
        }
    }
}
impl<'a> Iterator for Views<'a> {
    type Item = SeedView<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        Some(self.seed)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.claimed, Some(self.claimed))
    }
}
impl ExactSizeIterator for Views<'_> {}

#[test]
fn false_count_claims_refuse_before_source_work() {
    let program = Program::new(vec![], AdmissionLimits::default()).unwrap();
    let seed = Seed::new(&program, []).unwrap();
    for (claimed, actual) in [(0, 1), (1, 0), (1, 2), (2, 1)] {
        let views = Views {
            seed: seed.view(),
            remaining: actual,
            claimed,
            cloned: actual,
        };
        let error = lazy::check_with_views(
            &program,
            views,
            lazy::Limits::default(),
            &Control::default(),
            |_| -> Result<Vec<u32>, Stop> { panic!("bad count must not execute") },
        )
        .unwrap_err();
        assert!(matches!(
            error.cause,
            lazy::Cause::Source(Stop::InvalidProgram)
        ));
        assert_eq!(error.progress, lazy::Progress::default());
    }
}

#[test]
fn changed_final_iteration_cannot_publish_missing_worlds() {
    let program = Program::new(vec![], AdmissionLimits::default()).unwrap();
    let seed = Seed::new(&program, []).unwrap();
    for actual in [0, 2] {
        let views = Views {
            seed: seed.view(),
            remaining: actual,
            claimed: 1,
            cloned: 1,
        };
        let error = lazy::check_with_views(
            &program,
            views,
            lazy::Limits::default(),
            &Control::default(),
            lazy::evaluate,
        )
        .unwrap_err();
        assert!(matches!(
            error.cause,
            lazy::Cause::Source(Stop::InvalidProgram)
        ));
        assert_eq!(error.progress.rounds, 1);
    }
}
