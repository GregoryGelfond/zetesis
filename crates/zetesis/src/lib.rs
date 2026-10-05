//! Construct ASP programs, solve them and query their answer sets.
//!
//! [program](mod@program), [`solve`] and [`query`] re-export the pinned themelios crates:
//! their types are the original types, with the same ownership, failures and
//! operation costs. [`prelude`] gathers their working vocabulary with source
//! parsing and analysis. [`Solver`] implements [`solve::contract::Backend`] with
//! CPU execution over zetesis's native solver. [`solve::agent::Agent`] manages
//! the knowledge base, and [`query::AgentReading`] provides complete snapshots
//! and readings. The native `zetesis_solve::Session` API remains available for
//! GPU execution, objective selection, statistics and prepared-input control.
//!
//! The nine construction macros parse ASP during compilation and generate the
//! canonical program constructors. Their resulting values have those
//! constructors' runtime allocation and canonicalization costs. Construction
//! adds no solver admission check: an `#external` or optimization statement can
//! be represented even when the selected engine cannot execute that feature.
//!
//! ```
//! use zetesis::{Program, program};
//!
//! let source: Program = program! { p(1). q(X) :- p(X). };
//! assert_eq!(source.statements().count(), 2);
//! ```
//!
//! Macro invocations need only this crate, including when Cargo renames it.
//! The wrappers select this crate's canonical runtime with `$crate`. A wrapped
//! ASP operator such as `:-` can produce a different reading or an error in
//! rust-analyzer because its macro forwarding changes punctuation spacing.
//! rustc preserves the tokens. The [canonical macro documentation]
//! records this editor limitation and the remaining token-dialect limits.
//!
//! [canonical macro documentation]: https://github.com/GregoryGelfond/themelios/blob/3339a8abed1e00f21e42c2b3ed2677c16248eda5/docs/design/macros.md
#![forbid(unsafe_code)]

/// Native CPU backend, grounding choices and resource configuration.
pub use zetesis_engine as engine;
pub use zetesis_engine::{Config, Grounder, OutputError, OutputLimits, Solver, SymbolStop};

/// Structural program analysis.
pub use themelios_analysis as analysis;
/// Source identities, locations and diagnostics.
pub use themelios_base as base;
/// Canonical program, term, symbol and provenance values.
pub use themelios_program as program;
/// Epistemic readings over solving outcomes.
pub use themelios_query as query;
/// Engine-independent solving contracts, agents and outcomes.
pub use themelios_solve as solve;
/// ASP syntax trees and parsing.
pub use themelios_syntax as syntax;

pub use themelios_program::{Program, Symbol};

/// The canonical construction, parsing, analysis, solving and reading vocabulary.
///
/// `Query` here is the program tier's ASP query statement. The epistemic query
/// remains [`query::Query`]. `WorldView` is the query tier's
/// nonempty live reading; `Snapshot` retains its complete, nonempty family.
/// The native `zetesis_solve::WorldView` is a separate complete-family type
/// whose empty family establishes inconsistency.
pub mod prelude {
    pub use crate::Solver;
    // The analysis prelude already includes the full canonical program prelude.
    pub use themelios_analysis::prelude::*;
    pub use themelios_query::prelude::*;
    pub use themelios_solve::prelude::*;
    pub use themelios_syntax::prelude::*;
}

/// Runtime paths used by the exported construction macros.
#[doc(hidden)]
pub mod __private {
    pub use themelios_macros;
    pub use themelios_program;
}

/// Build a canonical [`program::Atom`] from one head atom.
///
/// Strong negation is accepted; a rule body, choice or disjunction is refused.
#[macro_export]
macro_rules! atom {
    ($($body:tt)*) => {
        $crate::__private::themelios_macros::atom! {
            #![crate = $crate::__private::themelios_program]
            $($body)*
        }
    };
}

/// Build a canonical fact [`program::Rule`] from a head, including a choice or aggregate.
#[macro_export]
macro_rules! fact {
    ($($body:tt)*) => {
        $crate::__private::themelios_macros::fact! {
            #![crate = $crate::__private::themelios_program]
            $($body)*
        }
    };
}

/// Build a canonical [`program::Rule`] from `head :- body`.
#[macro_export]
macro_rules! rule {
    ($($body:tt)*) => {
        $crate::__private::themelios_macros::rule! {
            #![crate = $crate::__private::themelios_program]
            $($body)*
        }
    };
}

/// Build a canonical integrity constraint [`program::Rule`] from `:- body`.
#[macro_export]
macro_rules! constraint {
    ($($body:tt)*) => {
        $crate::__private::themelios_macros::constraint! {
            #![crate = $crate::__private::themelios_program]
            $($body)*
        }
    };
}

/// Build a canonical `#minimize` statement from its braced elements.
#[macro_export]
macro_rules! minimize {
    ($($body:tt)*) => {
        $crate::__private::themelios_macros::minimize! {
            #![crate = $crate::__private::themelios_program]
            $($body)*
        }
    };
}

/// Build a canonical `#maximize` statement from its braced elements.
#[macro_export]
macro_rules! maximize {
    ($($body:tt)*) => {
        $crate::__private::themelios_macros::maximize! {
            #![crate = $crate::__private::themelios_program]
            $($body)*
        }
    };
}

/// Build a canonical `#show` directive from its payload.
#[macro_export]
macro_rules! show {
    ($($body:tt)*) => {
        $crate::__private::themelios_macros::show! {
            #![crate = $crate::__private::themelios_program]
            $($body)*
        }
    };
}

/// Build a canonical `#external` directive from its atom and optional body.
#[macro_export]
macro_rules! external {
    ($($body:tt)*) => {
        $crate::__private::themelios_macros::external! {
            #![crate = $crate::__private::themelios_program]
            $($body)*
        }
    };
}

/// Build a canonical [`Program`] from statements with their ASP terminators.
///
/// Supports rules with every head and body shape, optimization statements,
/// `#show` and `#external`. Weak constraints, `#const`, `#include`, `#project`,
/// `#defined`, `#edge`, `#heuristic`, `#theory`, queries, `#program` and `#script`
/// are compile errors. The program tier's typed constructors represent the
/// additional statement families. An empty block builds `Program::empty()`.
#[macro_export]
macro_rules! program {
    ($($body:tt)*) => {
        $crate::__private::themelios_macros::program! {
            #![crate = $crate::__private::themelios_program]
            $($body)*
        }
    };
}
