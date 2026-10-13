use std::cmp::Reverse;
use std::collections::BinaryHeap;

use super::readers::{ReaderKind, Readers};
use super::{
    Tick, TightAttempt, TightCheck, TightCheckLimits, TightError, TightPlan, TightProducer,
    TightVerdict, bytes,
};
use crate::numeric_evaluation::NumericRowScratch;
use crate::{Interpretation, NodeView, Theory};
use zetesis_cpu::{Cancellation, Meter, Stop};

#[cfg(test)]
mod incremental_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod update_tests;

impl TightPlan {
    /// Check original satisfaction and every present atom's producer support.
    /// Ranked support discharges all proper-subset reduct queries at once.
    /// Failed support is returned as a residual, without claiming an original
    /// formula failure or publishing an unverified countermodel witness.
    ///
    /// # Errors
    /// Refuses foreign theory identities, cancellation, deadlines, allocation,
    /// and byte/work limits. An interrupted scan never returns stable.
    pub fn check(
        &self,
        candidate: &Interpretation,
        limits: TightCheckLimits,
        cancellation: &Cancellation,
    ) -> Result<TightCheck, TightError> {
        self.check_accounted(candidate, limits, cancellation).result
    }

    /// Check the same membership conditions as [`Self::check`], retaining all
    /// charged work when evaluation stops before producing a verdict.
    #[must_use]
    pub fn check_accounted(
        &self,
        candidate: &Interpretation,
        limits: TightCheckLimits,
        cancellation: &Cancellation,
    ) -> TightAttempt<TightCheck> {
        TightWorkspace::default().check_with(self, candidate, limits, cancellation, false)
    }
}

/// Reusable truth and support storage for tight-certificate checks.
///
/// A workspace keeps, after a check that completed, the complete evaluation of
/// the candidate it checked, together with a handle to that candidate's theory.
/// When the plan has its readers index ([`TightPlan::prepare_updates`]), the next
/// check of a candidate of the same theory updates that evaluation: it
/// re-evaluates, in ascending node order, exactly the nodes with an operand
/// whose truth changed, and maintains support counts and the false-root and
/// unsupported-atom sets, so its verdict and witness are those of a complete
/// evaluation of the new candidate. A check whose candidate differs in more
/// atoms than an eighth of the theory's atoms (and at least 64), a different
/// theory, a plan without the
/// index, or a check after an interrupted one evaluates completely, recomputing
/// every formula truth. An interrupted check leaves nothing a later check can
/// read. Each conjunction or disjunction reads operands only
/// through its first decisive value. Checks are bounded by nodes, operand
/// occurrences, tested roots, producers and packed candidate words. Numeric nodes
/// read and charge every physical occurrence once per authoritative row and pass,
/// including repeats and zero weights, even
/// for endpoint constants. Dense measure growth and packed-validity resets are
/// charged; old measures are inaccessible until completed in the current pass. Truth uses
/// one byte per node; support uses one bit per atom, rounded to 32-bit words.
/// Each concurrent checker owns an exclusive workspace; immutable plans may be
/// shared.
#[derive(Debug, Default)]
pub struct TightWorkspace {
    values: Vec<u8>,
    supported: Vec<u32>,
    numeric: NumericRowScratch,
    retained: Retained,
}

/// The complete evaluation a workspace keeps between checks of one theory, and
/// the summaries an update maintains from it.
#[derive(Debug, Default)]
struct Retained {
    /// Present exactly when node truth and the row measures are the complete
    /// evaluation of `candidate` under this theory; holding the handle keeps the
    /// theory's identity exact for as long as the evaluation is kept.
    theory: Option<Theory>,
    /// Packed words of the evaluated candidate.
    candidate: Vec<u64>,
    /// Whether the support counts and the two sets summarize node truth.
    summarized: bool,
    /// For each atom, its producers that are bodyless or have a true body.
    support: Vec<u32>,
    /// Present atoms without support, and how many.
    missing: Vec<u64>,
    missing_count: usize,
    /// Original root positions whose node is false, and how many.
    false_roots: Vec<u64>,
    false_root_count: usize,
    /// Update scratch: candidate words that changed, nodes awaiting
    /// re-evaluation in ascending order, and their queued marks. The heap and the
    /// marks are empty between completed updates.
    changed: Vec<usize>,
    pending: BinaryHeap<Reverse<usize>>,
    queued: Vec<u64>,
}

