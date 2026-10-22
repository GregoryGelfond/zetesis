//! The update route against the scalar plan: per-slot routes, receipts and
//! verdicts across controlled candidate changes, and the route's availability.

use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, EvaluationLimits, EvaluationWorkspace, FormulaNodes, FormulaParts,
    FormulaTransaction, Interpretation, Node, NodeView, Theory, TightCheckLimits, TightPlan,
    TightPlanLimits,
};
use zetesis_wgpu::{
    GpuTightOracle, GpuWorkgroupSize, GpuWorkgroupSizing, TightGpuBatchStats, TightGpuCheck,
    TightGpuError, TightGpuLimits, TightGpuUpdateUnavailable, TightGpuUpdates,
};

const UNBOUNDED: usize = usize::MAX;
/// The update route's engagement factor, from which every fixture that must
/// engage, or must not, is sized.
const STRIDES: u64 = GpuTightOracle::UPDATE_ENGAGEMENT_STRIDES;
/// The ladder's threshold family's bounds over its first six inputs.
const BOUNDS: [i128; 3] = [2, 3, 5];
/// Words of a result record, a part of every update's floor.
const RECORD_WORDS: u64 = 9;

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

    /// Slot `slot`'s candidate: the model, the model without its first derived
    /// head (a false rule), the model with an unsupported head, or the model.
    fn candidate(&self, slot: usize, chosen: &[bool]) -> Vec<bool> {
        let mut atoms = self.model(chosen);
        let derived = self.inputs..self.atoms();
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

    fn patterns(&self, slots: usize, seed: usize) -> Vec<Vec<bool>> {
        (0..slots)
            .map(|slot| {
                (0..self.inputs)
                    .map(|input| !(input * 7 + slot + seed).is_multiple_of(3))
                    .collect()
            })
            .collect()
    }
}

