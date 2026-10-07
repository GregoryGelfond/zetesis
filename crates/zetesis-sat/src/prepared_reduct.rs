//! Immutable proper-subset queries with authenticated original-truth parameters.

mod query;
mod state;
pub use query::{ReductQueryStatistics, ReductStatistics, ReductWorkspace};
pub(crate) use state::State;

use std::{collections::HashMap, mem::size_of, sync::Arc};

use zetesis_ferraris::{NodeView, Theory};

use crate::{
    AdmissionLimits, Cancellation, Cnf, Incomplete, Literal, SearchLimits, SearchStatistics,
    encoding::{self, Encoded},
    search::{Budget, LocalQuota, Quota},
};

/// Independent construction limits for one immutable reduct-query owner.
#[derive(Clone, Copy, Debug)]
pub struct ReductPreparationLimits {
    /// Actual submitted CNF dimensions, including parameter and auxiliary variables.
    pub admission: AdmissionLimits,
    /// Cumulative original-node inspections and checked encoding operations.
    pub max_work: u64,
    /// Named construction vector/map capacity, including the retained result.
    /// Shared theory payload, hash bucket/control bytes, Arc/allocator metadata
    /// and bounded local variables are excluded. This is not process RSS.
    pub max_bytes: u64,
}

impl ReductPreparationLimits {
    /// Default ceiling for named preparation or standalone worker capacity.
    pub const DEFAULT_BYTES: u64 = 64 * 1024 * 1024;
}

impl Default for ReductPreparationLimits {
    fn default() -> Self {
        Self {
            admission: AdmissionLimits::default(),
            max_work: SearchLimits::default().max_work,
            max_bytes: Self::DEFAULT_BYTES,
        }
    }
}

/// Construction evidence. A refused proposal is not an observed capacity peak.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReductPreparationStatistics {
    /// Charged operations, including a failed construction's completed prefix.
    pub work: u64,
    /// Successfully admitted variables, including subset and parameter inputs.
    pub variables: usize,
    /// Successfully submitted clauses, before canonicalization.
    pub clauses: usize,
    /// Successfully submitted literal occurrences, before canonicalization.
    pub literals: usize,
    /// Named result storage; zero when no prepared owner was published.
    pub retained_bytes: u128,
    /// Maximum observed named construction capacity, including allocator slack.
    pub peak_bytes: u128,
}

/// One actual preparation attempt, with an independent receipt on either result.
/// A failed result publishes no owner; its original typed cause is unchanged.
#[derive(Debug)]
pub struct ReductPreparationAttempt {
    /// Complete prepared owner or work, control, shape, storage or allocation refusal.
    pub result: Result<PreparedReduct, Incomplete>,
    /// Actual construction progress, including a failed prefix. On success this
    /// is the same immutable receipt exposed by the prepared owner.
    pub statistics: ReductPreparationStatistics,
}

/// One candidate-parametric CNF bound to an exact immutable original theory.
///
/// The original semantic atoms are the prospective subset N. Separate inputs
/// represent candidate membership M and original implication truth. The latter
/// are supplied only by a successful, subject-bound formula evaluation. No
/// arbitrary external mask is accepted. Clones share this owner and its theory.
/// Each concurrent query requires its own [`ReductWorkspace`].
///
/// An implication is encoded as h(M) AND (r(left) IMPLIES r(right)). Under N⊆M,
/// atoms are exact and And/Or false-in-M cases collapse through false child
/// reducts. This is different from keeping the original classical equivalences
/// and adding units for false original nodes. All roots and N⊂M are asserted.
///
/// Construction is linear in DAG/atom/root visits plus expected hash lookup
/// and clause canonicalization work. It retains a larger fixed query than some
/// individually simplified reducts; reuse does not guarantee faster search.
/// Search state and witness validation remain per-query operations.
#[derive(Clone, Debug)]
pub struct PreparedReduct(Arc<Data>);

#[derive(Debug)]
struct Data {
    theory: Theory,
    cnf: Cnf,
    implications: Vec<usize>,
    units: Vec<usize>,
    has_empty_clause: bool,
    statistics: ReductPreparationStatistics,
}

