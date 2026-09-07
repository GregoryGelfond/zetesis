//! Synchronous lazy source rounds with iterative, backtracking relational joins.

use std::collections::{BTreeMap, BTreeSet};

use zetesis_core::{
    Atom, AtomPattern, Filter, Model, Predicate, Program, Seed, Template, Term, Value,
};

use crate::{Control, Stop};

mod window;
pub mod source;

/// Exact checking budgets, applied before the next charged operation/insertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Maximum charged template, tuple-probe, gate/filter, and output operations.
    /// Lazy joins also charge bound-prefix inspections and ordered comparisons,
    /// including both compared values' referenced payload bytes. Work counts can
    /// change with the execution algorithm; they are not ground-instance counts.
    pub max_work: u64,
    /// Maximum distinct derived atoms, including pending round outputs.
    pub max_derived_atoms: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_work: 10_000_000,
            max_derived_atoms: 1_000_000,
        }
    }
}

/// Counters for a completed oracle invocation; these are semantic work counts,
/// not a device performance estimate or a count of conceptual ground instances.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    /// Completed source rounds, including the final no-delta round. The empty
    /// program needs no rounds because its source coverage is vacuous.
    pub rounds: u64,
    /// Charged operations, as described by [`Limits::max_work`].
    pub work: u64,
    /// Fully matched enabled/filter-valid bindings visited across all rounds.
    pub bindings: u64,
    /// Distinct atoms in the final least consequence closure.
    pub derived_atoms: usize,
}

/// Exact closure and rejection reasons after a fully covered completion round.
/// A resource stop never constructs this value.
#[derive(Clone, Debug)]
pub struct Check {
    program: Program,
    closure: Model,
    constraint_violated: bool,
    seed_mismatch: bool,
    statistics: Statistics,
}

impl Check {
    /// The immutable instance whose reduct closure was checked. Constant time.
    #[must_use]
    pub const fn program(&self) -> &Program {
        &self.program
    }

    /// Exact least reduct closure, including rejected candidates. Constant-time
    /// borrow; the raw interpretation alone carries no stable-membership claim.
    #[must_use]
    pub const fn interpretation(&self) -> &zetesis_core::Interpretation {
        &self.closure
    }

    /// Transfer an accepted closure into an instance-bound stable receipt without
    /// rechecking membership, cloning atoms, or allocating.
    ///
    /// # Errors
    /// A rejected check is returned intact, retaining both rejection conditions.
    pub fn into_stable_interpretation(self) -> Result<crate::StableInterpretation, Self> {
        if self.accepted() {
            Ok(crate::StableInterpretation::new(self.program, self.closure))
        } else {
            Err(self)
        }
    }

    /// Whether the reconstructed closure is stable for the supplied seed.
    #[must_use]
    pub fn accepted(&self) -> bool {
        !self.constraint_violated && !self.seed_mismatch
    }
    /// The exact least closure of the frozen reduct, even for rejected seeds.
    #[must_use]
    pub fn closure(&self) -> &Model {
        &self.closure
    }
    /// An enabled constraint has a true positive body in the final closure.
    #[must_use]
    pub fn constraint_violated(&self) -> bool {
        self.constraint_violated
    }
    /// The closure's gate-carrier projection differs from the frozen seed.
    #[must_use]
    pub fn seed_mismatch(&self) -> bool {
        self.seed_mismatch
    }
    /// Completed invocation's counters.
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        self.statistics
    }
}

struct Work<'a> {
    control: &'a Control,
    limits: Limits,
    statistics: Statistics,
}

impl Work<'_> {
    fn tick(&mut self) -> Result<(), Stop> {
        self.control.poll()?;
        if self.statistics.work >= self.limits.max_work {
            return Err(Stop::WorkLimit);
        }
        self.statistics.work += 1;
        Ok(())
    }
}

