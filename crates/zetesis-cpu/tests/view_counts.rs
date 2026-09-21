//! Broken iterator size claims must refuse rather than index a missing world.

use zetesis_core::{AdmissionLimits, Program, Seed, SeedView};
use zetesis_cpu::{Cancellation, Stop, lazy};

struct Views<'a> {
    seed: SeedView<'a>,
    remaining: usize,
    claimed: usize,
    cloned: usize,
    clones: Option<std::sync::Arc<std::sync::atomic::AtomicUsize>>,
}
impl Clone for Views<'_> {
    fn clone(&self) -> Self {
        let first = self
            .clones
            .as_ref()
            .is_some_and(|count| count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0);
        Self {
            seed: self.seed,
            remaining: if first { self.claimed } else { self.cloned },
            claimed: self.claimed,
            cloned: self.cloned,
            clones: self.clones.clone(),
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
            clones: None,
        };
        let error = lazy::check_with_views(
            &program,
            views,
            lazy::Limits::default(),
            &Cancellation::default(),
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
            clones: None,
        };
        let error = lazy::check_with_views(
            &program,
            views,
            lazy::Limits::default(),
            &Cancellation::default(),
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

#[test]
fn changed_packing_count_refuses_before_evaluation() {
    let program = Program::new(vec![], AdmissionLimits::default()).unwrap();
    let seed = Seed::new(&program, []).unwrap();
    for actual in [0, 2] {
        let views = Views {
            seed: seed.view(),
            remaining: 1,
            claimed: 1,
            cloned: actual,
            clones: Some(std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0))),
        };
        let error = lazy::check_with_views(
            &program,
            views,
            lazy::Limits::default(),
            &Cancellation::default(),
            |_| -> Result<Vec<u32>, Stop> { panic!("packing count mismatch must not execute") },
        )
        .unwrap_err();
        assert!(matches!(
            error.cause,
            lazy::Cause::Source(Stop::InvalidProgram)
        ));
        assert_eq!(error.progress, lazy::Progress::default());
    }
}
