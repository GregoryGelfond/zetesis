//! Complete tuple/head validation over one canonical source authority.

use crate::formula_support::{Context, GroundingWork};

use std::cmp::Ordering;

use crate::ProgramSite;
use zetesis_core::catalog::{AssignmentError, TermKey};

use super::contribution;
use crate::diagnostic::unsupported;
use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_binding::Binding;
use crate::formula_ir::{ChoiceIr, HeadLiteral, HeadMeasure, HeadOperand};
use crate::formula_support::{
    Computation, Counters, Join, SourceSelection, StorageLease, Support, reserve,
};
use crate::{FormulaFailure, FormulaLimits, FormulaResource, ProfileFeature};

/// Only complete positive atom groups establish this certificate. Ordinary
/// choices use atom identity itself as the key; keyed groups establish an exact
/// distinct population, including zero. No source payload crosses this boundary.
#[derive(Debug)]
pub(crate) struct Bijection {
    keys: Keys,
}

#[derive(Debug)]
enum Keys {
    Atoms,
    Tuples(usize),
}

impl Bijection {
    pub(crate) fn accepts_members(&self, members: usize) -> bool {
        match self.keys {
            Keys::Atoms => true,
            Keys::Tuples(count) => count == members,
        }
    }
}

/// Validate the entire instantiated group before the caller derives support or
/// publishes a formula. The ordinary choice pass subsequently replays the same
/// support and assignment. Both passes charge their work. A missing certificate
/// never skips source validation or changes the complete aggregate semantics.
pub(crate) fn validate_group(
    group: &ChoiceIr,
    assignment: &Binding,
    support: &Support,
    budget: &mut Budget,
    context: Context<'_, &mut Computation<'_, '_>>,
) -> Result<Option<Bijection>, FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    let keyed = group
        .elements
        .first()
        .is_some_and(|element| element.key.tuple().is_some());
    let mut bijective = true;
    for element in &group.elements {
        counters.work(limits, location)?;
        if element.key.tuple().is_some() != keyed {
            return Err(unsupported(ProfileFeature::HeadAggregateAlias, location).into());
        }
        bijective &= element.head.positive_atom().is_some();
    }
    if !keyed {
        if group.measure != HeadMeasure::Count && !group.elements.is_empty() {
            return Err(unsupported(ProfileFeature::HeadAggregateAlias, location).into());
        }
        return Ok(bijective.then_some(Bijection { keys: Keys::Atoms }));
    }
    let mut tuples = TupleHeads::new(computation, limits, counters, location)?;
    let mut atoms = SourceSelection::new(computation, limits, counters, location)?;
    for element in &group.elements {
        let terms = element.key.tuple().expect("uniform tuple group checked");
        let mut local = Join::element(
            element,
            assignment,
            support,
            budget,
            Context::new(&*computation, limits, counters, location),
        )?;
        while let Some(binding) = local.next(computation, limits, budget, counters, location)? {
            counters.work(limits, location)?;
            let tuple = crate::formula_assignment::tuple(
                terms,
                &binding,
                computation,
                limits,
                counters,
                location,
            )?;
            if !group.guards.is_empty() {
                counters.work(limits, location)?;
                let value = computation.read().term(&tuple).map_err(|error| {
                    crate::formula_binding::assignment(AssignmentError::Read(error), location)
                })?;
                counters.work(limits, location)?;
                contribution(group.measure, value.child(0), location)?;
            }
            counters.work(limits, location)?;
            let operand = match &element.head.operand {
                HeadOperand::Boolean(value) => HeadOperand::Boolean(*value),
                HeadOperand::Atom(pattern) => {
                    let pattern =
                        computation.static_pattern(*pattern, limits, counters, location)?;
                    let atom = computation.atom(pattern, &binding, limits, counters, location)?;
                    let (position, _) = atoms.insert(
                        &atom,
                        (FormulaResource::Atoms, limits.theory.max_atoms),
                        computation,
                        limits,
                        counters,
                        location,
                    )?;
                    HeadOperand::Atom(position)
                }
            };
            let head = HeadLiteral {
                negation: element.head.negation,
                operand,
            };
            bijective &= tuples.insert(&tuple, head, computation, limits, counters, location)?;
        }
    }
    // For positive atom heads, the checked tuple->head function is onto this
    // selected atom population. Equal finite cardinalities establish the reverse
    // uniqueness condition, so no second atom->tuple payload or index is needed.
    counters.work(limits, location)?;
    bijective &= tuples.len() == atoms.len();
    Ok(bijective.then_some(Bijection {
        keys: Keys::Tuples(atoms.len()),
    }))
}

struct Entry {
    tuple: usize,
    head: HeadLiteral<usize>,
}

/// Distinct tuple roots occupy stable assignment slots. The sorted entries are
/// only a derived search order; they never compare canonical IDs as values.
struct TupleHeads {
    tuples: Binding<'static>,
    entries: Vec<Entry>,
    lease: StorageLease,
}

impl TupleHeads {
    fn new(
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let tuples = Binding::new(computation, limits, counters, location)?;
        let mut lease = computation.lease();
        let header = size_of::<Self>() - size_of::<Binding>();
        lease.observe(header, location)?;
        computation.storage_observed(&lease, 0, header, limits, counters, location)?;
        Ok(Self {
            tuples,
            entries: Vec::new(),
            lease,
        })
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether all occurrences of this tuple have the same complete signed or
    /// Boolean head. Scratch is private to validation and discarded on refusal.
    fn insert(
        &mut self,
        tuple: &TermKey,
        head: HeadLiteral<usize>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        let mut start = 0;
        let mut end = self.entries.len();
        while start < end {
            counters.work(limits, location)?;
            let middle = start + (end - start) / 2;
            let entry = &self.entries[middle];
            let read = computation.read();
            let previous = self.tuples.read(entry.tuple, read, location)?;
            let value = read.term(tuple).map_err(|error| {
                crate::formula_binding::assignment(AssignmentError::Read(error), location)
            })?;
            match previous.compare_ref_with(value, || counters.work(limits, location))? {
                Ordering::Less => start = middle + 1,
                Ordering::Greater => end = middle,
                Ordering::Equal => {
                    counters.work(limits, location)?;
                    return Ok(entry.head == head);
                }
            }
        }
        ceiling(
            FormulaResource::AggregateElements,
            self.entries.len() as u128 + 1,
            limits.aggregate.max_elements as u128,
            location,
        )?;
        let position = self.tuples.len();
        self.tuples
            .extend_scope(position + 1, computation, limits, counters, location)?;
        self.tuples
            .set(position, tuple, limits, counters, location)?;
        reserve(
            &mut self.entries,
            1,
            &mut self.lease,
            size_of::<Self>() - size_of::<Binding>(),
            Context::new(computation, limits, counters, location),
        )?;
        counters.charge_work((self.entries.len() - start) as u128 + 1, limits, location)?;
        self.entries.insert(
            start,
            Entry {
                tuple: position,
                head,
            },
        );
        Ok(true)
    }
}