impl Retained {
    fn bytes(&self) -> u128 {
        (self.candidate.capacity()
            + self.missing.capacity()
            + self.false_roots.capacity()
            + self.queued.capacity()
            + self.changed.capacity()) as u128
            * size_of::<u64>() as u128
            + self.support.capacity() as u128 * size_of::<u32>() as u128
            + self.pending.capacity() as u128 * size_of::<Reverse<usize>>() as u128
    }
}

impl TightWorkspace {
    /// Actual retained vector payload, excluding headers, the plan, its shared
    /// theory and allocator bookkeeping. A failed check can retain new capacity.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        self.values.capacity() as u128
            + self.supported.capacity() as u128 * size_of::<u32>() as u128
            + self.numeric.retained_bytes()
            + self.retained.bytes()
    }

    /// Check original satisfaction and ranked support using retained storage.
    ///
    /// Identity failure precedes cancellation, which precedes storage admission.
    /// Work and error attribution match [`TightPlan::check_accounted`]. Byte
    /// ceilings include existing capacity, even for a smaller subsequent theory;
    /// checks do not shrink storage. Every outcome retains its charged work and
    /// leaves actual capacity observable through [`Self::retained_bytes`].
    #[must_use]
    pub fn check(
        &mut self,
        plan: &TightPlan,
        candidate: &Interpretation,
        limits: TightCheckLimits,
        cancellation: &Cancellation,
    ) -> TightAttempt<TightCheck> {
        self.check_with(plan, candidate, limits, cancellation, true)
    }

    fn check_with(
        &mut self,
        plan: &TightPlan,
        candidate: &Interpretation,
        limits: TightCheckLimits,
        cancellation: &Cancellation,
        retain: bool,
    ) -> TightAttempt<TightCheck> {
        let metered = cancellation.metered(limits.max_work, |meter| {
            self.evaluate(plan, candidate, limits, meter, retain)
        });
        TightAttempt {
            result: metered.result,
            work: metered.charged,
        }
    }

    fn evaluate(
        &mut self,
        plan: &TightPlan,
        candidate: &Interpretation,
        limits: TightCheckLimits,
        work: &mut Meter<'_>,
        retain: bool,
    ) -> Result<TightCheck, TightError> {
        if !plan.theory.same_instance(candidate.theory()) {
            return Err(Stop::WrongProgram.into());
        }
        work.poll()?;
        // Taken for the whole check: an interrupted update or evaluation leaves
        // nothing a later check could read as a complete evaluation.
        let previous = self.retained.theory.take();
        if retain
            && let Some(readers) = plan.readers.as_ref()
            && previous
                .as_ref()
                .is_some_and(|theory| theory.same_instance(&plan.theory))
            && let Some(check) = self.update(plan, readers, candidate, limits, work)?
        {
            self.retained.theory = previous;
            return Ok(check);
        }
        self.complete(
            plan,
            candidate,
            limits,
            work,
            retain && plan.readers.is_some(),
        )
    }

    /// Evaluate `candidate` completely; with `keep`, retain the evaluation for
    /// the next check's update.
    fn complete(
        &mut self,
        plan: &TightPlan,
        candidate: &Interpretation,
        limits: TightCheckLimits,
        work: &mut Meter<'_>,
        keep: bool,
    ) -> Result<TightCheck, TightError> {
        self.admit(plan, limits.max_bytes)?;
        // Explicit byte cells give candidate tiles a fixed payload bound.
        // Rust's Vec<bool> also stores byte-sized elements; it is not bit-packed.
        self.values.clear();
        self.values
            .try_reserve_exact(plan.theory.nodes().len())
            .map_err(|_| Stop::Allocation)?;
        self.admit(plan, limits.max_bytes)?;
        let base = self.truth_support_bytes(plan);
        self.numeric.reserve_observed(&plan.theory, |actual| {
            bytes(base + actual, limits.max_bytes).map(|_| ())
        })?;
        let logical_bytes = self.admit(plan, limits.max_bytes)?;
        evaluate_truth(
            &plan.theory,
            candidate,
            &mut self.values,
            &mut self.numeric,
            work,
        )?;
        let logical_bytes = if keep {
            self.keep(plan, candidate, limits, work)?
        } else {
            logical_bytes
        };
        let (verdict, logical_bytes) =
            if let Some(root) = false_root(&plan.theory, &self.values, work)? {
                (TightVerdict::NotModel { root }, logical_bytes)
            } else {
                self.supported.clear();
                let words = plan.theory.atom_count().div_ceil(32);
                self.supported
                    .try_reserve_exact(words)
                    .map_err(|_| Stop::Allocation)?;
                let logical_bytes = self.admit(plan, limits.max_bytes)?;
                self.supported.resize(words, 0);
                mark_support(&plan.producers, &self.values, &mut self.supported, work)?;
                let verdict = match unsupported_atom(candidate, &self.supported, work)? {
                    Some(unsupported_atom) => TightVerdict::Residual { unsupported_atom },
                    None => TightVerdict::Stable,
                };
                (verdict, logical_bytes)
            };
        if keep {
            self.retained.theory = Some(plan.theory.clone());
        }
        Ok(TightCheck {
            verdict,
            work: work.charged(),
            logical_bytes,
        })
    }

    /// The bytes every vector of this workspace would hold, with `more` words
    /// requested beyond their present capacities.
    fn requested(&self, plan: &TightPlan, more: u128) -> u128 {
        u128::from(plan.statistics.resident_bytes)
            + self.values.capacity() as u128
            + self.supported.capacity() as u128 * size_of::<u32>() as u128
            + self.numeric.retained_bytes()
            + self.retained.bytes()
            + more
    }

    /// Keep the evaluated candidate's words and an empty update scratch, one
    /// charged operation per word; the summaries are derived when an update
    /// first needs them.
    fn keep(
        &mut self,
        plan: &TightPlan,
        candidate: &Interpretation,
        limits: TightCheckLimits,
        work: &mut Meter<'_>,
    ) -> Result<u64, TightError> {
        let words = candidate.words.len();
        let marks = plan.theory.nodes().len().div_ceil(64);
        self.retained.summarized = false;
        self.retained.pending.clear();
        self.retained.changed.clear();
        self.retained.candidate.clear();
        self.retained.queued.clear();
        let more = (words.saturating_sub(self.retained.candidate.capacity())
            + marks.saturating_sub(self.retained.queued.capacity())) as u128
            * size_of::<u64>() as u128;
        bytes(self.requested(plan, more), limits.max_bytes)?;
        let retained = &mut self.retained;
        retained
            .candidate
            .try_reserve_exact(words)
            .map_err(|_| Stop::Allocation)?;
        retained
            .queued
            .try_reserve_exact(marks)
            .map_err(|_| Stop::Allocation)?;
        for &word in &candidate.words {
            work.tick()?;
            retained.candidate.push(word);
        }
        for _ in 0..marks {
            work.tick()?;
            retained.queued.push(0);
        }
        self.admit(plan, limits.max_bytes)
    }

    /// Derive support counts, unsupported present atoms and false root positions
    /// from the retained complete evaluation, once per complete evaluation.
    fn summarize(
        &mut self,
        plan: &TightPlan,
        limits: TightCheckLimits,
        work: &mut Meter<'_>,
    ) -> Result<(), TightError> {
        let atoms = plan.theory.atom_count();
        let roots = plan.theory.roots();
        self.retained.support.clear();
        self.retained.missing.clear();
        self.retained.false_roots.clear();
        let more = atoms.saturating_sub(self.retained.support.capacity()) as u128
            * size_of::<u32>() as u128
            + (atoms
                .div_ceil(64)
                .saturating_sub(self.retained.missing.capacity())
                + roots
                    .len()
                    .div_ceil(64)
                    .saturating_sub(self.retained.false_roots.capacity())) as u128
                * size_of::<u64>() as u128;
        bytes(self.requested(plan, more), limits.max_bytes)?;
        let retained = &mut self.retained;
        retained
            .support
            .try_reserve_exact(atoms)
            .map_err(|_| Stop::Allocation)?;
        retained
            .missing
            .try_reserve_exact(atoms.div_ceil(64))
            .map_err(|_| Stop::Allocation)?;
        retained
            .false_roots
            .try_reserve_exact(roots.len().div_ceil(64))
            .map_err(|_| Stop::Allocation)?;
        retained.support.resize(atoms, 0);
        retained.missing.resize(atoms.div_ceil(64), 0);
        retained.false_roots.resize(roots.len().div_ceil(64), 0);
        for producer in &plan.producers {
            work.tick()?;
            if producer.body.is_none_or(|body| self.values[body] != 0) {
                let count = &mut retained.support[producer.head];
                *count = count.checked_add(1).ok_or(Stop::InvalidProgram)?;
            }
        }
        retained.false_root_count = 0;
        for (position, &root) in roots.iter().enumerate() {
            work.tick()?;
            if self.values[root] == 0 {
                retained.false_roots[position / 64] |= 1 << (position % 64);
                retained.false_root_count += 1;
            }
        }
        retained.missing_count = 0;
        for (word, &present) in retained.candidate.iter().enumerate() {
            work.tick()?;
            let mut remaining = present;
            while remaining != 0 {
                let atom = word * 64 + remaining.trailing_zeros() as usize;
                remaining &= remaining - 1;
                if retained.support[atom] == 0 {
                    retained.missing[word] |= 1 << (atom % 64);
                    retained.missing_count += 1;
                }
            }
        }
        retained.summarized = true;
        Ok(())
    }

    /// Bring the retained complete evaluation to `candidate`: re-evaluate, in
    /// ascending node order, exactly the atom nodes of changed atoms and the
    /// readers of changed values. Returns `None`, having changed nothing, when
    /// more atoms changed than [`large_change`] admits.
    fn update(
        &mut self,
        plan: &TightPlan,
        readers: &Readers,
        candidate: &Interpretation,
        limits: TightCheckLimits,
        work: &mut Meter<'_>,
    ) -> Result<Option<TightCheck>, TightError> {
        // First pass: find the changed words and count changed atoms, with no
        // change to the retained state.
        let threshold = large_change(plan.theory.atom_count());
        let mut changed_atoms = 0_usize;
        self.retained.changed.clear();
        for (word, &present) in candidate.words.iter().enumerate() {
            work.tick()?;
            let changed = self.retained.candidate[word] ^ present;
            if changed != 0 {
                changed_atoms += changed.count_ones() as usize;
                if changed_atoms > threshold {
                    return Ok(None);
                }
                if self.retained.changed.len() == self.retained.changed.capacity() {
                    let additional = self.retained.changed.capacity().max(8);
                    bytes(
                        self.requested(plan, additional as u128 * size_of::<usize>() as u128),
                        limits.max_bytes,
                    )?;
                    self.retained
                        .changed
                        .try_reserve_exact(additional)
                        .map_err(|_| Stop::Allocation)?;
                }
                self.retained.changed.push(word);
            }
        }
        if !self.retained.summarized {
            self.summarize(plan, limits, work)?;
        }
        let logical_bytes = self.admit(plan, limits.max_bytes)?;
        // Second pass: apply the changed words.
        for position in 0..self.retained.changed.len() {
            work.tick()?;
            let index = self.retained.changed[position];
            let present = candidate.words[index];
            let mut changed = self.retained.candidate[index] ^ present;
            self.retained.candidate[index] = present;
            while changed != 0 {
                work.tick()?;
                let atom = index * 64 + changed.trailing_zeros() as usize;
                changed &= changed - 1;
                self.presence(atom, present & (1 << (atom % 64)) != 0);
                for &node in readers.atom_nodes(atom) {
                    work.tick()?;
                    self.queue(node, plan, limits)?;
                }
            }
        }
        while let Some(Reverse(node)) = self.retained.pending.pop() {
            work.tick()?;
            self.retained.queued[node / 64] &= !(1 << (node % 64));
            let truth = evaluate_node(
                &plan.theory,
                node,
                candidate,
                &self.values,
                &mut self.numeric,
                work,
            )?;
            if truth == (self.values[node] != 0) {
                continue;
            }
            self.values[node] = u8::from(truth);
            for &reader in readers.of(node) {
                work.tick()?;
                let index = reader.index();
                match reader.kind() {
                    ReaderKind::Node => self.queue(index, plan, limits)?,
                    ReaderKind::Row => {
                        self.numeric.invalidate(index)?;
                        for &guard in readers.guards(index) {
                            work.tick()?;
                            self.queue(guard, plan, limits)?;
                        }
                    }
                    ReaderKind::Producer => self.support(plan.producers[index].head, truth),
                    ReaderKind::Root => self.root(index, truth),
                }
            }
        }
        let retained = &self.retained;
        let verdict = if retained.false_root_count > 0 {
            TightVerdict::NotModel {
                root: plan.theory.roots()[first_set(&retained.false_roots, work)?],
            }
        } else if retained.missing_count > 0 {
            TightVerdict::Residual {
                unsupported_atom: first_set(&retained.missing, work)?,
            }
        } else {
            TightVerdict::Stable
        };
        Ok(Some(TightCheck {
            verdict,
            work: work.charged(),
            logical_bytes,
        }))
    }

    /// Mark a node for re-evaluation once.
    fn queue(
        &mut self,
        node: usize,
        plan: &TightPlan,
        limits: TightCheckLimits,
    ) -> Result<(), TightError> {
        let bit = 1_u64 << (node % 64);
        if self.retained.queued[node / 64] & bit != 0 {
            return Ok(());
        }
        if self.retained.pending.len() == self.retained.pending.capacity() {
            let additional = self.retained.pending.capacity().max(64);
            bytes(
                self.requested(
                    plan,
                    additional as u128 * size_of::<Reverse<usize>>() as u128,
                ),
                limits.max_bytes,
            )?;
            self.retained
                .pending
                .try_reserve_exact(additional)
                .map_err(|_| Stop::Allocation)?;
        }
        self.retained.queued[node / 64] |= bit;
        self.retained.pending.push(Reverse(node));
        Ok(())
    }

    /// An atom's membership changed; its support count did not.
    fn presence(&mut self, atom: usize, present: bool) {
        let retained = &mut self.retained;
        let bit = 1_u64 << (atom % 64);
        if present {
            if retained.support[atom] == 0 {
                retained.missing[atom / 64] |= bit;
                retained.missing_count += 1;
            }
        } else if retained.missing[atom / 64] & bit != 0 {
            retained.missing[atom / 64] &= !bit;
            retained.missing_count -= 1;
        }
    }

    /// A producer body changed truth; membership is already the new candidate's.
    fn support(&mut self, head: usize, holds: bool) {
        let retained = &mut self.retained;
        let bit = 1_u64 << (head % 64);
        let present = retained.candidate[head / 64] & bit != 0;
        if holds {
            retained.support[head] += 1;
            if retained.support[head] == 1 && present {
                retained.missing[head / 64] &= !bit;
                retained.missing_count -= 1;
            }
        } else {
            retained.support[head] -= 1;
            if retained.support[head] == 0 && present {
                retained.missing[head / 64] |= bit;
                retained.missing_count += 1;
            }
        }
    }

    /// An original root position changed truth.
    fn root(&mut self, position: usize, holds: bool) {
        let retained = &mut self.retained;
        let bit = 1_u64 << (position % 64);
        if holds {
            retained.false_roots[position / 64] &= !bit;
            retained.false_root_count -= 1;
        } else {
            retained.false_roots[position / 64] |= bit;
            retained.false_root_count += 1;
        }
    }

    fn admit(&self, plan: &TightPlan, limit: u64) -> Result<u64, TightError> {
        bytes(
            self.truth_support_bytes(plan) + self.numeric.required_bytes(&plan.theory),
            limit,
        )
    }

    fn truth_support_bytes(&self, plan: &TightPlan) -> u128 {
        u128::from(plan.statistics.resident_bytes)
            + self.values.capacity().max(plan.theory.nodes().len()) as u128
            + self
                .supported
                .capacity()
                .max(plan.theory.atom_count().div_ceil(32)) as u128
                * size_of::<u32>() as u128
            + self.retained.bytes()
    }
}

