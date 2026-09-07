mod cursor;
mod probe;
mod quota;
mod shared_budget;

pub(crate) use quota::{BoundedQuota, LocalQuota, Quota};
pub(crate) use shared_budget::SharedBudget;

#[cfg(test)]
#[path = "../tests/support/finish_contracts.rs"]
mod finish_tests;

pub(crate) use cursor::Cursor;

use crate::{Assignment, Cnf, Control, Incomplete, Literal};

/// Cumulative search ceilings. Zero permits no operation of that kind.
#[derive(Clone, Copy, Debug)]
pub struct SearchLimits {
    /// Initialization, watch visits, literal tests, branching scans and undo steps.
    /// Stable-model enumeration also charges optional certificate work here.
    pub max_work: u64,
    /// Maximum fresh decision frames; flipping an existing frame is backtracking.
    pub max_decisions: u64,
}
impl Default for SearchLimits {
    fn default() -> Self {
        Self {
            max_work: 100_000_000,
            max_decisions: 1_000_000,
        }
    }
}

/// Exact accounting for charged search and certificate operations, including failures.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchStatistics {
    /// Charged primitive operations.
    pub work: u64,
    /// Fresh decision frames opened.
    pub decisions: u64,
    /// Forced assignments, including initial unit clauses.
    pub propagations: u64,
    /// Conflicts encountered, including an initially empty clause.
    pub conflicts: u64,
}

/// A completed classical decision or an explicit incomplete search.
#[derive(Clone, Debug)]
pub enum Solve {
    /// A total satisfying assignment.
    Sat(Assignment),
    /// Both branches of every remaining decision region were refuted.
    Unsat,
    /// The search cannot establish SAT or UNSAT within its available resources.
    Inconclusive(Incomplete),
}

pub(crate) struct Budget<'a, Q = LocalQuota> {
    pub(crate) quota: Q,
    pub(crate) limits: SearchLimits,
    pub(crate) control: &'a Control,
    pub(crate) statistics: SearchStatistics,
}
impl<Q: Quota> Budget<'_, Q> {
    pub(crate) fn tick(&mut self) -> Result<(), Incomplete> {
        self.control.poll()?;
        self.quota
            .work(self.statistics.work, self.limits.max_work)?;
        self.statistics.work += 1;
        Ok(())
    }
    fn decide(&mut self) -> Result<(), Incomplete> {
        self.tick()?;
        self.quota
            .decision(self.statistics.decisions, self.limits.max_decisions)?;
        self.statistics.decisions += 1;
        Ok(())
    }
}

pub(crate) fn increment(counter: &mut u64) -> Result<(), Incomplete> {
    *counter = counter.checked_add(1).ok_or(Incomplete::CounterOverflow)?;
    Ok(())
}

pub(crate) fn storage<T>(count: usize) -> Result<Vec<T>, Incomplete> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Incomplete::Allocation)?;
    Ok(result)
}

fn filled<T: Clone>(
    count: usize,
    value: T,
    budget: &mut Budget<'_, impl Quota>,
) -> Result<Vec<T>, Incomplete> {
    let mut result = storage(count)?;
    for _ in 0..count {
        budget.tick()?;
        result.push(value.clone());
    }
    Ok(result)
}

/// Search a finite CNF with deterministic false-first branching and no recursion.
/// Limits bound all search work, while [`crate::AdmissionLimits`] bound shape.
#[must_use]
pub fn solve(cnf: &Cnf, limits: SearchLimits, control: &Control) -> Solve {
    solve_with_statistics(cnf, limits, control).0
}

/// The same search as [`solve`], retaining accounting even when it is incomplete.
#[must_use]
pub fn solve_with_statistics(
    cnf: &Cnf,
    limits: SearchLimits,
    control: &Control,
) -> (Solve, SearchStatistics) {
    let mut budget = Budget {
        quota: crate::search::LocalQuota,
        limits,
        control,
        statistics: SearchStatistics::default(),
    };
    let outcome = query(cnf, &mut budget);
    (outcome, budget.statistics)
}

pub(crate) fn query(cnf: &Cnf, budget: &mut Budget<'_, impl Quota>) -> Solve {
    match search(cnf, budget) {
        Ok(Some(assignment)) => Solve::Sat(assignment),
        Ok(None) => Solve::Unsat,
        Err(error) => Solve::Inconclusive(error),
    }
}

