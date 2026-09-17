//! Source-certified positive gate conjunctions forbidden in every answer set.

use super::{Relations, Work, source::copy_atom, visit};
use crate::{CandidateRestrictionLimits, Control, Stop};

use std::sync::Arc;
use zetesis_core::{Atom, AtomPattern, GateAtom, Model, Program, Template, Term};

pub(crate) struct Restrictions {
    forbidden: Vec<Vec<Atom>>,
    pub(crate) atoms: usize,
    pub(crate) bytes: usize,
    pub(crate) peak_bytes: usize,
}

pub(crate) struct Attempt {
    pub(crate) result: Result<Restrictions, Stop>,
    pub(crate) work: u64,
}

pub(crate) enum Conflict {
    /// This conjunction has no gate premises and rules out every seed.
    Unconditional,
    /// Every premise stays true until the lowest selected bit is cleared.
    Selected(usize),
}

impl Restrictions {
    pub(crate) fn conjunctions(&self) -> usize {
        self.forbidden.len()
    }

    pub(crate) fn compile(
        program: &Program,
        limits: CandidateRestrictionLimits,
        control: &Control,
    ) -> Attempt {
        let mut work = Work::source(control, limits.max_work);
        let result = compile(program, limits, &mut work);
        Attempt {
            result,
            work: work.statistics.work,
        }
    }

    pub(crate) fn conflict(
        &self,
        atoms: &[Arc<GateAtom>],
        bits: &[bool],
        max_work: u64,
        control: &Control,
    ) -> (Result<Option<Conflict>, Stop>, u64) {
        let mut work = Work::source(control, max_work);
        let result = self.find_conflict(atoms, bits, &mut work);
        (result, work.statistics.work)
    }

