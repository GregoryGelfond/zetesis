//! A stopped batch produces a typed stop before CPU oracle execution.

use std::time::Instant;

use clap::Parser;
use zetesis_core::Seed;
use zetesis_cpu::{Control, Stop};
use zetesis_themelios::{AdmissionOptions, admit};

use super::Engine;
use crate::Options;

#[test]
fn collected_seeds_observe_cancellation_and_deadline_before_any_oracle() {
    let admitted = admit("p.".into(), AdmissionOptions::default()).unwrap();
    let program = admitted.program();
    let seed = Seed::new(program, []).unwrap();
    for grounder in ["lazy", "eager"] {
        let options = Options::try_parse_from([
            "zetesis",
            "--backend",
            "cpu",
            "--workers",
            "1",
            "--grounder",
            grounder,
        ])
        .unwrap();
        let mut diagnostics = Vec::new();
        let mut engine = Engine::new(&options, program, &mut diagnostics).unwrap();
        let initial = diagnostics.clone();
        let cancelled = Control::default();
        cancelled.cancel();
        for (control, reason) in [
            (cancelled, Stop::Cancelled),
            (Control::with_deadline(Instant::now()), Stop::Deadline),
        ] {
            let batch = engine
                .check(
                    &options,
                    program,
                    &[seed.clone(), seed.clone()],
                    &mut diagnostics,
                    &control,
                    &crate::phase_timing::Recorder::new(false),
                )
                .unwrap();
            assert_eq!(batch, vec![Err(reason)]);
            assert_eq!(
                diagnostics, initial,
                "no scheduling or fallback after a stop"
            );
        }
        let batch = engine
            .check(
                &options,
                program,
                &[seed.clone(), seed.clone()],
                &mut diagnostics,
                &Control::default(),
                &crate::phase_timing::Recorder::new(false),
            )
            .unwrap();
        assert_eq!(batch.len(), 2);
        for result in batch {
            let model = result
                .unwrap()
                .expect("the unchanged fact remains derivable");
            assert_eq!(model.atoms().len(), 1);
            assert_eq!(model.atoms().iter().next().unwrap().predicate().name(), "p");
        }
    }
}
