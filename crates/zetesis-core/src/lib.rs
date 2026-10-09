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
mod term_order;
mod term_hash;
mod template;
mod atom_key;
mod argument_unification;
mod program;
mod candidate;
mod model;
pub mod catalog;
pub mod retention;
mod identity;
mod ordered_index;
mod atom_lookup;
mod checked_sort;
mod word_mix;
#[cfg(test)]
mod test_support;
pub use catalog::interner as atom_interner;
mod carrier;
mod ground;
pub mod relation;

pub use argument_unification::{UnificationError, UnificationFailure};
pub use atom_key::{AtomKey, BindingView};
pub use atom_lookup::{
    AtomIndex, AtomIndexError, AtomLookup, AtomRow, AtomRows, CatalogIndex, PredicateLookup,
};
pub use candidate::{
    GateAtom, GateAtomError, GateAtoms, GateIndex, GateIndexError, GateIndexFailure, Seed,
    SeedAtom, SeedAtoms, SeedError, SeedSelection, SeedSelectionError, SeedView,
};
pub use carrier::{AtomIter, CarrierAtom, CarrierError, CarrierFailure};
pub use ground::{
    AtomId, GroundProgram, GroundRule, StaticError, StaticFailure, StaticLimits, WordError,
};
pub use model::{
    AtomCatalog, Interpretation, Model, ModelAtoms, ModelError, ModelFailure, ModelIter,
    ModelOrder, ModelPublication, ModelPublicationFailure,
};
pub use program::{
    AdmissionError, AdmissionLimits, AdmissionResource, Domain, FilterRef, Filters, PatternRef,
    PatternTerms, Patterns, Predicates, Program, TemplateRef, TemplateTerm, Templates,
};
pub use template::{
    AtomPattern, Filter, InstantiationError, Template, TemplateCatalog, TemplateCatalogBuilder,
    TemplateCatalogFailure, TemplateCatalogSelection, TemplateComponents, TemplateComponentsRef,
    TemplateRow, Term,
};
pub use value::{Atom, ConstructionError, Predicate, Sign, Value};
pub use word_mix::mix_word;

pub use structured::{
    StructuralValue, ValueError, ValueLimits, ValueNode, ValueNodeRef, ValueResource,
    ValueWriteError,
};
