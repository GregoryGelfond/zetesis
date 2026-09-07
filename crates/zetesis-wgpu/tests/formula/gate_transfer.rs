//! Test-only finite relation, promoted from experiments/gate-transfer at 93d2575.
//! Only the used bitwise and independent Boolean paths are retained; no lookup.
//!
//! A domain mask uses bit zero for false and bit one for true. All three
//! snapshots may differ even when slots alias: concurrent atomic loads can
//! observe intervening narrowing. The gate is enabled in one immutable frozen
//! query; disabled M-false connectives must never call these transfers.

/// Connectives represented by the production shader's tags 2, 3 and 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    /// Output equals left conjunction right.
    And,
    /// Output equals left disjunction right.
    Or,
    /// Output equals the material implication from left to right.
    Implies,
}
impl Operation {
    /// All admitted connectives, in lookup-table order.
    pub const ALL: [Self; 3] = [Self::And, Self::Or, Self::Implies];

    const fn rows(self) -> u8 {
        match self {
            Self::And => 0x87,
            Self::Or => 0xe1,
            Self::Implies => 0xd2,
        }
    }
}

/// The five realizable equality partitions of left, right and output slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aliases {
    /// Three distinct storage slots.
    Distinct,
    /// Left and right share a slot.
    LeftRight,
    /// Left and output share a slot.
    LeftOutput,
    /// Right and output share a slot.
    RightOutput,
    /// All positions share a slot.
    All,
}
impl Aliases {
    /// All realizable partitions, in lookup-table order.
    pub const ALL: [Self; 5] = [
        Self::Distinct,
        Self::LeftRight,
        Self::LeftOutput,
        Self::RightOutput,
        Self::All,
    ];

    /// Classify semantic storage slots, not merely original DAG node indices.
    #[must_use]
    pub const fn from_slots(left: u32, right: u32, output: u32) -> Self {
        if left == right && right == output {
            Self::All
        } else if left == right {
            Self::LeftRight
        } else if left == output {
            Self::LeftOutput
        } else if right == output {
            Self::RightOutput
        } else {
            Self::Distinct
        }
    }

    /// Canonical physical slot indices for the three positions.
    #[must_use]
    pub const fn slots(self) -> [usize; 3] {
        match self {
            Self::Distinct => [0, 1, 2],
            Self::LeftRight => [0, 0, 1],
            Self::LeftOutput => [0, 1, 0],
            Self::RightOutput => [0, 1, 1],
            Self::All => [0, 0, 0],
        }
    }

    const fn rows(self) -> u8 {
        match self {
            Self::Distinct => 0xff,
            Self::LeftRight => 0x99,
            Self::LeftOutput => 0xa5,
            Self::RightOutput => 0xc3,
            Self::All => 0x81,
        }
    }
}

/// Validated two-bit snapshots or projected support for the three positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Domains([u8; 3]);
impl Domains {
    /// Admit masks 0 through 3, including the empty domain.
    #[must_use]
    pub const fn new(left: u8, right: u8, output: u8) -> Option<Self> {
        if left <= 3 && right <= 3 && output <= 3 {
            Some(Self([left, right, output]))
        } else {
            None
        }
    }

    /// Position masks, ordered left, right, output.
    #[must_use]
    pub const fn masks(self) -> [u8; 3] {
        self.0
    }

    /// Decode an admitted six-bit code; no high bits are silently discarded.
    #[must_use]
    pub const fn from_code(code: u8) -> Option<Self> {
        if code < 64 {
            Some(Self([code & 3, (code >> 2) & 3, (code >> 4) & 3]))
        } else {
            None
        }
    }
}

const fn expand(domain: u8, false_rows: u8, true_rows: u8) -> u8 {
    let no = if domain & 1 != 0 { false_rows } else { 0 };
    let yes = if domain & 2 != 0 { true_rows } else { 0 };
    no | yes
}

const fn project(rows: u8, false_rows: u8, true_rows: u8) -> u8 {
    let no = if rows & false_rows != 0 { 1 } else { 0 };
    let yes = if rows & true_rows != 0 { 2 } else { 0 };
    no | yes
}

/// Direct finite transfer through bit sets of the eight relation rows.
///
/// Row number is `left + 2 * right + 4 * output`. These are truth-table bits,
/// not candidate bits or a packed domain store; atomics remain the caller's job.
#[must_use]
pub const fn bitwise(operation: Operation, aliases: Aliases, domains: Domains) -> Domains {
    let [left, right, output] = domains.masks();
    let rows = operation.rows()
        & aliases.rows()
        & expand(left, 0x55, 0xaa)
        & expand(right, 0x33, 0xcc)
        & expand(output, 0x0f, 0xf0);
    Domains([
        project(rows, 0x55, 0xaa),
        project(rows, 0x33, 0xcc),
        project(rows, 0x0f, 0xf0),
    ])
}

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
