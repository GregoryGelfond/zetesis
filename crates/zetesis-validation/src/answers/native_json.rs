//! Checked schema-1 native full-model records, separate from shown values.
//!
//! Success reconciles reported completion, publication, verification, costs and
//! ties. It does not certify solving correctness or executable/source identity.
//! The caller must independently require completed process capture and exit 0.
//! Closed-value shape and native atom construction use the canonical core types;
//! this decoder does not parse ASP or establish source-name lexical validity.
//! The ASP spelling view separately validates every predicate, symbol and function
//! name through the pinned themelios identifier lexer. Serde JSON object decoding
//! retains the last occurrence of a duplicate key; duplicates are not rejected.
//! The result checks that decoded document, not the uniqueness of its key spelling.

mod decode;
mod spelling;
mod values;

use zetesis_core::{Atom, Value, ValueLimits};

use super::{Error, Issue, Limits as AnswerLimits, Resource, check, invalid};

/// Inclusive native-report acceptance and per-value construction ceilings.
/// Initial JSON and temporary decoded node/text allocations are bounded by report
/// input bytes and node ceilings. `value` applies the core constructor's acceptance
/// and scratch accounting after those owned inputs exist; it is not a pre-allocation
/// ceiling for the entire decoder. None of these measures is allocator RSS.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Serialized input, raw records, shown symbols, and cost dimensions.
    pub report: AnswerLimits,
    /// Full atom occurrences across all native model records.
    pub max_atoms: usize,
    /// Combined preorder nodes, including both full and shown values.
    pub max_value_nodes: usize,
    /// Core construction/scratch ceilings for each closed value.
    pub value: ValueLimits,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            report: AnswerLimits::default(),
            max_atoms: 262_144,
            max_value_nodes: 1_048_576,
            value: ValueLimits::default(),
        }
    }
}

/// One producer record, retaining full identity separately from its shown view.
#[derive(Debug)]
pub struct ModelRecord {
    atoms: Vec<Atom>,
    shown_indices: Vec<usize>,
    shown_terms: Vec<Value>,
    costs: Option<Vec<(i32, i64)>>,
}
impl ModelRecord {
    /// Full atom sequence as recorded; each atom occurs at most once.
    #[must_use]
    pub fn full_model(&self) -> &[Atom] {
        &self.atoms
    }
    /// Strictly increasing selected positions in the recorded full atom sequence.
    #[must_use]
    pub fn shown_atom_indices(&self) -> &[usize] {
        &self.shown_indices
    }
    /// Additional closed shown values; occurrences are preserved.
    #[must_use]
    pub fn shown_terms(&self) -> &[Value] {
        &self.shown_terms
    }
    /// Reported costs in distinct descending priority slots, if present.
    /// These are producer evidence, not a constructed objective evaluation.
    #[must_use]
    pub fn costs(&self) -> Option<&[(i32, i64)]> {
        self.costs.as_deref()
    }
}

