//! Generated one-shape programs whose complete families have closed forms.
//!
//! Each family is a few lines of source with one size parameter. The bytes are
//! a pure function of the family and the size, so an observation can name its
//! input exactly without retaining a source file; the size range of each family
//! keeps the source bounded and its contract representable. These programs
//! exercise routes the pinned example corpus does not reach: the closure route,
//! deep derivation, cyclic and stratified negation, and large answer families.
//! Nothing here runs a solver or claims a family beyond the stated counts.

use std::fmt::{self, Write as _};
use std::num::NonZeroU64;
use std::ops::RangeInclusive;

use serde::Serialize;

use crate::examples::Contract;

/// A generated program shape with one integer size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Family {
    /// Independent sets of a path of `n` nodes, choice form: Fib(n+2) answers.
    IndependentChoice,
    /// The same independent sets in the `in/out` negation idiom.
    IndependentNegation,
    /// The negation idiom plus one aggregate constraint, which selects the
    /// formula route by admission.
    IndependentNegationAggregate,
    /// `p(X) | q(X)` over `n` values: 2ⁿ answers.
    Disjunction,
    /// Choose exactly two of `n` and pay one per chosen atom: C(n,2) optimal
    /// answers, every one at cost two.
    Ties,
    /// Transitive closure of a path of `n` nodes given as edge facts.
    TransitivePath,
    /// Transitive closure of the strict order on `n` values.
    TransitiveDense,
    /// A derivation chain of depth `n` over edge facts.
    Chain,
    /// A derivation chain of depth `n` by head arithmetic.
    ChainArithmetic,
    /// Stratified negation over derived atoms: one answer, decided entirely by
    /// the well-founded model.
    Stratified,
    /// `n` facts and `n − 1` two-literal rules over distinct predicates.
    ProducerChain,
    /// Latin squares of order `n` with the first row fixed to `1..n`:
    /// `(n − 1)!` times the number of reduced Latin squares, a constraint
    /// problem in the shape of Sudoku, every cell a choice of one value.
    LatinSquare,
    /// A line walked for `n` steps, one action a step, move or stay,
    /// ending at position `n / 2`: `C(n, n / 2)` plans, the inertia of the
    /// position carried by frame rules from step to step.
    Planning,
}

/// A refused generation request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The size lies outside the family's admitted range.
    Size {
        /// Requested family.
        family: Family,
        /// Requested size.
        size: u32,
    },
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Size { family, size } => write!(
                f,
                "family {} admits sizes {:?}, not {size}",
                family.label(),
                family.sizes()
            ),
        }
    }
}
impl std::error::Error for Error {}

impl Family {
    /// Every family, in presentation order, for a consumer that enumerates
    /// them; the library itself names the families it measures.
    pub const ALL: [Self; 13] = [
        Self::IndependentChoice,
        Self::IndependentNegation,
        Self::IndependentNegationAggregate,
        Self::Disjunction,
        Self::Ties,
        Self::TransitivePath,
        Self::TransitiveDense,
        Self::Chain,
        Self::ChainArithmetic,
        Self::Stratified,
        Self::ProducerChain,
        Self::LatinSquare,
        Self::Planning,
    ];

