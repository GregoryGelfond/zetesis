//! Finite DAG refinement of `Feedback.Witness` and `Feedback.Allow`.
//!
//! This compiler is experiment-local. Its input J need not be a countermodel:
//! the preservation law holds for any J. Only Store::learn accepts production
//! witnesses, through the opaque checked-subject record.

use std::mem::size_of;
use zetesis_cpu::Control;
use zetesis_ferraris::{AdmissionLimits, Interpretation, Node, Theory};

use super::{ConstructionLimits, Error, Resource};

#[derive(Debug)]
pub(super) struct Guard {
    source: Theory,
    pub(super) restriction: Theory,
    witness: Vec<usize>,
    bytes: usize,
}
impl Guard {
    pub(super) fn witness_bits(&self) -> u64 {
        self.witness.iter().fold(0, |bits, atom| bits | (1 << atom))
    }
    pub(super) fn owner(&self, source: &Theory) -> Result<(), Error> {
        if self.source.same_instance(source) { Ok(()) } else { Err(Error::Owner) }
    }
    pub(super) fn allows(
        &self,
        candidate: &Interpretation,
        max_work: u64,
        control: &Control,
    ) -> Result<bool, Error> {
        if !self.source.same_instance(candidate.theory()) {
            return Err(Error::Owner);
        }
        // Dense atom meanings are those of the retained original owner. A
        // same-sized foreign theory is insufficient to authorize this rebind.
        let rebound = Interpretation::new(&self.restriction, candidate.atoms())
            .map_err(Error::Admission)?;
        zetesis_ferraris::models(
            &self.restriction,
            &rebound,
            super::reference_limits(max_work),
            control,
        )
        .map_err(Error::Control)
    }

    fn matches(&self, witness: &Interpretation, budget: &mut Budget<'_>) -> Result<bool, Error> {
        let mut selected = 0;
        for atom in 0..self.source.atom_count() {
            budget.tick()?;
            if witness.contains(atom) {
                if self.witness.get(selected) != Some(&atom) {
                    return Ok(false);
                }
                selected += 1;
            }
        }
        Ok(selected == self.witness.len())
    }
}

#[derive(Debug)]
pub(super) struct Budget<'a> {
    pub(super) limits: ConstructionLimits,
    control: &'a Control,
    pub(super) work: u64,
    pub(super) peak_build_bytes: usize,
    pub(super) peak_live_bytes: usize,
    pub(super) retained_bytes: usize,
    pub(super) retained_nodes: usize,
}
impl<'a> Budget<'a> {
    pub(super) const fn new(limits: ConstructionLimits, control: &'a Control) -> Self {
        Self {
            limits,
            control,
            work: 0,
            peak_build_bytes: 0,
            peak_live_bytes: 0,
            retained_bytes: 0,
            retained_nodes: 0,
        }
    }

    // One unit per copied/emitted node, mapping/atom/root/dedup visit and
    // reservation/publication boundary. Theory admission's second node/root
    // scan is also precharged. These are specified units, not instructions.
    fn tick(&mut self) -> Result<(), Error> {
        self.control.poll().map_err(Error::Control)?;
        if self.work >= self.limits.max_work {
            return Err(Error::Limit(Resource::Work));
        }
        self.work += 1;
        Ok(())
    }

    fn capacity(&mut self, build: usize) -> Result<(), Error> {
        self.peak_build_bytes = self.peak_build_bytes.max(build);
        let live = self.retained_bytes.checked_add(build).ok_or(Error::Overflow)?;
        self.peak_live_bytes = self.peak_live_bytes.max(live);
        if build > self.limits.max_build_bytes {
            return Err(Error::Limit(Resource::BuildBytes));
        }
        if live > self.limits.max_retained_bytes {
            return Err(Error::Limit(Resource::LiveBytes));
        }
        Ok(())
    }

    fn reserve<T>(&mut self, count: usize, live: &mut usize) -> Result<Vec<T>, Error> {
        self.tick()?;
        let requested = count.checked_mul(size_of::<T>()).ok_or(Error::Overflow)?;
        self.capacity(live.checked_add(requested).ok_or(Error::Overflow)?)?;
        let mut values = Vec::new();
        values.try_reserve_exact(count).map_err(|_| Error::Allocation)?;
        let actual = values.capacity().checked_mul(size_of::<T>()).ok_or(Error::Overflow)?;
        *live = live.checked_add(actual).ok_or(Error::Overflow)?;
        self.capacity(*live)?;
        Ok(values)
    }
}

struct Builder<'a, 'b> {
    nodes: Vec<Node>,
    budget: &'a mut Budget<'b>,
}
impl Builder<'_, '_> {
    fn push(&mut self, node: Node) -> Result<usize, Error> {
        self.budget.tick()?;
        // The complete finite bound was reserved once; no unobserved growth.
        if self.nodes.len() >= self.budget.limits.max_nodes {
            return Err(Error::Limit(Resource::Nodes));
        }
        if self.nodes.len() >= self.nodes.capacity() {
            return Err(Error::Invariant);
        }
        let index = self.nodes.len();
        self.nodes.push(node);
        Ok(index)
    }
    fn witness_nodes(
        &mut self,
        source: &Theory,
        witness: &Interpretation,
        falsum: usize,
        mapping: &mut Vec<usize>,
    ) -> Result<(), Error> {
        for (original, &node) in source.nodes().iter().enumerate() {
            self.budget.tick()?;
            let transformed = match node {
                Node::Atom(atom) if witness.contains(atom) => original,
                Node::Atom(_) | Node::False => falsum,
                Node::And(left, right) | Node::Or(left, right) | Node::Implies(left, right) => {
                    let inner = match node {
                        Node::And(_, _) => Node::And(mapping[left], mapping[right]),
                        Node::Or(_, _) => Node::Or(mapping[left], mapping[right]),
                        _ => Node::Implies(mapping[left], mapping[right]),
                    };
                    let inner = self.push(inner)?;
                    self.push(Node::And(original, inner))?
                }
            };
            mapping.push(transformed);
        }
        Ok(())
    }

