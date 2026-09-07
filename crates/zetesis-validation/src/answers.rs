//! Checked solver-reported displays, independent of corpus policy and process I/O.
//!
//! A display is a multiset of printed symbols, not a hidden full interpretation.
//! Successful parsing reconciles the producer's status, enumeration, counts and
//! costs; it does not certify that the producer solved the original program.
//! Callers must separately require successful process capture and their intended
//! exit policy. Clingo optimization uses the supported optN replay convention.

use std::collections::BTreeMap;
use std::fmt;

mod clingo;
mod display;
mod native;
pub mod native_json;

// A display is a multiset: an atom and a shown term can print the same symbol.
// Sorted vectors retain those occurrences while ignoring output order.
type Model = Vec<String>;

/// Immutable, checked producer claims about final selected displays.
#[derive(Debug, PartialEq, Eq)]
pub struct ReportedAnswers {
    satisfiable: bool,
    cost: Option<Vec<i64>>,
    /// Canonical displayed records and their multiplicities, after optN replay removal.
    model_multiplicities: Vec<(Model, u64)>,
    model_count: u64,
    solver: String,
}

fn multiplicities(models: impl Iterator<Item = Model>) -> Result<Vec<(Model, u64)>, Error> {
    let mut counts = BTreeMap::<Model, u64>::new();
    for model in models {
        let count = counts.entry(model).or_default();
        *count = count
            .checked_add(1)
            .ok_or_else(|| invalid(Issue::CountOverflow, "display multiplicity overflow"))?;
    }
    Ok(counts.into_iter().collect())
}

/// Compare satisfiability, final cost, display multiplicities and selected count.
/// Producer names are excluded; hidden interpretation identity is unavailable.
#[must_use]
pub fn same_displays(reference: &ReportedAnswers, native: &ReportedAnswers) -> bool {
    reference.satisfiable == native.satisfiable
        && reference.cost == native.cost
        && reference.model_multiplicities == native.model_multiplicities
        && reference.model_count == native.model_count
}

/// Input and normalization acceptance ceilings. All are inclusive.
///
/// The byte ceiling is checked before JSON decoding. The JSON syntax tree is
/// allocated from that bounded input; witness/symbol ceilings are then checked
/// before canonical witness retention. Native text tokenization holds at most
/// one input-bounded display before its cumulative symbol check. These are not
/// allocator/RSS limits. Canonical sorting costs O(n log n) symbol comparisons,
/// each bounded by the input text; repeated printed symbols remain repeated.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Serialized UTF-8 input bytes, including nonsemantic JSON fields.
    pub max_input_bytes: usize,
    /// Raw witnesses, including optimization incumbents and the optN replay.
    pub max_witnesses: usize,
    /// Total symbol occurrences across raw witnesses, before replay removal.
    pub max_symbols: usize,
    /// Number of entries in each checked objective vector.
    pub max_cost_dimensions: usize,
}
impl Limits {
    /// Derive occurrence ceilings from a caller's already bounded text length.
    ///
    /// Each occurrence consumes at least one input byte. This is useful when
    /// adapting a capture boundary whose byte ceiling is the authored contract.
    #[must_use]
    pub const fn for_bytes(bytes: usize) -> Self {
        Self {
            max_input_bytes: bytes,
            max_witnesses: bytes,
            max_symbols: bytes,
            max_cost_dimensions: bytes,
        }
    }
}
impl Default for Limits {
    fn default() -> Self {
        Self::for_bytes(8 * 1024 * 1024)
    }
}

/// Quantity refused by normalization before returning a checked answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    /// Serialized input bytes.
    InputBytes,
    /// Raw witness occurrences.
    Witnesses,
    /// Printed symbol occurrences before replay removal.
    Symbols,
    /// Entries in one objective vector.
    CostDimensions,
    /// Atoms across native full-model records.
    Atoms,
    /// Closed-value nodes across native full and shown values.
    ValueNodes,
    /// Canonical ASP spelling bytes produced by a consumer view.
    SpellingBytes,
}

/// Stable classification of a malformed or contradictory producer report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Issue {
    /// A required report field is absent.
    MissingField,
    /// A field has an unsupported shape, spelling, or numeric representation.
    MalformedField,
    /// Enumeration or optimality was not reported complete.
    Incomplete,
    /// Status, witnesses, costs, replay, or summary counts disagree.
    Contradiction,
    /// A checked count cannot be represented.
    CountOverflow,
}