    fn find_conflict(
        &self,
        atoms: &[Arc<GateAtom>],
        bits: &[bool],
        work: &mut Work<'_>,
    ) -> Result<Option<Conflict>, Stop> {
        for forbidden in &self.forbidden {
            work.tick()?;
            let mut first = None;
            let mut matched = true;
            for premise in forbidden {
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

fn selected_index(
    premise: &Atom,
    atoms: &[Arc<GateAtom>],
    bits: &[bool],
    work: &mut Work<'_>,
) -> Result<Option<usize>, Stop> {
    let mut start = 0;
    let mut end = atoms.len();
    while start < end {
        let middle = start + (end - start) / 2;
        charge_atom(premise, work)?;
        charge_atom(atoms[middle].atom(), work)?;
        match atoms[middle].atom().cmp(premise) {
            std::cmp::Ordering::Less => start = middle + 1,
            std::cmp::Ordering::Greater => end = middle,
            std::cmp::Ordering::Equal => return Ok(bits[middle].then_some(middle)),
        }
    }
    Ok(None)
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
    let mut remaining_atoms = limits.max_atoms;
    let mut remaining_bytes = limits.max_bytes;
    let mut facts = Vec::new();
    for template in program.templates() {
        work.tick()?;
        if template.positive().is_empty()
            && template.gate_true().is_empty()
            && template.gate_false().is_empty()
            && template.filters().is_empty()
            && let Some(head) = template.head()
        {
            let atom = copy_atom(head, &[], &mut remaining_atoms, &mut remaining_bytes, work)?;
            facts.try_reserve(1).map_err(|_| Stop::Allocation)?;
            facts.push(atom);
        }
    }
    // Canonical ordering is required by the source join's prefix windows. Only
    // actual unconditional facts enter this snapshot. Model::new's ordering,
    // deduplication and allocation are not metered by restriction_work; copied
    // input size is bounded above, and this existing operation is indivisible.
    work.control.poll()?;
    let facts = Model::new(facts);
    work.control.poll()?;
    let mut relations = Relations::new();
    for atom in facts.atoms() {
        work.tick()?;
        relations.push(atom);
    }
    let mut forbidden = Vec::new();
    let mut peak_bytes = limits.max_bytes - remaining_bytes;
    for template in program.templates() {
        work.tick()?;
        if template.head().is_some() || !template.gate_false().is_empty() {
            continue;
        }
        if let Some((template, temporary_bytes)) = lift(program, template, remaining_bytes, work)? {
            let mut available_bytes = remaining_bytes - temporary_bytes;
            peak_bytes = peak_bytes.max(limits.max_bytes - available_bytes);
            visit(
                &template,
                &relations,
                None,
                None,
                work,
                |assignment, work| {
                    available_bytes = available_bytes
                        .checked_sub(size_of::<Vec<Atom>>())
                        .ok_or(Stop::Allocation)?;
                    work.charge(size_of::<Vec<Atom>>())?;
                    let mut conjunction = Vec::new();
                    conjunction
                        .try_reserve_exact(template.gate_true().len())
                        .map_err(|_| Stop::Allocation)?;
                    for premise in template.gate_true() {
                        conjunction.push(copy_atom(
                            premise,
                            assignment,
                            &mut remaining_atoms,
                            &mut available_bytes,
                            work,
                        )?);
                    }
                    forbidden.try_reserve(1).map_err(|_| Stop::Allocation)?;
                    forbidden.push(conjunction);
                    peak_bytes = peak_bytes.max(limits.max_bytes - available_bytes);
                    Ok::<(), Stop>(())
                },
            )?;
            remaining_bytes = available_bytes + temporary_bytes;
        }
    }
    Ok(Restrictions {
        forbidden,
        atoms: limits.max_atoms - remaining_atoms,
        bytes: limits.max_bytes - remaining_bytes,
        peak_bytes,
    })
}

/// Move only ordinary gate predicates into the positive candidate premises.
/// Every variable must still be bound by fact-side positive patterns; otherwise
/// decline this constraint rather than treating an unknown binding as false.
fn lift(
    program: &Program,
    template: &Template,
    max_bytes: usize,
    work: &mut Work<'_>,
) -> Result<Option<(Template, usize)>, Stop> {
    let mut bound = Vec::new();
    bound
        .try_reserve_exact(template.variable_count())
        .map_err(|_| Stop::Allocation)?;
    bound.resize(template.variable_count(), false);
    let mut fact_patterns = 0;
    for pattern in template.positive() {
        work.tick()?;
        if program
            .gate_predicates()
            .binary_search(pattern.predicate())
            .is_err()
        {
            fact_patterns += 1;
            for term in pattern.terms() {
                work.tick()?;
                if let Term::Variable(variable) = term {
                    bound[*variable] = true;
                }
            }
        }
    }
    for &bound in &bound {
        work.tick()?;
        if !bound {
            return Ok(None);
        }
    }
    let mut bytes = size_of::<Template>();
    for pattern in template.positive().iter().chain(template.gate_true()) {
        bytes = bytes
            .checked_add(pattern_bytes(pattern)?)
            .ok_or(Stop::Allocation)?;
    }
    for filter in template.filters() {
        let (left, right) = filter.terms();
        bytes = bytes
            .checked_add(size_of_val(filter))
            .and_then(|n| n.checked_add(term_payload(left)))
            .and_then(|n| n.checked_add(term_payload(right)))
            .ok_or(Stop::Allocation)?;
    }
    if bytes > max_bytes {
        return Err(Stop::Allocation);
    }
    work.charge(bytes)?;
    let mut positive = Vec::new();
    let mut gates = Vec::new();
    positive
        .try_reserve_exact(fact_patterns)
        .map_err(|_| Stop::Allocation)?;
    gates
        .try_reserve_exact(
            (template.positive().len() - fact_patterns)
                .checked_add(template.gate_true().len())
                .ok_or(Stop::Allocation)?,
        )
        .map_err(|_| Stop::Allocation)?;
    gates.extend_from_slice(template.gate_true());
    for pattern in template.positive() {
        if program
            .gate_predicates()
            .binary_search(pattern.predicate())
            .is_ok()
        {
            gates.push(pattern.clone());
        } else {
            positive.push(pattern.clone());
        }
    }
    Ok(Some((
        Template::new(
            None,
            positive,
            gates,
            Vec::new(),
            template.filters().to_vec(),
        ),
        bytes,
    )))
}

fn pattern_bytes(pattern: &AtomPattern) -> Result<usize, Stop> {
    let mut bytes = size_of::<AtomPattern>()
        .checked_add(pattern.predicate().name().len())
        .ok_or(Stop::Allocation)?;
    for term in pattern.terms() {
        bytes = bytes
            .checked_add(size_of::<Term>())
            .and_then(|n| n.checked_add(term_payload(term)))
            .ok_or(Stop::Allocation)?;
    }
    Ok(bytes)
}

fn term_payload(term: &Term) -> usize {
    match term {
        Term::Constant(value) => value.payload_bytes(),
        Term::Variable(_) => 0,
    }
}

fn charge_atom(atom: &Atom, work: &mut Work<'_>) -> Result<(), Stop> {
    work.charge(atom.predicate().name().len())?;
    for value in atom.values() {
        work.tick()?;
        work.charge(value.payload_bytes())?;
    }
    Ok(())
}