    fn proper_subset(
        &mut self,
        source: &Theory,
        witness: &Interpretation,
        falsum: usize,
        verum: usize,
        selected: &mut Vec<usize>,
    ) -> Result<usize, Error> {
        let mut contained = verum;
        let mut extra = falsum;
        for atom in 0..source.atom_count() {
            self.budget.tick()?;
            let node = self.push(Node::Atom(atom))?;
            if witness.contains(atom) {
                selected.push(atom);
                contained = self.push(Node::And(contained, node))?;
            } else {
                extra = self.push(Node::Or(extra, node))?;
            }
        }
        self.push(Node::And(contained, extra))
    }
}

pub(super) fn compile(
    source: &Theory,
    witness: &Interpretation,
    budget: &mut Budget<'_>,
) -> Result<Guard, Error> {
    if !source.same_instance(witness.theory()) {
        return Err(Error::Owner);
    }
    super::fixtures::shape(source)?;
    let count = source.nodes().len();
    // N copied originals, <=2N witness nodes, 2U subset nodes,
    // R root folds, falsum/verum and three final connective nodes.
    let capacity = 3 * count + 2 * source.atom_count() + source.roots().len() + 5;
    let mut live = 0;
    let nodes = budget.reserve(capacity.min(budget.limits.max_nodes), &mut live)?;
    let mut mapping = budget.reserve(count, &mut live)?;
    let mut selected = budget.reserve(source.atom_count(), &mut live)?;
    let mut roots = budget.reserve(1, &mut live)?;
    let retained = live - mapping.capacity() * size_of::<usize>();
    let mut builder = Builder { nodes, budget };
    for &node in source.nodes() {
        builder.push(node)?;
    }
    let falsum = builder.push(Node::False)?;
    let verum = builder.push(Node::Implies(falsum, falsum))?;
    builder.witness_nodes(source, witness, falsum, &mut mapping)?;
    let proper = builder.proper_subset(source, witness, falsum, verum, &mut selected)?;
    let mut witnessed = verum;
    for &root in source.roots() {
        builder.budget.tick()?;
        witnessed = builder.push(Node::And(witnessed, mapping[root]))?;
    }
    let excluded = builder.push(Node::And(proper, witnessed))?;
    roots.push(builder.push(Node::Implies(excluded, falsum))?);
    for _ in 0..builder.nodes.len() + roots.len() {
        builder.budget.tick()?;
    }
    let max_nodes = builder.budget.limits.max_nodes;
    let restriction = Theory::new(
        source.atom_count(),
        builder.nodes,
        roots,
        AdmissionLimits { max_atoms: 6, max_nodes, max_roots: 1 },
    )
    .map_err(Error::Admission)?;
    Ok(Guard { source: source.clone(), restriction, witness: selected, bytes: retained })
}

pub(super) struct Store {
    pub(super) guards: Vec<Guard>,
}
impl Store {
    pub(super) fn storage(&self) -> (usize, usize) {
        (self.guards.iter().map(|guard| guard.restriction.nodes().len()).sum(),
         self.guards.capacity() * size_of::<Guard>() + self.guards.iter().map(|guard| guard.bytes).sum::<usize>())
    }
    pub(super) fn new(budget: &mut Budget<'_>) -> Result<Self, Error> {
        let mut live = 0;
        let guards = budget.reserve(budget.limits.max_guards, &mut live)?;
        budget.retained_bytes = live;
        Ok(Self { guards })
    }

    pub(super) fn learn(
        &mut self,
        checked: &zetesis_sat::CheckedInterpretation,
        budget: &mut Budget<'_>,
    ) -> Result<bool, Error> {
        let zetesis_sat::Check::NonMinimal(witness) = checked.verdict() else {
            return Err(Error::Witness);
        };
        for guard in &self.guards {
            if !guard.source.same_instance(checked.candidate().theory()) {
                return Err(Error::Owner);
            }
            if guard.matches(witness, budget)? {
                return Ok(false);
            }
        }
        if self.guards.len() >= budget.limits.max_guards {
            return Err(Error::Limit(Resource::Guards));
        }
        let guard = compile(checked.candidate().theory(), witness, budget)?;
        let total = budget.retained_nodes + guard.restriction.nodes().len();
        if total > budget.limits.max_total_nodes {
            return Err(Error::Limit(Resource::TotalNodes));
        }
        budget.tick()?;
        budget.retained_nodes = total;
        budget.retained_bytes += guard.bytes;
        self.guards.push(guard);
        Ok(true)
    }
}