    /// Stable lowercase name, usable as a file stem.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::IndependentChoice => "independent-choice",
            Self::IndependentNegation => "independent-negation",
            Self::IndependentNegationAggregate => "independent-negation-aggregate",
            Self::Disjunction => "disjunction",
            Self::Ties => "ties",
            Self::TransitivePath => "transitive-path",
            Self::TransitiveDense => "transitive-dense",
            Self::Chain => "chain",
            Self::ChainArithmetic => "chain-arithmetic",
            Self::Stratified => "stratified",
            Self::ProducerChain => "producer-chain",
            Self::LatinSquare => "latin-square",
            Self::Planning => "planning",
        }
    }

    /// Admitted sizes. The upper bounds keep every count below 2⁶⁴, every
    /// source below a mebibyte, and the stratified pattern well formed; the
    /// Latin squares stop where their count is known in closed form.
    #[must_use]
    pub const fn sizes(self) -> RangeInclusive<u32> {
        match self {
            Self::IndependentChoice
            | Self::IndependentNegation
            | Self::IndependentNegationAggregate => 1..=64,
            Self::Disjunction => 1..=63,
            Self::Ties | Self::TransitivePath | Self::TransitiveDense => 2..=1024,
            Self::Chain | Self::ChainArithmetic => 1..=8192,
            Self::Stratified => 8..=4096,
            Self::ProducerChain => 2..=4096,
            Self::LatinSquare => 1..=5,
            Self::Planning => 1..=20,
        }
    }

    /// The program text at `size`, byte-exact and deterministic.
    ///
    /// # Errors
    /// Refuses a size outside [`Self::sizes`].
    pub fn source(self, size: u32) -> Result<String, Error> {
        self.admit(size)?;
        Ok(match self {
            Self::IndependentChoice => format!(
                "node(1..{size}).\n{}{{ in(X) }} :- node(X).\n:- edge(X,Y), in(X), in(Y).\n",
                path_edges("edge", 1, size)
            ),
            Self::IndependentNegation => independent_negation(size),
            Self::IndependentNegationAggregate => {
                format!("{}:- #count{{X:in(X)}} < 0.\n", independent_negation(size))
            }
            Self::Disjunction => format!("d(1..{size}).\np(X) | q(X) :- d(X).\n"),
            Self::Ties => format!(
                "n(1..{size}).\n{{ p(X) }} :- n(X).\n:- #count{{X:p(X)}} != 2.\n#minimize{{ 1,X : p(X) }}.\n"
            ),
            Self::TransitivePath => format!(
                "{}reach(X,Y) :- e(X,Y).\nreach(X,Z) :- reach(X,Y), e(Y,Z).\n",
                path_edges("e", 1, size)
            ),
            Self::TransitiveDense => format!(
                "v(1..{size}).\ne(X,Y) :- v(X), v(Y), X < Y.\nreach(X,Y) :- e(X,Y).\nreach(X,Z) :- reach(X,Y), e(Y,Z).\n"
            ),
            Self::Chain => format!("{}r(0).\nr(Y) :- r(X), e(X,Y).\n", path_edges("e", 0, size)),
            Self::ChainArithmetic => format!("p(0).\np(X+1) :- p(X), X < {size}.\n"),
            Self::Stratified => stratified(size),
            Self::ProducerChain => producer_chain(size),
            Self::LatinSquare => latin_square(size),
            Self::Planning => planning(size),
        })
    }

    /// The complete selected family the program has at `size`. Every admitted
    /// size has at least one answer, so the count is never zero.
    ///
    /// # Errors
    /// Refuses a size outside [`Self::sizes`].
    pub fn contract(self, size: u32) -> Result<Contract, Error> {
        self.admit(size)?;
        let count = match self {
            Self::IndependentChoice
            | Self::IndependentNegation
            | Self::IndependentNegationAggregate => fibonacci(size + 2),
            Self::Disjunction => 1u64 << size,
            Self::Ties => u64::from(size) * u64::from(size - 1) / 2,
            Self::TransitivePath
            | Self::TransitiveDense
            | Self::Chain
            | Self::ChainArithmetic
            | Self::Stratified
            | Self::ProducerChain => 1,
            Self::LatinSquare => latin_squares(size),
            Self::Planning => binomial(size, size / 2),
        };
        let count = NonZeroU64::new(count).ok_or(Error::Size { family: self, size })?;
        Ok(match self {
            Self::Ties => Contract::optimal_family(count, vec![2]),
            _ => Contract::complete_family(count),
        })
    }

    const fn admit(self, size: u32) -> Result<(), Error> {
        if *self.sizes().start() <= size && size <= *self.sizes().end() {
            Ok(())
        } else {
            Err(Error::Size { family: self, size })
        }
    }
}