#[derive(Clone, Copy, Debug)]
struct Decision {
    variable: usize,
    trail_start: usize,
    tried_true: bool,
}

#[derive(Debug)]
struct State {
    base_clauses: usize,
    values: Vec<Option<bool>>,
    trail: Vec<Literal>,
    propagation_head: usize,
    next_position: usize,
    order: Vec<usize>,
    ranks: Vec<usize>,
    decisions: Vec<Decision>,
    positions: Vec<[usize; 2]>,
    // Intrusive watch lists use exactly two nodes per clause. Moving a watch
    // neither allocates nor grows historical per-literal list capacities.
    heads: Vec<Option<usize>>,
    next: Vec<Option<usize>>,
}

impl State {
    fn new(cnf: &Cnf, budget: &mut Budget<'_, impl Quota>) -> Result<Self, Incomplete> {
        budget.tick()?;
        Ok(Self {
            base_clauses: cnf.clauses().len(),
            values: filled(cnf.variables(), None, budget)?,
            trail: storage(cnf.variables())?,
            propagation_head: 0,
            next_position: 0,
            order: Vec::new(),
            ranks: Vec::new(),
            decisions: storage(cnf.variables())?,
            positions: filled(cnf.clauses().len(), [0, 0], budget)?,
            heads: filled(cnf.variables() * 2, None, budget)?,
            next: filled(cnf.clauses().len() * 2, None, budget)?,
        })
    }

    fn assign(&mut self, literal: Literal) -> bool {
        if let Some(value) = self.values[literal.variable()] {
            value == literal.positive()
        } else {
            self.values[literal.variable()] = Some(literal.positive());
            self.trail.push(literal);
            true
        }
    }

    fn value(&self, literal: Literal) -> Option<bool> {
        self.values[literal.variable()].map(|value| value == literal.positive())
    }

    fn link(&mut self, node: usize, literal: Literal) {
        self.next[node] = self.heads[literal.index()];
        self.heads[literal.index()] = Some(node);
    }

    fn initialize(
        &mut self,
        cnf: &Cnf,
        budget: &mut Budget<'_, impl Quota>,
    ) -> Result<bool, Incomplete> {
        for clause in 0..self.base_clauses {
            budget.tick()?;
            match cnf.clauses()[clause].len() {
                0 => return Ok(false),
                1 => {
                    let literal = cnf.clauses()[clause][0];
                    if self.value(literal).is_none() {
                        increment(&mut budget.statistics.propagations)?;
                    }
                    if !self.assign(literal) {
                        return Ok(false);
                    }
                }
                _ => {
                    self.positions[clause] = [0, 1];
                    self.link(clause * 2, cnf.clauses()[clause][0]);
                    self.link(clause * 2 + 1, cnf.clauses()[clause][1]);
                }
            }
        }
        Ok(true)
    }

    fn propagate(
        &mut self,
        cnf: &Cnf,
        budget: &mut Budget<'_, impl Quota>,
    ) -> Result<bool, Incomplete> {
        while self.propagation_head < self.trail.len() {
            budget.tick()?;
            let false_literal = self.trail[self.propagation_head].negated();
            self.propagation_head += 1;
            let mut previous = None;
            let mut cursor = self.heads[false_literal.index()];
            while let Some(node) = cursor {
                budget.tick()?;
                let following = self.next[node];
                let clause = node / 2;
                let slot = node % 2;
                let other_position = self.positions[clause][1 - slot];
                let other = cnf.clauses()[clause][other_position];
                if self.value(other) != Some(true) {
                    if let Some(position) = self.replacement(cnf, clause, slot, budget)? {
                        if let Some(prior) = previous {
                            self.next[prior] = following;
                        } else {
                            self.heads[false_literal.index()] = following;
                        }
                        self.positions[clause][slot] = position;
                        self.link(node, cnf.clauses()[clause][position]);
                        cursor = following;
                        continue;
                    }
                    if self.value(other) == Some(false) {
                        return Ok(false);
                    }
                    increment(&mut budget.statistics.propagations)?;
                    if !self.assign(other) {
                        return Ok(false);
                    }
                }
                previous = Some(node);
                cursor = following;
            }
        }
        Ok(true)
    }