// The closure's Atom order groups signatures, then orders each relation by the
// tuple's Value storage order. Window lookup relies on this construction order;
// ASP term comparison is a different order and must not be used here.
type Relations<'a> = BTreeMap<&'a Predicate, Vec<&'a Atom>>;

/// Compute the exact least positive closure selected by a sparse frozen seed.
/// Joins read only current derived relations. A no-delta round checks every
/// template and constraint before the final gate-carrier seed comparison.
///
/// # Errors
/// Returns [`Stop`] for cancellation, deadlines, budgets, foreign seed identity,
/// or a violated admitted-program invariant; no partial result is accepted.
pub fn check(
    program: &Program,
    seed: &Seed,
    limits: Limits,
    control: &Control,
) -> Result<Check, Stop> {
    control.poll()?;
    if !program.same_instance(seed.program()) {
        return Err(Stop::WrongProgram);
    }
    if program.templates().is_empty() {
        return Ok(Check {
            program: program.clone(),
            closure: Model::default(),
            constraint_violated: false,
            seed_mismatch: false,
            statistics: Statistics::default(),
        });
    }
    let mut work = Work {
        control,
        limits,
        statistics: Statistics::default(),
    };
    let mut closure: BTreeSet<Atom> = BTreeSet::new();
    let mut constraint_violated = false;
    loop {
        work.tick()?;
        let mut relations = Relations::new();
        for atom in &closure {
            work.tick()?;
            relations.entry(atom.predicate()).or_default().push(atom);
        }
        let mut delta = BTreeSet::new();
        for template in program.templates() {
            work.tick()?;
            visit(
                template,
                &relations,
                Some(seed),
                &mut work,
                |assignment, work| {
                    work.tick()?;
                    if let Some(head) = template.head() {
                        let atom = instantiate(head, assignment)?.ok_or(Stop::InvalidProgram)?;
                        if !closure.contains(&atom) && !delta.contains(&atom) {
                            if closure
                                .len()
                                .checked_add(delta.len())
                                .ok_or(Stop::DerivedAtomLimit)?
                                >= limits.max_derived_atoms
                            {
                                return Err(Stop::DerivedAtomLimit);
                            }
                            delta.insert(atom);
                        }
                    } else {
                        constraint_violated = true;
                    }
                    Ok(())
                },
            )?;
        }
        work.statistics.rounds += 1;
        if delta.is_empty() {
            break;
        }
        closure.extend(delta);
    }
    let mut seed_mismatch = false;
    for atom in &closure {
        work.tick()?;
        if program.contains_gate_atom(atom) && !seed.contains(atom) {
            seed_mismatch = true;
        }
    }
    for atom in seed.atoms() {
        work.tick()?;
        if !closure.contains(atom) {
            seed_mismatch = true;
        }
    }
    work.statistics.derived_atoms = closure.len();
    let closure = Model::new(closure);
    control.poll()?;
    Ok(Check {
        program: program.clone(),
        closure,
        constraint_violated,
        seed_mismatch,
        statistics: work.statistics,
    })
}

