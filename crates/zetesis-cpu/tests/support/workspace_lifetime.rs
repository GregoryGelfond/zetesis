//! Live join ownership at the source-coordinator boundary.

use zetesis_core::{AdmissionLimits, AtomPattern, Predicate, Template};

use super::*;

fn pattern(name: &str) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap()
}

#[test]
fn completed_scans_retain_the_join_reservation() {
    let program = Program::new(
        vec![Template::new(
            Some(pattern("goal")),
            ["a", "b", "c"].into_iter().map(pattern).collect(),
            vec![],
            vec![],
            vec![],
        )],
        AdmissionLimits::default(),
    )
    .unwrap();
    let limits = Limits::default();
    let mut state = State::new(1, limits).unwrap();
    state
        .scan(
            &program,
            SourceSelection::Worlds,
            limits,
            &Control::default(),
            &mut Progress::default(),
            &mut evaluate,
        )
        .unwrap();
    let retained = state.world_workspace.as_ref().unwrap().bytes();
    assert_eq!(state.source_mask_bytes, retained);
    let atom = Atom::new(Predicate::new("a", 0).unwrap(), vec![]).unwrap();
    let next_payload = 4 * atom_bytes(&atom).unwrap();
    let ceiling = state.fixed_bytes + state.payload_bytes + next_payload + retained;
    let below = Limits {
        max_host_bytes: ceiling - 1,
        ..limits
    };
    assert!(matches!(state.intern(&atom, below), Err(Stop::Allocation)));
    assert!(state.catalog.is_empty());
    let exact = Limits {
        max_host_bytes: ceiling,
        ..limits
    };
    state.intern(&atom, exact).unwrap();
}

#[test]
fn failed_scans_retain_the_join_reservation() {
    let templates = ["a", "b"]
        .into_iter()
        .map(|name| Template::new(Some(pattern(name)), vec![], vec![], vec![], vec![]))
        .collect();
    let program = Program::new(templates, AdmissionLimits::default()).unwrap();
    let limits = Limits {
        max_chunk_rules: 1,
        ..Default::default()
    };
    let mut state = State::new(1, limits).unwrap();
    let failed = state.scan(
        &program,
        SourceSelection::Worlds,
        limits,
        &Control::default(),
        &mut Progress::default(),
        &mut |_| Err::<Vec<u32>, _>(Stop::Cancelled),
    );
    assert!(matches!(
        failed.unwrap_err().cause,
        source::ScanCause::Consumer(Cause::Execution(Stop::Cancelled))
    ));
    let retained = state.world_workspace.as_ref().unwrap().bytes();
    assert_eq!(state.source_mask_bytes, retained);
    assert_eq!(retained, size_of::<u32>());
}