impl PreparedReduct {
    /// Compile a fixed proper-subset query without evaluating any candidate.
    ///
    /// For A atoms, N DAG nodes, I implication nodes and R roots, construction
    /// reserves at most 3A+N+2I variables, 3N+3I+4A+R+1 clauses and
    /// 7N+7I+10A+R literals. CNF reservations are clipped to declared admission
    /// limits; actual submissions remain checked. Metadata reservations are
    /// finite and byte-admitted before allocation, then checked against actual
    /// capacities. After encoding, a charged scan counts the actual unit clauses
    /// before reserving their shared index, followed by a charged collection scan.
    /// The resulting unit indices and empty-clause fact refer to that exact CNF.
    ///
    /// Failure publishes no prepared owner and retains consumed work and
    /// observed capacities. Arc allocation follows the existing infallible
    /// shared-owner allocation contract; allocator metadata is not counted.
    #[must_use]
    pub fn prepare(
        theory: &Theory,
        limits: ReductPreparationLimits,
        cancellation: &Cancellation,
    ) -> ReductPreparationAttempt {
        let mut budget = Budget {
            quota: LocalQuota,
            limits: SearchLimits {
                max_work: limits.max_work,
                max_decisions: 0,
            },
            cancellation,
            statistics: SearchStatistics::default(),
        };
        Self::with_budget(theory, limits.admission, limits.max_bytes, &mut budget)
    }

    pub(crate) fn with_budget(
        theory: &Theory,
        admission: AdmissionLimits,
        max_bytes: u64,
        budget: &mut Budget<'_, impl Quota>,
    ) -> ReductPreparationAttempt {
        let before = budget.statistics.work;
        let limits = ReductPreparationLimits {
            admission,
            max_bytes,
            max_work: budget.limits.max_work,
        };
        let mut statistics = ReductPreparationStatistics::default();
        let result = Self::build(theory, limits, budget, &mut statistics);
        statistics.work = budget.statistics.work - before;
        let result = result.map(|mut data| {
            statistics.retained_bytes = data.bytes();
            data.statistics = statistics;
            Self(Arc::new(data))
        });
        ReductPreparationAttempt { result, statistics }
    }

    fn build(
        theory: &Theory,
        limits: ReductPreparationLimits,
        budget: &mut Budget<'_, impl Quota>,
        statistics: &mut ReductPreparationStatistics,
    ) -> Result<Data, Incomplete> {
        budget.tick()?;
        let mut implications = 0;
        let mut formula_gates = 0usize;
        for index in 0..theory.view().len() {
            budget.tick()?;
            let node = theory
                .view()
                .node(index)
                .map_err(|_| Incomplete::InvalidWitness)?;
            if matches!(node, NodeView::Implies(..)) {
                implications += 1;
            }
            formula_gates = formula_gates
                .checked_add(encoding::node_gates(node))
                .ok_or(Incomplete::CounterOverflow)?;
        }
        let shape = Shape::new(theory, implications, formula_gates, limits.admission)?;
        let mut builder = Builder {
            data: Data {
                theory: theory.clone(),
                cnf: Cnf::empty(shape.inputs, limits.admission)?,
                implications: Vec::new(),
                units: Vec::new(),
                has_empty_clause: false,
                statistics: ReductPreparationStatistics::default(),
            },
            nodes: Vec::new(),
            gates: HashMap::new(),
            strict: Vec::new(),
        };
        let result = builder
            .reserve(&shape, limits, statistics)
            .and_then(|()| builder.encode(budget))
            .and_then(|()| builder.initial_clauses(limits.max_bytes, budget, statistics));
        // A partially failed reservation still exposes actual retained capacity.
        builder.record(statistics);
        result?;
        bound(builder.bytes(), limits.max_bytes)?;
        Ok(builder.data)
    }

    /// Exact original theory; equality of separately admitted text is insufficient.
    #[must_use]
    pub fn theory(&self) -> &Theory {
        &self.0.theory
    }

    /// Whether two handles share this exact compiled query owner.
    #[must_use]
    pub fn same_owner(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn cnf_shape(&self) -> (usize, usize) {
        (self.0.cnf.variables(), self.0.cnf.clauses().len())
    }

    pub(crate) fn parameter_count(&self) -> usize {
        self.0.theory.atom_count() + self.0.implications.len()
    }

    /// Actual completed construction and retained result storage.
    #[must_use]
    pub fn statistics(&self) -> ReductPreparationStatistics {
        self.0.statistics
    }
}

impl Data {
    fn bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + self.cnf.retained_bytes()
            + (self.implications.capacity() as u128 + self.units.capacity() as u128)
                * size_of::<usize>() as u128
    }
}