/// Every slot's route across controlled changes, under the oracle's own
/// workgroup size and support construction: complete, then dense seeding the
/// state, then updates with nothing changed and with sparse changes; exactly
/// the threshold's change still updates, one more takes dense reuse after its
/// count, and the next update summarizes afresh; authority lost to a
/// cancelled call restarts from the complete route.
pub(super) fn qualify(oracle: &mut GpuTightOracle) {
    let ladder = Ladder::wide();
    let certificate = &ladder.certificate;
    let limits = TightGpuLimits::default();
    let mut chosen = ladder.patterns(4, 0);
    let mut candidates: Vec<Vec<bool>> = chosen
        .iter()
        .enumerate()
        .map(|(slot, chosen)| ladder.candidate(slot, chosen))
        .collect();
    let (first, _) = checked(oracle, certificate, &candidates, limits);
    assert_eq!(first.routes.complete, 4);
    assert_eq!(first.updates, TightGpuUpdates::Pending);
    let (seeding, _) = checked(oracle, certificate, &candidates, limits);
    assert_eq!(seeding.routes.dense_reuse, 4);
    assert_eq!(seeding.routes.beyond_threshold, 0);
    admitted(&seeding, 8, true);
    let (unchanged, _) = checked(oracle, certificate, &candidates, limits);
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
    let (sparse, _) = checked(oracle, certificate, &candidates, limits);
    assert_eq!(sparse.routes.update, 4);
    assert!(sparse.nodes_evaluated > 0);
    // Slot 0 changes exactly the threshold's atoms, max(atoms / 8, 64), then one
    // more.
    let threshold = (ladder.atoms() / 8).max(64);
    for (changed, beyond) in [(threshold, 0), (threshold + 1, 1)] {
        for atom in &mut candidates[0][..changed] {
            *atom = !*atom;
        }
        let (stats, _) = checked(oracle, certificate, &candidates, limits);
        assert_eq!(stats.routes.dense_reuse, beyond, "{changed} changed atoms");
        assert_eq!(stats.routes.beyond_threshold, beyond);
        assert_eq!(stats.routes.update, 4 - beyond);
    }
    // Slot 0's last route was dense: its next update summarizes afresh.
    candidates[0][100] = !candidates[0][100];
    let (summarized, _) = checked(oracle, certificate, &candidates, limits);
    assert_eq!(summarized.routes.update, 4);
    let cancelled = Cancellation::default();
    cancelled.cancel();
    let inputs: Vec<_> = candidates
        .iter()
        .map(|atoms| interpretation(certificate.theory(), atoms))
        .collect();
    assert!(matches!(
        oracle.check_batch(certificate, &inputs, limits, &cancelled),
        Err(TightGpuError::Stopped(_))
    ));
    let (restarted, _) = checked(oracle, certificate, &candidates, limits);
    assert_eq!(restarted.routes.complete, 4);
    admitted(&restarted, 8, false);
    let (resumed, _) = checked(oracle, certificate, &candidates, limits);
    assert_eq!(resumed.routes.update, 4);
    qualify_receipts(oracle);
    qualify_narrow_levels(oracle);
    qualify_engagement_by_size(oracle);
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

/// The update route's work model, read from the theory: per node its readers
/// entries and the reads of its evaluation, and per atom its atom nodes.
struct Model {
    theory: Theory,
    producers: Vec<(usize, Option<usize>)>,
    readers: Vec<u64>,
    /// Nodes reading each node as a connective operand or a row owner.
    queues: Vec<Vec<usize>>,
    reads: Vec<u64>,
    atom_nodes: Vec<Vec<usize>>,
}

impl Model {
    fn new(certificate: &TightPlan) -> Self {
        let theory = certificate.theory().clone();
        let view = theory.view();
        let nodes = view.len();
        let mut readers = vec![0_u64; nodes];
        let mut queues = vec![Vec::new(); nodes];
        let mut reads = vec![0_u64; nodes];
        let mut atom_nodes = vec![Vec::new(); theory.atom_count()];
        // Live threshold guards by row: the first is the row's owner.
        let mut rows = std::collections::BTreeMap::<usize, Vec<usize>>::new();
        for (node, read) in reads.iter_mut().enumerate() {
            match view.node(node).unwrap() {
                NodeView::And(children) | NodeView::Or(children) => {
                    for &child in children {
                        readers[child] += 1;
                        queues[child].push(node);
                    }
                    *read = children.len() as u64;
                }
                NodeView::Implies(left, right) => {
                    for child in [left, right] {
                        readers[child] += 1;
                        queues[child].push(node);
                    }
                    *read = 2;
                }
                NodeView::Atom(atom) => atom_nodes[atom].push(node),
                NodeView::Threshold(threshold) => {
                    let total: i128 = threshold
                        .weights
                        .iter()
                        .map(|&weight| i128::from(weight))
                        .sum();
                    if threshold.bound > 0 && threshold.bound <= total {
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
        Self {
            theory,
            producers,
            readers,
            queues,
            reads,
            atom_nodes,
        }
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

    /// The receipt of an update from `old` to `new` in a schedule of `levels`
    /// levels, summarizing first unless the slot's summary is current.
    fn receipt(
        &self,
        old: &Interpretation,
        new: &Interpretation,
        levels: u32,
        count_bits: u32,
        summarized: bool,
    ) -> Receipt {
        let (before, after) = (Self::truth(old), Self::truth(new));
        let atoms = self.theory.atom_count();
        let words = atoms.div_ceil(32) as u64;
        let nodes = before.len();
        let roots = self.theory.roots().len();
        let root_words = roots.div_ceil(32) as u64;
        let mut work = 5 * words + nodes.div_ceil(32) as u64 + u64::from(levels) + RECORD_WORDS;
        if !summarized {
            let per_word = 32 / count_bits as usize;
            work += atoms.div_ceil(per_word) as u64
                + 2 * words
                + root_words
                + self.producers.len() as u64
                + roots as u64
                + old.atoms().count() as u64;
        }
        let present =
            |interpretation: &Interpretation, atom| interpretation.atoms().any(|a| a == atom);
        let changed_atoms: Vec<usize> = (0..atoms)
            .filter(|&atom| present(old, atom) != present(new, atom))
            .collect();
        let mut taken = std::collections::BTreeSet::new();
        for &atom in &changed_atoms {
            work += 1 + self.atom_nodes[atom].len() as u64;
            taken.extend(self.atom_nodes[atom].iter().copied());
        }
        let changed: Vec<usize> = (0..nodes)
            .filter(|&node| before[node] != after[node])
            .collect();
        for &node in &changed {
            work += self.readers[node];
            taken.extend(self.queues[node].iter().copied());
        }
        for &node in &taken {
            work += 1 + self.reads[node];
        }
        let mut touched: std::collections::BTreeSet<usize> =
            changed_atoms.iter().copied().collect();
        for &(head, body) in &self.producers {
            if body.is_some_and(|body| before[body] != after[body]) {
                touched.insert(head);
            }
        }
        work += touched.len() as u64;
        let false_root = self.theory.roots().iter().any(|&root| !after[root]);
        let unsupported = new.atoms().any(|atom| {
            !self
                .producers
                .iter()
                .any(|&(head, body)| head == atom && body.is_none_or(|body| after[body]))
        });
        if false_root {
            work += root_words;
        } else if unsupported {
            work += words;
        }
        Receipt {
            nodes: taken.len() as u64,
            work,
        }
    }
}

/// An update's receipts equal the host model's: the nodes taken from the
/// queued set, and the work of its floor, its summary when one runs, its
/// presence pass, every evaluation and readers visit, settling and the verdict
/// scan. Two updates in turn: the first summarizes, the second does not.
fn qualify_receipts(oracle: &mut GpuTightOracle) {
    let ladder = Ladder::wide();
    let certificate = &ladder.certificate;
    let theory = certificate.theory();
    let model = Model::new(certificate);
    let limits = TightGpuLimits::default();
    let mut chosen = ladder.patterns(4, 1);
    let mut candidates: Vec<Vec<bool>> = chosen
        .iter()
        .enumerate()
        .map(|(slot, chosen)| ladder.candidate(slot, chosen))
        .collect();
    checked(oracle, certificate, &candidates, limits);
    let (seeding, _) = checked(oracle, certificate, &candidates, limits);
    let TightGpuUpdates::Admitted { count_bits, .. } = seeding.updates else {
        panic!("the update route is admitted: {:?}", seeding.updates);
    };
    for (round, summarized) in [(0, false), (1, true)] {
        let previous = candidates.clone();
        for (slot, pattern) in chosen.iter_mut().enumerate() {
            let input = (5 * slot + 3 * round + 1) % ladder.inputs;
            pattern[input] = !pattern[input];
            candidates[slot] = ladder.candidate(slot, pattern);
        }
        let (stats, results) = checked(oracle, certificate, &candidates, limits);
        assert_eq!(stats.routes.update, 4);
        let mut nodes = 0;
        for slot in 0..4 {
            let receipt = model.receipt(
                &interpretation(theory, &previous[slot]),
                &interpretation(theory, &candidates[slot]),
                stats.truth_levels,
                count_bits,
                summarized,
            );
            assert_eq!(
                results[slot].work(),
                receipt.work,
                "slot {slot} round {round}"
            );
            nodes += receipt.nodes;
        }
        assert_eq!(stats.nodes_evaluated, nodes, "round {round}");
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
        let sequence = [vec![0, 1], vec![0, 1], vec![1], vec![], vec![5, 299]];
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
