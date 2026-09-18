mod cursor;
mod probe;
mod prepared;
pub(crate) use prepared::PreparedWorkspace;
mod quota;
mod shared_budget;
mod watch_node;

#[cfg(test)]
#[path = "../tests/support/search_workspace.rs"]
mod workspace_tests;

use watch_node::WatchNode;

pub(crate) use quota::{BoundedQuota, LocalQuota, Quota};
pub(crate) use shared_budget::SharedBudget;

#[cfg(test)]
#[path = "../tests/support/finish_contracts.rs"]
mod finish_tests;

#[cfg(test)]
#[path = "../tests/support/watch_contracts.rs"]
mod watch_tests;

#[cfg(test)]
#[path = "../tests/support/watch_traces.rs"]
mod watch_traces;

#[cfg(test)]
#[path = "../tests/support/propagation_profile.rs"]
mod propagation_profile;

#[cfg(test)]
#[path = "../tests/support/binary_watch_contracts.rs"]
mod binary_watch_tests;

#[cfg(test)]
#[path = "../tests/support/ternary_watch_contracts.rs"]
mod ternary_watch_tests;

pub(crate) use cursor::Cursor;

use crate::{Assignment, Cnf, Control, Incomplete, Literal};

/// Cumulative search ceilings. Zero permits no operation of that kind.
#[derive(Clone, Copy, Debug)]
pub struct SearchLimits {
    /// Initialization, watch visits, literal tests, branching scans and undo steps.
    /// Stable-model enumeration also charges optional certificate work here.
    /// Binary clauses have no replacement candidates, so propagation charges
    /// their watch visit without a replacement-position scan.
    /// Ternary clauses inspect their sole unwatched position once.
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
    pub(crate) fn decide(&mut self) -> Result<(), Incomplete> {
        self.tick()?;
        self.quota
            .decision(self.statistics.decisions, self.limits.max_decisions)?;
        self.statistics.decisions += 1;
        Ok(())
    }
    /// The work left before the ceiling, for an operation that charges its
    /// own work and reports it afterwards through [`Self::charge`].
    pub(crate) fn remaining_work(&self) -> u64 {
        self.limits.max_work.saturating_sub(self.statistics.work)
    }
    /// Charge work an operation already performed, one poll for the lot,
    /// reserved through the quota as ticks would be.
    pub(crate) fn charge(&mut self, work: u64) -> Result<(), Incomplete> {
        self.control.poll()?;
        self.quota
            .charge(self.statistics.work, self.limits.max_work, work)?;
        self.statistics.work = self
            .statistics
            .work
            .checked_add(work)
            .ok_or(Incomplete::CounterOverflow)?;
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

pub(crate) fn reserve<T>(values: &mut Vec<T>, count: usize) -> Result<(), Incomplete> {
    values
        .try_reserve_exact(count.saturating_sub(values.len()))
        .map_err(|_| Incomplete::Allocation)
}

fn filled<T: Clone>(
    result: &mut Vec<T>,
    count: usize,
    value: T,
    budget: &mut Budget<'_, impl Quota>,
) -> Result<(), Incomplete> {
    result.clear();
    reserve(result, count)?;
    for _ in 0..count {
        budget.tick()?;
        result.push(value.clone());
    }
    Ok(())
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

#[derive(Debug, Default)]
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
    heads: Vec<Option<WatchNode>>,
    next: Vec<Option<WatchNode>>,
    ordering: crate::ordering::Workspace,
}

impl State {
    fn new(cnf: &Cnf, budget: &mut Budget<'_, impl Quota>) -> Result<Self, Incomplete> {
        let mut state = Self::default();
        state.reset(cnf, budget)?;
        Ok(state)
    }

    fn reserve(&mut self, variables: usize, clauses: usize) -> Result<(), Incomplete> {
        reserve(&mut self.values, variables)?;
        reserve(&mut self.trail, variables)?;
        reserve(&mut self.decisions, variables)?;
        reserve(&mut self.order, variables)?;
        reserve(&mut self.ranks, variables)?;
        reserve(&mut self.positions, clauses)?;
        reserve(
            &mut self.heads,
            variables
                .checked_mul(2)
                .ok_or(Incomplete::CounterOverflow)?,
        )?;
        reserve(
            &mut self.next,
            clauses.checked_mul(2).ok_or(Incomplete::CounterOverflow)?,
        )?;
        self.ordering.reserve(variables)
    }

    fn reset(&mut self, cnf: &Cnf, budget: &mut Budget<'_, impl Quota>) -> Result<(), Incomplete> {
        budget.tick()?;
        self.base_clauses = cnf.clauses().len();
        self.trail.clear();
        self.decisions.clear();
        self.order.clear();
        self.ranks.clear();
        self.ordering.clear();
        self.propagation_head = 0;
        self.next_position = 0;
        filled(&mut self.values, cnf.variables(), None, budget)?;
        reserve(&mut self.trail, cnf.variables())?;
        reserve(&mut self.decisions, cnf.variables())?;
        filled(&mut self.positions, cnf.clauses().len(), [0, 0], budget)?;
        filled(&mut self.heads, cnf.variables() * 2, None, budget)?;
        filled(&mut self.next, cnf.clauses().len() * 2, None, budget)
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

    fn link(&mut self, node: WatchNode, literal: Literal) {
        self.next[node.index()] = self.heads[literal.index()];
        self.heads[literal.index()] = Some(node);
    }

    fn initialize(
        &mut self,
        cnf: &Cnf,
        budget: &mut Budget<'_, impl Quota>,
    ) -> Result<bool, Incomplete> {
        for clause in 0..self.base_clauses {
            budget.tick()?;
            match cnf.clause_at(clause).len() {
                0 => return Ok(false),
                1 => {
                    let literal = cnf.clause_at(clause).at(0);
                    if self.value(literal).is_none() {
                        increment(&mut budget.statistics.propagations)?;
                    }
                    if !self.assign(literal) {
                        return Ok(false);
                    }
                }
                _ => self.index_clause(cnf, clause)?,
            }
        }
        Ok(true)
    }

    fn index_clause(&mut self, cnf: &Cnf, clause: usize) -> Result<(), Incomplete> {
        // Only clauses with at least two distinct literals receive watches.
        // Admission bounds twice the submitted clause count and its successors.
        let first = WatchNode::new(clause * 2)?;
        let second = WatchNode::new(clause * 2 + 1)?;
        self.positions[clause] = [0, 1];
        self.link(first, cnf.clause_at(clause).at(0));
        self.link(second, cnf.clause_at(clause).at(1));
        Ok(())
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
            let mut previous: Option<WatchNode> = None;
            let mut cursor = self.heads[false_literal.index()];
            while let Some(node) = cursor {
                budget.tick()?;
                #[cfg(test)]
                propagation_profile::visit();
                let following = self.next[node.index()];
                let clause = node.index() / 2;
                let slot = node.index() % 2;
                let other_position = self.positions[clause][1 - slot];
                let other = cnf.clause_at(clause).at(other_position);
                if self.value(other) != Some(true) {
                    if let Some(position) = self.replacement(cnf, clause, slot, budget)? {
                        if let Some(prior) = previous {
                            self.next[prior.index()] = following;
                        } else {
                            self.heads[false_literal.index()] = following;
                        }
                        self.positions[clause][slot] = position;
                        self.link(node, cnf.clause_at(clause).at(position));
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
        #[cfg(test)]
        propagation_profile::replacement_attempt(cnf.clause_at(clause).len());
        // Two distinct watches cover every position of a binary clause. The
        // calling watch visit has already polled control and charged its work;
        // there is no replacement position to examine or watch to relocate.
        if cnf.clause_at(clause).len() == 2 {
            return Ok(None);
        }
        if cnf.clause_at(clause).len() == 3 {
            const POSITION_SUM: usize = 3;
            budget.tick()?;
            // Valid, distinct watches occupy two of positions 0, 1 and 2.
            // Subtracting them from their sum gives the sole remaining index.
            // The generic scan can return only this position, or no position.
            let [first, second] = self.positions[clause];
            let position = POSITION_SUM - first - second;
            #[cfg(test)]
            propagation_profile::ternary_inspection([first, second], |position| {
                self.value(cnf.clause_at(clause).at(position)) != Some(false)
            });
            return Ok(
                (self.value(cnf.clause_at(clause).at(position)) != Some(false)).then_some(position),
            );
        }
        for position in 0..cnf.clause_at(clause).len() {
            budget.tick()?;
            if position != self.positions[clause][slot]
                && position != self.positions[clause][1 - slot]
                && self.value(cnf.clause_at(clause).at(position)) != Some(false)
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
        self.ordering.variables(
            &mut self.order,
            cnf.clauses().take(self.base_clauses),
            &self.values,
            budget,
        )?;
        filled(&mut self.ranks, self.values.len(), 0, budget)?;
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
        // A complete assignment that propagation left without conflict
        // satisfies every clause: a clause with both watches false would
        // have propagated or conflicted. The truth-table tests state that
        // property; a debug build re-checks it, uncharged, and a release
        // build does not rescan the clauses per witness.
        debug_assert!(
            cnf.clauses().take(self.base_clauses).all(|clause| clause
                .iter()
                .any(|literal| assignment[literal.variable()] == literal.positive())),
            "a completed witness falsifies a base clause"
        );
        Ok(Assignment(assignment))
    }
}

fn search(
    cnf: &Cnf,
    budget: &mut Budget<'_, impl Quota>,
) -> Result<Option<Assignment>, Incomplete> {
    let mut state = State::new(cnf, budget)?;
    search_initialized(&mut state, cnf, budget)
}

fn search_initialized(
    state: &mut State,
    cnf: &Cnf,
    budget: &mut Budget<'_, impl Quota>,
) -> Result<Option<Assignment>, Incomplete> {
    search_assuming(state, cnf, &[], budget)
}

fn search_assuming(
    state: &mut State,
    cnf: &Cnf,
    assumptions: &[Literal],
    budget: &mut Budget<'_, impl Quota>,
) -> Result<Option<Assignment>, Incomplete> {
    if !state.initialize(cnf, budget)? {
        increment(&mut budget.statistics.conflicts)?;
        return Ok(None);
    }
    search_seeded(state, cnf, assumptions, budget)
}

// The current CNF's unit clauses are assigned, either freshly replayed or
// retained with their unconditional propagation closure. Both callers preserve
// independent clause and assumption validation at the completed witness boundary.
fn search_seeded(
    state: &mut State,
    cnf: &Cnf,
    assumptions: &[Literal],
    budget: &mut Budget<'_, impl Quota>,
) -> Result<Option<Assignment>, Incomplete> {
    // Parameters are level-zero assignments: every decision starts after them
    // on the trail, so chronological backtracking never retracts a parameter.
    for &literal in assumptions {
        budget.tick()?;
        if literal.variable() >= cnf.variables() {
            return Err(crate::AdmissionError::Variable {
                variable: literal.variable(),
                variables: cnf.variables(),
            }
            .into());
        }
        if !state.assign(literal) {
            increment(&mut budget.statistics.conflicts)?;
            return Ok(None);
        }
    }
    if !state.propagate(cnf, budget)? {
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
            let assignment = state.finish(cnf, budget)?;
            // Clause validation alone cannot authenticate the parameter input.
            for &literal in assumptions {
                budget.tick()?;
                if assignment.value(literal.variable()) != Some(literal.positive()) {
                    return Err(Incomplete::InvalidWitness);
                }
            }
            return Ok(Some(assignment));
        }
    }
}

/// Reusable allocations with fresh watch/assignment/decision state per query.
#[derive(Debug, Default)]
pub(crate) struct Workspace(State);

impl Workspace {
    pub(crate) fn reserve(&mut self, variables: usize, clauses: usize) -> Result<(), Incomplete> {
        self.0.reserve(variables, clauses)
    }

    pub(crate) fn retained_bytes(&self) -> u128 {
        self.required_bytes(0, 0)
    }

    /// Current retained capacity or the requested final slots for this shape,
    /// whichever is larger for each vector. Reservation overlap is excluded.
    pub(crate) fn required_bytes(&self, variables: usize, clauses: usize) -> u128 {
        use std::mem::size_of;
        let state = &self.0;
        size_of::<Self>() as u128
            + state.values.capacity().max(variables) as u128 * size_of::<Option<bool>>() as u128
            + state.trail.capacity().max(variables) as u128 * size_of::<Literal>() as u128
            + state.decisions.capacity().max(variables) as u128 * size_of::<Decision>() as u128
            + (state.order.capacity().max(variables) as u128
                + state.ranks.capacity().max(variables) as u128)
                * size_of::<usize>() as u128
            + state.positions.capacity().max(clauses) as u128 * size_of::<[usize; 2]>() as u128
            + (state.heads.capacity() as u128).max(2 * variables as u128)
                * size_of::<Option<WatchNode>>() as u128
            + (state.next.capacity() as u128).max(2 * clauses as u128)
                * size_of::<Option<WatchNode>>() as u128
            + state.ordering.required_bytes(variables)
    }

    pub(crate) fn query(&mut self, cnf: &Cnf, budget: &mut Budget<'_, impl Quota>) -> Solve {
        self.query_assuming(cnf, &[], budget)
    }

    pub(crate) fn query_assuming(
        &mut self,
        cnf: &Cnf,
        assumptions: &[Literal],
        budget: &mut Budget<'_, impl Quota>,
    ) -> Solve {
        match self
            .0
            .reset(cnf, budget)
            .and_then(|()| search_assuming(&mut self.0, cnf, assumptions, budget))
        {
            Ok(Some(assignment)) => Solve::Sat(assignment),
            Ok(None) => Solve::Unsat,
            Err(error) => Solve::Inconclusive(error),
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
                + 3 * size_of::<usize>()
                + size_of::<u64>()
                + 2 * size_of::<Option<WatchNode>>()
                + size_of::<bool>()) as u128
        + clauses * (size_of::<[usize; 2]>() + 2 * size_of::<Option<WatchNode>>()) as u128
}
