//! The update route against the scalar plan: per-slot routes, receipts and
//! verdicts across controlled candidate changes, and the route's availability.

use zetesis_backend::GpuApi;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, EvaluationLimits, EvaluationWorkspace, FormulaNodes, FormulaParts,
    FormulaTransaction, Interpretation, Node, NodeView, SlateId, SlateRef, Theory, Threshold,
    TightCheckLimits, TightPlan, TightPlanLimits,
};
use zetesis_wgpu::{
    GpuTightOracle, GpuWorkgroupSize, GpuWorkgroupSizing, TightGpuBatchStats, TightGpuCheck,
    TightGpuError, TightGpuLimits, TightGpuUpdateUnavailable, TightGpuUpdates, TightSupport,
};

const UNBOUNDED: usize = usize::MAX;
/// The update route's engagement factor, from which every fixture that must
/// engage, or must not, is sized.
const STRIDES: u64 = GpuTightOracle::UPDATE_ENGAGEMENT_STRIDES;
/// The ladder's threshold family's bounds over its first six inputs.
const BOUNDS: [i128; 3] = [2, 3, 5];
/// Words of a result record, a part of every update's floor.
const RECORD_WORDS: u64 = 9;
/// The most batches the runtime guard waits after a losing judgment.
const GUARD_WAIT_LIMIT: u32 = 32;

/// Dense evaluation's strided iterations over level widths `widths` at `size`
/// invocations.
fn strided_depth(widths: &[u64], size: u64) -> u64 {
    widths.iter().map(|width| width.div_ceil(size)).sum()
}

/// Whether a schedule of level widths `widths` engages the update route at
/// every workgroup size; 256 invocations take the fewest strides.
fn engages_at_every_size(widths: &[u64]) -> bool {
    strided_depth(widths, 256) >= STRIDES * u64::try_from(widths.len()).unwrap()
}

/// The least of `start`, `start + step`, ... whose schedule, of level widths
/// `widths(count)`, engages the update route at every workgroup size.
fn least_engaging(start: u64, step: u64, widths: impl Fn(u64) -> Vec<u64>) -> usize {
    let count = (0..1 << 20)
        .map(|index| start + index * step)
        .find(|&count| engages_at_every_size(&widths(count)))
        .expect("a fixture engages within 2^20 steps");
    usize::try_from(count).unwrap()
}

fn interpretation(theory: &Theory, atoms: &[bool]) -> Interpretation {
    Interpretation::new(
        theory,
        atoms
            .iter()
            .enumerate()
            .filter_map(|(atom, &present)| present.then_some(atom)),
    )
    .unwrap()
}

/// Check `candidates` as one exact batch; every verdict and witness must equal
/// the scalar plan's, and the routes must cover every slot once.
fn checked(
    oracle: &mut GpuTightOracle,
    certificate: &TightPlan,
    candidates: &[Vec<bool>],
    limits: TightGpuLimits,
) -> (TightGpuBatchStats, Vec<TightGpuCheck>) {
    let theory = certificate.theory();
    let candidates: Vec<_> = candidates
        .iter()
        .map(|atoms| interpretation(theory, atoms))
        .collect();
    let results = oracle
        .check_batch(certificate, &candidates, limits, &Cancellation::default())
        .unwrap();
    compare(certificate, &candidates, &results);
    let stats = *oracle.last_batch_stats().unwrap();
    assert_eq!(
        stats.routes.complete + stats.routes.dense_reuse + stats.routes.update,
        candidates.len() as u64
    );
    assert_eq!(
        stats.work,
        results.iter().map(TightGpuCheck::work).sum::<u64>()
    );
    assert_eq!(oracle.activity().completed_work, stats.work);
    assert!(oracle.activity().completed_work <= oracle.activity().scheduled_work);
    (stats, results)
}

