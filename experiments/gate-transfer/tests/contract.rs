//! Complete finite properties, including mixed snapshots for aliased slots.

use zetesis_gate_transfer_experiment::{
    Aliases, Domains, LOOKUP_TABLE, Operation, TABLE_LEN, bitwise, lookup, reference,
};

fn domains(code: u8) -> Domains {
    Domains::from_code(code).expect("enumerated six-bit domain code")
}

fn subset(smaller: Domains, larger: Domains) -> bool {
    smaller
        .masks()
        .into_iter()
        .zip(larger.masks())
        .all(|(small, large)| small & !large == 0)
}

#[test]
fn all_960_transfers_match_the_independent_relation() {
    let mut compared = 0;
    for operation in Operation::ALL {
        for aliases in Aliases::ALL {
            for code in 0..64 {
                let input = domains(code);
                let expected = reference(operation, aliases, input);
                assert_eq!(bitwise(operation, aliases, input), expected);
                assert_eq!(lookup(operation, aliases, input), expected);
                compared += 1;
            }
        }
    }
    assert_eq!(compared, TABLE_LEN);
}

fn inputs() -> impl Iterator<Item = (Operation, Aliases, Domains)> {
    Operation::ALL.into_iter().flat_map(|operation| {
        Aliases::ALL
            .into_iter()
            .flat_map(move |aliases| (0..64).map(move |code| (operation, aliases, domains(code))))
    })
}

#[test]
fn projected_support_is_contractive() {
    for (operation, aliases, input) in inputs() {
        assert!(subset(bitwise(operation, aliases, input), input));
    }
}

#[test]
fn projected_support_is_idempotent() {
    for (operation, aliases, input) in inputs() {
        let narrowed = bitwise(operation, aliases, input);
        assert_eq!(bitwise(operation, aliases, narrowed), narrowed);
    }
}

#[test]
fn lookup_codes_fit_the_domain_encoding() {
    assert!(LOOKUP_TABLE.iter().all(|&code| code < 64));
}

#[test]
fn narrowing_inputs_never_adds_supported_values() {
    let mut compared = 0;
    for operation in Operation::ALL {
        for aliases in Aliases::ALL {
            for small in (0..64).map(domains) {
                for large in (0..64).map(domains) {
                    if subset(small, large) {
                        assert!(subset(
                            bitwise(operation, aliases, small),
                            bitwise(operation, aliases, large),
                        ));
                        compared += 1;
                    }
                }
            }
        }
    }
    assert_eq!(compared, 15 * 9_usize.pow(3));
}

fn holds(operation: Operation, left: bool, right: bool, output: bool) -> bool {
    match operation {
        Operation::And => output == (left && right),
        Operation::Or => output == (left || right),
        Operation::Implies => output == (!left || right),
    }
}

fn fits(domains: [u8; 3], values: [bool; 3]) -> bool {
    domains
        .into_iter()
        .zip(values)
        .all(|(domain, value)| domain & (if value { 2 } else { 1 }) != 0)
}

#[test]
fn support_is_exact_for_each_position_value() {
    for operation in Operation::ALL {
        for aliases in Aliases::ALL {
            let slots = aliases.slots();
            for input in (0..64).map(domains) {
                let projected = bitwise(operation, aliases, input).masks();
                for position in 0..3 {
                    for wanted in [false, true] {
                        let mut found = false;
                        for left in [false, true] {
                            for right in [false, true] {
                                for output in [false, true] {
                                    let row = [left, right, output];
                                    let coherent = (0..3).all(|a| {
                                        (0..3).all(|b| slots[a] != slots[b] || row[a] == row[b])
                                    });
                                    found |= coherent
                                        && fits(input.masks(), row)
                                        && holds(operation, left, right, output)
                                        && row[position] == wanted;
                                }
                            }
                        }
                        assert_eq!(
                            projected[position] & (if wanted { 2 } else { 1 }) != 0,
                            found
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn stale_and_mixed_snapshots_preserve_every_current_completion() {
    for operation in Operation::ALL {
        for aliases in Aliases::ALL {
            let slots = aliases.slots();
            for snapshot in (0..64).map(domains) {
                let support = bitwise(operation, aliases, snapshot).masks();
                for current in (0..64).map(|code| domains(code).masks()) {
                    if !(0..3).all(|p| current[slots[p]] & !snapshot.masks()[p] == 0) {
                        continue;
                    }
                    let mut narrowed = current;
                    for position in 0..3 {
                        narrowed[slots[position]] &= support[position];
                    }
                    for a in [false, true] {
                        for b in [false, true] {
                            for c in [false, true] {
                                let physical = [a, b, c];
                                if fits(current, physical)
                                    && holds(
                                        operation,
                                        physical[slots[0]],
                                        physical[slots[1]],
                                        physical[slots[2]],
                                    )
                                {
                                    assert!(fits(narrowed, physical));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn slot_triples() -> impl Iterator<Item = [u32; 3]> {
    (0..3).flat_map(|left| (0..3).flat_map(move |right| (0..3).map(move |out| [left, right, out])))
}

#[test]
fn alias_canonicalization_preserves_slot_identity() {
    for original in slot_triples() {
        let [left, right, output] = original;
        let canonical = Aliases::from_slots(left, right, output).slots();
        for a in 0..3 {
            for b in 0..3 {
                assert_eq!(canonical[a] == canonical[b], original[a] == original[b]);
            }
        }
    }
}

#[test]
fn slot_triples_cover_every_alias_partition() {
    let mut seen = [false; 5];
    for [left, right, output] in slot_triples() {
        let alias = Aliases::from_slots(left, right, output);
        seen[Aliases::ALL.iter().position(|&item| item == alias).unwrap()] = true;
    }
    assert!(seen.into_iter().all(|value| value));
}

#[test]
fn largest_slot_identity_remains_distinct_from_zero() {
    assert_eq!(
        Aliases::from_slots(u32::MAX, 0, u32::MAX),
        Aliases::LeftOutput
    );
}

#[test]
fn self_implication_supports_only_true_output() {
    assert_eq!(
        bitwise(
            Operation::Implies,
            Aliases::All,
            Domains::new(3, 3, 3).unwrap()
        ),
        Domains::new(2, 2, 2).unwrap(),
    );
}

#[test]
fn contradictory_alias_observations_have_no_support() {
    assert_eq!(
        bitwise(
            Operation::And,
            Aliases::LeftRight,
            Domains::new(1, 2, 3).unwrap()
        ),
        Domains::new(0, 0, 0).unwrap(),
    );
}

#[test]
fn operation_tags_match_the_shader_contract() {
    for (tag, expected) in [
        (0, None),
        (1, None),
        (2, Some(Operation::And)),
        (3, Some(Operation::Or)),
        (4, Some(Operation::Implies)),
        (5, None),
        (u32::MAX, None),
    ] {
        assert_eq!(Operation::from_shader_tag(tag), expected);
    }
}

#[test]
fn invalid_position_masks_are_refused_without_truncation() {
    for mask in 4..=u8::MAX {
        assert_eq!(Domains::new(mask, 3, 3), None);
        assert_eq!(Domains::new(3, mask, 3), None);
        assert_eq!(Domains::new(3, 3, mask), None);
    }
}

#[test]
fn invalid_domain_codes_are_refused_without_truncation() {
    for code in 64..=u8::MAX {
        assert_eq!(Domains::from_code(code), None);
    }
}

#[test]
fn valid_domain_codes_round_trip() {
    for code in 0..64 {
        assert_eq!(domains(code).code(), code);
    }
}