fn visit<E: From<Stop>>(
    template: &Template,
    relations: &Relations<'_>,
    seed: Option<&Seed>,
    work: &mut Work<'_>,
    mut emit: impl FnMut(&[Option<Value>], &mut Work<'_>) -> Result<(), E>,
) -> Result<(), E> {
    let mut assignment = vec![None; template.variable_count()];
    if !guards(template, &assignment, seed, work)? {
        return Ok(());
    }
    let count = template.positive().len();
    if count == 0 {
        if !assignment.is_empty() {
            return Err(Stop::InvalidProgram.into());
        }
        work.statistics.bindings += 1;
        return emit(&assignment, work);
    }
    // None means this depth has not yet been opened for the current parent
    // assignment. A retained range advances in the original relation order.
    let mut cursors = vec![None; count];
    let mut undo: Vec<Vec<usize>> = vec![Vec::new(); count];
    let mut depth = 0;
    loop {
        work.tick()?;
        if depth == count {
            if assignment.iter().any(Option::is_none) {
                return Err(Stop::InvalidProgram.into());
            }
            work.statistics.bindings += 1;
            emit(&assignment, work)?;
            depth -= 1;
            clear(&mut assignment, &mut undo[depth]);
            continue;
        }
        let pattern = &template.positive()[depth];
        let tuples = relations
            .get(pattern.predicate())
            .map_or(&[][..], Vec::as_slice);
        let cursor = &mut cursors[depth];
        if cursor.is_none() {
            *cursor = Some(window::matching_prefix(pattern, tuples, &assignment, work)?);
        }
        let Some(index) = cursor.as_mut().and_then(Iterator::next) else {
            *cursor = None;
            if depth == 0 {
                return Ok(());
            }
            depth -= 1;
            clear(&mut assignment, &mut undo[depth]);
            continue;
        };
        let atom = tuples[index];
        if bind(pattern, atom, &mut assignment, &mut undo[depth], work)?
            && guards(template, &assignment, seed, work)?
        {
            depth += 1;
        } else {
            clear(&mut assignment, &mut undo[depth]);
        }
    }
}

fn bind(
    pattern: &AtomPattern,
    atom: &Atom,
    assignment: &mut [Option<Value>],
    undo: &mut Vec<usize>,
    work: &mut Work<'_>,
) -> Result<bool, Stop> {
    for (term, value) in pattern.terms().iter().zip(atom.values()) {
        structural_work(value, work)?;
        match term {
            Term::Constant(expected) if expected != value => return Ok(false),
            Term::Variable(variable) => match &assignment[*variable] {
                Some(expected) if expected != value => return Ok(false),
                Some(_) => {}
                None => {
                    assignment[*variable] = Some(value.clone());
                    undo.push(*variable);
                }
            },
            Term::Constant(_) => {}
        }
    }
    Ok(true)
}

fn structural_work(value: &Value, work: &mut Work<'_>) -> Result<(), Stop> {
    if let Value::Structured(value) = value {
        for _ in 0..value.payload_bytes() {
            work.tick()?;
        }
    }
    Ok(())
}

fn clear(assignment: &mut [Option<Value>], undo: &mut Vec<usize>) {
    for variable in undo.drain(..) {
        assignment[variable] = None;
    }
}

fn resolve<'a>(term: &'a Term, assignment: &'a [Option<Value>]) -> Option<&'a Value> {
    match term {
        Term::Constant(value) => Some(value),
        Term::Variable(variable) => assignment[*variable].as_ref(),
    }
}

fn instantiate(pattern: &AtomPattern, assignment: &[Option<Value>]) -> Result<Option<Atom>, Stop> {
    let Some(values) = pattern
        .terms()
        .iter()
        .map(|term| resolve(term, assignment).cloned())
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };
    Atom::new(pattern.predicate().clone(), values)
        .map(Some)
        .map_err(|_| Stop::InvalidProgram)
}

fn guards(
    template: &Template,
    assignment: &[Option<Value>],
    seed: Option<&Seed>,
    work: &mut Work<'_>,
) -> Result<bool, Stop> {
    for filter in template.filters() {
        work.tick()?;
        let (left, right) = filter.terms();
        if let (Some(left), Some(right)) = (resolve(left, assignment), resolve(right, assignment)) {
            structural_work(left, work)?;
            structural_work(right, work)?;
            let equal = left == right;
            if matches!(filter, Filter::Eq(..)) != equal {
                return Ok(false);
            }
        }
    }
    let Some(seed) = seed else {
        return Ok(true);
    };
    for (patterns, required) in [(template.gate_true(), true), (template.gate_false(), false)] {
        for pattern in patterns {
            work.tick()?;
            if let Some(atom) = instantiate(pattern, assignment)?
                && seed.contains(&atom) != required
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
