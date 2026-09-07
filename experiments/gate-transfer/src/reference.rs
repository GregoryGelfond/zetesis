//! Explicit row enumeration mirroring the current shader's mathematical relation.

use crate::{Aliases, Domains, Operation};

/// Exact position supports by enumerating all eight Boolean assignments.
///
/// This definition deliberately uses Boolean connective evaluation and slot
/// equality, independently of the alternative's hexadecimal truth-vector masks.
#[must_use]
pub const fn reference(operation: Operation, aliases: Aliases, domains: Domains) -> Domains {
    let [left, right, output] = domains.masks();
    let slots = aliases.slots();
    let mut support = [0; 3];
    let mut row = 0_u8;
    while row < 8 {
        let x = row & 1;
        let y = (row >> 1) & 1;
        let z = (row >> 2) & 1;
        let present = left & (1 << x) != 0 && right & (1 << y) != 0 && output & (1 << z) != 0;
        let coherent = (slots[0] != slots[1] || x == y)
            && (slots[0] != slots[2] || x == z)
            && (slots[1] != slots[2] || y == z);
        let value = match operation {
            Operation::And => x == 1 && y == 1,
            Operation::Or => x == 1 || y == 1,
            Operation::Implies => x == 0 || y == 1,
        };
        if present && coherent && value == (z == 1) {
            support[0] |= 1 << x;
            support[1] |= 1 << y;
            support[2] |= 1 << z;
        }
        row += 1;
    }
    Domains(support)
}
