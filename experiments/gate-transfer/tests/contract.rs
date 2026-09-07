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
                assert!(subset(expected, input));
                assert_eq!(bitwise(operation, aliases, expected), expected);
                compared += 1;
            }
        }
    }
    assert_eq!(compared, TABLE_LEN);
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

#[test]
fn aliases_use_physical_slot_identity_and_cover_all_partitions() {
    let mut seen = [false; 5];
    for left in 0..3 {
        for right in 0..3 {
            for output in 0..3 {
                let alias = Aliases::from_slots(left, right, output);
                let canonical = alias.slots();
                let original = [left, right, output];
                for a in 0..3 {
                    for b in 0..3 {
                        assert_eq!(canonical[a] == canonical[b], original[a] == original[b]);
                    }
                }
                seen[Aliases::ALL.iter().position(|&item| item == alias).unwrap()] = true;
            }
        }
    }
    assert!(seen.into_iter().all(|value| value));
    assert_eq!(
        Aliases::from_slots(u32::MAX, 0, u32::MAX),
        Aliases::LeftOutput
    );
    assert_eq!(
        bitwise(
            Operation::Implies,
            Aliases::All,
            Domains::new(3, 3, 3).unwrap()
        ),
        Domains::new(2, 2, 2).unwrap(),
    );
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
fn invalid_tags_and_masks_are_refused_without_truncation() {
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
    for mask in 4..=u8::MAX {
        assert_eq!(Domains::new(mask, 3, 3), None);
        assert_eq!(Domains::new(3, mask, 3), None);
        assert_eq!(Domains::new(3, 3, mask), None);
    }
    for code in 64..=u8::MAX {
        assert_eq!(Domains::from_code(code), None);
    }
    for code in 0..64 {
        assert_eq!(domains(code).code(), code);
    }
}
