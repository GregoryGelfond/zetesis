//! Explicit budgeted static lowering. This backend profile materializes the
//! whole admitted carrier and all filter-valid ground rules before solving.

use std::fmt;

use crate::atom_interner::{AtomInterner, Failure as InternFailure, Limits as InternLimits};
use crate::carrier::advance;
use crate::{
    AtomCatalog, CarrierError, Model, ModelError, PatternRef, Patterns, Program, Seed, SeedError,
    SeedView, TemplateRef, catalog::TermRef,
};

/// Dense atom index within one [`GroundProgram`]. It is never a symbolic identity.
pub type AtomId = u32;
const WORD_BITS: usize = u32::BITS as usize;
const DEFAULT_MAX_ATOMS: usize = 1_000_000;
const DEFAULT_MAX_GROUND_RULES: usize = 1_000_000;
const DEFAULT_MAX_SUBSTITUTIONS: usize = 10_000_000;

/// Explicit limits for the eager static profile; zero is not unlimited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticLimits {
    /// Maximum number of atom tuples materialized across every predicate.
    pub max_atoms: usize,
    /// Maximum filter-valid rules retained, including constraints.
    pub max_ground_rules: usize,
    /// Maximum complete variable assignments inspected across every template.
    pub max_substitutions: usize,
}
impl Default for StaticLimits {
    fn default() -> Self {
        Self {
            max_atoms: DEFAULT_MAX_ATOMS,
            max_ground_rules: DEFAULT_MAX_GROUND_RULES,
            max_substitutions: DEFAULT_MAX_SUBSTITUTIONS,
        }
    }
}

/// A ground support rule. Every list is sorted and duplicate-free after
/// substitution. Filters have already been checked; constraints have no head.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroundRule {
    head: Option<AtomId>,
    positive: Vec<AtomId>,
    gate_true: Vec<AtomId>,
    gate_false: Vec<AtomId>,
}
impl GroundRule {
    /// Dense consequence ID, or `None` for a constraint.
    #[must_use]
    pub fn head(&self) -> Option<AtomId> {
        self.head
    }
    /// Distinct ordinary positive antecedent IDs.
    #[must_use]
    pub fn positive(&self) -> &[AtomId] {
        &self.positive
    }
    /// Distinct IDs required true in the frozen candidate.
    #[must_use]
    pub fn gate_true(&self) -> &[AtomId] {
        &self.gate_true
    }
    /// Distinct IDs required false in the frozen candidate.
    #[must_use]
    pub fn gate_false(&self) -> &[AtomId] {
        &self.gate_false
    }
}

/// An explicitly eager graph with canonical dense IDs and instance identity.
#[derive(Clone, Debug)]
pub struct GroundProgram {
    program: Program,
    atoms: AtomCatalog,
    rules: Vec<GroundRule>,
    gate_atom_ids: Vec<AtomId>,
}
impl GroundProgram {
    /// Materialize the complete carrier and all substitutions under hard bounds.
    /// Filters use exact equality and ground antecedent duplicates are removed.
    ///
    /// # Errors
    /// Returns [`StaticError`] for count overflow, budget exhaustion, allocation
    /// refusal, or an internal inconsistency. None denotes logical UNSAT.
    pub fn compile(program: &Program, limits: StaticLimits) -> Result<Self, StaticError> {
        match Self::compile_with(program, limits, || Ok::<_, std::convert::Infallible>(())) {
            Ok(graph) => Ok(graph),
            Err(StaticFailure::Static(error)) => Err(error),
            Err(StaticFailure::Stopped(never)) => match never {},
        }
    }

