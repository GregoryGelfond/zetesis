//! Explicit budgeted static lowering. This backend profile materializes the
//! whole admitted carrier and all filter-valid ground rules before solving.

use std::fmt;

use crate::carrier::advance;
use crate::{Atom, AtomPattern, CarrierError, Model, Program, Seed, SeedError, Template, Value};

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
    atoms: Vec<Atom>,
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
        let mut atom_count = 0usize;
        for predicate in program.predicates() {
            atom_count = atom_count
                .checked_add(power(program.domain().len(), predicate.arity())?)
                .ok_or(StaticError::CountOverflow)?;
            bound("atoms", atom_count, limits.max_atoms)?;
        }
        if u32::try_from(atom_count).is_err() {
            return Err(StaticError::DenseIdOverflow);
        }
        let mut substitution_count = 0usize;
        for template in program.templates() {
            substitution_count = substitution_count
                .checked_add(power(program.domain().len(), template.variable_count())?)
                .ok_or(StaticError::CountOverflow)?;
            bound(
                "substitutions",
                substitution_count,
                limits.max_substitutions,
            )?;
        }
        let mut atoms = Vec::new();
        atoms
            .try_reserve_exact(atom_count)
            .map_err(|_| StaticError::Allocation)?;
        for atom in program.carrier_atoms() {
            atoms.push(atom.map_err(StaticError::Carrier)?);
        }
        let mut graph = Self {
            program: program.clone(),
            atoms,
            rules: Vec::new(),
            gate_atom_ids: Vec::new(),
        };
        for (index, atom) in graph.atoms.iter().enumerate() {
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
            graph.compile_template(template, limits.max_ground_rules)?;
        }
        Ok(graph)
    }

    fn compile_template(
        &mut self,
        template: &Template,
        max_rules: usize,
    ) -> Result<(), StaticError> {
        if self.program.domain().is_empty() && template.variable_count() != 0 {
            return Ok(());
        }
        let mut coordinates = Vec::new();
        coordinates
            .try_reserve_exact(template.variable_count())
            .map_err(|_| StaticError::Allocation)?;
        coordinates.resize(template.variable_count(), 0);
        loop {
            let assignment: Vec<Value> = coordinates
                .iter()
                .map(|index| self.program.domain()[*index].clone())
                .collect();
            let mut enabled = true;
            for filter in template.filters() {
                if !filter
                    .evaluate(&assignment)
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
                    .map(|pattern| self.instantiate_id(pattern, &assignment))
                    .transpose()?;
                let rule = GroundRule {
                    head,
                    positive: self.instantiate_ids(template.positive(), &assignment)?,
                    gate_true: self.instantiate_ids(template.gate_true(), &assignment)?,
                    gate_false: self.instantiate_ids(template.gate_false(), &assignment)?,
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

    fn instantiate_id(
        &self,
        pattern: &AtomPattern,
        assignment: &[Value],
    ) -> Result<AtomId, StaticError> {
        let atom = pattern
            .instantiate(assignment)
            .map_err(|_| StaticError::InvalidAdmittedProgram)?;
        self.atom_id(&atom)
            .ok_or(StaticError::InvalidAdmittedProgram)
    }
    fn instantiate_ids(
        &self,
        patterns: &[AtomPattern],
        assignment: &[Value],
    ) -> Result<Vec<AtomId>, StaticError> {
        let mut ids = Vec::new();
        ids.try_reserve_exact(patterns.len())
            .map_err(|_| StaticError::Allocation)?;
        for pattern in patterns {
            ids.push(self.instantiate_id(pattern, assignment)?);
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
    /// Canonical atoms; slice index equals dense ID.
    #[must_use]
    pub fn atoms(&self) -> &[Atom] {
        &self.atoms
    }
    /// Number of materialized atom tuples.
    #[must_use]
    pub fn atom_count(&self) -> usize {
        self.atoms.len()
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
    pub fn atom_id(&self, atom: &Atom) -> Option<AtomId> {
        self.atoms
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
        for atom in seed.atoms() {
            let Some(id) = self.atom_id(atom) else {
                return Err(SeedError::OutsideCarrier { atom: atom.clone() });
            };
            let index = id as usize;
            words[index / WORD_BITS] |= 1 << (index % WORD_BITS);
        }
        Ok(words)
    }

    /// Decode exact words after validating length and zero unused tail bits.
    /// The empty graph accepts the empty slice. The result alone claims no stability.
    ///
    /// # Errors
    /// Returns [`WordError`] for an invalid word count or nonzero padding.
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
        Ok(Model::new(
            self.atoms
                .iter()
                .enumerate()
                .filter(|(index, _)| words[index / WORD_BITS] & (1 << (index % WORD_BITS)) != 0)
                .map(|(_, atom)| atom.clone()),
        ))
    }

    /// Decode an arbitrary interpretation with this graph's atom-index meanings.
    /// This is the accurately named counterpart of [`Self::model_from_words`];
    /// no satisfaction or stability check occurs. Decoding scans the whole carrier
    /// and clones selected atoms into a canonical set. Costs include their payload
    /// and set comparisons; no compilation occurs. The owned tree uses infallible
    /// allocation, so allocator failure is not represented by [`WordError`].
    ///
    /// # Errors
    /// Refuses an incorrect word count or nonzero tail padding. Raw words carry
    /// no instance identity; callers must establish their correspondence to this graph.
    pub fn interpretation_from_words(
        &self,
        words: &[u32],
    ) -> Result<crate::Interpretation, WordError> {
        self.model_from_words(words)
    }
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
}
impl fmt::Display for WordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { expected, actual } => {
                write!(f, "expected {expected} result words, received {actual}")
            }
            Self::TailBits => f.write_str("result contains nonzero bits outside the atom carrier"),
        }
    }
}
impl std::error::Error for WordError {}
