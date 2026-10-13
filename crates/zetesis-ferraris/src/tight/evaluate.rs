use std::cmp::Reverse;
use std::collections::{BinaryHeap, TryReserveError};

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
/// A complete check gives every formula node its truth in admitted order. Each
/// conjunction or disjunction reads operands only through its first decisive
/// value. Numeric nodes read and charge every physical occurrence once per
/// authoritative row and pass, including repeats and zero weights, even for
/// endpoint constants; dense measure growth and packed-validity resets are
/// charged. Truth uses one byte per node, and the complete route's support one
/// bit per atom, rounded to 32-bit words.
///
/// When the plan has its readers index ([`TightPlan::prepare_updates`]), a
/// workspace keeps, after a check that completed, the complete evaluation of the
/// candidate it checked together with a handle to that candidate's theory, and
/// the next check of a candidate of the same theory updates it. A node's inputs
/// are its atom's membership for an atom node, the truth of its row's
/// conditions for a numeric guard, and its operands' truth otherwise. The update
/// re-evaluates, in ascending node order, exactly the nodes with an input whose
/// value changed, keeps the measures of rows none of whose conditions changed,
/// and maintains a four-byte support count per atom and the unsupported-atom and
/// false-root sets, so its verdict and witness are those of a complete
/// evaluation of the new candidate. A candidate that differs in more atoms than
/// an eighth of the theory's atoms (and at least 64), a different theory, or a
/// plan without the index evaluates completely. A check that ends after its
/// first observation of control (refused for its storage or its work, failing an
/// allocation, or stopped at a later observation) leaves nothing the next check
/// reads, so the next check evaluates completely; a check refused for a foreign
/// theory, or stopped at that first observation, changes nothing. Each
/// concurrent checker owns an exclusive workspace; immutable plans may be
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
///
/// Each summary is a count over its inputs: a support count over its atom's
/// producers, a missing flag over its atom's membership and support count, a
/// false-root flag over its node's truth, and the two set counts over those
/// flags. An update visits each changed input once, with its old and new value,
/// and moves the count it feeds by one in the matching direction. Membership is
/// applied before propagation, so a missing flag sees each of its two inputs
/// change exactly when that input changes.
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
            + self.queued.capacity()) as u128
            * size_of::<u64>() as u128
            + self.changed.capacity() as u128 * size_of::<usize>() as u128
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
    /// A completed check's verdict and witness are always those of
    /// [`TightPlan::check_accounted`]. Under a plan without the readers index,
    /// work and error attribution match it as well. Under a plan with the index,
    /// work and logical bytes are this workspace's own: an update charges its
    /// update units, a complete evaluation that keeps its result also charges
    /// the kept candidate and queued-mark words, retained storage counts against
    /// the byte ceiling, and every check stays within
    /// [`TightPlan::check_work_bound`]. Byte ceilings include existing capacity,
    /// even for a smaller subsequent theory; checks do not shrink storage. Every
    /// outcome retains its charged work and leaves actual capacity observable
    /// through [`Self::retained_bytes`].
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
        meter: &mut Meter<'_>,
        retain: bool,
    ) -> Result<TightCheck, TightError> {
        if !plan.theory.same_instance(candidate.theory()) {
            return Err(Stop::WrongProgram.into());
        }
        meter.poll()?;
        // Taken for the whole check: an interrupted update or evaluation leaves
        // nothing a later check could read as a complete evaluation. The same
        // theory instance also authenticates the retained summaries, which the
        // last plan's producers computed: a plan's producers and readers are
        // functions of its theory alone.
        let previous = self.retained.theory.take();
        // Storage admission precedes either route's work. It counts what the
        // workspace holds, each vector at no less than a complete check
        // requires; each later growth passes through `grow`, which admits it
        // before reserving it.
        self.admit(plan, limits.max_bytes)?;
        if retain
            && let Some(readers) = plan.readers.as_ref()
            && previous
                .as_ref()
                .is_some_and(|theory| theory.same_instance(&plan.theory))
            && let Some(check) = self.update(plan, readers, candidate, limits, meter)?
        {
            self.retained.theory = previous;
            return Ok(check);
        }
        self.complete(
            plan,
            candidate,
            limits,
            meter,
            retain && plan.readers.is_some(),
        )
    }

    /// Evaluate `candidate` completely; with `keep`, retain the evaluation for
    /// the next check's update. Precondition: the caller has admitted the
    /// workspace's holdings ([`Self::admit`]), which counts the truth and support
    /// vectors and the numeric scratch at a complete check's sizes, so the
    /// reservations below grow nothing unadmitted.
    fn complete(
        &mut self,
        plan: &TightPlan,
        candidate: &Interpretation,
        limits: TightCheckLimits,
        meter: &mut Meter<'_>,
        keep: bool,
    ) -> Result<TightCheck, TightError> {
        // Explicit byte cells give candidate tiles a fixed payload bound.
        // Rust's Vec<bool> also stores byte-sized elements; it is not bit-packed.
        self.values.clear();
        self.values
            .try_reserve_exact(plan.theory.nodes().len())
            .map_err(|_| Stop::Allocation)?;
        self.admit(plan, limits.max_bytes)?;
        let base = self.bytes_beside_numeric(plan);
        self.numeric.reserve_observed(&plan.theory, |actual| {
            bytes(base + actual, limits.max_bytes).map(|_| ())
        })?;
        let logical_bytes = self.admit(plan, limits.max_bytes)?;
        evaluate_truth(
            &plan.theory,
            candidate,
            &mut self.values,
            &mut self.numeric,
            meter,
        )?;
        let logical_bytes = if keep {
            self.keep(plan, candidate, limits, meter)?
        } else {
            logical_bytes
        };
        let (verdict, logical_bytes) =
            if let Some(root) = false_root(&plan.theory, &self.values, meter)? {
                (TightVerdict::NotModel { root }, logical_bytes)
            } else {
                self.supported.clear();
                let words = plan.theory.atom_count().div_ceil(32);
                self.supported
                    .try_reserve_exact(words)
                    .map_err(|_| Stop::Allocation)?;
                let logical_bytes = self.admit(plan, limits.max_bytes)?;
                self.supported.resize(words, 0);
                mark_support(&plan.producers, &self.values, &mut self.supported, meter)?;
                let verdict = match unsupported_atom(candidate, &self.supported, meter)? {
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
            work: meter.charged(),
            logical_bytes,
        })
    }

    /// The bytes this workspace holds under `plan`, counting each vector at
    /// the larger of its capacity and what a complete check requires, with
    /// `more` bytes requested beyond them: the one measure every admission uses.
    fn requested(&self, plan: &TightPlan, more: u128) -> u128 {
        self.bytes_beside_numeric(plan) + self.numeric.required_bytes(&plan.theory) + more
    }

    /// Grow the retained evaluation past what the check's start admitted: admit
    /// the workspace's measure with `more` bytes beside it, the most `reserve`
    /// can add, against `limit`, and only then run `reserve`. Every such growth
    /// passes through here, so a growth the byte limit refuses reserves nothing;
    /// the work charged before it stays charged, as on every refusal. Storage
    /// that grows only to a complete check's size needs no step of its own: the
    /// measure already counts it at that size.
    fn grow(
        &mut self,
        plan: &TightPlan,
        limit: u64,
        more: u128,
        reserve: impl FnOnce(&mut Retained) -> Result<(), TryReserveError>,
    ) -> Result<(), TightError> {
        bytes(self.requested(plan, more), limit)?;
        reserve(&mut self.retained).map_err(|_| Stop::Allocation.into())
    }

    /// Keep the evaluated candidate's words and an empty update scratch, one
    /// charged operation per word; the summaries are derived when an update
    /// first needs them.
    fn keep(
        &mut self,
        plan: &TightPlan,
        candidate: &Interpretation,
        limits: TightCheckLimits,
        meter: &mut Meter<'_>,
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
        self.grow(plan, limits.max_bytes, more, |retained| {
            retained.candidate.try_reserve_exact(words)?;
            retained.queued.try_reserve_exact(marks)
        })?;
        let retained = &mut self.retained;
        for &word in &candidate.words {
            meter.tick()?;
            retained.candidate.push(word);
        }
        for _ in 0..marks {
            meter.tick()?;
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
        meter: &mut Meter<'_>,
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
        self.grow(plan, limits.max_bytes, more, |retained| {
            retained.support.try_reserve_exact(atoms)?;
            retained.missing.try_reserve_exact(atoms.div_ceil(64))?;
            retained
                .false_roots
                .try_reserve_exact(roots.len().div_ceil(64))
        })?;
        let retained = &mut self.retained;
        retained.support.resize(atoms, 0);
        retained.missing.resize(atoms.div_ceil(64), 0);
        retained.false_roots.resize(roots.len().div_ceil(64), 0);
        for producer in &plan.producers {
            meter.tick()?;
            if producer.body.is_none_or(|body| self.values[body] != 0) {
                let count = &mut retained.support[producer.head];
                *count = count.checked_add(1).ok_or(Stop::InvalidProgram)?;
            }
        }
        retained.false_root_count = 0;
        for (position, &root) in roots.iter().enumerate() {
            meter.tick()?;
            if self.values[root] == 0 {
                retained.false_roots[position / 64] |= 1 << (position % 64);
                retained.false_root_count += 1;
            }
        }
        retained.missing_count = 0;
        for (word, &present) in retained.candidate.iter().enumerate() {
            meter.tick()?;
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
    /// more atoms changed than [`large_change`] admits. Precondition: the caller
    /// has admitted the workspace's holdings ([`Self::admit`]); each growth here
    /// passes through [`Self::grow`].
    fn update(
        &mut self,
        plan: &TightPlan,
        readers: &Readers,
        candidate: &Interpretation,
        limits: TightCheckLimits,
        meter: &mut Meter<'_>,
    ) -> Result<Option<TightCheck>, TightError> {
        // First pass: find the changed words and count changed atoms, with no
        // change to the retained state.
        let threshold = large_change(plan.theory.atom_count());
        let mut changed_atoms = 0_usize;
        self.retained.changed.clear();
        for (word, &present) in candidate.words.iter().enumerate() {
            meter.tick()?;
            let changed = self.retained.candidate[word] ^ present;
            if changed != 0 {
                changed_atoms += changed.count_ones() as usize;
                if changed_atoms > threshold {
                    return Ok(None);
                }
                if self.retained.changed.len() == self.retained.changed.capacity() {
                    let additional = self.retained.changed.capacity().max(8);
                    self.grow(
                        plan,
                        limits.max_bytes,
                        additional as u128 * size_of::<usize>() as u128,
                        |retained| retained.changed.try_reserve_exact(additional),
                    )?;
                }
                self.retained.changed.push(word);
            }
        }
        if !self.retained.summarized {
            self.summarize(plan, limits, meter)?;
        }
        // Second pass: apply the changed words.
        for position in 0..self.retained.changed.len() {
            meter.tick()?;
            let index = self.retained.changed[position];
            let present = candidate.words[index];
            let mut changed = self.retained.candidate[index] ^ present;
            self.retained.candidate[index] = present;
            while changed != 0 {
                meter.tick()?;
                let atom = index * 64 + changed.trailing_zeros() as usize;
                changed &= changed - 1;
                self.presence(atom, present & (1 << (atom % 64)) != 0);
                for &node in readers.atom_nodes(atom) {
                    meter.tick()?;
                    self.queue(node, plan, limits)?;
                }
            }
        }
        // Invariant: every node below the least queued node holds its truth under
        // the new candidate, and a node is queued exactly when one of its inputs
        // changed: an atom node's membership, a condition of a guard's row (the
        // row is withdrawn then, before any of its guards is re-evaluated), or an
        // operand's truth. A reader has a larger index than everything it reads,
        // so ascending pops re-evaluate each node from final inputs, and a popped
        // node is never queued again: each is re-evaluated at most once.
        while let Some(Reverse(node)) = self.retained.pending.pop() {
            meter.tick()?;
            self.retained.queued[node / 64] &= !(1 << (node % 64));
            let truth = evaluate_node(
                &plan.theory,
                node,
                candidate,
                &self.values,
                &mut self.numeric,
                meter,
            )?;
            if truth == (self.values[node] != 0) {
                continue;
            }
            self.values[node] = u8::from(truth);
            for &reader in readers.of(node) {
                meter.tick()?;
                let index = reader.index();
                match reader.kind() {
                    ReaderKind::Node => self.queue(index, plan, limits)?,
                    ReaderKind::Row => {
                        self.numeric.invalidate(index)?;
                        for &guard in readers.guards(index) {
                            meter.tick()?;
                            self.queue(guard, plan, limits)?;
                        }
                    }
                    ReaderKind::Producer => self.support(plan.producers[index].head, truth),
                    ReaderKind::Root => self.root(index, truth),
                }
            }
        }
        // Everything held was admitted, before the check's work or before its
        // reservation. While each reservation's capacity is what was admitted
        // for it, this only reports the bytes the check holds after its last
        // reservation; an allocator may return more capacity than requested,
        // and that excess is admitted here, against the same limit.
        let logical_bytes = self.admit(plan, limits.max_bytes)?;
        let retained = &self.retained;
        let verdict = if retained.false_root_count > 0 {
            TightVerdict::NotModel {
                root: plan.theory.roots()[first_set(&retained.false_roots, meter)?],
            }
        } else if retained.missing_count > 0 {
            TightVerdict::Residual {
                unsupported_atom: first_set(&retained.missing, meter)?,
            }
        } else {
            TightVerdict::Stable
        };
        Ok(Some(TightCheck {
            verdict,
            work: meter.charged(),
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
            self.grow(
                plan,
                limits.max_bytes,
                additional as u128 * size_of::<Reverse<usize>>() as u128,
                |retained| retained.pending.try_reserve_exact(additional),
            )?;
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

    /// Admit the workspace's bytes under `plan` against `limit`, in one of two
    /// roles. At a check's start it admits the holdings before any work. After a
    /// reservation whose growth was admitted before it, it refuses only capacity
    /// an allocator returned beyond the request, and otherwise reports the bytes
    /// held.
    fn admit(&self, plan: &TightPlan, limit: u64) -> Result<u64, TightError> {
        bytes(self.requested(plan, 0), limit)
    }

    /// Every byte the workspace counts under `plan` except the numeric scratch:
    /// the plan's resident bytes, the truth and support vectors at the larger of
    /// capacity and a complete check's size, and the retained evaluation.
    fn bytes_beside_numeric(&self, plan: &TightPlan) -> u128 {
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
/// node it re-evaluates. A macro, so that both per-node loops receive the rule
/// inline.
macro_rules! node_truth {
    ($theory:expr, $view:expr, $index:expr, $candidate:expr, $values:expr, $numeric:expr, $meter:expr) => {
        match $view.node($index).map_err(|_| Stop::InvalidProgram)? {
            NodeView::Atom(atom) => $candidate.contains(atom),
            NodeView::False => false,
            NodeView::And(operands) => evaluate_operands(operands, $values, true, $meter)?,
            NodeView::Or(operands) => evaluate_operands(operands, $values, false, $meter)?,
            NodeView::Threshold(threshold) => {
                $numeric.threshold::<TightError>($theory, $index, threshold, |child| {
                    $meter.tick()?;
                    Ok($values[child] != 0)
                })?
            }
            NodeView::ExtremumCut(cut) => {
                cut.rule.holds(
                    $numeric.extremum::<TightError>($theory, $index, cut, |child| {
                        $meter.tick()?;
                        Ok($values[child] != 0)
                    })?,
                )
            }
            NodeView::Implies(a, b) => {
                $meter.tick()?;
                let left = $values[a];
                $meter.tick()?;
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
    meter: &mut Meter<'_>,
) -> Result<(), TightError> {
    numeric.reset(theory, || meter.tick())?;
    let view = theory.view();
    for index in 0..view.len() {
        meter.tick()?;
        let truth = node_truth!(theory, view, index, candidate, values, numeric, meter);
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
    meter: &mut Meter<'_>,
) -> Result<bool, TightError> {
    let view = theory.view();
    Ok(node_truth!(
        theory, view, index, candidate, values, numeric, meter
    ))
}

/// The first original root whose node is false, one charged test per root.
/// Like the passes below, it stays out of line so that its loop receives the
/// inlined charge.
#[inline(never)]
fn false_root(
    theory: &Theory,
    values: &[u8],
    meter: &mut Meter<'_>,
) -> Result<Option<usize>, TightError> {
    for &root in theory.roots() {
        meter.tick()?;
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
    meter: &mut Meter<'_>,
) -> Result<(), TightError> {
    for producer in producers {
        meter.tick()?;
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
    meter: &mut Meter<'_>,
) -> Result<Option<usize>, TightError> {
    let mut present = candidate.words32();
    for (word, &support) in supported.iter().enumerate() {
        meter.tick()?;
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

/// The most changed atoms an update takes on; a larger change evaluates
/// completely. An execution choice: both routes return the same verdict.
pub(super) fn large_change(atoms: usize) -> usize {
    (atoms / 8).max(64)
}

/// The least set position of a packed set, one charged operation per word read.
fn first_set(words: &[u64], meter: &mut Meter<'_>) -> Result<usize, TightError> {
    for (index, &word) in words.iter().enumerate() {
        meter.tick()?;
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
    meter: &mut Meter<'_>,
) -> Result<bool, TightError> {
    for &child in operands {
        meter.tick()?;
        let truth = values[child] != 0;
        // A false conjunct or true disjunct determines this node, irrespective
        // of the remaining operands. Other DAG nodes still get their own truth.
        if truth != conjunction {
            return Ok(truth);
        }
    }
    Ok(conjunction)
}
