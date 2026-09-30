//! The finite transfer's contract, checked against the model: the projection's
//! laws, the alias partition, two worked cases and the domain encoding.

use std::collections::BTreeSet;

use super::{Aliases, Domains, Operation, bitwise};

/// The snapshot a six-bit code encodes.
pub(in crate::formula) fn domains(code: u8) -> Domains {
    Domains::from_code(code).expect("finite six-bit snapshot")
}

/// Whether each value lies in its position's domain.
pub(in crate::formula) fn fits(domains: [u8; 3], values: [bool; 3]) -> bool {
    domains
        .into_iter()
        .zip(values)
        .all(|(domain, value)| domain & (1 << u8::from(value)) != 0)
}

/// Whether the values, ordered left, right and output, satisfy the connective.
pub(in crate::formula) fn holds(operation: Operation, values: [bool; 3]) -> bool {
    let [left, right, output] = values;
    output
        == match operation {
            Operation::And => left && right,
            Operation::Or => left || right,
            Operation::Implies => !left || right,
        }
}

fn subset(smaller: Domains, larger: Domains) -> bool {
    smaller
        .masks()
        .into_iter()
        .zip(larger.masks())
        .all(|(small, large)| small & !large == 0)
}

/// All 960 transfers: every connective, alias partition and domain triple.
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

#[test]
fn support_is_exact_for_each_position_value() {
    for (operation, aliases, input) in inputs() {
        let slots = aliases.slots();
        let projected = bitwise(operation, aliases, input).masks();
        for position in 0..3 {
            for wanted in [false, true] {
                let mut found = false;
                for left in [false, true] {
                    for right in [false, true] {
                        for output in [false, true] {
                            let row = [left, right, output];
                            let coherent = (0..3)
                                .all(|a| (0..3).all(|b| slots[a] != slots[b] || row[a] == row[b]));
                            found |= coherent
                                && fits(input.masks(), row)
                                && holds(operation, row)
                                && row[position] == wanted;
                        }
                    }
                }
                assert_eq!(projected[position] & (1 << u8::from(wanted)) != 0, found);
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
fn the_64_domain_codes_decode_to_64_distinct_domains() {
    let decoded: BTreeSet<[u8; 3]> = (0..64).map(|code| domains(code).masks()).collect();
    assert_eq!(decoded.len(), 64);
}
