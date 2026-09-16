//! Ordered access as a stack of sorted runs, maintained across appends.

use std::mem::size_of;

use crate::ordered_index::position;

use super::{Catalog, CatalogFailure, Failure, Limits, Storage, Work};

/// The prepared canonical order of one extent as sorted runs of row IDs.
///
/// The runs are the levels, each in canonical order and each less than half
/// the length of the level before it, followed by the tail: the run merged in
/// by the last preparation that appended anything. A row belongs to exactly
/// one run. Every run is a sorted sequence, so a bound-prefix window is one
/// binary search per run; the runs together are not one sequence, and rank
/// access across them needs [`Catalog::canonical`]. The borrow prevents
/// append. Row IDs are stable catalog-local insertion IDs.
#[derive(Clone, Copy)]
pub struct Runs<'a> {
    levels: &'a [Vec<usize>],
    tail: &'a [usize],
    storage: Storage,
}

impl<'a> Runs<'a> {
    /// Number of rows across every run. Linear in the number of runs.
    #[must_use]
    pub fn len(self) -> usize {
        self.levels.iter().map(Vec::len).sum::<usize>() + self.tail.len()
    }

    /// Whether no run holds a row.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// The levels, oldest first: the rows present before the last appending
    /// preparation, in geometrically shrinking sorted runs.
    #[must_use]
    pub fn levels(self) -> &'a [Vec<usize>] {
        self.levels
    }

    /// The rows the last appending preparation merged in, in canonical order.
    /// After the first preparation of a nonempty extent this is every row.
    #[must_use]
    pub fn tail(self) -> &'a [usize] {
        self.tail
    }

    /// Every run, levels first and then the tail; empty runs are omitted.
    pub fn runs(self) -> impl Iterator<Item = &'a [usize]> {
        self.levels
            .iter()
            .map(Vec::as_slice)
            .chain(std::iter::once(self.tail))
            .filter(|run| !run.is_empty())
    }

    /// Current owner capacity and work of the preparation or reuse operation.
    #[must_use]
    pub const fn storage(self) -> Storage {
        self.storage
    }
}