    /// Materialize the same ordered carrier and rule family with caller control.
    /// The callback runs before each dimension, carrier row, gate inspection,
    /// substitution, assignment field, filter and instantiated pattern, and at
    /// the canonical writer's existing checked operation boundaries. It does not
    /// change count ceilings or establish a wall-clock latency bound for allocation,
    /// one term comparison or sorting a rule's admitted antecedents.
    ///
    /// # Errors
    /// Preserves static admission failures and returns the callback's exact error
    /// as [`StaticFailure::Stopped`]. A refused private prefix is dropped; no
    /// partial graph is published and the source program remains unchanged.
    pub fn compile_with<E>(
        program: &Program,
        limits: StaticLimits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Self, StaticFailure<E>> {
        before().map_err(StaticFailure::Stopped)?;
        let mut atom_count = 0usize;
        for predicate in program.predicates() {
            before().map_err(StaticFailure::Stopped)?;
            atom_count = atom_count
                .checked_add(power(program.domain().len(), predicate.arity())?)
                .ok_or(StaticError::CountOverflow)?;
            bound("atoms", atom_count, limits.max_atoms)?;
        }
        if u32::try_from(atom_count).is_err() {
            return Err(StaticError::DenseIdOverflow.into());
        }
        let mut substitution_count = 0usize;
        for template in program.templates() {
            before().map_err(StaticFailure::Stopped)?;
            substitution_count = substitution_count
                .checked_add(power(program.domain().len(), template.variable_count())?)
                .ok_or(StaticError::CountOverflow)?;
            bound(
                "substitutions",
                substitution_count,
                limits.max_substitutions,
            )?;
        }
        // Static admission already bounds the complete carrier. Its tuple
        // writer shares the exact Program vocabulary; only rows and discovery
        // positions are constructed here, never a second term store.
        let mut atoms =
            AtomInterner::for_program(program, usize::MAX).map_err(StaticError::Catalog)?;
        let atom_limits = InternLimits {
            max_atoms: atom_count,
            max_bytes: u128::MAX,
        };
        for atom in program.carrier_atoms() {
            before().map_err(StaticFailure::Stopped)?;
            let atom = atom.map_err(StaticError::Carrier)?;
            atoms
                .entry_atom_with(atom.atom(), atom_limits, &mut before)
                .map_err(intern_failure)?
                .insert_with(atom_limits, &mut before)
                .map_err(intern_failure)?;
        }
        let mut graph = Self {
            program: program.clone(),
            atoms: atoms
                .into_catalog_with(atom_limits, &mut before)
                .map_err(intern_failure)?,
            rules: Vec::new(),
            gate_atom_ids: Vec::new(),
        };
        for (index, atom) in graph.atoms.atoms().iter().enumerate() {
            before().map_err(StaticFailure::Stopped)?;
            if program.contains_gate_atom(atom) {
                graph
                    .gate_atom_ids
                    .try_reserve(1)
                    .map_err(|_| StaticError::Allocation)?;
                graph
                    .gate_atom_ids
                    .push(u32::try_from(index).map_err(|_| StaticError::DenseIdOverflow)?);
            }
        }
        for template in program.templates() {
            graph.compile_template(template, limits.max_ground_rules, &mut before)?;
        }
        Ok(graph)
    }

    fn compile_template<E>(
        &mut self,
        template: TemplateRef<'_>,
        max_rules: usize,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<(), StaticFailure<E>> {
        if self.program.domain().is_empty() && template.variable_count() != 0 {
            return Ok(());
        }
        let mut coordinates = Vec::new();
        coordinates
            .try_reserve_exact(template.variable_count())
            .map_err(|_| StaticError::Allocation)?;
        coordinates.resize(template.variable_count(), 0);
        loop {
            before().map_err(StaticFailure::Stopped)?;
            let mut assignment = Vec::new();
            assignment
                .try_reserve_exact(coordinates.len())
                .map_err(|_| StaticError::Allocation)?;
            for index in &coordinates {
                before().map_err(StaticFailure::Stopped)?;
                assignment.push(
                    self.program
                        .domain()
                        .get(*index)
                        .ok_or(StaticError::InvalidAdmittedProgram)?,
                );
            }
            let mut enabled = true;
            for filter in template.filters() {
                before().map_err(StaticFailure::Stopped)?;
                if !filter
                    .evaluate(assignment.as_slice())
                    .map_err(|_| StaticError::InvalidAdmittedProgram)?
                {
                    enabled = false;
                    break;
                }
            }
            if enabled {
                bound(
                    "ground rules",
                    self.rules
                        .len()
                        .checked_add(1)
                        .ok_or(StaticError::CountOverflow)?,
                    max_rules,
                )?;
                let head = template
                    .head()
                    .map(|pattern| self.instantiate_id(pattern, &assignment, before))
                    .transpose()?;
                let rule = GroundRule {
                    head,
                    positive: self.instantiate_ids(template.positive(), &assignment, before)?,
                    gate_true: self.instantiate_ids(template.gate_true(), &assignment, before)?,
                    gate_false: self.instantiate_ids(template.gate_false(), &assignment, before)?,
                };
                self.rules
                    .try_reserve(1)
                    .map_err(|_| StaticError::Allocation)?;
                self.rules.push(rule);
            }
            if !advance(&mut coordinates, self.program.domain().len()) {
                break;
            }
        }
        Ok(())
    }

    fn instantiate_id<E>(
        &self,
        pattern: PatternRef<'_>,
        assignment: &[TermRef<'_>],
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<AtomId, StaticFailure<E>> {
        before().map_err(StaticFailure::Stopped)?;
        let key = pattern
            .key(assignment)
            .map_err(|_| StaticError::InvalidAdmittedProgram)?;
        let position = self
            .atoms
            .atoms()
            .binary_search_key(&key)
            .map_err(|_| StaticError::InvalidAdmittedProgram)?;
        u32::try_from(position).map_err(|_| StaticError::DenseIdOverflow.into())
    }
    fn instantiate_ids<E>(
        &self,
        patterns: Patterns<'_>,
        assignment: &[TermRef<'_>],
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Vec<AtomId>, StaticFailure<E>> {
        let mut ids = Vec::new();
        ids.try_reserve_exact(patterns.len())
            .map_err(|_| StaticError::Allocation)?;
        for pattern in patterns {
            ids.push(self.instantiate_id(pattern, assignment, before)?);
        }
        ids.sort_unstable();
        ids.dedup();
        Ok(ids)
    }

    /// The immutable source instance from which this graph was compiled.
    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }
    /// Canonical atom occurrences; the original position equals the dense ID.
    #[must_use]
    pub fn atoms(&self) -> crate::catalog::Atoms<'_> {
        self.atoms.atoms()
    }
    /// Shared dense-order atom ownership used by decoded interpretations.
    #[must_use]
    pub fn atom_catalog(&self) -> &AtomCatalog {
        &self.atoms
    }
    /// Number of materialized atom tuples.
    #[must_use]
    pub fn atom_count(&self) -> usize {
        self.atoms.atoms().len()
    }
    /// All filter-valid ground rules in template/substitution order.
    #[must_use]
    pub fn rules(&self) -> &[GroundRule] {
        &self.rules
    }
    /// The entire conservative gate carrier as sorted dense IDs.
    #[must_use]
    pub fn gate_atom_ids(&self) -> &[AtomId] {
        &self.gate_atom_ids
    }
    /// Find a symbolic atom's dense ID without changing the graph.
    #[must_use]
    pub fn atom_id<'a>(&self, atom: impl Into<crate::catalog::AtomRef<'a>>) -> Option<AtomId> {
        let atom = atom.into();
        self.atoms
            .atoms()
            .binary_search(atom)
            .ok()
            .and_then(|index| u32::try_from(index).ok())
    }
    /// Number of u32 words required for one complete interpretation.
    #[must_use]
    pub fn word_count(&self) -> usize {
        self.atom_count().div_ceil(WORD_BITS)
    }

    /// Pack an instance-matched sparse seed, leaving all complement/tail bits zero.
    ///
    /// # Errors
    /// Returns [`SeedError::WrongProgram`] for a foreign seed, or a capacity
    /// refusal. Equal syntax admitted separately does not satisfy identity.
    pub fn seed_words(&self, seed: &Seed) -> Result<Vec<u32>, SeedError> {
        if !self.program.same_instance(seed.program()) {
            return Err(SeedError::WrongProgram);
        }
        let mut words = Vec::new();
        words
            .try_reserve_exact(self.word_count())
            .map_err(|_| SeedError::Allocation)?;
        words.resize(self.word_count(), 0);
        self.seed_words_into(seed.view(), &mut words)?;
        Ok(words)
    }

    /// Pack a borrowed seed into caller-owned, exact-width storage. On success
    /// every output word is replaced, including zero complement and tail bits.
    /// No input tree, payload copy or temporary word vector is constructed.
    /// Core-minted entries resolve by gate position; manual entries use canonical
    /// binary lookup. Both initialize all output words and set each selected bit.
    ///
    /// # Errors
    /// Refuses foreign program identity, then a wrong output word count, before
    /// changing output. Equal source syntax admitted separately is foreign.
    /// An atom absent from the graph reports [`SeedError::OutsideCarrier`]; a
    /// missing indexed position reports [`SeedError::InvalidGatePosition`]. Both
    /// violate the admitted complete-carrier invariant and may leave partial
    /// output, which must not be used as a packed candidate.
    pub fn seed_words_into(&self, seed: SeedView<'_>, words: &mut [u32]) -> Result<(), SeedError> {
        if !self.program.same_instance(seed.program()) {
            return Err(SeedError::WrongProgram);
        }
        if words.len() != self.word_count() {
            return Err(SeedError::WordCount {
                expected: self.word_count(),
                actual: words.len(),
            });
        }
        words.fill(0);
        for atom in seed.entries() {
            let id = atom.resolve_in(self)?;
            let index = id as usize;
            words[index / WORD_BITS] |= 1 << (index % WORD_BITS);
        }
        Ok(())
    }

    /// Decode exact words after validating length and zero unused tail bits.
    /// The empty graph accepts the empty slice. The result alone claims no stability.
    /// The interpretation shares this graph's whole atom catalog and retains
    /// only a canonical selected-position vector of its own. No atom is cloned.
    ///
    /// # Errors
    /// Returns [`WordError`] for an invalid word count, nonzero padding or
    /// unavailable selected-position storage.
    pub fn model_from_words(&self, words: &[u32]) -> Result<Model, WordError> {
        if words.len() != self.word_count() {
            return Err(WordError::Length {
                expected: self.word_count(),
                actual: words.len(),
            });
        }
        let remainder = self.atom_count() % WORD_BITS;
        if remainder != 0 && words.last().is_some_and(|word| word >> remainder != 0) {
            return Err(WordError::TailBits);
        }
        Model::from_positions(
            &self.atoms,
            (0..self.atom_count())
                .filter(|index| words[index / WORD_BITS] & (1 << (index % WORD_BITS)) != 0),
        )
        .map_err(WordError::Model)
    }

    /// Decode an arbitrary interpretation with this graph's atom-index meanings.
    /// This is the accurately named counterpart of [`Self::model_from_words`];
    /// no satisfaction or stability check occurs. Decoding scans the whole carrier
    /// and selects positions in canonical order without cloning atom payloads.
    /// Index storage is fallible; the shared Arc envelope allocation is infallible.
    /// The result retains the entire atom catalog after the graph is dropped.
    ///
    /// # Errors
    /// Refuses an incorrect word count, nonzero tail padding or unavailable
    /// selection storage. Raw words carry
    /// no instance identity; callers must establish their correspondence to this graph.
    pub fn interpretation_from_words(
        &self,
        words: &[u32],
    ) -> Result<crate::Interpretation, WordError> {
        self.model_from_words(words)
    }
}

fn intern_failure<E>(error: InternFailure<E>) -> StaticFailure<E> {
    let error = match error {
        InternFailure::Catalog(error) => StaticError::Catalog(error),
        InternFailure::Atoms { required, limit } => StaticError::LimitExceeded {
            resource: "atoms",
            actual: required,
            limit,
        },
        InternFailure::Allocation(_) => StaticError::Allocation,
        InternFailure::Overflow | InternFailure::Bytes { .. } => StaticError::CountOverflow,
        InternFailure::Stopped(error) => return StaticFailure::Stopped(error),
    };
    StaticFailure::Static(error)
}

fn power(base: usize, exponent: usize) -> Result<usize, StaticError> {
    let mut result = 1usize;
    for _ in 0..exponent {
        result = result.checked_mul(base).ok_or(StaticError::CountOverflow)?;
    }
    Ok(result)
}
fn bound(resource: &'static str, actual: usize, limit: usize) -> Result<(), StaticError> {
    if actual > limit {
        Err(StaticError::LimitExceeded {
            resource,
            actual,
            limit,
        })
    } else {
        Ok(())
    }
}

/// Controlled static compilation refused before publishing a graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StaticFailure<E> {
    /// Existing count, allocation, catalog or admitted-program failure.
    Static(StaticError),
    /// Exact caller cancellation, deadline or injected refusal.
    Stopped(E),
}
impl<E> From<StaticError> for StaticFailure<E> {
    fn from(error: StaticError) -> Self {
        Self::Static(error)
    }
}
impl<E: fmt::Display> fmt::Display for StaticFailure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Static(error) => error.fmt(f),
            Self::Stopped(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for StaticFailure<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Static(error) => error,
            Self::Stopped(error) => error,
        })
    }
}