/// The truth of node `$index` from the truth of its operands, which precede it:
/// the one rule a complete pass applies to every node and an update to each
/// node it re-evaluates. It is a macro rather than a function so that both
/// per-node loops receive the rule inline. Compiled as a function with two
/// callers, it stayed out of line, and a call on every node made a complete
/// check of the wide Mastermind control take 3.5 ms instead of 2.15 ms.
macro_rules! node_truth {
    ($theory:expr, $view:expr, $index:expr, $candidate:expr, $values:expr, $numeric:expr, $work:expr) => {
        match $view.node($index).map_err(|_| Stop::InvalidProgram)? {
            NodeView::Atom(atom) => $candidate.contains(atom),
            NodeView::False => false,
            NodeView::And(operands) => evaluate_operands(operands, $values, true, $work)?,
            NodeView::Or(operands) => evaluate_operands(operands, $values, false, $work)?,
            NodeView::Threshold(threshold) => {
                $numeric.threshold::<TightError>($theory, $index, threshold, |child| {
                    $work.tick()?;
                    Ok($values[child] != 0)
                })?
            }
            NodeView::ExtremumCut(cut) => {
                cut.rule.holds(
                    $numeric.extremum::<TightError>($theory, $index, cut, |child| {
                        $work.tick()?;
                        Ok($values[child] != 0)
                    })?,
                )
            }
            NodeView::Implies(a, b) => {
                $work.tick()?;
                let left = $values[a];
                $work.tick()?;
                let right = $values[b];
                left == 0 || right != 0
            }
        }
    };
}

