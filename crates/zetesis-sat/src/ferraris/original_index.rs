//! The original theory's narrowing index: one owner, built when a walk first
//! needs it.
//!
//! An enumeration or a standalone membership check owns one
//! [`OriginalIndex`]. Its candidate walk and every proper-subset query of its
//! reduct read the same immutable [`IndexedTheory`], so the index is built at
//! most once per owner, and only by [`OriginalIndex::ensure`]: no other code
//! can construct an `IndexedTheory`. A run decided without a region walk, by a
//! positive certificate or by the clause kernel, builds none.

use std::sync::Arc;

use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Narrower, Theory};

use super::regions::stopped;
use crate::Incomplete;

/// One immutable original-theory index. Construction binds the index to the
/// exact admitted instance; equal independently admitted DAGs are not its
/// subject. Candidate and reduct traversals share this owner, never their
/// mutable knowledge.
#[derive(Debug)]
pub(crate) struct IndexedTheory {
    theory: Theory,
    narrower: Narrower,
}

impl IndexedTheory {
    pub(crate) fn theory(&self) -> &Theory {
        &self.theory
    }

    pub(crate) fn narrower(&self) -> &Narrower {
        &self.narrower
    }

    /// Authenticate before borrowing the indexed subject for any traversal.
    pub(crate) fn subject(&self, theory: &Theory) -> Result<(&Theory, &Narrower), Incomplete> {
        if self.theory.same_instance(theory) {
            Ok((&self.theory, &self.narrower))
        } else {
            Err(Incomplete::WrongTheory)
        }
    }
}

/// The admitted theory and, once a walk has needed it, its index.
#[derive(Debug)]
pub(crate) struct OriginalIndex {
    theory: Theory,
    index: Option<Arc<IndexedTheory>>,
    native_byte_limit: u64,
}

impl OriginalIndex {
    /// An owner of the theory's index, not yet built.
    pub(crate) fn new(theory: &Theory, native_byte_limit: u64) -> Self {
        Self {
            theory: theory.clone(),
            index: None,
            native_byte_limit,
        }
    }

    /// The index, if a walk has built it; never builds.
    pub(crate) fn get(&self) -> Option<&Arc<IndexedTheory>> {
        self.index.as_ref()
    }

    /// The index, built on the first call that finds none.
    ///
    /// Before building, `charge` receives the index's documented cost: one
    /// work unit per node and operand occurrence of the theory, which equals the built narrower's
    /// [`Narrower::work`] and is known before building. A refused charge
    /// builds nothing and leaves the owner unbuilt. An admitted charge stays
    /// admitted even if the build then fails, as a failed narrowing keeps
    /// its admitted prefix: `charge` is where the caller records it, in the
    /// budget and in the receipt that counts index work, so the two agree on
    /// every exit. A built index is returned without calling `charge` again.
    ///
    /// # Errors
    /// Cancellation, the charge or native-storage refusal, or allocation failure.
    pub(crate) fn ensure(
        &mut self,
        cancellation: &Cancellation,
        charge: impl FnOnce(u64) -> Result<(), Incomplete>,
    ) -> Result<&Arc<IndexedTheory>, Incomplete> {
        cancellation.poll()?;
        if let Some(index) = &self.index {
            admit(&index.narrower, self.native_byte_limit)?;
        }
        let index = if let Some(index) = self.index.take() {
            index
        } else {
            let work = u64::try_from(
                self.theory.nodes().len() as u128 + self.theory.parts().occurrences() as u128,
            )
            .map_err(|_| Incomplete::CounterOverflow)?;
            charge(work)?;
            let narrower = prepare(&self.theory, self.native_byte_limit, cancellation)?;
            debug_assert_eq!(narrower.work(), work, "the charge is the index's work");
            Arc::new(IndexedTheory {
                theory: self.theory.clone(),
                narrower,
            })
        };
        Ok(self.index.insert(index))
    }
}

/// Construct an original, restriction or objective-bound index under the same
/// additional native-storage policy. The original owner retains its own logical
/// work receipt; restrictions and bounds preserve their existing work contract.
///
/// The check bounds retained publication, not peak construction memory. Legacy
/// Boolean index storage and constructor-internal cancellation are unchanged.
pub(super) fn prepare(
    theory: &Theory,
    native_byte_limit: u64,
    cancellation: &Cancellation,
) -> Result<Narrower, Incomplete> {
    cancellation.poll()?;
    // Every live native cut retains at least its original node identity, even
    // when it becomes constant. Unused numeric descriptors imply no index.
    let mut minimum = 0u128;
    if !theory.parts().extremum_cuts().is_empty() {
        for &node in theory.nodes() {
            cancellation.poll()?;
            if node.extremum_cut_descriptor().is_some() {
                minimum += size_of::<usize>() as u128;
            }
        }
    }
    check_bytes(minimum, native_byte_limit)?;
    #[cfg(test)]
    hooks::observe(theory)?;
    // Allocation failure remains Allocation, not a fabricated storage receipt.
    // The limit bounds retained publication; temporary construction allocations
    // and allocator excess can precede the final actual-capacity check.
    let narrower = Narrower::try_new(theory).map_err(stopped)?;
    cancellation.poll()?;
    admit(&narrower, native_byte_limit)?;
    Ok(narrower)
}

pub(super) fn admit(narrower: &Narrower, native_byte_limit: u64) -> Result<(), Incomplete> {
    check_bytes(narrower.extremum_index_bytes(), native_byte_limit)
}

fn check_bytes(required: u128, limit: u64) -> Result<(), Incomplete> {
    if required > u128::from(limit) {
        Err(Incomplete::NativeNarrowingStorage {
            required,
            limit: u128::from(limit),
        })
    } else {
        Ok(())
    }
}

/// Test-only observation of the construction site: a count of builds per
/// admitted instance, and an injected build failure. Neither exists in a
/// non-test build.
#[cfg(test)]
pub(crate) mod hooks {
    use std::cell::Cell;
    use std::sync::{Mutex, PoisonError};

    use zetesis_ferraris::Theory;

    use crate::Incomplete;

    /// Every instance whose index construction was entered, once per entry.
    /// Instances are compared by identity, so concurrently running tests,
    /// each with its own admitted theory, never count each other's builds.
    static BUILDS: Mutex<Vec<Theory>> = Mutex::new(Vec::new());

    thread_local! {
        /// Fail the next construction entered on this thread.
        static FAIL: Cell<bool> = const { Cell::new(false) };
    }

    pub(super) fn observe(theory: &Theory) -> Result<(), Incomplete> {
        BUILDS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(theory.clone());
        if FAIL.with(Cell::take) {
            Err(Incomplete::Allocation)
        } else {
            Ok(())
        }
    }

    /// Index constructions entered for this exact instance, on any thread.
    pub(crate) fn builds(theory: &Theory) -> usize {
        BUILDS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter(|built| built.same_instance(theory))
            .count()
    }

    /// Make the next construction entered on this thread refuse its
    /// allocation, as `Narrower::try_new` refuses one.
    pub(crate) fn fail_next_build() {
        FAIL.with(|fail| fail.set(true));
    }
}

#[cfg(test)]
mod storage_tests;
