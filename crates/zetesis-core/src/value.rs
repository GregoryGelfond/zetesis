//! Canonical values, predicate signatures, and arity-checked ground atoms.

use std::fmt;

/// A closed value, with extrema surrounding numeric, string, then symbol
/// storage order. Each finite class uses its ordinary numeric or UTF-8 lexical
/// order; classes never coerce. ASP term order is [`Self::compare_terms`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Value {
    /// The ASP value `#inf`, strictly below every other term.
    Infimum,
    /// A signed source integer.
    Number(i32),
    /// A decoded string.
    String(String),
    /// An uninterpreted constant name.
    Symbol(String),
    /// A validated closed function or tuple, stored without recursive children.
    Structured(crate::StructuralValue),
    /// The ASP value `#sup`, strictly above every other term.
    Supremum,
}

impl Value {
    /// Compare closed terms in ASP order: extrema, numbers, signed constants,
    /// strings, then constructor sign, arity, name and ordered argument values.
    /// This is separate from the canonical storage order of [`Ord`]; no value
    /// coercion or arithmetic evaluation occurs. Structural traversal is iterative.
    /// Extrema are distinct from integers and
    /// from a string or symbolic name containing their printed spelling.
    /// This borrows both terms and allocates nothing. Cost includes the visited
    /// node prefix and the compared text bytes; it is not constant in term size.
    #[must_use]
    pub fn compare_terms(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        match (self, other) {
            (Self::Structured(a), Self::Structured(b)) => a
                .nodes()
                .iter()
                .zip(b.nodes())
                .map(|(a, b)| a.compare(b))
                .find(|order| !order.is_eq())
                .unwrap_or_else(|| a.nodes().len().cmp(&b.nodes().len())),
            (Self::Structured(a), b) => {
                crate::structured::compare_scalar(b, &a.nodes()[0]).reverse()
            }
            (a, Self::Structured(b)) => crate::structured::compare_scalar(a, &b.nodes()[0]),
            (Self::Infimum, Self::Infimum) | (Self::Supremum, Self::Supremum) => Ordering::Equal,
            (Self::Infimum, _) | (_, Self::Supremum) => Ordering::Less,
            (Self::Supremum, _) | (_, Self::Infimum) => Ordering::Greater,
            (Self::Number(left), Self::Number(right)) => left.cmp(right),
            (Self::Symbol(left), Self::Symbol(right))
            | (Self::String(left), Self::String(right)) => left.cmp(right),
            (Self::Number(_), _) | (Self::Symbol(_), Self::String(_)) => Ordering::Less,
            (_, Self::Number(_)) | (Self::String(_), Self::Symbol(_)) => Ordering::Greater,
        }
    }
}

/// The sign of a logical predicate, independent of default negation.
/// Negative predicates represent explicit falsity, not absence of a positive
/// atom. Source adapters must supply the corresponding coherence constraints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sign {
    /// An ordinary predicate occurrence.
    #[default]
    Positive,
    /// A classically negated predicate occurrence, written `-p` in ASP.
    Negative,
}

/// A predicate's name, arity and sign; all three participate in identity.
///
/// The name is shared: every atom a program mints for one predicate refers
/// to the one allocation the program admitted, so comparing two such
/// predicates is a pointer comparison and cloning one is a reference count.
/// Two names spelled alike from different allocations still compare and
/// hash by their bytes, so identity never depends on the sharing.
#[derive(Clone, Debug)]
pub struct Predicate {
    name: std::sync::Arc<str>,
    arity: usize,
    sign: Sign,
}

impl PartialEq for Predicate {
    fn eq(&self, other: &Self) -> bool {
        self.arity == other.arity
            && self.sign == other.sign
            && (std::sync::Arc::ptr_eq(&self.name, &other.name) || self.name == other.name)
    }
}
impl Eq for Predicate {}
impl PartialOrd for Predicate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Predicate {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let name = if std::sync::Arc::ptr_eq(&self.name, &other.name) {
            std::cmp::Ordering::Equal
        } else {
            self.name.cmp(&other.name)
        };
        name.then_with(|| self.arity.cmp(&other.arity))
            .then_with(|| self.sign.cmp(&other.sign))
    }
}
impl std::hash::Hash for Predicate {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (*self.name).hash(state);
        self.arity.hash(state);
        self.sign.hash(state);
    }
}

