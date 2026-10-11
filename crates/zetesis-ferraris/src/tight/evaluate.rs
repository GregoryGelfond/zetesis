use super::{
    TightAttempt, TightCheck, TightCheckLimits, TightError, TightPlan, TightVerdict, Work, bytes,
};
use crate::extremum_cut::ExtremaScratch;
use crate::{Interpretation, NodeView, Theory};
use zetesis_cpu::{Cancellation, Stop};

#[cfg(test)]
mod tests;

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
        TightWorkspace::default().check(self, candidate, limits, cancellation)
    }
}

/// Reusable truth and support storage for complete tight-certificate checks.
///
/// Each call recomputes every formula truth and clears support before reading
/// producers. Capacity survives checks and interruptions; no candidate truth
/// survives as evidence. Each conjunction or disjunction reads operands only
/// through its first decisive value. Checks are bounded by nodes, operand
/// occurrences, tested roots, producers and packed candidate words. Thresholds
/// read and charge every contribution, including zero weights. Ordinal cuts
/// read every physical occurrence once per authoritative row and pass, even
/// for endpoint constants. Dense measure growth and packed-validity resets are
/// charged; old measures are inaccessible until completed in the current pass. Truth uses
/// one byte per node; support uses one bit per atom, rounded to 32-bit words.
/// Each concurrent checker owns an exclusive workspace; immutable plans may be
/// shared.
#[derive(Debug, Default)]
pub struct TightWorkspace {
    values: Vec<u8>,
    supported: Vec<u32>,
    extrema: ExtremaScratch,
}

impl TightWorkspace {
    /// Actual retained vector payload, excluding headers, the plan, its shared
    /// theory and allocator bookkeeping. A failed check can retain new capacity.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        self.values.capacity() as u128
            + self.supported.capacity() as u128 * size_of::<u32>() as u128
            + self.extrema.retained_bytes()
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
        let mut work = Work {
            used: 0,
            max: limits.max_work,
            cancellation: cancellation.polling(),
        };
        let result = self.evaluate(plan, candidate, limits, &mut work);
        TightAttempt {
            result,
            work: work.used,
        }
    }

    fn evaluate(
        &mut self,
        plan: &TightPlan,
        candidate: &Interpretation,
        limits: TightCheckLimits,
        work: &mut Work<'_>,
    ) -> Result<TightCheck, TightError> {
        if !plan.theory.same_instance(candidate.theory()) {
            return Err(Stop::WrongProgram.into());
        }
        work.cancellation.poll()?;
        self.admit(plan, limits.max_bytes)?;
        // Explicit byte cells give candidate tiles a fixed payload bound.
        // Rust's Vec<bool> also stores byte-sized elements; it is not bit-packed.
        self.values.clear();
        self.values
            .try_reserve_exact(plan.theory.nodes().len())
            .map_err(|_| Stop::Allocation)?;
        self.admit(plan, limits.max_bytes)?;
        let base = self.truth_support_bytes(plan);
        self.extrema.reserve_observed(&plan.theory, |actual| {
            bytes(base + actual, limits.max_bytes).map(|_| ())
        })?;
        let logical_bytes = self.admit(plan, limits.max_bytes)?;
        evaluate_truth(
            &plan.theory,
            candidate,
            &mut self.values,
            &mut self.extrema,
            work,
        )?;
        for &root in plan.theory.roots() {
            work.tick()?;
            if self.values[root] == 0 {
                return Ok(TightCheck {
                    verdict: TightVerdict::NotModel { root },
                    work: work.used,
                    logical_bytes,
                });
            }
        }
        self.supported.clear();
        let words = plan.theory.atom_count().div_ceil(32);
        self.supported
            .try_reserve_exact(words)
            .map_err(|_| Stop::Allocation)?;
        let logical_bytes = self.admit(plan, limits.max_bytes)?;
        self.supported.resize(words, 0);
        for producer in &plan.producers {
            work.tick()?;
            if producer.body.is_none_or(|body| self.values[body] != 0) {
                self.supported[producer.head / 32] |= 1 << (producer.head % 32);
            }
        }
        let mut present = candidate.words32();
        for word in 0..words {
            work.tick()?;
            let missing = present
                .next()
                .expect("support and candidate cover the same admitted atom universe")
                & !self.supported[word];
            if missing != 0 {
                return Ok(TightCheck {
                    verdict: TightVerdict::Residual {
                        unsupported_atom: word * 32 + missing.trailing_zeros() as usize,
                    },
                    work: work.used,
                    logical_bytes,
                });
            }
        }
        Ok(TightCheck {
            verdict: TightVerdict::Stable,
            work: work.used,
            logical_bytes,
        })
    }

    fn admit(&self, plan: &TightPlan, limit: u64) -> Result<u64, TightError> {
        bytes(
            self.truth_support_bytes(plan) + self.extrema.required_bytes(&plan.theory),
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
    }
}

/// Write each formula truth in admitted topological order. The caller clears
/// and reserves the target first; a stopped prefix is never membership evidence.
fn evaluate_truth(
    theory: &Theory,
    candidate: &Interpretation,
    values: &mut Vec<u8>,
    extrema: &mut ExtremaScratch,
    work: &mut Work<'_>,
) -> Result<(), TightError> {
    extrema.reset(theory, || work.tick())?;
    let view = theory.view();
    for index in 0..view.len() {
        work.tick()?;
        values.push(u8::from(
            match view.node(index).map_err(|_| Stop::InvalidProgram)? {
                NodeView::Atom(atom) => candidate.contains(atom),
                NodeView::False => false,
                NodeView::And(operands) => evaluate_operands(operands, values, true, work)?,
                NodeView::Or(operands) => evaluate_operands(operands, values, false, work)?,
                NodeView::Threshold(threshold) => threshold.evaluate::<TightError>(|child| {
                    work.tick()?;
                    Ok(values[child] != 0)
                })?,
                NodeView::ExtremumCut(cut) => {
                    cut.rule
                        .holds(extrema.measure::<TightError>(theory, index, cut, |child| {
                            work.tick()?;
                            Ok(values[child] != 0)
                        })?)
                }
                NodeView::Implies(a, b) => {
                    work.tick()?;
                    let left = values[a];
                    work.tick()?;
                    let right = values[b];
                    left == 0 || right != 0
                }
            },
        ));
    }
    Ok(())
}

fn evaluate_operands(
    operands: &[usize],
    values: &[u8],
    conjunction: bool,
    work: &mut Work<'_>,
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
