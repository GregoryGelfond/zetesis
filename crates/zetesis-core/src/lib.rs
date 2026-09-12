//! Device-independent relational vocabulary for zetesis.
//!
//! Programs are immutable admitted templates. A [`Seed`] is an exact sparse true
//! set bound to one program instance; absent tuples are its false complement.
//! [`GroundProgram::compile`] is explicitly eager. Admission, seed construction,
//! and carrier-iterator creation never invoke it. See the
//! [library map](https://github.com/GregoryGelfond/zetesis/blob/main/docs/book/rust/libraries.md)
//! and [grounding architecture](https://github.com/GregoryGelfond/zetesis/blob/main/docs/book/architecture/grounding.md).
//!
//! ```
//! use zetesis_core::{AdmissionLimits, Atom, AtomPattern, GroundProgram,
//!     Predicate, Program, Seed, StaticLimits, Template};
//!
//! let signature = Predicate::new("selected", 0)?;
//! let selected = AtomPattern::new(signature.clone(), vec![])?;
//! // The normalized singleton choice `{selected}.` uses a true candidate gate.
//! let choice = Template::new(Some(selected.clone()), vec![], vec![selected], vec![], vec![]);
//! let program = Program::new(vec![choice], AdmissionLimits::default())?;
//! let seed = Seed::new(&program, [Atom::new(signature, vec![])?])?;
//! let graph = GroundProgram::compile(&program, StaticLimits::default())?;
//! assert_eq!(graph.seed_words(&seed)?, vec![1]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
#![forbid(unsafe_code)]

mod value;
mod structured;
mod template;
mod atom_key;
mod program;
mod candidate;
mod carrier;
mod ground;
pub mod relation;

pub use atom_key::{AtomKey, BindingView};
pub use candidate::{
    GateAtom, GateAtomError, GateAtoms, Interpretation, Model, Seed, SeedAtom, SeedError,
    SeedSelection, SeedSelectionError, SeedView,
};
pub use carrier::{AtomIter, CarrierError};
pub use ground::{AtomId, GroundProgram, GroundRule, StaticError, StaticLimits, WordError};
pub use program::{AdmissionError, AdmissionLimits, AdmissionResource, Program};
pub use template::{AtomPattern, Filter, InstantiationError, Template, Term};
pub use value::{Atom, ConstructionError, Predicate, Sign, Value};

pub use structured::{
    StructuralValue, ValueError, ValueLimits, ValueNode, ValueNodeRef, ValueResource,
};