/// Typed normalization refusal; diagnostic strings are not an interchange key.
#[derive(Debug)]
pub enum Error {
    /// The retained byte sequence is not UTF-8.
    Utf8(std::str::Utf8Error),
    /// The bounded document is not supported JSON syntax.
    Json(serde_json::Error),
    /// A native flat value is malformed or exceeds its construction limits.
    Value(zetesis_core::ValueError),
    /// A native atom has an invalid signature or arity.
    Atom(zetesis_core::ConstructionError),
    /// A typed native name cannot be represented as an unambiguous ASP identifier.
    Identifier(themelios_program::symbol::NotAnIdentifier),
    /// Bounded retained normalization storage could not be reserved.
    Allocation,
    /// An inclusive normalization ceiling was exceeded.
    Limit {
        /// Refused quantity.
        resource: Resource,
        /// Configured inclusive ceiling.
        limit: usize,
        /// Quantity at the first refusal check.
        attempted: usize,
    },
    /// A producer claim cannot be reconciled with its report.
    Invalid {
        /// Stable refusal class.
        issue: Issue,
        /// Human diagnostic only; consumers must not parse this text.
        detail: &'static str,
    },
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Utf8(error) => write!(f, "report UTF-8: {error}"),
            Self::Json(error) => write!(f, "report JSON: {error}"),
            Self::Value(error) => write!(f, "report value: {error}"),
            Self::Atom(error) => write!(f, "report atom: {error}"),
            Self::Identifier(error) => write!(f, "ASP spelling: {error}"),
            Self::Allocation => f.write_str("report storage allocation failed"),
            Self::Limit {
                resource,
                limit,
                attempted,
            } => write!(f, "{resource:?} ceiling {limit} refused {attempted}"),
            Self::Invalid { issue, detail } => write!(f, "{issue:?}: {detail}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Utf8(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Value(error) => Some(error),
            Self::Atom(error) => Some(error),
            Self::Identifier(error) => Some(error),
            _ => None,
        }
    }
}
fn invalid(issue: Issue, detail: &'static str) -> Error {
    Error::Invalid { issue, detail }
}
fn check(resource: Resource, limit: usize, attempted: usize) -> Result<(), Error> {
    if attempted > limit {
        Err(Error::Limit {
            resource,
            limit,
            attempted,
        })
    } else {
        Ok(())
    }
}
fn text(bytes: &[u8], limits: Limits) -> Result<&str, Error> {
    check(Resource::InputBytes, limits.max_input_bytes, bytes.len())?;
    std::str::from_utf8(bytes).map_err(Error::Utf8)
}

impl ReportedAnswers {
    /// Whether the producer reported at least one satisfying interpretation.
    #[must_use]
    pub const fn satisfiable(&self) -> bool {
        self.satisfiable
    }
    /// Final lexicographically minimal reported cost, when optimization applies.
    #[must_use]
    pub fn cost(&self) -> Option<&[i64]> {
        self.cost.as_deref()
    }
    /// Sorted symbol multisets paired with their reported model multiplicities.
    /// Equal displays can originate from different hidden interpretations.
    #[must_use]
    pub fn displays(&self) -> &[(Vec<String>, u64)] {
        &self.model_multiplicities
    }
    /// Selected reported model count after optN discovery removal if applicable.
    #[must_use]
    pub const fn model_count(&self) -> u64 {
        self.model_count
    }
    /// Producer label retained from the report, not an executable identity seal.
    #[must_use]
    pub fn solver(&self) -> &str {
        &self.solver
    }
}

/// Check clingo's JSON complete-enumeration or optN report convention.
///
/// Exactly the first final-cost incumbent discovery is removed in optN. Its
/// complete display multiset must recur among the declared optimal witnesses.
/// Every incumbent is checked for valid symbols, dimensions, and cost ordering.
///
/// # Errors
/// Refuses invalid UTF-8/JSON, limits, incomplete reports, and inconsistent
/// status, costs, witness multiplicities or declared counts.
pub fn clingo_json(bytes: &[u8], limits: Limits) -> Result<ReportedAnswers, Error> {
    clingo::parse(text(bytes, limits)?, limits)
}

/// Check the historical plain native report used by the kr-domains adapter.
///
/// `optimized` is the caller's objective contract. Exhausted weighted native
/// enumeration retains the exact best vector and all ties. This is a legacy
/// presentation protocol, not the native typed model-record interface.
///
/// # Errors
/// Refuses invalid UTF-8, limits, missing/contradictory completion summaries,
/// inconsistent witness counts, and absent or malformed objective vectors.
pub fn native_text(
    bytes: &[u8],
    optimized: bool,
    limits: Limits,
) -> Result<ReportedAnswers, Error> {
    native::parse(text(bytes, limits)?, optimized, limits)
}

/// Tokenize one printed display without interpreting ASP syntax.
///
/// Parentheses and quoted strings prevent internal separators from splitting
/// symbols. `comma_separated` additionally accepts top-level commas, as used by
/// the legacy annotation contract. Symbol occurrences are sorted and retained.
///
/// # Errors
/// Refuses the input/symbol ceiling, unmatched parentheses, or unterminated text.
pub fn split_display(
    value: &str,
    comma_separated: bool,
    limits: Limits,
) -> Result<Vec<String>, Error> {
    text(value.as_bytes(), limits)?;
    let display = display::split_atoms(value, comma_separated)?;
    check(Resource::Symbols, limits.max_symbols, display.len())?;
    Ok(display)
}

/// Parse a whitespace-separated signed 64-bit reported objective vector.
///
/// # Errors
/// Refuses the input/dimension ceiling or a nonrepresentable integer.
pub fn parse_costs(value: &str, limits: Limits) -> Result<Vec<i64>, Error> {
    text(value.as_bytes(), limits)?;
    display::integers(value, limits)
}
