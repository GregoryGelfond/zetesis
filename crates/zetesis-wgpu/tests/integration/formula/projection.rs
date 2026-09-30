//! Finite transition evidence for the explicit alternative; no device execution.

use super::gate_transfer::{Aliases, Operation, bitwise, domains, fits, holds, reference};
use zetesis_wgpu::GateProjection;

#[test]
fn default_projection_is_enumerated() {
    assert_eq!(GateProjection::default(), GateProjection::Enumerated);
    assert_eq!(
        GateProjection::ALL.map(GateProjection::label),
        ["enumerated", "bitwise"]
    );
}

#[test]
fn every_projection_matches_independent_boolean_rows() {
    let mut cases = 0;
    // All masks, including empty domains and unequal observations of aliased
    // slots. The reference evaluates Boolean relations, not hexadecimal masks.
    for operation in Operation::ALL {
        for aliases in Aliases::ALL {
            for code in 0..64 {
                let snapshot = domains(code);
                assert_eq!(
                    bitwise(operation, aliases, snapshot),
                    reference(operation, aliases, snapshot)
                );
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 960);
}

#[test]
fn stale_observations_preserve_current_completions() {
    for operation in Operation::ALL {
        for aliases in Aliases::ALL {
            let slots = aliases.slots();
            for snapshot in (0..64).map(domains) {
                let support = bitwise(operation, aliases, snapshot).masks();
                for current in (0..64).map(|code| domains(code).masks()) {
                    if !(0..3)
                        .all(|position| current[slots[position]] & !snapshot.masks()[position] == 0)
                    {
                        continue;
                    }
                    let mut narrowed = current;
                    for position in 0..3 {
                        narrowed[slots[position]] &= support[position];
                    }
                    for left in [false, true] {
                        for right in [false, true] {
                            for output in [false, true] {
                                let assignment = [left, right, output];
                                if fits(current, assignment)
                                    && holds(operation, slots.map(|slot| assignment[slot]))
                                {
                                    assert!(fits(narrowed, assignment));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
