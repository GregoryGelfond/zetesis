//! Conservative argument domains over the exact borrowed themelios program.
//!
//! This crate neither parses nor normalizes, grounds, or solves a program.
//! Finite domains are upper bounds; [`Domain::Unknown`] permits every symbol.
//! [`Status::FixedPoint`] describes abstract convergence, not source admission,
//! finite grounding, or precise correlations between predicate arguments.
//! [`terminal`] independently classifies flat positive terminal definitions,
//! preserving their exact input carriers without rewriting or executing them.
//!
//! ```
//! use themelios_program::program::{Atom, Program, Rule};
//! use themelios_program::symbol::{Name, Sign, Signature, Symbol};
//! use themelios_program::term::Term;
//! use zetesis_domain::{Limits, Status, analyze};
//!
//! let name = Name::new("p").expect("valid constant predicate name");
//! let fact = Rule::fact(Atom::new(name.clone(), [Term::from(Symbol::Number(7))]));
//! let program = Program::of([fact]);
//! let signature = Signature { sign: Sign::Positive, name, arity: 1 };
//! let analysis = analyze(&program, Limits::default());
//! assert_eq!(analysis.status(), Status::FixedPoint);
//! assert!(analysis.belongs_to(&program));
//! assert!(analysis.domain(&signature, 0).permits(&Symbol::Number(7)));
//! assert!(!analysis.domain(&signature, 0).permits(&Symbol::Number(8)));
//! ```
#![forbid(unsafe_code)]

mod analysis;
mod compile;
mod keys;
mod limits;
mod value;

pub mod terminal;

pub use analysis::analyze;
pub use keys::{FactIndex, KeyWork, KeyedRelation, atom_signature, keys};
pub use limits::{Limits, Resource, Statistics, Stop};
pub use value::{Analysis, Argument, Context, Domain, Status, UnknownReason, Widening};