struct Shape {
    inputs: usize,
    nodes: usize,
    implications: usize,
    gates: usize,
    atoms: usize,
    clauses: usize,
    literals: usize,
}

impl Shape {
    fn new(
        theory: &Theory,
        implications: usize,
        formula_gates: usize,
        limits: AdmissionLimits,
    ) -> Result<Self, Incomplete> {
        let atoms = theory.atom_count() as u128;
        let gates_bound = formula_gates as u128;
        let roots = theory.roots().len() as u128;
        let imps = implications as u128;
        let narrow = |count| usize::try_from(count).map_err(|_| Incomplete::CounterOverflow);
        let inputs = narrow(2 * atoms + imps)?;
        // A refused fresh variable can still follow a gate-map reservation.
        // Reserve the complete finite gate bound, including that failed attempt.
        let gates = narrow(gates_bound + imps + atoms)?;
        Ok(Self {
            inputs,
            nodes: theory.nodes().len(),
            implications,
            gates,
            atoms: theory.atom_count(),
            clauses: narrow(
                (3 * gates_bound + 3 * imps + 4 * atoms + roots + 1)
                    .min(limits.max_clauses as u128),
            )?,
            literals: narrow(
                (7 * gates_bound + 7 * imps + 10 * atoms + roots).min(limits.max_literals as u128),
            )?,
        })
    }

    fn bytes(&self) -> u128 {
        size_of::<Builder>() as u128
            + (self.clauses as u128 + self.literals as u128 + self.implications as u128)
                * size_of::<usize>() as u128
            + self.nodes as u128 * size_of::<Encoded>() as u128
            + self.gates as u128 * size_of::<((usize, usize), Literal)>() as u128
            + self.atoms as u128 * size_of::<Literal>() as u128
    }
}

struct Builder {
    data: Data,
    nodes: Vec<Encoded>,
    gates: HashMap<(usize, usize), Literal>,
    strict: Vec<Literal>,
}

impl Builder {
    fn reserve(
        &mut self,
        shape: &Shape,
        limits: ReductPreparationLimits,
        statistics: &mut ReductPreparationStatistics,
    ) -> Result<(), Incomplete> {
        bound(shape.bytes(), limits.max_bytes)?;
        self.data.cnf.reserve(shape.clauses, shape.literals)?;
        self.observe(limits.max_bytes, statistics)?;
        self.nodes
            .try_reserve_exact(shape.nodes)
            .map_err(|_| Incomplete::Allocation)?;
        self.observe(limits.max_bytes, statistics)?;
        self.data
            .implications
            .try_reserve_exact(shape.implications)
            .map_err(|_| Incomplete::Allocation)?;
        self.observe(limits.max_bytes, statistics)?;
        self.strict
            .try_reserve_exact(shape.atoms)
            .map_err(|_| Incomplete::Allocation)?;
        self.observe(limits.max_bytes, statistics)?;
        self.gates
            .try_reserve(shape.gates)
            .map_err(|_| Incomplete::Allocation)?;
        self.observe(limits.max_bytes, statistics)
    }

