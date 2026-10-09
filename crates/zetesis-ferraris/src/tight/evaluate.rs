use super::{
    TightAttempt, TightCheck, TightCheckLimits, TightError, TightPlan, TightVerdict, Work, bytes,
};
use crate::{Interpretation, NodeView};
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
/// survives as evidence. Checks remain linear in nodes, operand occurrences,
/// tested roots, producers and atoms. Storage is linear in the largest admitted
/// node and atom counts. Each concurrent checker owns an exclusive workspace;
/// immutable plans may be shared.
#[derive(Debug, Default)]
pub struct TightWorkspace {
    values: Vec<u8>,
    supported: Vec<u8>,
}

impl TightWorkspace {
    /// Actual retained vector payload, excluding headers, the plan, its shared
    /// theory and allocator bookkeeping. A failed check can retain new capacity.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        self.values.capacity() as u128 + self.supported.capacity() as u128
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
        let logical_bytes = self.admit(plan, limits.max_bytes)?;
        for index in 0..plan.theory.view().len() {
            work.tick()?;
            self.values.push(u8::from(
                match plan
                    .theory
                    .view()
                    .node(index)
                    .map_err(|_| Stop::InvalidProgram)?
                {
                    NodeView::Atom(atom) => candidate.contains(atom),
                    NodeView::False => false,
                    NodeView::And(operands) => {
                        evaluate_operands(operands, &self.values, true, work)?
                    }
                    NodeView::Or(operands) => {
                        evaluate_operands(operands, &self.values, false, work)?
                    }
                    NodeView::Implies(a, b) => {
                        work.tick()?;
                        let left = self.values[a];
                        work.tick()?;
                        let right = self.values[b];
                        left == 0 || right != 0
                    }
                },
            ));
        }
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
        self.supported
            .try_reserve_exact(plan.theory.atom_count())
            .map_err(|_| Stop::Allocation)?;
        let logical_bytes = self.admit(plan, limits.max_bytes)?;
        self.supported.resize(plan.theory.atom_count(), 0);
        for producer in &plan.producers {
            work.tick()?;
            if producer.body.is_none_or(|body| self.values[body] != 0) {
                self.supported[producer.head] = 1;
            }
        }
        for (atom, &support) in self.supported.iter().enumerate() {
            work.tick()?;
            if candidate.contains(atom) && support == 0 {
                return Ok(TightCheck {
                    verdict: TightVerdict::Residual {
                        unsupported_atom: atom,
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
            u128::from(plan.statistics.resident_bytes)
                + self.values.capacity().max(plan.theory.nodes().len()) as u128
                + self.supported.capacity().max(plan.theory.atom_count()) as u128,
            limit,
        )
    }
}

fn evaluate_operands(
    operands: &[usize],
    values: &[u8],
    conjunction: bool,
    work: &mut Work<'_>,
) -> Result<bool, TightError> {
    let mut value = conjunction;
    for &child in operands {
        work.tick()?;
        let truth = values[child] != 0;
        value = if conjunction {
            value & truth
        } else {
            value | truth
        };
    }
    Ok(value)
}
