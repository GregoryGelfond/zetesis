//! Fact-certified forbidden conjunctions retaining only Program coordinates.

use super::{Relations, Work, visit};
use crate::{Cancellation, CandidateRestrictionLimits, Stop};

use std::sync::Arc;
use zetesis_core::{
    CarrierAtom, CarrierFailure, GateAtom, PatternRef, Program, TemplateRef, TemplateTerm,
    catalog::TermRef,
};

pub(crate) struct Restrictions {
    forbidden: Vec<Vec<CarrierAtom>>,
    pub(crate) atoms: usize,
    pub(crate) bytes: usize,
    pub(crate) peak_bytes: usize,
}
pub(crate) struct Attempt {
    pub(crate) result: Result<Restrictions, Stop>,
    pub(crate) work: u64,
}
pub(crate) enum Conflict {
    Unconditional,
    Selected(usize),
}

impl Restrictions {
    pub(crate) fn conjunctions(&self) -> usize {
        self.forbidden.len()
    }
    pub(crate) fn compile(
        program: &Program,
        limits: CandidateRestrictionLimits,
        cancellation: &Cancellation,
    ) -> Attempt {
        let mut work = Work::source(cancellation, limits.max_work);
        let result = compile(program, limits, &mut work);
        Attempt {
            result,
            work: work.statistics.work,
        }
    }
    pub(crate) fn conflict(
        &self,
        necessary: &[CarrierAtom],
        atoms: &[Arc<GateAtom>],
        bits: &[bool],
        max_work: u64,
        cancellation: &Cancellation,
    ) -> (Result<Option<Conflict>, Stop>, u64) {
        let mut work = Work::source(cancellation, max_work);
        let result = self.find_conflict(necessary, atoms, bits, &mut work);
        (result, work.statistics.work)
    }
    fn find_conflict(
        &self,
        necessary: &[CarrierAtom],
        atoms: &[Arc<GateAtom>],
        bits: &[bool],
        work: &mut Work<'_>,
    ) -> Result<Option<Conflict>, Stop> {
        for forbidden in &self.forbidden {
            work.tick()?;
            let mut first = None;
            let mut matched = true;
            for premise in forbidden {
                if held(premise, necessary, work)? {
                    continue;
                }
                let Some(index) = selected_index(premise, atoms, bits, work)? else {
                    matched = false;
                    break;
                };
                first = Some(first.map_or(index, |previous: usize| previous.min(index)));
            }
            if matched {
                return Ok(Some(
                    first.map_or(Conflict::Unconditional, Conflict::Selected),
                ));
            }
        }
        Ok(None)
    }
}

fn held(
    premise: &CarrierAtom,
    necessary: &[CarrierAtom],
    work: &mut Work<'_>,
) -> Result<bool, Stop> {
    let (mut low, mut high) = (0, necessary.len());
    while low < high {
        let middle = low + (high - low) / 2;
        work.tick()?;
        match necessary[middle]
            .atom()
            .compare_ref_with(premise.atom(), || work.tick())?
        {
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Greater => high = middle,
            std::cmp::Ordering::Equal => return Ok(true),
        }
    }
    Ok(false)
}
fn selected_index(
    premise: &CarrierAtom,
    atoms: &[Arc<GateAtom>],
    bits: &[bool],
    work: &mut Work<'_>,
) -> Result<Option<usize>, Stop> {
    let (mut low, mut high) = (0, atoms.len());
    while low < high {
        let middle = low + (high - low) / 2;
        work.tick()?;
        match atoms[middle]
            .atom()
            .compare_ref_with(premise.atom(), || work.tick())?
        {
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Greater => high = middle,
            std::cmp::Ordering::Equal => return Ok(bits[middle].then_some(middle)),
        }
    }
    Ok(None)
}

