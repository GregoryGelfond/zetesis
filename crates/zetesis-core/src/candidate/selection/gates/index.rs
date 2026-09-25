//! Checked ranks in the full symbolic gate carrier, without tuple enumeration.

use super::GateAtom;
use crate::{CarrierAtom, Program, catalog::AtomRef};
use std::num::NonZeroUsize;

/// Canonical gate-carrier positions for one immutable admitted program.
///
/// The index retains one shared program handle and one offset per gate
/// signature, not the Cartesian atoms. Construction takes O(p log(a + 1)) checked
/// arithmetic operations and O(p) words for p signatures of maximum arity a.
/// Looking up a supplied atom takes O(log(p + 1) + a log(d + 1)) typed comparisons for
/// domain size d; a comparison can inspect structured value payload.
/// The index establishes identity and carrier membership, not possible support
/// or answer-set membership. Its positions also apply to any complete graph of
/// the same program instance.
pub struct GateIndex {
    program: Program,
    offsets: Vec<usize>,
    cardinality: usize,
}

impl GateIndex {
    /// Index an admitted program's full gate carrier symbolically.
    ///
    /// Offset storage is reserved fallibly under the admitted signature count.
    /// No atom payload is copied and no carrier tuple is requested.
    ///
    /// # Errors
    /// Refuses unavailable offset storage or a full carrier cardinality that
    /// cannot fit usize. The latter is the same position boundary as exhausting
    /// [`Program::indexed_gate_atoms`], established without walking the tuples.
    pub fn new(program: &Program) -> Result<Self, GateIndexError> {
        let mut offsets = Vec::new();
        offsets
            .try_reserve_exact(program.gate_predicates().len())
            .map_err(|_| GateIndexError::Allocation)?;
        let mut cardinality = 0_usize;
        for predicate in program.gate_predicates() {
            offsets.push(cardinality);
            cardinality = cardinality
                .checked_add(tuple_count(program.domain().len(), predicate.arity())?)
                .ok_or(GateIndexError::OrdinalOverflow)?;
        }
        Ok(Self {
            program: program.clone(),
            offsets,
            cardinality,
        })
    }

    /// Exact number of atoms in the full symbolic gate carrier.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.cardinality
    }

    /// Whether the full symbolic gate carrier has no atom.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.cardinality == 0
    }

    /// Locate a borrowed atom at its original full-carrier position.
    /// The token retains only Program-bound tuple coordinates, not the supplied
    /// payload. No preceding tuple is enumerated; coordinate reservation is
    /// fallible. Signature order and last-coordinate-fastest mixed radix agree
    /// with the lazy iterator. Supported-subset filtering never renumbers it.
    /// Nullary signatures have one tuple even when the domain is empty.
    ///
    /// # Errors
    /// Refuses outside-carrier input, unavailable coordinate storage or position
    /// arithmetic that cannot fit usize.
    pub fn locate<'a>(&self, atom: impl Into<AtomRef<'a>>) -> Result<GateAtom, GateIndexError> {
        let carrier = self
            .program
            .locate_atom(atom, true)
            .map_err(|_| GateIndexError::Allocation)?
            .ok_or(GateIndexError::OutsideCarrier)?;
        match self.locate_carrier_with(carrier, || Ok::<(), std::convert::Infallible>(())) {
            Ok(atom) => Ok(atom),
            Err(GateIndexFailure::Index(error)) => Err(error),
            Err(GateIndexFailure::Stopped(never)) => match never {},
        }
    }

    /// Mint the full-carrier position while retaining this token's coordinates.
    /// No tuple is looked up again or copied. Applicability requires the exact
    /// admitted Program; equal content from another Program is refused.
    ///
    /// # Errors
    /// Returns an index applicability/arithmetic failure or the first callback
    /// refusal before the corresponding read, rank step or witness publication.
    pub fn locate_carrier_with<E>(
        &self,
        carrier: CarrierAtom,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<GateAtom, GateIndexFailure<E>> {
        let mut checked = || before().map_err(GateIndexFailure::Stopped);
        checked()?;
        if !self.program.same_instance(carrier.program()) {
            return Err(GateIndexFailure::Index(GateIndexError::OutsideCarrier));
        }
        checked()?;
        let signature = self
            .program
            .gate_predicates()
            .binary_search_with(carrier.predicate(), &mut checked)?
            .map_err(|_| GateIndexFailure::Index(GateIndexError::OutsideCarrier))?;
        let mut tuple = 0usize;
        for digit in carrier.coordinates() {
            checked()?;
            tuple = tuple
                .checked_mul(self.program.domain().len())
                .and_then(|rank| rank.checked_add(*digit))
                .ok_or(GateIndexFailure::Index(GateIndexError::OrdinalOverflow))?;
        }
        checked()?;
        let position = self.offsets[signature]
            .checked_add(tuple)
            .and_then(|rank| rank.checked_add(1))
            .and_then(NonZeroUsize::new)
            .ok_or(GateIndexFailure::Index(GateIndexError::OrdinalOverflow))?;
        checked()?;
        Ok(GateAtom { carrier, position })
    }
}