fn compare(certificate: &TightPlan, candidates: &[Interpretation], results: &[TightGpuCheck]) {
    assert_eq!(results.len(), candidates.len());
    for (candidate, result) in candidates.iter().zip(results) {
        let scalar = certificate
            .check(
                candidate,
                TightCheckLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(
            result.verdict(),
            scalar.verdict,
            "candidate {:?}",
            candidate.atoms().collect::<Vec<_>>()
        );
    }
}

fn admitted(stats: &TightGpuBatchStats, count_bits: u32, index_built: bool) {
    assert_eq!(
        stats.updates,
        TightGpuUpdates::Admitted {
            count_bits,
            index_built
        }
    );
}

fn unavailable(stats: &TightGpuBatchStats, reason: TightGpuUpdateUnavailable) {
    assert_eq!(stats.updates, TightGpuUpdates::Unavailable(reason));
}

fn push(transaction: &mut FormulaTransaction<'_>, view: NodeView<'_>) -> usize {
    transaction.push(view, UNBOUNDED, UNBOUNDED).unwrap()
}

// Admits as many atoms as the fixture declares; 65,536 producers of one head
// need one atom beyond the default ceiling.
fn compile(atoms: usize, nodes: FormulaNodes, roots: Vec<usize>) -> TightPlan {
    let limits = AdmissionLimits {
        max_atoms: atoms.max(AdmissionLimits::default().max_atoms),
        ..AdmissionLimits::default()
    };
    let theory = Theory::new(atoms, nodes.into_parts(), roots, limits).unwrap();
    TightPlan::compile(
        &theory,
        TightPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap()
}

/// Inputs choose freely (`x ∨ ¬x`). Each of `inputs - 3` heads follows the
/// disjunction of the pairs of adjacent inputs starting at its index and two
/// after it; three more heads follow the guards of one threshold family over
/// the first six inputs, each by two duplicate rules, so the family's owner
/// re-evaluates followers and heads have several producers.
struct Ladder {
    certificate: TightPlan,
    inputs: usize,
    disjunctions: usize,
}

impl Ladder {
    fn new(inputs: usize) -> Self {
        let disjunctions = inputs - 3;
        let atoms = inputs + disjunctions + BOUNDS.len();
        let mut nodes = FormulaNodes::new(FormulaParts::new(vec![Node::falsum()], vec![]).unwrap());
        let mut transaction = nodes.transaction();
        let atom: Vec<usize> = (0..atoms)
            .map(|atom| push(&mut transaction, NodeView::Atom(atom)))
            .collect();
        let mut roots = Vec::new();
        for &input in &atom[..inputs] {
            let negation = push(&mut transaction, NodeView::Implies(input, 0));
            let choice = [input, negation];
            roots.push(push(&mut transaction, NodeView::Or(&choice)));
        }
        let pairs: Vec<usize> = (0..inputs - 1)
            .map(|index| {
                let pair = [atom[index], atom[index + 1]];
                push(&mut transaction, NodeView::And(&pair))
            })
            .collect();
        for index in 0..disjunctions {
            let disjuncts = [pairs[index], pairs[index + 2]];
            let body = push(&mut transaction, NodeView::Or(&disjuncts));
            roots.push(push(
                &mut transaction,
                NodeView::Implies(body, atom[inputs + index]),
            ));
        }
        let guards = transaction
            .push_threshold_family(&atom[..6], &[1; 6], &BOUNDS, UNBOUNDED, UNBOUNDED)
            .unwrap();
        for (index, &guard) in guards.iter().enumerate() {
            let head = atom[inputs + disjunctions + index];
            for _ in 0..2 {
                roots.push(
                    transaction
                        .push(NodeView::Implies(guard, head), UNBOUNDED, UNBOUNDED)
                        .unwrap(),
                );
            }
        }
        transaction.commit();
        Self {
            certificate: compile(atoms, nodes, roots),
            inputs,
            disjunctions,
        }
    }

    fn atoms(&self) -> usize {
        self.inputs + self.disjunctions + BOUNDS.len()
    }

    /// The level widths of a ladder of `inputs` inputs: the falsum and every
    /// atom; the negations, the adjacent pairs and the three guards; the
    /// choices, the disjunctions and the six guarded rules; the disjunctions'
    /// rules.
    fn widths(inputs: u64) -> Vec<u64> {
        vec![2 * inputs + 1, 2 * inputs + 2, 2 * inputs + 3, inputs - 3]
    }

    /// The fewest inputs, in steps of 64, with which the update route engages
    /// at every workgroup size.
    fn wide() -> Self {
        Self::new(least_engaging(64, 64, Self::widths))
    }

    /// The answer set choosing `chosen` inputs.
    fn model(&self, chosen: &[bool]) -> Vec<bool> {
        let mut atoms = vec![false; self.atoms()];
        atoms[..self.inputs].copy_from_slice(chosen);
        let pair = |index: usize| chosen[index] && chosen[index + 1];
        for index in 0..self.disjunctions {
            atoms[self.inputs + index] = pair(index) || pair(index + 2);
        }
        let count = chosen[..6].iter().filter(|chosen| **chosen).count();
        for (index, bound) in BOUNDS.iter().enumerate() {
            atoms[self.inputs + self.disjunctions + index] =
                i128::try_from(count).unwrap() >= *bound;
        }
        atoms
    }

    /// Slot `slot`'s candidate: the model, perturbed by slot.
    fn candidate(&self, slot: usize, chosen: &[bool]) -> Vec<bool> {
        perturbed(self.model(chosen), self.inputs..self.atoms(), slot)
    }

    fn patterns(&self, slots: usize, seed: usize) -> Vec<Vec<bool>> {
        patterns(self.inputs, slots, seed)
    }
}

/// Slot `slot`'s candidate from the answer set `atoms`, whose derived heads
/// are `derived`: the answer set, the answer set without its first derived
/// head (a false rule), the answer set with an unsupported head, or the
/// answer set.
fn perturbed(mut atoms: Vec<bool>, derived: std::ops::Range<usize>, slot: usize) -> Vec<bool> {
    match slot % 4 {
        1 => {
            if let Some(head) = derived.clone().find(|&head| atoms[head]) {
                atoms[head] = false;
            }
        }
        2 => {
            if let Some(head) = derived.clone().find(|&head| !atoms[head]) {
                atoms[head] = true;
            }
        }
        _ => {}
    }
    atoms
}

/// The inputs `slots` slots choose among `inputs`, from `seed`.
fn patterns(inputs: usize, slots: usize, seed: usize) -> Vec<Vec<bool>> {
    (0..slots)
        .map(|slot| {
            (0..inputs)
                .map(|input| !(input * 7 + slot + seed).is_multiple_of(3))
                .collect()
        })
        .collect()
}

/// Every slot's route across controlled changes, under the oracle's own
/// workgroup size and support construction: complete, then dense seeding the
/// state, then updates with nothing changed and with sparse changes; exactly
/// the threshold's change still updates, one more takes dense reuse after its
/// count, and the next update summarizes afresh, its summary counted in its
/// path; authority lost to a cancelled call restarts from the complete route.
/// After every batch the runtime guard decides as the host model predicts.
pub(super) fn qualify(oracle: &mut GpuTightOracle) {
    let ladder = Ladder::wide();
    let mut chosen = ladder.patterns(4, 0);
    let mut candidates: Vec<Vec<bool>> = chosen
        .iter()
        .enumerate()
        .map(|(slot, chosen)| ladder.candidate(slot, chosen))
        .collect();
    let mut sequence = Sequence::new(oracle, &ladder.certificate);
    let (first, _) = sequence.run(&candidates);
    assert_eq!(first.routes.complete, 4);
    assert_eq!(first.updates, TightGpuUpdates::Pending);
    let (seeding, _) = sequence.run(&candidates);
    assert_eq!(seeding.routes.dense_reuse, 4);
    assert_eq!(seeding.routes.beyond_threshold, 0);
    admitted(&seeding, 8, true);
    let (unchanged, _) = sequence.run(&candidates);
    assert_eq!(unchanged.routes.update, 4);
    assert_eq!(unchanged.nodes_evaluated, 0);
    admitted(&unchanged, 8, false);
    // Sparse changes: one to four inputs per slot, with their consequences.
    for (slot, pattern) in chosen.iter_mut().enumerate() {
        for flip in 0..=slot {
            let input = (7 * flip + slot) % ladder.inputs;
            pattern[input] = !pattern[input];
        }
        candidates[slot] = ladder.candidate(slot, pattern);
    }
    let (sparse, _) = sequence.run(&candidates);
    assert_eq!(sparse.routes.update, 4);
    assert!(sparse.nodes_evaluated > 0);
    // Slot 0 changes exactly the threshold's atoms, max(atoms / 8, 64), then one
    // more.
    let threshold = (ladder.atoms() / 8).max(64);
    for (changed, beyond) in [(threshold, 0), (threshold + 1, 1)] {
        for atom in &mut candidates[0][..changed] {
            *atom = !*atom;
        }
        let (stats, _) = sequence.run(&candidates);
        assert_eq!(stats.routes.dense_reuse, beyond, "{changed} changed atoms");
        assert_eq!(stats.routes.beyond_threshold, beyond);
        assert_eq!(stats.routes.update, 4 - beyond);
    }
    // Slot 0's last route was dense reuse beyond the threshold: its next update
    // summarizes afresh, and `judge` asserts the summary in its path.
    candidates[0][100] = !candidates[0][100];
    assert_eq!(sequence.beyond, [true, false, false, false]);
    let (summarized, _) = sequence.run(&candidates);
    assert_eq!(summarized.routes.update, 4);
    let cancelled = Cancellation::default();
    cancelled.cancel();
    let inputs: Vec<_> = candidates
        .iter()
        .map(|atoms| interpretation(ladder.certificate.theory(), atoms))
        .collect();
    assert!(matches!(
        sequence.oracle.check_batch(
            &ladder.certificate,
            &inputs,
            TightGpuLimits::default(),
            &cancelled
        ),
        Err(TightGpuError::Stopped(_))
    ));
    let (restarted, _) = sequence.run(&candidates);
    assert_eq!(restarted.routes.complete, 4);
    admitted(&restarted, 8, false);
    let (resumed, _) = sequence.run(&candidates);
    assert_eq!(resumed.routes.update, 4);
    qualify_receipts(oracle);
    qualify_narrow_levels(oracle);
    qualify_engagement_by_size(oracle);
    qualify_guard(oracle);
}

/// Overlapping checks of a wide ladder's slates match the same slates checked
/// synchronously by a second oracle of the same construction, batch by batch:
/// equal checks, the scalar plan's verdicts, and equal routes and update
/// availability, across the complete route, dense seeding, an unchanged update
/// and sparse changes, so the update route runs across consecutive overlapping
/// checks. Each overlapping check's production returns a marker, so the
/// comparison isolates the schedule.
pub(super) fn qualify_overlapped(backend: GpuApi, support: TightSupport) {
    let ladder = Ladder::wide();
    let certificate = &ladder.certificate;
    let theory = certificate.theory();
    let mut synchronous = super::oracle(backend, support);
    let mut overlapped = super::oracle(backend, support);
    let control = Cancellation::default();
    let mut chosen = ladder.patterns(4, 0);
    let mut slate = SlateId::first(theory).unwrap();
    let mut updates = 0;
    for round in 0..6 {
        if round >= 3 {
            for (slot, pattern) in chosen.iter_mut().enumerate() {
                let input = (7 * round + slot) % ladder.inputs;
                pattern[input] = !pattern[input];
            }
        }
        let candidates: Vec<Interpretation> = chosen
            .iter()
            .enumerate()
            .map(|(slot, pattern)| interpretation(theory, &ladder.candidate(slot, pattern)))
            .collect();
        let expected = synchronous
            .check_prefix(
                certificate,
                &candidates,
                TightGpuLimits::default(),
                &control,
            )
            .unwrap()
            .into_checks(theory, &candidates)
            .unwrap();
        let expected_stats = *synchronous.last_batch_stats().unwrap();
        let view = SlateRef::new(slate, theory, &candidates).unwrap();
        let prepared = overlapped
            .prepare(certificate, view, 0, TightGpuLimits::default(), &control)
            .unwrap();
        assert_eq!(prepared.count(), candidates.len());
        let (checked, produced) = prepared.check_overlapping(&control, || round);
        assert_eq!(produced, Some(round));
        let checks = checked.unwrap().into_checks(theory, slate, 0).unwrap();
        compare(certificate, &candidates, &checks);
        assert_eq!(checks, expected, "round {round}");
        let stats = *overlapped.last_batch_stats().unwrap();
        assert_eq!(stats.routes, expected_stats.routes, "round {round}");
        assert_eq!(stats.updates, expected_stats.updates, "round {round}");
        updates += stats.routes.update;
        slate = slate.successor().unwrap();
    }
    assert!(updates > 0, "no overlapping check took the update route");
    println!("tight overlapped checks: {updates} update slots over six slates");
}

/// Exact batches of four slots on one certificate under default limits, each
/// checked against the host model's paths and the predicted guard. While the
/// guard waits after a batch, the sequence runs that batch's candidates
/// unchanged until the probe has seeded every slot again.
struct Sequence<'a> {
    oracle: &'a mut GpuTightOracle,
    certificate: &'a TightPlan,
    model: Model,
    guard: Predicted,
    /// Each slot's candidate in the last batch.
    previous: Vec<Vec<bool>>,
    /// Whether the last batch's wait ended in a probe, which seeded every slot.
    reseeded: bool,
    /// Each slot that took dense reuse beyond the threshold in the last batch
    /// and was not seeded again since: its next update counts its summary into
    /// its path.
    beyond: Vec<bool>,
}

impl<'a> Sequence<'a> {
    fn new(oracle: &'a mut GpuTightOracle, certificate: &'a TightPlan) -> Self {
        Self {
            oracle,
            certificate,
            model: Model::new(certificate),
            guard: Predicted::new(),
            previous: Vec::new(),
            reseeded: false,
            beyond: Vec::new(),
        }
    }

    /// Check `candidates` as the next batch, judge it, and wait out the guard.
    fn run(&mut self, candidates: &[Vec<bool>]) -> (TightGpuBatchStats, Vec<TightGpuCheck>) {
        let batch = checked(
            self.oracle,
            self.certificate,
            candidates,
            TightGpuLimits::default(),
        );
        let previous = if self.previous.is_empty() {
            candidates
        } else {
            &self.previous
        };
        let beyond = judge(
            self.oracle,
            &self.model,
            previous,
            candidates,
            &self.beyond,
            &batch,
            &mut self.guard,
        );
        self.reseeded = wait_out(
            self.oracle,
            self.certificate,
            candidates,
            &mut self.guard,
            &batch.0,
        );
        self.beyond = if self.reseeded { Vec::new() } else { beyond };
        self.previous = candidates.to_vec();
        batch
    }
}

/// The engagement rule is decided per resident theory and workgroup size. Over
/// two atom leaves, `32 * (2 * STRIDES - 1)` rules take `2 * STRIDES` strides
/// of 32 invocations over two levels, enough to engage, but only
/// `STRIDES + 1` of 64 and fewer at 128 and 256, which the fitted size
/// selects; those decline the route.
fn qualify_engagement_by_size(oracle: &mut GpuTightOracle) {
    let rules = 32 * (2 * STRIDES - 1);
    assert_eq!(strided_depth(&[2, rules], 32), 2 * STRIDES);
    assert!(strided_depth(&[2, rules], 64) < 2 * STRIDES);
    let certificate = super::level_boundary(usize::try_from(rules).unwrap());
    let engages = oracle.kernel_options().workgroup_sizing
        == GpuWorkgroupSizing::Fixed(GpuWorkgroupSize::Threads32);
    let candidates = [vec![true, true], vec![false, false], vec![true, false]];
    let limits = TightGpuLimits::default();
    checked(oracle, &certificate, &candidates, limits);
    checked(oracle, &certificate, &candidates, limits);
    let (stats, _) = checked(oracle, &certificate, &candidates, limits);
    if engages {
        admitted(&stats, 16, false);
        assert_eq!(stats.routes.update, 3);
    } else {
        unavailable(&stats, TightGpuUpdateUnavailable::NarrowSchedule);
        assert_eq!(stats.routes.dense_reuse, 3);
    }
}

/// The update's receipt for one slot computed on the host from two complete
/// CPU evaluations: the nodes it takes and the operations it performs.
struct Receipt {
    nodes: u64,
    work: u64,
}

/// One update's change, from two complete CPU evaluations.
struct Change {
    before: Vec<bool>,
    after: Vec<bool>,
    /// The retained candidate's present atoms in each candidate word.
    retained: Vec<u64>,
    /// The atoms whose presence changed.
    atoms: Vec<usize>,
    /// The nodes taken from the queued set.
    taken: std::collections::BTreeSet<usize>,
    /// The atoms settling recomputes: the changed atoms and the heads of the
    /// producers whose bodies changed.
    touched: std::collections::BTreeSet<usize>,
    /// The words the verdict scans: the false-root set's, the unsupported
    /// set's, or none.
    verdict: usize,
}

impl Change {
    fn changed(&self, node: usize) -> bool {
        self.before[node] != self.after[node]
    }
}

/// The update route's work model, read from the theory: per node its readers
/// entries, the reads of its evaluation and dense reuse's least cost by the
/// guard's table, per atom its atom nodes, and the dependency schedule whose
/// words the workgroup's invocations divide.
struct Model {
    theory: Theory,
    producers: Vec<(usize, Option<usize>)>,
    readers: Vec<u64>,
    /// Nodes reading each node as a connective operand or a row owner.
    queues: Vec<Vec<usize>>,
    reads: Vec<u64>,
    atom_nodes: Vec<Vec<usize>>,
    /// Each row owner's live guards, the owner first: the owner's evaluation
    /// visits the readers of every one of them that changes.
    rows: std::collections::BTreeMap<usize, Vec<usize>>,
    /// Each wide conjunction's and disjunction's operands, and whether it is a
    /// conjunction: its reduction ends at the first operand that absorbs it.
    gates: std::collections::BTreeMap<usize, (bool, Vec<usize>)>,
    /// Dense reuse's least cost of each node by the guard's table.
    dense: Vec<u64>,
    /// Each node's schedule position.
    positions: Vec<usize>,
    /// The node at each schedule position.
    order: Vec<usize>,
    /// Each level's first position, then the schedule's end.
    offsets: Vec<usize>,
}

/// Whether a threshold guard reads its row: neither every candidate nor none
/// meets its bound.
fn live(threshold: &Threshold<'_>) -> bool {
    let total: i128 = threshold
        .weights
        .iter()
        .map(|&weight| i128::from(weight))
        .sum();
    threshold.bound > 0 && threshold.bound <= total
}

/// The dependency schedule of `theory`'s nodes: each node's position, the
/// levels in order of depth and a level's nodes in their original order; and
/// each level's first position, then the schedule's end. Leaves and constant
/// guards have depth zero; any other node lies one level beyond its deepest
/// operand or condition.
fn schedule(theory: &Theory) -> (Vec<usize>, Vec<usize>) {
    let view = theory.view();
    let mut depths: Vec<usize> = Vec::with_capacity(view.len());
    for node in 0..view.len() {
        let beyond = |operands: &[usize]| {
            1 + operands
                .iter()
                .map(|&operand| depths[operand])
                .max()
                .unwrap()
        };
        let depth = match view.node(node).unwrap() {
            NodeView::And(operands) | NodeView::Or(operands) => beyond(operands),
            NodeView::Implies(left, right) => beyond(&[left, right]),
            NodeView::Threshold(threshold) if live(&threshold) => beyond(threshold.conditions),
            NodeView::ExtremumCut(_) => unreachable!("the model covers thresholds only"),
            NodeView::Atom(_) | NodeView::False | NodeView::Threshold(_) => 0,
        };
        depths.push(depth);
    }
    let levels = depths.iter().max().unwrap() + 1;
    let mut offsets = vec![0; levels + 1];
    for &depth in &depths {
        offsets[depth + 1] += 1;
    }
    for level in 0..levels {
        offsets[level + 1] += offsets[level];
    }
    let mut next = offsets.clone();
    let positions = depths
        .iter()
        .map(|&depth| {
            next[depth] += 1;
            next[depth] - 1
        })
        .collect();
    (positions, offsets)
}

/// The most cost one of `size` invocations incurs in a phase whose items are
/// strided among them from item `first`, given each share's item and cost.
fn busiest(size: usize, first: usize, shares: impl IntoIterator<Item = (usize, u64)>) -> u64 {
    let mut invocations = vec![0_u64; size];
    for (item, cost) in shares {
        invocations[(item - first) % size] += cost;
    }
    invocations.into_iter().max().unwrap()
}

impl Model {
    fn new(certificate: &TightPlan) -> Self {
        let theory = certificate.theory().clone();
        let view = theory.view();
        let nodes = view.len();
        let mut readers = vec![0_u64; nodes];
        let mut queues = vec![Vec::new(); nodes];
        let mut reads = vec![0_u64; nodes];
        let mut dense = vec![1_u64; nodes];
        let mut atom_nodes = vec![Vec::new(); theory.atom_count()];
        let mut gates = std::collections::BTreeMap::new();
        // Live threshold guards by row: the first is the row's owner.
        let mut rows = std::collections::BTreeMap::<usize, Vec<usize>>::new();
        for (node, read) in reads.iter_mut().enumerate() {
            match view.node(node).unwrap() {
                view @ (NodeView::And(children) | NodeView::Or(children)) => {
                    for &child in children {
                        readers[child] += 1;
                        queues[child].push(node);
                    }
                    *read = children.len() as u64;
                    if children.len() > 2 {
                        let conjunction = matches!(view, NodeView::And(_));
                        gates.insert(node, (conjunction, children.to_vec()));
                        dense[node] = 2;
                    } else {
                        dense[node] = 3;
                    }
                }
                NodeView::Implies(left, right) => {
                    for child in [left, right] {
                        readers[child] += 1;
                        queues[child].push(node);
                    }
                    *read = 2;
                    dense[node] = 3;
                }
                NodeView::Atom(atom) => atom_nodes[atom].push(node),
                NodeView::Threshold(threshold) => {
                    if live(&threshold) {
                        let descriptor = theory.nodes()[node].threshold_descriptor().unwrap();
                        let row = theory.parts().thresholds()[descriptor].row;
                        rows.entry(row).or_default().push(node);
                    }
                }
                NodeView::ExtremumCut(_) => unreachable!("the model covers thresholds only"),
                NodeView::False => {}
            }
        }
        for (&row, guards) in &rows {
            let owner = guards[0];
            let span = theory.parts().rows()[row];
            for &condition in &theory.parts().conditions()[span.start..span.start + span.length] {
                readers[condition] += 1;
                queues[condition].push(owner);
            }
            reads[owner] = (span.length + guards.len()) as u64;
            // An unchanged row's change-bit reads and guard clears.
            dense[owner] = reads[owner];
        }
        let producers: Vec<_> = certificate
            .producers()
            .iter()
            .map(|producer| (producer.head(), producer.body()))
            .collect();
        for &(_, body) in &producers {
            if let Some(body) = body {
                readers[body] += 1;
            }
        }
        for &root in theory.roots() {
            readers[root] += 1;
        }
        let (positions, offsets) = schedule(&theory);
        let mut order = vec![0; nodes];
        for (node, &position) in positions.iter().enumerate() {
            order[position] = node;
        }
        Self {
            theory,
            producers,
            readers,
            queues,
            reads,
            atom_nodes,
            rows: rows
                .into_values()
                .map(|guards| (guards[0], guards))
                .collect(),
            gates,
            dense,
            positions,
            order,
            offsets,
        }
    }

    /// The schedule's level widths.
    fn widths(&self) -> Vec<u64> {
        self.offsets
            .windows(2)
            .map(|level| (level[1] - level[0]) as u64)
            .collect()
    }

    fn truth(candidate: &Interpretation) -> Vec<bool> {
        let mut workspace = EvaluationWorkspace::default();
        let attempt = workspace.evaluate(
            candidate,
            EvaluationLimits::default(),
            &Cancellation::default(),
        );
        attempt.result.unwrap().truth().to_vec()
    }

    /// The receipt of an update from `old` to `new`, summarizing first unless
    /// the slot's summary is current.
    fn receipt(
        &self,
        old: &Interpretation,
        new: &Interpretation,
        count_bits: u32,
        summarized: bool,
    ) -> Receipt {
        let change = self.change(old, new);
        Receipt {
            nodes: change.taken.len() as u64,
            work: self.work(&change, count_bits, summarized),
        }
    }

    fn change(&self, old: &Interpretation, new: &Interpretation) -> Change {
        let (before, after) = (Self::truth(old), Self::truth(new));
        let atoms = self.theory.atom_count();
        let mut retained = vec![0_u64; atoms.div_ceil(32)];
        for atom in old.atoms() {
            retained[atom / 32] += 1;
        }
        let presence = |interpretation: &Interpretation| {
            let mut present = vec![false; atoms];
            for atom in interpretation.atoms() {
                present[atom] = true;
            }
            present
        };
        let (was, is) = (presence(old), presence(new));
        let changed_atoms: Vec<usize> = (0..atoms).filter(|&atom| was[atom] != is[atom]).collect();
        let mut taken = std::collections::BTreeSet::new();
        for &atom in &changed_atoms {
            taken.extend(self.atom_nodes[atom].iter().copied());
        }
        for node in (0..before.len()).filter(|&node| before[node] != after[node]) {
            taken.extend(self.queues[node].iter().copied());
        }
        let mut touched: std::collections::BTreeSet<usize> =
            changed_atoms.iter().copied().collect();
        for &(head, body) in &self.producers {
            if body.is_some_and(|body| before[body] != after[body]) {
                touched.insert(head);
            }
        }
        let mut supported = vec![false; atoms];
        for &(head, body) in &self.producers {
            if body.is_none_or(|body| after[body]) {
                supported[head] = true;
            }
        }
        let unsupported = new.atoms().any(|atom| !supported[atom]);
        let roots = self.theory.roots();
        let verdict = if roots.iter().any(|&root| !after[root]) {
            roots.len().div_ceil(32)
        } else if unsupported {
            atoms.div_ceil(32)
        } else {
            0
        };
        Change {
            before,
            after,
            retained,
            atoms: changed_atoms,
            taken,
            touched,
            verdict,
        }
    }

    /// The work of an update: its floor, its summary when one runs, its
    /// presence pass, every evaluation and readers visit, settling and the
    /// verdict scan.
    fn work(&self, change: &Change, count_bits: u32, summarized: bool) -> u64 {
        let words = change.retained.len() as u64;
        let levels = (self.offsets.len() - 1) as u64;
        let mut work = 5 * words + change.before.len().div_ceil(32) as u64 + levels + RECORD_WORDS;
        if !summarized {
            let atoms = self.theory.atom_count();
            let roots = self.theory.roots().len();
            // The clears of the counts, the unsupported set and the false-root
            // set; the producer and root visits; the retained words, with one
            // count read per present atom.
            work += (atoms.div_ceil(32 / count_bits as usize)
                + atoms.div_ceil(32)
                + roots.div_ceil(32)
                + self.producers.len()
                + roots) as u64
                + words
                + change.retained.iter().sum::<u64>();
        }
        for &atom in &change.atoms {
            work += 1 + self.atom_nodes[atom].len() as u64;
        }
        for node in (0..change.before.len()).filter(|&node| change.changed(node)) {
            work += self.readers[node];
        }
        for &node in &change.taken {
            work += 1 + self.reads[node];
        }
        work + change.touched.len() as u64 + change.verdict as u64
    }

    /// The critical path of an update at `size` invocations, by the guard's
    /// cost table: per phase after the summary, the costliest invocation, each
    /// word's operations belonging to the invocation the phase's strided loop
    /// hands that word; and the floor, each invocation's share of the uniform
    /// scans and of the record.
    fn path(&self, change: &Change, size: usize) -> u64 {
        // A uniform strided loop's busiest invocation: the first.
        let strided = |count: usize| count.div_ceil(size) as u64;
        // The change count's, presence's and settling's scans, each level's
        // bounds and queued words, and the record.
        let mut path = 5 * strided(change.retained.len())
            + self
                .offsets
                .windows(2)
                .map(|level| strided(level[1].div_ceil(32) - level[0] / 32) + 1)
                .sum::<u64>()
            + RECORD_WORDS;
        path += busiest(
            size,
            0,
            change
                .atoms
                .iter()
                .map(|&atom| (atom / 32, 1 + self.atom_nodes[atom].len() as u64)),
        );
        for level in self.offsets.windows(2) {
            let span = level[0]..level[1];
            path += busiest(
                size,
                level[0] / 32,
                change
                    .taken
                    .iter()
                    .filter(|&&node| span.contains(&self.positions[node]))
                    .map(|&node| (self.positions[node] / 32, self.evaluation(change, node))),
            );
        }
        path += busiest(size, 0, change.touched.iter().map(|&atom| (atom / 32, 1)));
        path + strided(change.verdict)
    }

    /// What a summary adds to an update's path at `size` invocations when the
    /// slot took dense reuse beyond the threshold before it: the costliest
    /// invocation of each of its three phases. The clears of the counts at
    /// `count_bits`, the unsupported set and the false-root set, and then the
    /// producer and root visits, are uniform strided loops, whose first
    /// invocation is the busiest; the retained-word scan costs each word one
    /// operation and a count read per present atom.
    fn summary_path(&self, change: &Change, count_bits: u32, size: usize) -> u64 {
        let strided = |count: usize| count.div_ceil(size) as u64;
        let atoms = self.theory.atom_count();
        let roots = self.theory.roots().len();
        let clears = strided(atoms.div_ceil(32 / count_bits as usize))
            + strided(atoms.div_ceil(32))
            + strided(roots.div_ceil(32));
        let visits = strided(self.producers.len()) + strided(roots);
        let scan = busiest(
            size,
            0,
            change
                .retained
                .iter()
                .enumerate()
                .map(|(word, &present)| (word, 1 + present)),
        );
        clears + visits + scan
    }

    /// A taken node's cost by the guard's table: the node, the operands or
    /// conditions it reads, a wide gate's only up to the first that absorbs
    /// it, and the readers of each value that changes, its own or, for a row's
    /// owner, every guard's.
    fn evaluation(&self, change: &Change, node: usize) -> u64 {
        let visits: u64 = match self.rows.get(&node) {
            Some(guards) => guards
                .iter()
                .filter(|&&guard| change.changed(guard))
                .map(|&guard| self.readers[guard])
                .sum(),
            None if change.changed(node) => self.readers[node],
            None => 0,
        };
        let reads = match self.gates.get(&node) {
            Some((conjunction, operands)) => operands
                .iter()
                .position(|&operand| change.after[operand] != *conjunction)
                .map_or(operands.len(), |absorbing| absorbing + 1)
                as u64,
            None => self.reads[node],
        };
        1 + reads + visits
    }

    /// Dense reuse's least critical path for one candidate at `size`
    /// invocations, by the guard's cost table: each level's bounds and its
    /// busiest invocation over the positions `start + lane + j * size`; the
    /// support construction's phases under `support`; the unsupported scan's
    /// two words per candidate word; and the record.
    fn dense_path(&self, size: usize, support: TightSupport) -> u64 {
        let strided = |count: usize| count.div_ceil(size) as u64;
        let levels: u64 = self
            .offsets
            .windows(2)
            .map(|level| {
                1 + busiest(
                    size,
                    level[0],
                    (level[0]..level[1])
                        .map(|position| (position, self.dense[self.order[position]])),
                )
            })
            .sum();
        let words = self.theory.atom_count().div_ceil(32);
        let roots = self.theory.roots().len();
        let construction = match support {
            TightSupport::Atomic => strided(words) + strided(roots) + strided(self.producers.len()),
            TightSupport::Grouped => {
                let mut groups = vec![0_u64; words];
                for &(head, _) in &self.producers {
                    groups[head / 32] += 1;
                }
                busiest(
                    size,
                    0,
                    (0..roots).map(|root| (root, 1)).chain(
                        groups
                            .iter()
                            .enumerate()
                            .map(|(word, &producers)| (word, 1 + producers)),
                    ),
                )
            }
        };
        levels + construction + 2 * strided(words) + RECORD_WORDS
    }
}

/// The runtime guard as its specification predicts it: engaged, or declined
/// for a count of decoded batches, beside a backoff that starts at one batch
/// and doubles up to the wait limit after every losing judgment.
struct Predicted {
    waiting: u32,
    backoff: u32,
}

impl Predicted {
    fn new() -> Self {
        Self {
            waiting: 0,
            backoff: 1,
        }
    }

    fn allows(&self) -> bool {
        self.waiting == 0
    }

    /// Settle a decoded batch whose slowest update path is `slowest`, against
    /// dense reuse's `dense`: declined, the batch counts the wait down;
    /// engaged, a batch with update slots is judged.
    fn settle(&mut self, slowest: Option<u64>, dense: u64) {
        if self.waiting != 0 {
            self.waiting -= 1;
        } else if slowest.is_some_and(|slowest| slowest > dense) {
            self.waiting = self.backoff;
            self.backoff = (2 * self.backoff).min(GUARD_WAIT_LIMIT);
        }
    }
}

/// Check a decoded batch's critical paths against the host model and settle
/// the predicted guard with them: dense reuse's least path at the dispatched
/// size and the oracle's support construction; each update slot's path from
/// its previous candidate, a slot having updated exactly when its check
/// carries a path, with its summary when `beyond` marks the slot as having
/// taken dense reuse beyond the threshold before; and the batch's slowest.
/// Returns the slots that took dense reuse beyond the threshold in this batch:
/// a batch with such slots runs with every slot seeded, so they are exactly
/// its slots without a path.
fn judge(
    oracle: &GpuTightOracle,
    model: &Model,
    previous: &[Vec<bool>],
    candidates: &[Vec<bool>],
    beyond: &[bool],
    (stats, results): &(TightGpuBatchStats, Vec<TightGpuCheck>),
    guard: &mut Predicted,
) -> Vec<bool> {
    let size = stats.workgroup_size as usize;
    let dense = model.dense_path(size, oracle.support());
    assert_eq!(stats.dense_critical_path, dense);
    let theory = &model.theory;
    let mut slowest = None;
    let mut updates = 0;
    for (slot, result) in results.iter().enumerate() {
        if result.critical_path() != 0 {
            let change = model.change(
                &interpretation(theory, &previous[slot]),
                &interpretation(theory, &candidates[slot]),
            );
            let mut path = model.path(&change, size);
            if beyond.get(slot) == Some(&true) {
                let TightGpuUpdates::Admitted { count_bits, .. } = stats.updates else {
                    panic!("an updating batch admits the route: {:?}", stats.updates);
                };
                path += model.summary_path(&change, count_bits, size);
            }
            assert_eq!(result.critical_path(), path, "slot {slot}");
            slowest = slowest.max(Some(result.critical_path()));
            updates += 1;
        }
    }
    assert_eq!(stats.routes.update, updates);
    assert_eq!(stats.update_critical_path, slowest.unwrap_or(0));
    guard.settle(slowest, dense);
    let took: Vec<bool> = results
        .iter()
        .map(|result| stats.routes.beyond_threshold != 0 && result.critical_path() == 0)
        .collect();
    assert_eq!(
        took.iter().filter(|&&took| took).count() as u64,
        stats.routes.beyond_threshold
    );
    took
}

/// While the predicted guard waits, run `candidates` unchanged: each batch
/// reports `GuardDeclined` and reuses dense evaluation on every slot, its
/// state still accounted as in `admitted`'s batch; then the probe seeds every
/// slot, so that the next batch can update. Returns whether the guard waited.
fn wait_out(
    oracle: &mut GpuTightOracle,
    certificate: &TightPlan,
    candidates: &[Vec<bool>],
    guard: &mut Predicted,
    admitted: &TightGpuBatchStats,
) -> bool {
    if guard.allows() {
        return false;
    }
    let slots = candidates.len() as u64;
    let limits = TightGpuLimits::default();
    while !guard.allows() {
        let (declined, _) = checked(oracle, certificate, candidates, limits);
        unavailable(&declined, TightGpuUpdateUnavailable::GuardDeclined);
        assert_eq!(declined.routes.dense_reuse, slots);
        assert_eq!(
            declined.resident_transport_bytes,
            admitted.resident_transport_bytes
        );
        guard.settle(None, 0);
    }
    let (probe, _) = checked(oracle, certificate, candidates, limits);
    assert!(
        matches!(
            probe.updates,
            TightGpuUpdates::Admitted {
                index_built: false,
                ..
            }
        ),
        "{:?}",
        probe.updates
    );
    assert_eq!(probe.routes.dense_reuse, slots);
    guard.settle(None, 0);
    true
}

/// An update's receipts equal the host model's: the nodes taken from the
/// queued set; the work of its floor, its summary when one runs, its presence
/// pass, every evaluation and readers visit, settling and the verdict scan;
/// and its critical path by the guard's cost table, each phase after the
/// summary at its costliest invocation over the words it owns, plus the
/// floor's share of the uniform scans. Two updates in turn: the first
/// summarizes, the second does not unless the guard declined between them.
/// On the ladder, and on wide gates, whose paths count the operands their
/// reductions read.
fn qualify_receipts(oracle: &mut GpuTightOracle) {
    let ladder = Ladder::wide();
    assert_eq!(
        Model::new(&ladder.certificate).widths(),
        Ladder::widths(u64::try_from(ladder.inputs).unwrap())
    );
    exact_receipts(
        oracle,
        &ladder.certificate,
        ladder.patterns(4, 1),
        |slot, chosen| ladder.candidate(slot, chosen),
    );
    let gates = Gates::wide();
    assert_eq!(
        Model::new(&gates.certificate).widths(),
        Gates::widths(u64::try_from(gates.inputs).unwrap())
    );
    exact_receipts(
        oracle,
        &gates.certificate,
        patterns(gates.inputs, 4, 1),
        |slot, chosen| gates.candidate(slot, chosen),
    );
}

/// Two updates in turn of four slots holding `candidate(slot, inputs)` for
/// `chosen` inputs, each slot flipping one input per round, against the host
/// model's receipts and paths.
fn exact_receipts(
    oracle: &mut GpuTightOracle,
    certificate: &TightPlan,
    mut chosen: Vec<Vec<bool>>,
    candidate: impl Fn(usize, &[bool]) -> Vec<bool>,
) {
    let theory = certificate.theory();
    let mut candidates: Vec<Vec<bool>> = chosen
        .iter()
        .enumerate()
        .map(|(slot, chosen)| candidate(slot, chosen))
        .collect();
    let mut sequence = Sequence::new(oracle, certificate);
    sequence.run(&candidates);
    let (seeding, _) = sequence.run(&candidates);
    let TightGpuUpdates::Admitted { count_bits, .. } = seeding.updates else {
        panic!("the update route is admitted: {:?}", seeding.updates);
    };
    // The first update after seeding summarizes; a probe after a wait seeds
    // the state again, so the next update summarizes once more.
    let mut summarized = false;
    for round in 0..2 {
        let previous = candidates.clone();
        for (slot, pattern) in chosen.iter_mut().enumerate() {
            let input = (5 * slot + 3 * round + 1) % pattern.len();
            pattern[input] = !pattern[input];
            candidates[slot] = candidate(slot, pattern);
        }
        let (stats, results) = sequence.run(&candidates);
        assert_eq!(stats.routes.update, 4);
        let mut nodes = 0;
        for (slot, result) in results.iter().enumerate() {
            let receipt = sequence.model.receipt(
                &interpretation(theory, &previous[slot]),
                &interpretation(theory, &candidates[slot]),
                count_bits,
                summarized,
            );
            assert_eq!(result.work(), receipt.work, "slot {slot} round {round}");
            nodes += receipt.nodes;
        }
        assert_eq!(stats.nodes_evaluated, nodes, "round {round}");
        summarized = !sequence.reseeded;
    }
}

/// Inputs choose freely; each three consecutive inputs derive one head by
/// their conjunction and another by their disjunction, so that every update
/// evaluates wide gates, whose reductions end at their first absorbing
/// operand.
struct Gates {
    certificate: TightPlan,
    inputs: usize,
}

impl Gates {
    fn new(inputs: usize) -> Self {
        let triples = inputs - 2;
        let atoms = inputs + 2 * triples;
        let mut nodes = FormulaNodes::new(FormulaParts::new(vec![Node::falsum()], vec![]).unwrap());
        let mut transaction = nodes.transaction();
        let atom: Vec<usize> = (0..atoms)
            .map(|atom| push(&mut transaction, NodeView::Atom(atom)))
            .collect();
        let mut roots = Vec::new();
        for &input in &atom[..inputs] {
            let negation = push(&mut transaction, NodeView::Implies(input, 0));
            let choice = [input, negation];
            roots.push(push(&mut transaction, NodeView::Or(&choice)));
        }
        for index in 0..triples {
            let triple = &atom[index..index + 3];
            let conjunction = push(&mut transaction, NodeView::And(triple));
            roots.push(push(
                &mut transaction,
                NodeView::Implies(conjunction, atom[inputs + index]),
            ));
            let disjunction = push(&mut transaction, NodeView::Or(triple));
            roots.push(push(
                &mut transaction,
                NodeView::Implies(disjunction, atom[inputs + triples + index]),
            ));
        }
        transaction.commit();
        Self {
            certificate: compile(atoms, nodes, roots),
            inputs,
        }
    }

    /// The level widths of `inputs` inputs: the falsum and every atom; the
    /// negations and the wide gates; the choices and the gates' rules.
    fn widths(inputs: u64) -> Vec<u64> {
        let gates = inputs + 2 * (inputs - 2);
        vec![1 + gates, gates, gates]
    }

    /// The fewest inputs, in steps of 64, with which the update route engages
    /// at every workgroup size.
    fn wide() -> Self {
        Self::new(least_engaging(64, 64, Self::widths))
    }

    /// Slot `slot`'s candidate for `chosen` inputs: the answer set, perturbed
    /// by slot.
    fn candidate(&self, slot: usize, chosen: &[bool]) -> Vec<bool> {
        let triples = self.inputs - 2;
        let mut atoms = chosen.to_vec();
        atoms.extend((0..triples).map(|index| chosen[index..index + 3].iter().all(|&input| input)));
        atoms.extend((0..triples).map(|index| chosen[index..index + 3].iter().any(|&input| input)));
        let derived = self.inputs..atoms.len();
        perturbed(atoms, derived, slot)
    }
}

/// The reach fixture's inputs: one candidate word.
const REACH_INPUTS: usize = 32;

/// Inputs no rule derives, each read by `fan` rules deriving a head of its
/// own (`y :- x`): the two-level reach fixture of the route's crossover
/// measurement, over one candidate word of inputs. Each input's node has
/// `2 * fan` readers entries, its rules and their producers, all evaluated by
/// the invocation owning the input's queued word.
fn reach(fan: usize) -> TightPlan {
    let atoms = REACH_INPUTS * (1 + fan);
    let mut nodes = FormulaNodes::new(FormulaParts::new(vec![Node::falsum()], vec![]).unwrap());
    let mut transaction = nodes.transaction();
    let atom: Vec<usize> = (0..atoms)
        .map(|atom| push(&mut transaction, NodeView::Atom(atom)))
        .collect();
    let roots = (0..REACH_INPUTS * fan)
        .map(|rule| {
            push(
                &mut transaction,
                NodeView::Implies(atom[rule / fan], atom[REACH_INPUTS + rule]),
            )
        })
        .collect();
    transaction.commit();
    compile(atoms, nodes, roots)
}

/// The reach fixture's candidates: each slot's inputs follow a pattern of its
/// own, every one flipped when `flipped` holds, and no head is held.
fn reach_candidates(atoms: usize, flipped: bool) -> Vec<Vec<bool>> {
    (0..4)
        .map(|slot| {
            (0..atoms)
                .map(|atom| atom < REACH_INPUTS && (5 * atom + slot).is_multiple_of(3) != flipped)
                .collect()
        })
        .collect()
}

/// The runtime guard on the reach fixture, with the fewest heads per input,
/// in steps of 8, with which the route engages at every size. Each batch flips
/// every input of every slot, a clustered change of one candidate word whose
/// reach gives every update a path beyond dense reuse's, by the host model.
/// The first judged batch updates and loses; the guard then declines the
/// route for 1, 2 and 4 batches in turn, which reuse dense evaluation on the
/// kernel without the update route with the state still accounted; each probe
/// after a wait seeds the state, and the batch after it updates and is judged
/// again. A cancelled call while the guard waits is not judged and consumes no
/// wait. Every verdict is the scalar plan's.
fn qualify_guard(oracle: &mut GpuTightOracle) {
    let widths = |fan: u64| {
        let inputs = REACH_INPUTS as u64;
        vec![1 + inputs * (1 + fan), inputs * fan]
    };
    let certificate = &reach(least_engaging(8, 8, widths));
    let model = Model::new(certificate);
    let mut guard = Predicted::new();
    let atoms = certificate.theory().atom_count();
    let limits = TightGpuLimits::default();
    let mut previous = reach_candidates(atoms, false);
    let mut beyond = Vec::new();
    let mut flipped = false;
    let mut next = |oracle: &mut GpuTightOracle, guard: &mut Predicted| {
        flipped = !flipped;
        let candidates = reach_candidates(atoms, flipped);
        let batch = checked(oracle, certificate, &candidates, limits);
        beyond = judge(
            oracle,
            &model,
            &previous,
            &candidates,
            &beyond,
            &batch,
            guard,
        );
        previous = candidates;
        batch.0
    };
    let first = next(oracle, &mut guard);
    assert_eq!(first.routes.complete, 4);
    let seeding = next(oracle, &mut guard);
    assert_eq!(seeding.routes.dense_reuse, 4);
    let TightGpuUpdates::Admitted {
        count_bits,
        index_built: true,
    } = seeding.updates
    else {
        panic!("the seeding batch builds the index: {:?}", seeding.updates);
    };
    let cancelled = Cancellation::default();
    cancelled.cancel();
    let inputs: Vec<_> = reach_candidates(atoms, false)
        .iter()
        .map(|atoms| interpretation(certificate.theory(), atoms))
        .collect();
    for wait in [1, 2, 4] {
        let judged = next(oracle, &mut guard);
        admitted(&judged, count_bits, false);
        assert_eq!(judged.routes.update, 4);
        // The host model predicts the loss, and the wait it sets.
        assert_eq!(guard.waiting, wait);
        for waited in 0..wait {
            // The cancelled call drops reuse authority, so the batch after it
            // restarts from the complete route, still declined.
            let restarted = wait == 2 && waited == 1;
            if restarted {
                assert!(matches!(
                    oracle.check_batch(certificate, &inputs, limits, &cancelled),
                    Err(TightGpuError::Stopped(_))
                ));
            }
            let declined = next(oracle, &mut guard);
            unavailable(&declined, TightGpuUpdateUnavailable::GuardDeclined);
            let reused = if restarted { 0 } else { 4 };
            assert_eq!(
                (declined.routes.dense_reuse, declined.routes.update),
                (reused, 0)
            );
            assert_eq!(
                declined.resident_transport_bytes,
                judged.resident_transport_bytes
            );
        }
        // The probe sets the route's parameter again: records written while
        // the guard declined say unseeded, so every slot seeds.
        let probe = next(oracle, &mut guard);
        admitted(&probe, count_bits, false);
        assert_eq!(probe.routes.dense_reuse, 4);
    }
}

/// Inputs choose freely; a ladder of rungs follows them, each rung combining
/// the previous rung with one input, alternating disjunction and conjunction,
/// so later levels hold one rung and one rule each and one 32-bit word of the
/// queued set spans many levels. Each input feeds every fourth rung: a change
/// queues rungs at several levels of one word at once, and each level's take
/// must leave the others queued. Each rung derives a head. A filler level of
/// `filler` negations of one atom no candidate holds lets dense evaluation
/// need enough strides per level for the route to engage; nothing queues it.
struct Narrow {
    certificate: TightPlan,
    rungs: usize,
}

const NARROW_INPUTS: usize = 4;

impl Narrow {
    fn new(rungs: usize, filler: usize) -> Self {
        // Inputs, then the filler atom, then one head per rung.
        let atoms = NARROW_INPUTS + 1 + rungs;
        let mut nodes = FormulaNodes::new(FormulaParts::new(vec![Node::falsum()], vec![]).unwrap());
        let mut transaction = nodes.transaction();
        let atom: Vec<usize> = (0..atoms)
            .map(|atom| push(&mut transaction, NodeView::Atom(atom)))
            .collect();
        let mut roots = Vec::new();
        for &input in &atom[..NARROW_INPUTS] {
            let negation = push(&mut transaction, NodeView::Implies(input, 0));
            let choice = [input, negation];
            roots.push(push(&mut transaction, NodeView::Or(&choice)));
        }
        for _ in 0..filler {
            push(&mut transaction, NodeView::Implies(atom[NARROW_INPUTS], 0));
        }
        let mut rung = atom[0];
        for index in 0..rungs {
            let operands = [rung, atom[index % NARROW_INPUTS]];
            rung = push(
                &mut transaction,
                if index % 2 == 0 {
                    NodeView::Or(&operands)
                } else {
                    NodeView::And(&operands)
                },
            );
            roots.push(push(
                &mut transaction,
                NodeView::Implies(rung, atom[NARROW_INPUTS + 1 + index]),
            ));
        }
        transaction.commit();
        Self {
            certificate: compile(atoms, nodes, roots),
            rungs,
        }
    }

    /// The level widths of `rungs` rungs beside `filler` filler nodes: the
    /// falsum, inputs, filler atom and heads; the negations, the filler and the
    /// first rung; the choices, the first rule and the second rung; then one
    /// rung and one rule per level; the last rule.
    fn widths(rungs: u64, filler: u64) -> Vec<u64> {
        let inputs = u64::try_from(NARROW_INPUTS).unwrap();
        let mut widths = vec![1 + inputs + 1 + rungs, inputs + filler + 1, inputs + 2];
        widths.extend(std::iter::repeat_n(2, usize::try_from(rungs - 2).unwrap()));
        widths.push(1);
        widths
    }

    fn model(&self, chosen: [bool; NARROW_INPUTS]) -> Vec<bool> {
        let mut atoms = chosen.to_vec();
        atoms.push(false);
        let mut rung = chosen[0];
        for index in 0..self.rungs {
            let input = chosen[index % NARROW_INPUTS];
            rung = if index % 2 == 0 {
                rung || input
            } else {
                rung && input
            };
            atoms.push(rung);
        }
        atoms
    }
}

fn qualify_narrow_levels(oracle: &mut GpuTightOracle) {
    // Sixteen rungs over 18 levels, and the fewest filler nodes, in steps of
    // 256, with which the route engages at every size.
    let filler = least_engaging(0, 256, |filler| Narrow::widths(16, filler));
    let narrow = Narrow::new(16, filler);
    let certificate = &narrow.certificate;
    let limits = TightGpuLimits::default();
    // Input patterns in turn, in two slots: the model, and the model with the
    // last rung's head flipped.
    let mut updated = 0;
    for pattern in [
        0b0000, 0b0000, 0b0001, 0b0011, 0b0111, 0b1111, 0b1110, 0b1100, 0b1000, 0b0101,
    ] {
        let chosen = std::array::from_fn(|input| pattern & (1 << input) != 0);
        let model = narrow.model(chosen);
        let mut flipped = model.clone();
        let last = flipped.len() - 1;
        flipped[last] = !flipped[last];
        let (stats, _) = checked(oracle, certificate, &[model, flipped], limits);
        updated += stats.routes.update;
    }
    // Two complete and seeding batches, then every slot updates.
    assert_eq!(updated, 2 * 8);
}

/// A head with `producers` rules, one from each freely chosen input.
fn fan_in(producers: usize) -> TightPlan {
    let mut nodes = FormulaNodes::new(FormulaParts::new(vec![Node::falsum()], vec![]).unwrap());
    let mut transaction = nodes.transaction();
    let head = push(&mut transaction, NodeView::Atom(producers));
    let mut roots = Vec::new();
    for input in 0..producers {
        let atom = push(&mut transaction, NodeView::Atom(input));
        let negation = push(&mut transaction, NodeView::Implies(atom, 0));
        let choice = [atom, negation];
        roots.push(push(&mut transaction, NodeView::Or(&choice)));
        roots.push(push(&mut transaction, NodeView::Implies(atom, head)));
    }
    transaction.commit();
    compile(producers + 1, nodes, roots)
}

// Support counts take the narrowest width holding the most producers of one
// head: 16 bits from 256 producers, 32 from 65,536. The first fan has the
// fewest producers, in steps of 256 from 256, with which the route engages at
// every size; the second engages too.
fn qualify_count_widths(oracle: &mut GpuTightOracle) {
    // The falsum, the head and the inputs; their negations and rules; the
    // choices.
    let widths = |producers: u64| vec![producers + 2, 2 * producers, producers];
    assert!(engages_at_every_size(&widths(65_536)));
    let sixteen = least_engaging(256, 256, widths);
    for (producers, bits) in [(sixteen, 16), (65_536, 32)] {
        let certificate = fan_in(producers);
        let candidate = |chosen: &[usize]| {
            let mut atoms = vec![false; producers + 1];
            for &input in chosen {
                atoms[input] = true;
            }
            atoms[producers] = !chosen.is_empty();
            atoms
        };
        let limits = TightGpuLimits::default();
        // The head's own change, as the chosen set empties, reaches every
        // producer's rule: beyond dense reuse's path under atomic support, so
        // the runtime guard may decline the next batch. Two unchanged batches
        // let its wait and probe pass, and the count returns from zero by an
        // update.
        let sequence = [
            vec![0, 1],
            vec![0, 1],
            vec![1],
            vec![],
            vec![],
            vec![],
            vec![5, 299],
        ];
        let mut last = None;
        for chosen in &sequence {
            // The second slot keeps its head but loses its support.
            let mut unsupported = candidate(chosen);
            unsupported[producers] = true;
            last = Some(
                checked(
                    oracle,
                    &certificate,
                    &[candidate(chosen), unsupported],
                    limits,
                )
                .0,
            );
        }
        let last = last.unwrap();
        admitted(&last, bits, false);
        assert_eq!(last.routes.update, 2);
    }
}

/// Each reason the route can be unavailable, observed in the batch that would
/// otherwise have admitted it, while dense reuse keeps exact verdicts. A later
/// call that cannot admit the state releases it, keeping its truth and result
/// authority, and the next call admitting it allocates it zeroed and seeds it.
fn qualify_unavailable(oracle: &mut GpuTightOracle) {
    use TightGpuUpdateUnavailable::{
        Bytes, CandidateCeiling, Declined, NarrowSchedule, SerialSchedule, Work,
    };
    // A narrow ladder: even at 32 invocations, the size with the most strides,
    // dense evaluation takes fewer than the route needs.
    assert!(strided_depth(&Ladder::widths(40), 32) < 4 * STRIDES);
    let narrow = Ladder::new(40);
    let narrow_candidates: Vec<_> = narrow
        .patterns(2, 2)
        .iter()
        .enumerate()
        .map(|(slot, chosen)| narrow.candidate(slot, chosen))
        .collect();
    // The schedule declines the route from the fresh theory's first batch on.
    for reused in [false, true] {
        let (stats, _) = checked(
            oracle,
            &narrow.certificate,
            &narrow_candidates,
            TightGpuLimits::default(),
        );
        unavailable(&stats, NarrowSchedule);
        assert_eq!(stats.routes.dense_reuse, if reused { 2 } else { 0 });
    }
    let ladder = Ladder::wide();
    let certificate = &ladder.certificate;
    let candidates: Vec<_> = ladder
        .patterns(2, 2)
        .iter()
        .enumerate()
        .map(|(slot, chosen)| ladder.candidate(slot, chosen))
        .collect();
    let defaults = TightGpuLimits::default();
    let declined = TightGpuLimits {
        min_update_candidates: 0,
        ..defaults
    };
    // Engage the route, then decline it: the state is released, dense reuse
    // keeps its authority, and the stateless layout's bytes are observed.
    for _ in 0..3 {
        checked(oracle, certificate, &candidates, defaults);
    }
    let (hot, _) = checked(oracle, certificate, &candidates, declined);
    unavailable(&hot, Declined);
    assert_eq!(hot.routes.dense_reuse, 2);
    // Exactly the stateless layout's bytes leave no room for the state.
    let bytes = TightGpuLimits {
        max_batch_bytes: hot.accounted_bytes,
        ..defaults
    };
    let (refused, _) = checked(oracle, certificate, &candidates, bytes);
    unavailable(&refused, Bytes);
    assert_eq!(refused.routes.dense_reuse, 2);
    // A fresh, zeroed state is seeded by dense reuse, then slots update.
    let (seeding, _) = checked(oracle, certificate, &candidates, defaults);
    admitted(&seeding, 8, false);
    assert_eq!(seeding.routes.dense_reuse, 2);
    let (updating, _) = checked(oracle, certificate, &candidates, defaults);
    assert_eq!(updating.routes.update, 2);
    // The update route's reservation exceeds dense reuse's charge.
    let dense = hot.work / 2;
    let work = TightGpuLimits {
        max_work_per_candidate: dense,
        ..defaults
    };
    let (refused, results) = checked(oracle, certificate, &candidates, work);
    unavailable(&refused, Work);
    assert_eq!(refused.routes.dense_reuse, 2);
    assert!(results.iter().all(|result| result.work() == dense));
    // A prefix offering fewer candidates than the floor.
    let floor = TightGpuLimits {
        min_update_candidates: 3,
        ..defaults
    };
    let theory = certificate.theory();
    let offered: Vec<_> = candidates
        .iter()
        .map(|atoms| interpretation(theory, atoms))
        .collect();
    for _ in 0..2 {
        let batch = oracle
            .check_prefix(certificate, &offered, floor, &Cancellation::default())
            .unwrap();
        compare(certificate, &offered, batch.checks());
    }
    let stats = oracle.last_batch_stats().unwrap();
    unavailable(stats, CandidateCeiling);
    assert_eq!(stats.routes.dense_reuse, 2);
    // A single node is evaluated serially, which has no levels to update by.
    let serial = super::certificate(1, vec![Node::atom(0)], vec![0]);
    let fact = [vec![true], vec![false]];
    for _ in 0..2 {
        let (stats, _) = checked(oracle, &serial, &fact, defaults);
        assert_eq!(stats.truth_levels, 0);
        unavailable(&stats, SerialSchedule);
    }
}

/// With the state counted in each slot's footprint, a byte-bound prefix
/// shrinks once the state is admitted: the batch that builds the index runs
/// complete at the smaller count, counts settle at the state-inclusive
/// prefix after at most one more complete batch, and then slots update. The
/// floor guard instead keeps the stateless count when the state-inclusive
/// prefix falls below `min_update_candidates`.
fn qualify_prefix_transition(oracle: &GpuTightOracle) {
    let ladder = Ladder::wide();
    let certificate = &ladder.certificate;
    let theory = certificate.theory();
    let candidates: Vec<_> = ladder
        .patterns(64, 3)
        .iter()
        .enumerate()
        .map(|(slot, chosen)| interpretation(theory, &ladder.candidate(slot, chosen)))
        .collect();
    let fresh = || {
        GpuTightOracle::from_context_with_kernel(
            oracle.context(),
            oracle.support(),
            oracle.kernel_options(),
        )
        .unwrap()
    };
    let prefix = |oracle: &mut GpuTightOracle, limits| {
        let batch = oracle
            .check_prefix(certificate, &candidates, limits, &Cancellation::default())
            .unwrap();
        let count = batch.checks().len();
        compare(certificate, &candidates[..count], batch.checks());
        *oracle.last_batch_stats().unwrap()
    };
    // The stateless layout's bytes at 64 candidates, hot.
    let mut measured = fresh();
    let declined = TightGpuLimits {
        min_update_candidates: 0,
        ..TightGpuLimits::default()
    };
    prefix(&mut measured, declined);
    let hot = prefix(&mut measured, declined);
    assert_eq!(hot.candidates, 64);
    for (floor, shrinks) in [(64, false), (1, true)] {
        let limits = TightGpuLimits {
            max_batch_bytes: hot.accounted_bytes,
            min_update_candidates: floor,
            ..TightGpuLimits::default()
        };
        let mut oracle = fresh();
        // Cold packing bytes can shrink the first prefix; the second runs at 64.
        prefix(&mut oracle, limits);
        let stateless = prefix(&mut oracle, limits);
        assert_eq!(stateless.candidates, 64);
        let admission = prefix(&mut oracle, limits);
        if !shrinks {
            unavailable(&admission, TightGpuUpdateUnavailable::Bytes);
            assert_eq!(
                (admission.candidates, admission.routes.dense_reuse),
                (64, 64)
            );
            continue;
        }
        admitted(&admission, 8, true);
        assert!(admission.candidates < 64);
        assert_eq!(admission.routes.complete, admission.candidates);
        let mut settled = prefix(&mut oracle, limits);
        if settled.routes.complete == settled.candidates {
            settled = prefix(&mut oracle, limits);
        }
        assert!(settled.candidates < 64);
        assert_eq!(settled.routes.update, settled.candidates);
    }
}

/// The availability, count-width and prefix qualifications under the oracle's
/// default kernel.
pub(super) fn qualify_admission(oracle: &mut GpuTightOracle) {
    qualify_unavailable(oracle);
    qualify_count_widths(oracle);
    qualify_prefix_transition(oracle);
}
