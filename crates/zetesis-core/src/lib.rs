//! Device-independent canonical vocabulary and storage for zetesis.
//!
//! Typed values, signed atoms, borrowed template components, and exact models
//! share canonical storage without imposing rule semantics or enumerating a
//! symbolic carrier. Semantic owners validate their own admission policies.
//! See the
//! [library map](https://github.com/GregoryGelfond/zetesis/blob/main/docs/book/rust/libraries.md).
//!
//! ```
//! use zetesis_core::{Atom, Model, Predicate, Value};
//!
//! let selected = Atom::new(Predicate::new("selected", 1)?, vec![Value::Number(7)])?;
//! let model = Model::new([selected.clone(), selected.clone()])?;
//! assert_eq!(model.atoms().len(), 1);
//! assert!(model.contains(&selected));
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
mod admission;
mod model;
pub mod catalog;
pub mod retention;
pub mod table;
mod identity;
mod ordered_index;
mod atom_lookup;
mod checked_sort;
mod word_mix;
#[cfg(test)]
mod test_support;
pub use catalog::interner as atom_interner;
pub mod relation;

pub use admission::AdmissionLimits;
pub use argument_unification::{UnificationError, UnificationFailure};
pub use atom_key::{AtomKey, BindingView};
pub use atom_lookup::{
    AtomIndex, AtomIndexError, AtomLookup, AtomRow, AtomRows, CatalogIndex, PredicateLookup,
};
pub use model::{
    AtomCatalog, Interpretation, Model, ModelAtoms, ModelError, ModelFailure, ModelIter,
    ModelOrder, ModelPublication, ModelPublicationFailure,
};
pub use template::{
    AtomPattern, Filter, FilterRef, Filters, InstantiationError, PatternRef, PatternTerms,
    Patterns, Template, TemplateCatalog, TemplateCatalogBuilder, TemplateCatalogFailure,
    TemplateCatalogSelection, TemplateComponents, TemplateComponentsRef, TemplateRow, TemplateTerm,
    Term,
};
pub use value::{Atom, ConstructionError, Predicate, Sign, Value};
pub use word_mix::mix_word;

pub use structured::{
    StructuralValue, ValueError, ValueLimits, ValueNode, ValueNodeRef, ValueResource,
    ValueWriteError,
};