    fn replacement(
        &self,
        cnf: &Cnf,
        clause: usize,
        slot: usize,
        budget: &mut Budget<'_, impl Quota>,
    ) -> Result<Option<usize>, Incomplete> {
        for position in 0..cnf.clauses()[clause].len() {
            budget.tick()?;
            if position != self.positions[clause][slot]
                && position != self.positions[clause][1 - slot]
                && self.value(cnf.clauses()[clause][position]) != Some(false)
            {
                return Ok(Some(position));
            }
        }
        Ok(None)
    }

    fn prioritize(
        &mut self,
        cnf: &Cnf,
        budget: &mut Budget<'_, impl Quota>,
    ) -> Result<(), Incomplete> {
        self.order =
            crate::ordering::variables(&cnf.clauses()[..self.base_clauses], &self.values, budget)?;
        self.ranks = filled(self.values.len(), 0, budget)?;
        for (position, &variable) in self.order.iter().enumerate() {
            budget.tick()?;
            self.ranks[variable] = position;
        }
        Ok(())
    }

    fn branch(&mut self, budget: &mut Budget<'_, impl Quota>) -> Result<bool, Incomplete> {
        while self.next_position < self.order.len() {
            budget.tick()?;
            let variable = self.order[self.next_position];
            self.next_position += 1;
            if self.values[variable].is_none() {
                budget.decide()?;
                self.decisions.push(Decision {
                    variable,
                    trail_start: self.trail.len(),
                    tried_true: false,
                });
                self.assign(Literal::new(variable, false));
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn backtrack(&mut self, budget: &mut Budget<'_, impl Quota>) -> Result<bool, Incomplete> {
        while let Some(mut decision) = self.decisions.pop() {
            budget.tick()?;
            while self.trail.len() > decision.trail_start {
                budget.tick()?;
                let literal = self.trail.pop().ok_or(Incomplete::InvalidWitness)?;
                self.values[literal.variable()] = None;
                self.next_position = self.next_position.min(self.ranks[literal.variable()]);
            }
            self.propagation_head = self.propagation_head.min(self.trail.len());
            if !decision.tried_true {
                decision.tried_true = true;
                self.decisions.push(decision);
                self.assign(Literal::new(decision.variable, true));
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn finish(
        &self,
        cnf: &Cnf,
        budget: &mut Budget<'_, impl Quota>,
    ) -> Result<Assignment, Incomplete> {
        let mut assignment = storage(self.values.len())?;
        for value in &self.values {
            budget.tick()?;
            assignment.push(value.ok_or(Incomplete::InvalidWitness)?);
        }
        // Validate the completed witness against clauses, independently of watches.
        for clause in &cnf.clauses()[..self.base_clauses] {
            let mut satisfied = false;
            for literal in clause {
                budget.tick()?;
                if assignment[literal.variable()] == literal.positive() {
                    satisfied = true;
                    break;
                }
            }
            if !satisfied {
                return Err(Incomplete::InvalidWitness);
            }
        }
        Ok(Assignment(assignment))
    }
}

fn search(
    cnf: &Cnf,
    budget: &mut Budget<'_, impl Quota>,
) -> Result<Option<Assignment>, Incomplete> {
    let mut state = State::new(cnf, budget)?;
    if !state.initialize(cnf, budget)? || !state.propagate(cnf, budget)? {
        increment(&mut budget.statistics.conflicts)?;
        return Ok(None);
    }
    state.prioritize(cnf, budget)?;
    loop {
        budget.tick()?;
        if !state.propagate(cnf, budget)? {
            increment(&mut budget.statistics.conflicts)?;
            if !state.backtrack(budget)? {
                return Ok(None);
            }
        } else if !state.branch(budget)? {
            return state.finish(cnf, budget).map(Some);
        }
    }
}

/// Requested vector slots for one search state and its temporary ordering/output.
/// This includes retained capacity even when fewer variables receive assignments.
pub(crate) fn scratch_bytes(variables: u128, clauses: u128) -> u128 {
    use std::mem::size_of;
    size_of::<State>() as u128
        + variables
            * (size_of::<Option<bool>>()
                + size_of::<Literal>()
                + size_of::<Decision>()
                + 4 * size_of::<usize>()
                + size_of::<u64>()
                + 2 * size_of::<Option<usize>>()
                + size_of::<bool>()) as u128
        + clauses * (size_of::<[usize; 2]>() + 2 * size_of::<Option<usize>>()) as u128
}