/// Write each formula truth in admitted topological order. The caller clears
/// and reserves the target first; a stopped prefix is never membership evidence.
/// The pass stays out of line so that its per-node loop, not its caller,
/// receives the inlined operand reads.
#[inline(never)]
fn evaluate_truth(
    theory: &Theory,
    candidate: &Interpretation,
    values: &mut Vec<u8>,
    numeric: &mut NumericRowScratch,
    work: &mut Meter<'_>,
) -> Result<(), TightError> {
    numeric.reset(theory, || work.tick())?;
    let view = theory.view();
    for index in 0..view.len() {
        work.tick()?;
        let truth = node_truth!(theory, view, index, candidate, values, numeric, work);
        values.push(u8::from(truth));
    }
    Ok(())
}

/// One node's truth by [`node_truth`], for an update's re-evaluation.
fn evaluate_node(
    theory: &Theory,
    index: usize,
    candidate: &Interpretation,
    values: &[u8],
    numeric: &mut NumericRowScratch,
    work: &mut Meter<'_>,
) -> Result<bool, TightError> {
    let view = theory.view();
    Ok(node_truth!(
        theory, view, index, candidate, values, numeric, work
    ))
}

/// The first original root whose node is false, one charged test per root.
/// Like the passes below, it stays out of line so that its loop receives the
/// inlined charge.
#[inline(never)]
fn false_root(
    theory: &Theory,
    values: &[u8],
    work: &mut Meter<'_>,
) -> Result<Option<usize>, TightError> {
    for &root in theory.roots() {
        work.tick()?;
        if values[root] == 0 {
            return Ok(Some(root));
        }
    }
    Ok(None)
}