fn tuple_count(radix: usize, arity: usize) -> Result<usize, GateIndexError> {
    match (radix, arity) {
        (_, 0) | (1, _) => Ok(1),
        (0, _) => Ok(0),
        _ => u32::try_from(arity)
            .ok()
            .and_then(|exponent| radix.checked_pow(exponent))
            .ok_or(GateIndexError::OrdinalOverflow),
    }
}

/// Symbolic gate indexing failed before a positional token was returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateIndexError {
    /// An offset or tuple-coordinate vector could not reserve its storage.
    Allocation,
    /// The supplied atom's signature or a value is outside the gate carrier.
    OutsideCarrier,
    /// The full carrier cardinality or a positive position cannot fit usize.
    OrdinalOverflow,
}

impl std::fmt::Display for GateIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Allocation => "gate-position index or coordinate storage could not be reserved",
            Self::OutsideCarrier => "atom lies outside the program's gate carrier",
            Self::OrdinalOverflow => "gate-carrier position cannot be represented",
        })
    }
}

impl std::error::Error for GateIndexError {}

/// Checked positional lookup stopped without publishing a gate witness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GateIndexFailure<E> {
    /// The index's Program or representable position contract was not met.
    Index(GateIndexError),
    /// Caller refusal before an operation.
    Stopped(E),
}
impl<E: std::fmt::Display> std::fmt::Display for GateIndexFailure<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Index(error) => error.fmt(f),
            Self::Stopped(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for GateIndexFailure<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Index(error) => error,
            Self::Stopped(error) => error,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdmissionLimits, AtomPattern, Predicate, Template, Term, Value};

    #[test]
    fn indexed_witness_reuses_coordinate_storage() {
        let pattern = AtomPattern::new(
            Predicate::new("p", 1).unwrap(),
            vec![Term::Constant(Value::Number(7))],
        )
        .unwrap();
        let program = Program::new(
            vec![Template::new(
                Some(pattern.clone()),
                vec![],
                vec![pattern],
                vec![],
                vec![],
            )],
            AdmissionLimits::default(),
        )
        .unwrap();
        let carrier = program.gate_atoms().next().unwrap().unwrap();
        let retained = carrier.clone();
        let index = GateIndex::new(&program).unwrap();
        let gate = index
            .locate_carrier_with(carrier, || Ok::<(), ()>(()))
            .unwrap();
        assert_eq!(
            gate.carrier.coordinates().as_ptr(),
            retained.coordinates().as_ptr()
        );
    }

    #[test]
    fn checked_witness_refuses_every_callback_cutoff() {
        let pattern = AtomPattern::new(
            Predicate::new("p", 1).unwrap(),
            vec![Term::Constant(Value::Number(7))],
        )
        .unwrap();
        let program = Program::new(
            vec![Template::new(
                Some(pattern.clone()),
                vec![],
                vec![pattern],
                vec![],
                vec![],
            )],
            AdmissionLimits::default(),
        )
        .unwrap();
        let carrier = program.gate_atoms().next().unwrap().unwrap();
        let index = GateIndex::new(&program).unwrap();
        let mut calls = 0;
        index
            .locate_carrier_with(carrier.clone(), || {
                calls += 1;
                Ok::<(), usize>(())
            })
            .unwrap();
        for cutoff in 0..calls {
            let mut admitted = 0;
            let result = index.locate_carrier_with(carrier.clone(), || {
                if admitted == cutoff {
                    return Err(cutoff);
                }
                admitted += 1;
                Ok(())
            });
            assert!(matches!(result, Err(GateIndexFailure::Stopped(at)) if at == cutoff));
            assert_eq!(admitted, cutoff);
        }
    }
}
