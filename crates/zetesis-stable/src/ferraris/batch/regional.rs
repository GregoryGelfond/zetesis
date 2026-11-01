//! Scoped decisions about supplied subregions of authenticated frozen queries.

use std::convert::Infallible;

use zetesis_algebra::region::Region;
use zetesis_ferraris::Interpretation;

use super::{
    BatchError, BatchVerdict, CompletionExecutor, OriginalCheck, SlateTicket, StableModels,
    completion_error,
};

/// The result of an exact search restricted to one supplied region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionalOutcome {
    /// The original candidate has an independently validated proper reduct model.
    ValidatedCountermodel,
    /// The supplied region contains no proper reduct model. Stability of the
    /// whole candidate additionally requires evidence covering all other points.
    NoWitnessInSuppliedRegion,
}

/// A joined completion awaiting the checker's whole-candidate conclusions.
/// Holds exclusive access to the oldest pending slate. Dropping the pass
/// abandons its unpublished conclusions and retains work already performed.
#[derive(Debug)]
#[must_use = "finish the pass to publish the oldest slate"]
pub struct RegionalPass<'s> {
    stream: &'s mut StableModels,
    classified: Vec<BatchVerdict>,
    flags: Vec<bool>,
}

impl StableModels {
    /// Check the oldest joint slate's residuals inside supplied regions.
    /// The checker supplies a root for each provisional Residual in ascending
    /// slot order. Original truth and terminal assertions carry the same trust
    /// obligations as [`Self::commit_slate`]. Each root must cover all proper
    /// witnesses if its absence result will be used to assert global stability.
    /// Existing certificates can settle slots before a regional search.
    ///
    /// # Errors
    /// Ordering, shape and completion-scratch refusals leave the slate
    /// retryable. Native search failures terminate the stream without coverage.
    pub fn begin_regional_pass<'s>(
        &'s mut self,
        ticket: SlateTicket,
        mut provisional: Vec<BatchVerdict>,
        mut roots: Vec<(usize, Region)>,
        completion: &mut CompletionExecutor,
    ) -> Result<RegionalPass<'s>, BatchError<Infallible>> {
        self.require_oldest(ticket)?;
        let result = (|| {
            self.check_tags(&provisional, OriginalCheck::Joint)?;
            validate_roots(&self.batch.pending, &provisional, &roots)?;
            self.certify_tags(&mut provisional)?;
            roots.retain(|(slot, _)| provisional[*slot] == BatchVerdict::Residual);
            self.complete_pending(&provisional, Some(roots), completion)
                .map_err(completion_error)
        })();
        self.terminate_on_search(&result);
        Ok(RegionalPass {
            flags: result?,
            classified: provisional,
            stream: self,
        })
    }
}

fn validate_roots(
    candidates: &[Interpretation],
    tags: &[BatchVerdict],
    roots: &[(usize, Region)],
) -> Result<(), BatchError<Infallible>> {
    let wanted = tags
        .iter()
        .enumerate()
        .filter_map(|(i, t)| (*t == BatchVerdict::Residual).then_some(i));
    let expected = wanted.clone().count();
    if expected != roots.len() {
        return Err(BatchError::Shape {
            expected,
            actual: roots.len(),
        });
    }
    for (prefix, (index, (slot, root))) in wanted.zip(roots).enumerate() {
        let candidate = &candidates[index];
        // Region's constructor preserves disjoint masks and clear padding.
        // Every atom outside this interpretation must already be cut.
        if *slot != index
            || root.len() != candidate.theory().atom_count()
            || !(0..root.len()).all(|atom| candidate.contains(atom) || root.is_cut(atom))
        {
            return Err(BatchError::Shape {
                expected,
                actual: prefix,
            });
        }
    }
    Ok(())
}

impl RegionalPass<'_> {
    /// Results for the residual slots that entered the exact regional query,
    /// in ascending slate order. CPU-certified slots have no regional result.
    pub fn outcomes(&self) -> impl Iterator<Item = (usize, RegionalOutcome)> + '_ {
        self.classified
            .iter()
            .zip(&self.flags)
            .enumerate()
            .filter_map(|(index, (tag, &absent))| {
                (*tag == BatchVerdict::Residual).then_some((
                    index,
                    if absent {
                        RegionalOutcome::NoWitnessInSuppliedRegion
                    } else {
                        RegionalOutcome::ValidatedCountermodel
                    },
                ))
            })
    }

    /// Publish the slate after the checker supplies exactly one trusted
    /// whole-candidate decision for every exposed regional result, in order.
    /// Asserting `NoProperSubset` for regional absence requires independent
    /// coverage of every proper witness. A validated witness requires `Refuted`.
    ///
    /// # Errors
    /// A malformed decision list publishes nothing and leaves the oldest
    /// slate retryable. Allocation or accounting failure retains its typed stop.
    pub fn finish(
        self,
        decisions: Vec<BatchVerdict>,
    ) -> Result<Vec<Interpretation>, BatchError<Infallible>> {
        let expected = self.outcomes().count();
        if decisions.len() != expected {
            return Err(BatchError::Shape {
                expected,
                actual: decisions.len(),
            });
        }
        for (prefix, ((_, outcome), decision)) in self.outcomes().zip(&decisions).enumerate() {
            if !matches!(
                (outcome, decision),
                (
                    RegionalOutcome::ValidatedCountermodel,
                    BatchVerdict::Refuted
                ) | (
                    RegionalOutcome::NoWitnessInSuppliedRegion,
                    BatchVerdict::NoProperSubset
                )
            ) {
                return Err(BatchError::Shape {
                    expected,
                    actual: prefix,
                });
            }
        }
        drop(decisions);
        let result = self
            .stream
            .publish_pending(&self.classified, self.flags)
            .map_err(BatchError::Search);
        self.stream.terminate_on_search(&result);
        result
    }
}