/// Named live coordinate and metadata allocations. Program payload is shared
/// once by the caller, never charged once per fact or premise. Buffer growth
/// admits old/replacement overlap; allocator bookkeeping is excluded.
struct Storage {
    bytes: u128,
    peak: u128,
    limit: usize,
}
impl Storage {
    fn admit(&mut self, bytes: u128) -> Result<(), Stop> {
        if bytes > self.limit as u128 {
            return Err(Stop::Allocation);
        }
        self.peak = self.peak.max(bytes);
        Ok(())
    }
    fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        let required = values
            .len()
            .checked_add(additional)
            .ok_or(Stop::Allocation)?;
        if required <= values.capacity() {
            return Ok(());
        }
        let target = required.max(values.capacity().saturating_mul(2)).max(4);
        let old = buffer_bytes(values);
        self.admit(self.bytes + target as u128 * size_of::<T>() as u128)?;
        work.charge(values.len().max(1))?;
        values
            .try_reserve_exact(target - values.len())
            .map_err(|_| Stop::Allocation)?;
        self.admit(self.bytes + buffer_bytes(values))?;
        self.bytes = self.bytes - old + buffer_bytes(values);
        Ok(())
    }
    fn release<T>(&mut self, values: Vec<T>) {
        self.bytes -= buffer_bytes(&values);
        drop(values);
    }
    fn remaining(&self) -> Result<usize, Stop> {
        usize::try_from(
            (self.limit as u128)
                .checked_sub(self.bytes)
                .ok_or(Stop::Allocation)?,
        )
        .map_err(|_| Stop::Allocation)
    }
}
fn buffer_bytes<T>(values: &Vec<T>) -> u128 {
    values.capacity() as u128 * size_of::<T>() as u128
}
fn located(
    program: &Program,
    pattern: PatternRef<'_>,
    assignment: &[Option<TermRef<'_>>],
    gates_only: bool,
    remaining_atoms: &mut usize,
    storage: &mut Storage,
    work: &mut Work<'_>,
) -> Result<CarrierAtom, Stop> {
    if *remaining_atoms == 0 {
        return Err(Stop::CarrierLimit);
    }
    work.charge(pattern.terms().len())?;
    let key = pattern.key(assignment).map_err(|_| Stop::InvalidProgram)?;
    let atom = program
        .locate_key_with(&key, gates_only, storage.remaining()?, || work.tick())
        .map_err(|error| match error {
            CarrierFailure::Stopped(stop) => stop,
            CarrierFailure::Storage(zetesis_core::CarrierError::Coordinates) => {
                Stop::InvalidProgram
            }
            CarrierFailure::Storage(_) => Stop::Allocation,
        })?
        .ok_or(Stop::InvalidProgram)?;
    storage.bytes += atom.coordinate_bytes();
    storage.admit(storage.bytes)?;
    *remaining_atoms -= 1;
    Ok(atom)
}

/// Discover and deduplicate actual unconditional facts before they become the
/// immutable input rows for constraint joins. Located coordinates and sorting
/// retain the same storage account used by the later joins.
fn unconditional_facts(
    program: &Program,
    remaining_atoms: &mut usize,
    storage: &mut Storage,
    work: &mut Work<'_>,
) -> Result<Vec<CarrierAtom>, Stop> {
    let mut facts: Vec<CarrierAtom> = Vec::new();
    for template in program.templates() {
        work.tick()?;
        if template.positive().is_empty()
            && template.gate_true().is_empty()
            && template.gate_false().is_empty()
            && template.filters().is_empty()
            && let Some(head) = template.head()
        {
            let atom = located(program, head, &[], false, remaining_atoms, storage, work)?;
            let (mut low, mut high) = (0, facts.len());
            let mut duplicate = false;
            while low < high {
                let middle = low + (high - low) / 2;
                work.tick()?;
                match facts[middle]
                    .atom()
                    .compare_ref_with(atom.atom(), || work.tick())?
                {
                    std::cmp::Ordering::Less => low = middle + 1,
                    std::cmp::Ordering::Greater => high = middle,
                    std::cmp::Ordering::Equal => {
                        duplicate = true;
                        break;
                    }
                }
            }
            if duplicate {
                storage.bytes -= atom.coordinate_bytes();
            } else {
                storage.reserve(&mut facts, 1, work)?;
                work.charge(facts.len() - low + 1)?;
                facts.insert(low, atom);
            }
        }
    }
    Ok(facts)
}

fn compile(
    program: &Program,
    limits: CandidateRestrictionLimits,
    work: &mut Work<'_>,
) -> Result<Restrictions, Stop> {
    let mut eligible = false;
    for template in program.templates() {
        work.tick()?;
        eligible |= template.head().is_none() && template.gate_false().is_empty();
    }
    if !eligible {
        return Ok(Restrictions {
            forbidden: Vec::new(),
            atoms: 0,
            bytes: 0,
            peak_bytes: 0,
        });
    }
    let mut storage = Storage {
        bytes: 0,
        peak: 0,
        limit: limits.max_bytes,
    };
    let mut remaining_atoms = limits.max_atoms;
    let facts = unconditional_facts(program, &mut remaining_atoms, &mut storage, work)?;
    // Only actual unconditional facts enter the shared join; neither possible
    // support nor the entire carrier is supplied as a source truth relation.
    let mut relations = Relations::new();
    storage.bytes += relations.retained_bytes();
    storage.admit(storage.bytes)?;
    for atom in &facts {
        let previous = relations.retained_bytes();
        relations.push_with(
            atom.atom(),
            storage.bytes - previous,
            limits.max_bytes,
            &mut storage.peak,
            work,
        )?;
        storage.bytes = storage.bytes - previous + relations.retained_bytes();
        storage.admit(storage.bytes)?;
    }
    let facts_bytes = storage.bytes;
    let mut forbidden = Vec::new();
    for template in program.templates() {
        work.tick()?;
        if template.head().is_some() || !template.gate_false().is_empty() {
            continue;
        }
        let Some(partition) = lift(program, template, &mut storage, work)? else {
            continue;
        };
        let query = template.with_patterns(&partition.positive, &partition.gates);
        let scratch = visit(
            query,
            &relations,
            super::Gates::Unjudged,
            None,
            super::QueryStorage {
                retained_bytes: storage.bytes,
                max_bytes: limits.max_bytes,
            },
            work,
            |assignment, scratch, work| {
                storage.bytes += scratch;
                storage.admit(storage.bytes)?;
                storage.reserve(&mut forbidden, 1, work)?;
                let mut conjunction = Vec::new();
                storage.reserve(&mut conjunction, query.gate_true().len(), work)?;
                for premise in query.gate_true() {
                    conjunction.push(located(
                        program,
                        premise,
                        assignment,
                        true,
                        &mut remaining_atoms,
                        &mut storage,
                        work,
                    )?);
                }
                work.tick()?;
                forbidden.push(conjunction);
                storage.bytes -= scratch;
                Ok::<(), Stop>(())
            },
        )?;
        storage.admit(storage.bytes + scratch)?;
        storage.bytes -= buffer_bytes(&partition.positive) + buffer_bytes(&partition.gates);
    }
    storage.bytes -= facts_bytes;
    Ok(Restrictions {
        forbidden,
        atoms: limits.max_atoms - remaining_atoms,
        bytes: usize::try_from(storage.bytes).map_err(|_| Stop::Allocation)?,
        peak_bytes: usize::try_from(storage.peak).map_err(|_| Stop::Allocation)?,
    })
}

struct Partition<'a> {
    positive: Vec<PatternRef<'a>>,
    gates: Vec<PatternRef<'a>>,
}
/// Every variable must be bound by nongate facts. Other constraints remain for
/// the membership oracle. The partition copies pattern handles, never payload.
fn lift<'a>(
    program: &Program,
    template: TemplateRef<'a>,
    storage: &mut Storage,
    work: &mut Work<'_>,
) -> Result<Option<Partition<'a>>, Stop> {
    let mut bound = Vec::new();
    storage.reserve(&mut bound, template.variable_count(), work)?;
    work.charge(template.variable_count())?;
    bound.resize(template.variable_count(), false);
    let mut positive = Vec::new();
    let mut gates = Vec::new();
    for pattern in template.positive() {
        work.tick()?;
        if program
            .gate_predicates()
            .binary_search_with(pattern.predicate(), || work.tick())?
            .is_err()
        {
            storage.reserve(&mut positive, 1, work)?;
            for term in pattern.terms() {
                work.tick()?;
                if let TemplateTerm::Variable(variable) = term {
                    *bound.get_mut(variable).ok_or(Stop::InvalidProgram)? = true;
                }
            }
            work.tick()?;
            positive.push(pattern);
        } else {
            storage.reserve(&mut gates, 1, work)?;
            work.tick()?;
            gates.push(pattern);
        }
    }
    let mut complete = true;
    for present in &bound {
        work.tick()?;
        complete &= *present;
    }
    storage.release(bound);
    if !complete {
        storage.bytes -= buffer_bytes(&positive) + buffer_bytes(&gates);
        return Ok(None);
    }
    for pattern in template.gate_true() {
        storage.reserve(&mut gates, 1, work)?;
        work.tick()?;
        gates.push(pattern);
    }
    Ok(Some(Partition { positive, gates }))
}
