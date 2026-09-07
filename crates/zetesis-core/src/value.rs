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
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Predicate {
    name: String,
    arity: usize,
    sign: Sign,
}

impl Predicate {
    /// Construct a signature. Source-language spelling belongs to the adapter.
    ///
    /// # Errors
    /// Returns [`ConstructionError::EmptyPredicateName`] for an empty name.
    pub fn new(name: impl Into<String>, arity: usize) -> Result<Self, ConstructionError> {
        Self::with_sign(name, arity, Sign::Positive)
    }
    /// Construct a signed signature without modifying its name. Equal names
    /// and arities with opposite signs denote distinct logical atoms.
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
        Ok(Self { name, arity, sign })
    }
    /// The exact predicate name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
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
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Atom {
    predicate: Predicate,
    values: Vec<Value>,
}

impl Atom {
    /// Construct an atom with exactly the signature's arity.
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