/// Mark the head of every bodyless producer and of every producer whose body
/// is true, one charged visit per producer.
#[inline(never)]
fn mark_support(
    producers: &[TightProducer],
    values: &[u8],
    supported: &mut [u32],
    work: &mut Meter<'_>,
) -> Result<(), TightError> {
    for producer in producers {
        work.tick()?;
        if producer.body.is_none_or(|body| values[body] != 0) {
            supported[producer.head / 32] |= 1 << (producer.head % 32);
        }
    }
    Ok(())
}

/// The least present atom without support, one charged operation per 32-bit
/// support word.
#[inline(never)]
fn unsupported_atom(
    candidate: &Interpretation,
    supported: &[u32],
    work: &mut Meter<'_>,
) -> Result<Option<usize>, TightError> {
    let mut present = candidate.words32();
    for (word, &support) in supported.iter().enumerate() {
        work.tick()?;
        let missing = present
            .next()
            .expect("support and candidate cover the same admitted atom universe")
            & !support;
        if missing != 0 {
            return Ok(Some(word * 32 + missing.trailing_zeros() as usize));
        }
    }
    Ok(None)
}

/// The most changed atoms an update takes on; beyond it a complete evaluation
/// is expected to cost less. An execution choice: both routes return the same
/// verdict.
pub(super) fn large_change(atoms: usize) -> usize {
    (atoms / 8).max(64)
}

/// The least set position of a packed set, one charged operation per word read.
fn first_set(words: &[u64], work: &mut Meter<'_>) -> Result<usize, TightError> {
    for (index, &word) in words.iter().enumerate() {
        work.tick()?;
        if word != 0 {
            return Ok(index * 64 + word.trailing_zeros() as usize);
        }
    }
    Err(Stop::InvalidProgram.into())
}

#[inline]
fn evaluate_operands(
    operands: &[usize],
    values: &[u8],
    conjunction: bool,
    work: &mut Meter<'_>,
) -> Result<bool, TightError> {
    for &child in operands {
        work.tick()?;
        let truth = values[child] != 0;
        // A false conjunct or true disjunct determines this node, irrespective
        // of the remaining operands. Other DAG nodes still get their own truth.
        if truth != conjunction {
            return Ok(truth);
        }
    }
    Ok(conjunction)
}