impl Predicate {
    /// Construct a signature. Source-language spelling belongs to the adapter.
    /// After converting `name` into an owned string, validation is constant time.
    /// String input transfers storage; borrowed text conversion copies its bytes.
    /// No program admission or source-language lexical validation occurs.
    ///
    /// # Errors
    /// Returns [`ConstructionError::EmptyPredicateName`] for an empty name.
    pub fn new(name: impl Into<String>, arity: usize) -> Result<Self, ConstructionError> {
        Self::with_sign(name, arity, Sign::Positive)
    }
    /// Construct a signed signature without modifying its name. Equal names
    /// and arities with opposite signs denote distinct logical atoms.
    /// Has the ownership and conversion costs of [`Self::new`].
    ///
    /// # Errors
    /// Returns [`ConstructionError::EmptyPredicateName`] for an empty name.
    pub fn with_sign(
        name: impl Into<String>,
        arity: usize,
        sign: Sign,
    ) -> Result<Self, ConstructionError> {
        let name = name.into();
        if name.is_empty() {
            return Err(ConstructionError::EmptyPredicateName);
        }
        Ok(Self {
            name: std::sync::Arc::from(name),
            arity,
            sign,
        })
    }
    /// Whether the two predicates share one name allocation, as a program's
    /// atoms of one predicate do after admission.
    #[must_use]
    pub fn shares_name(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.name, &other.name)
    }
    /// The exact predicate name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// The name's bytes, excluding this inline signature and the shared
    /// allocation's reference counts. Constant time. Allocator bookkeeping
    /// is outside it, and a name shared by many predicates is counted by
    /// each.
    #[must_use]
    pub fn name_bytes(&self) -> usize {
        self.name.len()
    }

    /// The number of arguments.
    #[must_use]
    pub fn arity(&self) -> usize {
        self.arity
    }
    /// The predicate's classical sign, independent of any surrounding `not`.
    #[must_use]
    pub fn sign(&self) -> Sign {
        self.sign
    }
}

/// A ground atom, ordered by signature and then by its arity-checked tuple.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Atom {
    predicate: Predicate,
    values: Vec<Value>,
}

impl std::hash::Hash for Atom {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        crate::atom_key::hash_atom(self, state);
    }
}

impl Atom {
    /// Construct an atom with exactly the signature's arity.
    /// Checks vector length in constant time and transfers the supplied signature
    /// and value vector without copying them. No program membership or source
    /// spelling check occurs; input construction costs remain with the caller.
    ///
    /// # Errors
    /// Returns [`ConstructionError::ArityMismatch`] for the wrong tuple length.
    pub fn new(predicate: Predicate, values: Vec<Value>) -> Result<Self, ConstructionError> {
        if predicate.arity() != values.len() {
            return Err(ConstructionError::ArityMismatch {
                expected: predicate.arity(),
                actual: values.len(),
            });
        }
        Ok(Self { predicate, values })
    }
    /// The atom's signature.
    #[must_use]
    pub fn predicate(&self) -> &Predicate {
        &self.predicate
    }
    /// The tuple in argument order.
    #[must_use]
    pub fn values(&self) -> &[Value] {
        &self.values
    }
    /// Named nested capacity, excluding this inline atom and allocator metadata.
    /// Counts the predicate name, actual argument-vector capacity and each
    /// value's payload capacity. Shared structural buffers are conservatively
    /// counted per occurrence. Visits arguments and structural node descriptors.
    /// Returns `None` if the wide accounting sum cannot be represented.
    #[must_use]
    pub fn checked_payload_capacity_bytes(&self) -> Option<u128> {
        let fixed = (self.values.capacity() as u128)
            .checked_mul(std::mem::size_of::<Value>() as u128)?
            .checked_add(self.predicate.name_bytes() as u128)?;
        self.values.iter().try_fold(fixed, |bytes, value| {
            bytes.checked_add(value.checked_payload_capacity_bytes()?)
        })
    }

    /// Refer to the program's shared name for this atom's predicate; `shared`
    /// must be equal to the atom's predicate.
    pub(crate) fn share_predicate(&mut self, shared: Predicate) {
        debug_assert!(
            self.predicate == shared,
            "a shared name spells the same predicate"
        );
        self.predicate = shared;
    }
    pub(crate) fn from_valid_parts(predicate: Predicate, values: Vec<Value>) -> Self {
        Self { predicate, values }
    }
}

/// A local construction refusal, before whole-program admission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstructionError {
    /// A closed value failed bounded structural validation.
    Value(crate::ValueError),
    /// Predicate names cannot be empty.
    EmptyPredicateName,
    /// The supplied argument count differs from the predicate signature.
    ArityMismatch {
        /// Required argument count.
        expected: usize,
        /// Supplied argument count.
        actual: usize,
    },
}
impl fmt::Display for ConstructionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(error) => error.fmt(f),
            Self::EmptyPredicateName => f.write_str("predicate name is empty"),
            Self::ArityMismatch { expected, actual } => write!(
                f,
                "predicate expects {expected} arguments, received {actual}"
            ),
        }
    }
}
impl std::error::Error for ConstructionError {}