impl Catalog {
    /// Prepare ordered row access without cloning tuple payload.
    ///
    /// A prepared extent is reused with zero traversal work. The first
    /// preparation of a nonempty extent traverses the row index once, O(n),
    /// into the tail. A later preparation first promotes the previous tail to
    /// a level and merges levels until each is less than half its
    /// predecessor, then sorts the `d` rows appended since into the new tail.
    /// Merging two runs of `a` and `b` rows costs `a + b` charged comparisons
    /// and copies; over a whole derivation each row is merged O(log n) times,
    /// so the views cost O(n log n) charged work in all, and no preparation
    /// copies the whole extent. Sorting the appended rows costs O(d log d)
    /// comparisons. The levels and tail are this owner's retained capacity; a
    /// merge holds one scratch run of the merged length until it replaces its
    /// two inputs. Only successful insertion invalidates the runs; a refused
    /// insertion or a refused preparation leaves every published run whole.
    ///
    /// # Errors
    /// Current shape, bytes and work must fit `limits`. Refused preparation does
    /// not publish a partial run or change atom/equality identity.
    pub fn prepare_ordered(&mut self, limits: Limits) -> Result<Runs<'_>, CatalogFailure> {
        let mut work = self.work(limits)?;
        if self.prepared != self.atoms.len() {
            let prepare = if self.levels.is_empty() && self.tail.is_empty() {
                // A refused traversal leaves no partial run.
                self.traverse(&mut work).inspect_err(|_| self.tail.clear())
            } else {
                self.append_run(&mut work)
            };
            prepare.map_err(|error| self.failed(error, &work))?;
            self.prepared = self.atoms.len();
        }
        Ok(Runs {
            levels: &self.levels,
            tail: &self.tail,
            storage: self.receipt(&work),
        })
    }

    /// Borrow already prepared runs without allocating or traversing rows.
    /// `None` means a successful append requires a new preparation. It does not
    /// mean this relation has no tuples. A new empty catalog is already prepared.
    #[must_use]
    pub fn ordered(&self) -> Option<Runs<'_>> {
        (self.prepared == self.atoms.len()).then(|| Runs {
            levels: &self.levels,
            tail: &self.tail,
            storage: Storage {
                retained_bytes: self.retained_bytes(),
                peak_construction_bytes: self.retained_bytes(),
                referenced_payload_bytes: self.payload,
                borrowed_mapping_bytes: 0,
                construction_work: 0,
            },
        })
    }

    /// The complete canonical order of a prepared extent as one sequence of
    /// row IDs, merged from the runs: O(n log k) charged comparisons for `k`
    /// runs and `n` copies, into a new vector the caller owns. This is the
    /// rank access the runs themselves do not offer.
    ///
    /// # Errors
    /// Refuses an unprepared extent as [`Failure::Order`], or work and bytes
    /// above `limits`.
    pub fn canonical(&self, limits: Limits) -> Result<Vec<usize>, CatalogFailure> {
        let mut work = self.work(limits)?;
        let merge = (|| {
            if self.prepared != self.atoms.len() {
                return Err(Failure::Order);
            }
            let mut merged = work.reserve::<usize>(self.atoms.len())?;
            let runs: Vec<&[usize]> = self
                .ordered()
                .map_or(Vec::new(), |runs| runs.runs().collect());
            let mut cursors = vec![0; runs.len()];
            for _ in 0..self.atoms.len() {
                let mut least: Option<usize> = None;
                for (run, &cursor) in cursors.iter().enumerate() {
                    if cursor == runs[run].len() {
                        continue;
                    }
                    least = Some(match least {
                        Some(best)
                            if self
                                .order(runs[best][cursors[best]], runs[run][cursor], &mut work)?
                                .is_le() =>
                        {
                            best
                        }
                        _ => run,
                    });
                }
                let run = least.ok_or(Failure::Order)?;
                work.tick(1)?;
                merged.push(runs[run][cursors[run]]);
                cursors[run] += 1;
            }
            Ok(merged)
        })();
        merge.map_err(|error| self.failed(error, &work))
    }

    /// Build the first run by an in-order traversal of the row index.
    fn traverse(&mut self, work: &mut Work) -> Result<(), Failure> {
        work.grow(&mut self.tail, self.atoms.len())?;
        work.include(size_of::<Vec<usize>>())?;
        let mut path = work.reserve::<usize>(0)?;
        self.tail.clear();
        let mut cursor = self.rows.root;
        loop {
            while let Some(link) = cursor {
                work.tick(1)?;
                let id = position(link);
                work.grow(&mut path, 1)?;
                work.tick(1)?;
                path.push(id);
                cursor = self.rows.nodes[id].children[0];
            }
            let Some(id) = path.pop() else {
                break;
            };
            work.tick(1)?;
            self.tail.push(id);
            cursor = self.rows.nodes[id].children[1];
        }
        work.release(path);
        work.live -= size_of::<Vec<usize>>();
        Ok(())
    }

    /// Promote the previous tail to a level, compact the levels, and sort the
    /// rows appended since the last preparation into the new tail. Each step
    /// publishes only complete runs, so a refusal at any point leaves the
    /// levels whole and the extent unprepared.
    fn append_run(&mut self, work: &mut Work) -> Result<(), Failure> {
        if !self.tail.is_empty() {
            // Admit the level slot before the move, so a refusal leaves the
            // run where it was.
            work.grow(&mut self.levels, 1)?;
            work.include(size_of::<Vec<usize>>())?;
            let promoted = std::mem::take(&mut self.tail);
            self.levels.push(promoted);
            self.compact(work)?;
        }
        let appended = self.prepared..self.atoms.len();
        let mut run = work.reserve::<usize>(appended.len())?;
        run.extend(appended);
        work.tick(run.len() as u128)?;
        let mut scratch = work.reserve::<usize>(run.len())?;
        self.sort(&mut run, &mut scratch, work)?;
        work.release(scratch);
        work.release(std::mem::replace(&mut self.tail, run));
        Ok(())
    }

    /// Merge the top two levels while the newer is at least half the older,
    /// so the levels shrink geometrically and their count stays O(log n).
    fn compact(&mut self, work: &mut Work) -> Result<(), Failure> {
        while let [.., below, top] = &self.levels[..] {
            if top.len() * 2 < below.len() {
                return Ok(());
            }
            let mut merged = work.reserve::<usize>(below.len() + top.len())?;
            let (mut left, mut right) = (0, 0);
            while left < below.len() && right < top.len() {
                if self.order(top[right], below[left], work)?.is_lt() {
                    merged.push(top[right]);
                    right += 1;
                } else {
                    merged.push(below[left]);
                    left += 1;
                }
            }
            merged.extend_from_slice(&below[left..]);
            merged.extend_from_slice(&top[right..]);
            work.tick(merged.len() as u128)?;
            let top = self.levels.pop().expect("two levels");
            let below = self.levels.pop().expect("two levels");
            work.release(top);
            work.release(below);
            work.live -= size_of::<Vec<usize>>();
            self.levels.push(merged);
        }
        Ok(())
    }

    /// Bottom-up merge sort of a run by typed row comparison; `scratch` has
    /// capacity for the whole run. O(d log d) comparisons, each charged.
    fn sort(
        &self,
        run: &mut Vec<usize>,
        scratch: &mut Vec<usize>,
        work: &mut Work,
    ) -> Result<(), Failure> {
        let length = run.len();
        let mut width = 1;
        while width < length {
            scratch.clear();
            let mut start = 0;
            while start < length {
                let middle = (start + width).min(length);
                let end = (start + 2 * width).min(length);
                let (mut left, mut right) = (start, middle);
                while left < middle && right < end {
                    if self.order(run[right], run[left], work)?.is_lt() {
                        scratch.push(run[right]);
                        right += 1;
                    } else {
                        scratch.push(run[left]);
                        left += 1;
                    }
                }
                scratch.extend_from_slice(&run[left..middle]);
                scratch.extend_from_slice(&run[right..end]);
                start = end;
            }
            work.tick(length as u128)?;
            std::mem::swap(run, scratch);
            width *= 2;
        }
        Ok(())
    }

    /// Typed identity order of two rows, charged per compared value.
    fn order(
        &self,
        left: usize,
        right: usize,
        work: &mut Work,
    ) -> Result<std::cmp::Ordering, Failure> {
        work.tick(1)?;
        for (left, right) in self.atoms[left]
            .values()
            .iter()
            .zip(self.atoms[right].values())
        {
            let order = work.compare(left, right)?;
            if !order.is_eq() {
                return Ok(order);
            }
        }
        Ok(std::cmp::Ordering::Equal)
    }
}