/// Checked native claims with complete publication of the selected result set.
#[derive(Debug)]
pub struct NativeAnswers {
    records: Vec<ModelRecord>,
    verified_models: u64,
    checked: u64,
    costs: Option<Vec<(i32, i64)>>,
}
impl NativeAnswers {
    /// Records in producer publication order; duplicate model records remain.
    #[must_use]
    pub fn records(&self) -> &[ModelRecord] {
        &self.records
    }
    /// Reported verified models, including discarded optimization incumbents.
    #[must_use]
    pub const fn verified_models(&self) -> u64 {
        self.verified_models
    }
    /// Schema-1 `checked` candidate-progress count, independently of publication.
    /// It does not establish that every proposed candidate completed evaluation.
    #[must_use]
    pub const fn checked(&self) -> u64 {
        self.checked
    }
    /// Final optimum vector when optimization was reported established.
    #[must_use]
    pub fn costs(&self) -> Option<&[(i32, i64)]> {
        self.costs.as_deref()
    }
    /// Whether the completed producer report contains satisfying models.
    #[must_use]
    pub fn satisfiable(&self) -> bool {
        !self.records.is_empty()
    }
    /// Canonical selected displays, preserving symbol and model multiplicities.
    /// Full hidden interpretations are deliberately not part of this view.
    /// `max_bytes` bounds the combined produced symbol spelling bytes; typed
    /// records were already checked by [`parse`].
    ///
    /// # Errors
    /// Refuses invalid identifier spellings, allocation failure or the spelling
    /// byte ceiling before retaining each new symbol.
    pub fn reported_displays(&self, max_bytes: usize) -> Result<super::ReportedAnswers, Error> {
        let mut models = Vec::new();
        models
            .try_reserve_exact(self.records.len())
            .map_err(|_| Error::Allocation)?;
        let mut used = 0usize;
        for record in &self.records {
            let mut symbols = Vec::new();
            symbols
                .try_reserve_exact(record.shown_indices.len() + record.shown_terms.len())
                .map_err(|_| Error::Allocation)?;
            for &index in &record.shown_indices {
                let atom = &record.atoms[index];
                let bytes = spelling::atom_bytes(atom)?;
                admit_spelling(&mut used, bytes, max_bytes)?;
                symbols.push(spelling::atom(atom, bytes)?);
            }
            for value in &record.shown_terms {
                let bytes = spelling::value_bytes(value)?;
                admit_spelling(&mut used, bytes, max_bytes)?;
                symbols.push(spelling::value(value, bytes)?);
            }
            symbols.sort_unstable();
            models.push(symbols);
        }
        Ok(super::ReportedAnswers {
            satisfiable: self.satisfiable(),
            cost: self
                .costs
                .as_ref()
                .map(|costs| costs.iter().map(|(_, value)| *value).collect()),
            model_multiplicities: super::multiplicities(models.into_iter())?,
            model_count: u64::try_from(self.records.len())
                .map_err(|_| invalid(Issue::CountOverflow, "native display count"))?,
            solver: "zetesis".into(),
        })
    }

    /// Canonical ASP spellings of each full atom set, preserving model multiplicity.
    ///
    /// This is a consumer view of typed atoms, not a parser or an inference from
    /// shown values. Atoms and records are sorted for comparison; equal records
    /// remain repeated. `max_bytes` counts the combined atom spelling bytes.
    /// Source identifiers are checked after each atom's byte preflight. The pinned
    /// validator temporarily copies at most twice that atom's longest name bytes,
    /// separately from retained spelling bytes and existing typed values.
    ///
    /// # Errors
    /// Refuses spelling-byte overflow or the inclusive ceiling before retention,
    /// invalid source identifiers, or fallible owned-buffer reservation.
    pub fn full_model_symbols(&self, max_bytes: usize) -> Result<Vec<Vec<String>>, Error> {
        let mut result = Vec::new();
        let mut used = 0usize;
        result
            .try_reserve_exact(self.records.len())
            .map_err(|_| Error::Allocation)?;
        for record in &self.records {
            let mut symbols = Vec::new();
            symbols
                .try_reserve_exact(record.atoms.len())
                .map_err(|_| Error::Allocation)?;
            for atom in &record.atoms {
                let size = spelling::atom_bytes(atom)?;
                used = used.checked_add(size).ok_or_else(|| {
                    invalid(Issue::CountOverflow, "full-model spelling byte count")
                })?;
                check(Resource::SpellingBytes, max_bytes, used)?;
                symbols.push(spelling::atom(atom, size)?);
            }
            symbols.sort_unstable();
            result.push(symbols);
        }
        result.sort_unstable();
        Ok(result)
    }
}

/// Decode native JSON schema 1 with exhausted coverage and a successful outcome.
/// Every full and shown value is validated, even when a consumer uses only full
/// atoms. Failed/incomplete envelopes cannot qualify through retained optimum data.
///
/// # Errors
/// Refuses input/value limits, unsupported schema, partial or contradictory
/// outcomes, malformed typed values, duplicate atoms, invalid shown indices,
/// or inconsistent counts, objective priorities, costs and final ties.
pub fn parse(bytes: &[u8], limits: Limits) -> Result<NativeAnswers, Error> {
    decode::parse(super::text(bytes, limits.report)?, limits)
}

fn admit_spelling(used: &mut usize, bytes: usize, maximum: usize) -> Result<(), Error> {
    let attempted = used
        .checked_add(bytes)
        .ok_or_else(|| invalid(Issue::CountOverflow, "display spelling bytes"))?;
    check(Resource::SpellingBytes, maximum, attempted)?;
    *used = attempted;
    Ok(())
}