/// Static compilation failed before any graph could be accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StaticError {
    /// A finite Cartesian count exceeded the host integer representation.
    CountOverflow,
    /// The dense representation cannot identify every atom with a u32 ID.
    DenseIdOverflow,
    /// A configured materialization bound was exceeded.
    LimitExceeded {
        /// The counted representation or work category.
        resource: &'static str,
        /// Actual or proposed count.
        actual: usize,
        /// Configured maximum.
        limit: usize,
    },
    /// A vector's storage could not be reserved.
    Allocation,
    /// The streaming atom carrier refused tuple storage.
    Carrier(CarrierError),
    /// Canonical term or atom storage refused publication.
    Catalog(crate::catalog::Error),
    /// An admitted pattern could not be instantiated inside its carrier.
    InvalidAdmittedProgram,
}
impl fmt::Display for StaticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CountOverflow => f.write_str("static Cartesian count overflow"),
            Self::DenseIdOverflow => f.write_str("static atom carrier exceeds dense u32 IDs"),
            Self::LimitExceeded {
                resource,
                actual,
                limit,
            } => write!(f, "static {resource} count {actual} exceeds {limit}"),
            Self::Allocation => f.write_str("static storage could not be reserved"),
            Self::Carrier(error) => error.fmt(f),
            Self::Catalog(error) => error.fmt(f),
            Self::InvalidAdmittedProgram => {
                f.write_str("admitted template escaped its validated carrier")
            }
        }
    }
}
impl std::error::Error for StaticError {}

/// A dense result has an invalid representation, independently of its semantics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WordError {
    /// The buffer has the wrong number of words.
    Length {
        /// Required words.
        expected: usize,
        /// Supplied words.
        actual: usize,
    },
    /// Bits outside the finite atom carrier were set.
    TailBits,
    /// A decoded interpretation could not retain its checked selection.
    Model(ModelError),
}
impl fmt::Display for WordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { expected, actual } => {
                write!(f, "expected {expected} result words, received {actual}")
            }
            Self::TailBits => f.write_str("result contains nonzero bits outside the atom carrier"),
            Self::Model(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for WordError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Model(error) => Some(error),
            Self::Length { .. } | Self::TailBits => None,
        }
    }
}