// Edge facts `name(i,i+1)` for `i` in `first..last`, one line.
fn path_edges(name: &str, first: u32, last: u32) -> String {
    let mut text = String::new();
    for (position, i) in (first..last).enumerate() {
        if position > 0 {
            text.push(' ');
        }
        // Writing to a `String` cannot fail.
        let _ = write!(text, "{name}({i},{}).", i + 1);
    }
    text.push('\n');
    text
}

fn independent_negation(size: u32) -> String {
    format!(
        "node(1..{size}).\n{}in(X) :- node(X), not out(X).\nout(X) :- node(X), not in(X).\n:- edge(X,Y), in(X), in(Y).\n",
        path_edges("edge", 1, size)
    )
}

// Every seventh node from 3 is bad and blocks its successor; a skip edge
// around each node keeps the last node reachable, so the program is
// consistent and its unique answer is fixed by the well-founded model.
fn stratified(size: u32) -> String {
    let bad: Vec<String> = (3..size - 2)
        .step_by(7)
        .map(|node| format!("bad({node})."))
        .collect();
    let skip: Vec<String> = (1..size - 1)
        .map(|node| format!("e({node},{}).", node + 2))
        .collect();
    format!(
        "node(1..{size}).\n{}{}\n{}{}\nblocked(Y) :- bad(X), next(X,Y).\nreach(1).\nreach(Y) :- reach(X), e(X,Y), not blocked(Y).\n:- not reach({size}).\n",
        path_edges("e", 1, size),
        skip.join(" "),
        path_edges("next", 1, size),
        bad.join(" ")
    )
}

fn producer_chain(size: u32) -> String {
    let facts: Vec<String> = (1..=size).map(|i| format!("p{i}.")).collect();
    let rules: Vec<String> = (1..size)
        .map(|i| format!("q{i} :- p{i}, p{}.\n", i + 1))
        .collect();
    format!("{}\n{}", facts.join(" "), rules.concat())
}

// One value in every cell, no value twice in a row or a column, and the
// first row in order; the count is the reduced count times `(n − 1)!`.
fn latin_square(size: u32) -> String {
    format!(
        "n(1..{size}).\n1 {{ cell(R,C,V) : n(V) }} 1 :- n(R), n(C).\n:- cell(R,C1,V), cell(R,C2,V), C1 < C2.\n:- cell(R1,C,V), cell(R2,C,V), R1 < R2.\n:- n(C), not cell(1,C,C).\n#show cell/3.\n"
    )
}

// The reduced Latin squares of orders one through five, those with the
// first row and the first column in order; the squares with the first row
// fixed are `(n − 1)!` times as many, one for each permutation of the first
// column below its first cell.
fn latin_squares(size: u32) -> u64 {
    const REDUCED: [u64; 5] = [1, 1, 1, 4, 56];
    let factorial: u64 = (1..u64::from(size)).product();
    factorial * REDUCED[size as usize - 1]
}

// Move or stay at each step; the position carries by inertia when the
// walker stays and advances when it moves; the goal fixes the end.
fn planning(size: u32) -> String {
    let goal = size / 2;
    format!(
        "step(0..{}).\ntime(0..{size}).\npos(0..{size}).\nat(0,0).\n1 {{ move(T); stay(T) }} 1 :- step(T).\nat(P+1,T+1) :- at(P,T), move(T), pos(P+1).\nat(P,T+1) :- at(P,T), stay(T).\n:- not at({goal},{size}).\n#show move/1.\n",
        size - 1
    )
}

fn binomial(n: u32, k: u32) -> u64 {
    (1..=u64::from(k)).fold(1, |acc, i| acc * (u64::from(n) - i + 1) / i)
}

fn fibonacci(index: u32) -> u64 {
    let (mut previous, mut current) = (0u64, 1u64);
    for _ in 0..index {
        (previous, current) = (current, previous + current);
    }
    previous
}