    fn bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + self.data.cnf.retained_bytes()
            + (self.data.implications.capacity() as u128 + self.data.units.capacity() as u128)
                * size_of::<usize>() as u128
            + self.nodes.capacity() as u128 * size_of::<Encoded>() as u128
            + self.strict.capacity() as u128 * size_of::<Literal>() as u128
            + self.gates.capacity() as u128 * size_of::<((usize, usize), Literal)>() as u128
    }

    fn initial_clauses(
        &mut self,
        max_bytes: u64,
        budget: &mut Budget<'_, impl Quota>,
        statistics: &mut ReductPreparationStatistics,
    ) -> Result<(), Incomplete> {
        let mut count = 0_usize;
        for clause in self.data.cnf.clauses() {
            budget.tick()?;
            if clause.len() == 1 {
                count = count.checked_add(1).ok_or(Incomplete::CounterOverflow)?;
            }
        }
        // This vector is still empty: requested slots are checked before growth,
        // and actual allocator capacity is recorded before any later refusal.
        bound(
            self.bytes() + count as u128 * size_of::<usize>() as u128,
            max_bytes,
        )?;
        self.data
            .units
            .try_reserve_exact(count)
            .map_err(|_| Incomplete::Allocation)?;
        self.observe(max_bytes, statistics)?;
        for (index, clause) in self.data.cnf.clauses().enumerate() {
            budget.tick()?;
            match clause.len() {
                0 => self.data.has_empty_clause = true,
                1 => self.data.units.push(index),
                _ => (),
            }
        }
        Ok(())
    }

    fn record(&self, statistics: &mut ReductPreparationStatistics) {
        statistics.peak_bytes = statistics.peak_bytes.max(self.bytes());
        statistics.variables = self.data.cnf.variables();
        (statistics.clauses, statistics.literals) = self.data.cnf.submitted_counts();
    }

    fn observe(
        &self,
        limit: u64,
        statistics: &mut ReductPreparationStatistics,
    ) -> Result<(), Incomplete> {
        self.record(statistics);
        bound(self.bytes(), limit)
    }

    fn gate(
        &mut self,
        disjunction: bool,
        left: Encoded,
        right: Encoded,
        budget: &mut Budget<'_, impl Quota>,
    ) -> Result<Encoded, Incomplete> {
        encoding::gate(
            &mut self.data.cnf,
            &mut self.gates,
            disjunction,
            left,
            right,
            budget,
        )
    }

    fn encode(&mut self, budget: &mut Budget<'_, impl Quota>) -> Result<(), Incomplete> {
        let atoms = self.data.theory.atom_count();
        // A shallow Arc clone separates the immutable operand borrow from
        // mutations to this query builder. No formula storage is copied.
        let theory = self.data.theory.clone();
        for index in 0..theory.view().len() {
            budget.tick()?;
            let value = match theory
                .view()
                .node(index)
                .map_err(|_| Incomplete::InvalidWitness)?
            {
                NodeView::False => Encoded::Constant(false),
                NodeView::Atom(atom) => Encoded::Literal(Literal::new(atom, true)),
                NodeView::And(operands) => encoding::gate_operands(
                    &mut self.data.cnf,
                    &mut self.gates,
                    false,
                    operands,
                    &self.nodes,
                    budget,
                )?,
                NodeView::Or(operands) => encoding::gate_operands(
                    &mut self.data.cnf,
                    &mut self.gates,
                    true,
                    operands,
                    &self.nodes,
                    budget,
                )?,
                NodeView::Implies(left, right) => {
                    let parameter = Literal::new(2 * atoms + self.data.implications.len(), true);
                    self.data.implications.push(index);
                    budget.tick()?;
                    let left = self.nodes[left].negated();
                    budget.tick()?;
                    let right = self.nodes[right];
                    let implication = self.gate(true, left, right, budget)?;
                    self.gate(false, Encoded::Literal(parameter), implication, budget)?
                }
            };
            self.nodes.push(value);
        }
        for root in 0..self.data.theory.roots().len() {
            budget.tick()?;
            match self.nodes[self.data.theory.roots()[root]] {
                Encoded::Constant(true) => (),
                Encoded::Constant(false) => encoding::clause(&mut self.data.cnf, [], budget)?,
                Encoded::Literal(value) => encoding::clause(&mut self.data.cnf, [value], budget)?,
            }
        }
        for atom in 0..atoms {
            budget.tick()?;
            let subset = Literal::new(atom, true);
            let candidate = Literal::new(atoms + atom, true);
            encoding::clause(&mut self.data.cnf, [subset.negated(), candidate], budget)?;
            let difference = self.gate(
                false,
                Encoded::Literal(candidate),
                Encoded::Literal(subset.negated()),
                budget,
            )?;
            let Encoded::Literal(difference) = difference else {
                return Err(Incomplete::InvalidWitness);
            };
            self.strict.push(difference);
        }
        // One candidate member is absent from N. The empty universe deliberately
        // submits an empty clause: it has no proper subset.
        self.data.cnf.append_slice(&mut self.strict)?;
        Ok(())
    }
}

fn bound(required: u128, limit: u64) -> Result<(), Incomplete> {
    if required > u128::from(limit) {
        Err(Incomplete::ReductStorage {
            required,
            limit: u128::from(limit),
        })
    } else {
        Ok(())
    }
}

pub(crate) const fn retained_header_bytes() -> u128 {
    size_of::<Data>() as u128
}

#[cfg(test)]
mod tests;
