//! Typed expectations over a producer's complete selected display family.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::answers::ReportedAnswers;

/// Required satisfiability of a runnable example.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Satisfiability {
    /// At least one answer set exists.
    Sat,
    /// No answer set exists.
    Unsat,
}

/// Scope of reported displays selected for contract checking.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    /// Complete ordinary answer-set enumeration, without final-optimum selection.
    All,
    /// Complete enumeration of final optimum ties; incumbents are excluded.
    Optimal,
}

/// Immutable expectations on selected reported displays, retaining symbol
/// multiplicity within witnesses. Required symbols are membership assertions
/// over every selected display, not a general cautious-consequence query.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub(super) satisfiability: Satisfiability,
    pub(super) family: Family,
    pub(super) model_count: Option<u64>,
    pub(super) cost: Option<Vec<i64>>,
    pub(super) witnesses: Vec<Vec<String>>,
    pub(super) required_symbols: Vec<String>,
    pub(super) notes: Vec<String>,
}
impl Contract {
    pub(crate) fn validate(&self) -> Result<(), super::Error> {
        super::document::validate_contract(self)
    }

    /// The complete ordinary family of a satisfiable program with `count`
    /// answers, without witnesses or required symbols. A generated program's
    /// closed-form count is the intended producer.
    #[must_use]
    pub fn complete_family(count: std::num::NonZeroU64) -> Self {
        Self {
            satisfiability: Satisfiability::Sat,
            family: Family::All,
            model_count: Some(count.get()),
            cost: None,
            witnesses: Vec::new(),
            required_symbols: Vec::new(),
            notes: Vec::new(),
        }
    }
    /// Require these complete displayed witnesses in addition to this contract's
    /// count and cost. Symbol multiplicities are retained; ordering is canonicalized.
    ///
    /// # Errors
    /// Refuses witnesses inconsistent with an unsatisfiable contract.
    pub fn with_witnesses(mut self, mut witnesses: Vec<Vec<String>>) -> Result<Self, super::Error> {
        for witness in &mut witnesses {
            witness.sort();
        }
        self.witnesses = witnesses;
        self.validate()?;
        Ok(self)
    }
    /// The complete family of final optimum ties with `count` answers at `cost`.
    #[must_use]
    pub fn optimal_family(count: std::num::NonZeroU64, cost: Vec<i64>) -> Self {
        Self {
            satisfiability: Satisfiability::Sat,
            family: Family::Optimal,
            model_count: Some(count.get()),
            cost: Some(cost),
            witnesses: Vec::new(),
            required_symbols: Vec::new(),
            notes: Vec::new(),
        }
    }
    /// Required satisfiability.
    #[must_use]
    pub const fn satisfiability(&self) -> Satisfiability {
        self.satisfiability
    }
    /// Selected reported family.
    #[must_use]
    pub const fn family(&self) -> Family {
        self.family
    }
    /// Required selected model count, including equal-display occurrences.
    #[must_use]
    pub const fn model_count(&self) -> Option<u64> {
        self.model_count
    }
    /// Required final cost vector, when one was recorded.
    #[must_use]
    pub fn cost(&self) -> Option<&[i64]> {
        self.cost.as_deref()
    }
    /// Canonical displayed multisets that must occur in the selected family.
    #[must_use]
    pub fn witnesses(&self) -> &[Vec<String>] {
        &self.witnesses
    }
    /// Symbols required in every selected display.
    #[must_use]
    pub fn required_symbols(&self) -> &[String] {
        &self.required_symbols
    }
    /// Preserved explanatory notes; these impose no additional assertion.
    #[must_use]
    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    /// Check the producer's reported selected family against these expectations.
    /// The caller must separately check process completion and successful capture.
    /// This does not certify hidden interpretations or recompute ASP semantics.
    ///
    /// # Errors
    /// Identifies the first violated typed expectation.
    pub fn check(&self, answers: &ReportedAnswers) -> Result<(), ContractMismatch> {
        if super::document::validate_contract(self).is_err() {
            return Err(ContractMismatch::InvalidContract);
        }
        if answers.satisfiable() != (self.satisfiability == Satisfiability::Sat) {
            return Err(ContractMismatch::Satisfiability);
        }
        if answers.satisfiable() && (answers.cost().is_some() != (self.family == Family::Optimal)) {
            return Err(ContractMismatch::Family);
        }
        if let Some(expected) = self.model_count
            && answers.model_count() != expected
        {
            return Err(ContractMismatch::Count {
                expected,
                actual: answers.model_count(),
            });
        }
        if let Some(expected) = self.cost()
            && answers.cost() != Some(expected)
        {
            return Err(ContractMismatch::Cost);
        }
        for (index, witness) in self.witnesses.iter().enumerate() {
            if !answers
                .displays()
                .iter()
                .any(|(display, _)| display == witness)
            {
                return Err(ContractMismatch::Witness { index });
            }
        }
        for (index, symbol) in self.required_symbols.iter().enumerate() {
            if answers.displays().is_empty()
                || answers
                    .displays()
                    .iter()
                    .any(|(display, _)| display.binary_search(symbol).is_err())
            {
                return Err(ContractMismatch::RequiredSymbol { index });
            }
        }
        Ok(())
    }
}

/// The precise kind of disagreement with a typed display contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractMismatch {
    /// The supplied typed contract has contradictory fields or noncanonical witnesses.
    InvalidContract,
    /// Satisfiability differs.
    Satisfiability,
    /// Complete ordinary enumeration and final optimum ties were confused.
    Family,
    /// Selected model multiplicity differs.
    Count {
        /// Expected count.
        expected: u64,
        /// Reported count.
        actual: u64,
    },
    /// Final objective vector differs.
    Cost,
    /// An expected displayed multiset is missing.
    Witness {
        /// Zero-based index in the contract's witness list.
        index: usize,
    },
    /// A symbol is absent from at least one selected display.
    RequiredSymbol {
        /// Zero-based index in the contract's required-symbol list.
        index: usize,
    },
}
impl fmt::Display for ContractMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "reported-display contract mismatch: {self:?}")
    }
}
impl std::error::Error for ContractMismatch {}
