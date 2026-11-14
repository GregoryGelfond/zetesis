//! Split ordering over already-maintained unknown-parent counts, and over the
//! chains a region has shortened.
//!
//! A chain is a conjunction or disjunction read as one node over its
//! operands. In a region it has *members* still to be decided: its open
//! operands, and its own truth while that is not yet known. A disjunction
//! known to hold needs one of its open operands to hold, and a conjunction
//! known to fail needs one to fail; while the chain's truth is open it is one
//! more thing the operands decide, as the body of `h :- a, b` with `a` held
//! leaves `b` and `h`.
//!
//! The region has *shortened* a chain when its decisions settled an operand
//! without settling the chain and two or more members remain. Its members are
//! where the next decision does the most: one more settled the same way
//! brings the chain to the single member the closure decides. A chain with
//! every operand open says nothing the program's structure does not already
//! say, so it counts nothing here; that structure is read by the generator
//! order ([`super::GeneratorOrder`]) where one is prepared, and by the
//! unknown-parent counts otherwise.
//!
//! An atom's score adds, for each occurrence of the atom in a shortened chain,
//! the chain's weight now less its weight with every operand open. A weight
//! is a quarter for each member, so one chain down to two members outweighs
//! three down to three, and a chain's first settled operand counts least. The
//! atom is found as an operand of the chain, or under at most two enclosing
//! implications whose truth is not yet known, which reaches the chains that
//! hold its negation or double negation and a conditional it is part of.
//!
//! The closure marks a region's knowledge when it shortens a chain
//! ([`Knowledge::shortened`]), and descendants inherit the mark. Without it
//! every score is zero and no ranking reads one. The score is read where no
//! restriction is active: under a generator order among the heads of least
//! choice depth, otherwise among the eligible atoms before their
//! unknown-parent counts. Under a candidate restriction or an objective bound
//! the ranking subject is the restriction's own knowledge, whose chains are
//! not the original's, and the degree ranking is unchanged. The score orders
//! an exhaustive split and is no part of any membership decision.

use zetesis_algebra::{Cancellation, Class, Failure, Quota};

use super::{
    ChoiceHeads, Count, Knowledge, Known, Meter, Narrower, PreferenceAttempt, Region, Width,
    chain_position,
};

/// Members beyond which every chain weighs the same. A chain that long is far
/// from deciding anything, and sixteen quarters keep the heaviest weight at
/// `2^40`, so the weights of a million chains still add without saturating.
const WEIGHED_MEMBERS: usize = 16;

/// Enclosing implications, not yet known, an occurrence is read through: two
/// reach an atom under `not` and under `not not`.
const ENCLOSING_IMPLICATIONS: u8 = 2;

/// A chain's weight with `members` members: a quarter for each, down to 256
/// for [`WEIGHED_MEMBERS`] or more.
const fn weight(members: usize) -> u64 {
    let members = if members < WEIGHED_MEMBERS {
        members
    } else {
        WEIGHED_MEMBERS
    };
    1 << (8 + 2 * (WEIGHED_MEMBERS - members))
}

impl Narrower {
    /// Whether this index was built for a theory of `atoms` atoms.
    pub(super) fn indexes(&self, atoms: usize) -> bool {
        self.atom_nodes.len() == atoms
    }

    /// What the chains the region has shortened weigh for `atom` (the module
    /// header states the score). `knowledge` must be this index's, closed for
    /// the region.
    ///
    /// One unit per parent read: the parents of the atom's nodes, and those
    /// of each enclosing implication read through. Constant additional space.
    ///
    /// # Errors
    /// The meter's refusal, with the units charged before it.
    pub(super) fn shortened_weight<C: Class, Q: Quota + ?Sized>(
        &self,
        knowledge: &Knowledge<C>,
        atom: usize,
        work: &mut Meter<'_, Q>,
    ) -> Result<u64, Failure<Q::Error>> {
        match &knowledge.width {
            Width::Compact16(known) => self.shortened_weight_at(known, atom, work),
            Width::Compact32(known) => self.shortened_weight_at(known, atom, work),
            Width::Native(known) => self.shortened_weight_at(known, atom, work),
        }
    }

    /// [`Self::shortened_weight`] at the knowledge's own counter width.
    pub(super) fn shortened_weight_at<W: Count, Q: Quota + ?Sized>(
        &self,
        known: &Known<W>,
        atom: usize,
        work: &mut Meter<'_, Q>,
    ) -> Result<u64, Failure<Q::Error>> {
        let mut score = 0_u64;
        for &node in &self.atom_nodes[atom] {
            score = score.saturating_add(self.weight_above(
                known,
                node,
                ENCLOSING_IMPLICATIONS,
                work,
            )?);
        }
        Ok(score)
    }

    /// The weight of the shortened chains that hold `node` as an operand, and
    /// of those above each implication that reads `node` and is not yet known,
    /// through `enclosing` of them.
    fn weight_above<W: Count, Q: Quota + ?Sized>(
        &self,
        known: &Known<W>,
        node: usize,
        enclosing: u8,
        work: &mut Meter<'_, Q>,
    ) -> Result<u64, Failure<Q::Error>> {
        let mut score = 0_u64;
        for parent in self.logical_parents(node) {
            work.tick()?;
            let root_known = known.masks.node_known(parent);
            if let Some(chain) = self.chain_of[parent] {
                let chain = chain_position(chain);
                if known.masks.witnessed(chain) {
                    continue;
                }
                let written = self.chains[chain].leaves(&self.extrema).len();
                let open = written.saturating_sub(known.neutral_operands.get(chain));
                // The chain's own truth is a member while it is unknown.
                let own = usize::from(!root_known);
                if open == written || open + own < 2 {
                    continue;
                }
                score = score.saturating_add(weight(open + own) - weight(written + own));
            } else if enclosing > 0 && !root_known {
                score =
                    score.saturating_add(self.weight_above(known, parent, enclosing - 1, work)?);
            }
        }
        Ok(score)
    }
}

impl<C: Class> Knowledge<C> {
    /// Whether a closure of the region or of an ancestor shortened a chain.
    /// Without the mark every [`Narrower::shortened_weight`] is zero. A chain
    /// settled since is not forgotten, so with it every weight may be zero
    /// too.
    pub(super) fn shortened(&self) -> bool {
        match &self.width {
            Width::Compact16(known) => known.shortened,
            Width::Compact32(known) => known.shortened,
            Width::Native(known) => known.shortened,
        }
    }

    fn unknown_count(&self, atom: usize) -> usize {
        match &self.width {
            Width::Compact16(known) => known.unknown.get(atom),
            Width::Compact32(known) => known.unknown.get(atom),
            Width::Native(known) => known.unknown.get(atom),
        }
    }

    /// Select by original choice marks, this subject's unknown-parent count,
    /// then `secondary`'s count; remaining ties select the lowest atom. Both
    /// knowledge values must use the region's atom identities and be closed
    /// for it. This only orders an exhaustive split and changes neither value.
    ///
    /// The scan takes `O(A / 64 + E)` time for `A` atoms and `E` eligible atoms,
    /// with constant additional space. Eligible atoms are the open choice heads
    /// when any remain, otherwise all open atoms. Each primary count is charged
    /// once; a secondary count is read at most once, only when a primary tie
    /// requires it. An uncontested winner needs no secondary read. Packed mask
    /// traversal is not separately charged.
    /// Cancellation and quota reservations follow
    /// [`Self::preferred_atom_reserved`]; every failure retains its exact work
    /// prefix and returns no partial preference.
    ///
    /// # Errors
    /// Retains cancellation, a quota refusal or a zero-grant work refusal.
    ///
    /// # Panics
    /// Panics if an open atom is outside either knowledge's universe.
    pub fn preferred_atom_with_secondary_reserved<Q: Quota + ?Sized>(
        &self,
        region: &Region,
        choices: &ChoiceHeads,
        secondary: &Self,
        cancellation: &Cancellation,
        work: &mut Meter<'_, Q>,
    ) -> PreferenceAttempt<Failure<Q::Error>> {
        let before = work.charged();
        let result = cancellation
            .poll()
            .map_err(Failure::from)
            .and_then(|()| self.preferred_with_secondary(region, choices, secondary, work));
        work.refund();
        PreferenceAttempt {
            result,
            work: work.charged() - before,
        }
    }

    fn read_unknown<Q: Quota + ?Sized>(
        &self,
        atom: usize,
        work: &mut Meter<'_, Q>,
    ) -> Result<usize, Failure<Q::Error>> {
        work.tick()?;
        Ok(self.unknown_count(atom))
    }

    fn preferred_with_secondary<Q: Quota + ?Sized>(
        &self,
        region: &Region,
        choices: &ChoiceHeads,
        secondary: &Self,
        work: &mut Meter<'_, Q>,
    ) -> Result<Option<usize>, Failure<Q::Error>> {
        let mut best: Option<(usize, usize, Option<usize>)> = None;
        choices.try_for_each_eligible(region, |atom| -> Result<(), Failure<Q::Error>> {
            let primary = self.read_unknown(atom, work)?;
            let Some((previous, degree, cached)) = best.as_mut() else {
                best = Some((atom, primary, None));
                return Ok(());
            };
            match primary.cmp(degree) {
                std::cmp::Ordering::Greater => {
                    *previous = atom;
                    *degree = primary;
                    *cached = None;
                }
                std::cmp::Ordering::Equal => {
                    let previous_count = if let Some(count) = *cached {
                        count
                    } else {
                        let count = secondary.read_unknown(*previous, work)?;
                        *cached = Some(count);
                        count
                    };
                    let count = secondary.read_unknown(atom, work)?;
                    if count > previous_count {
                        *previous = atom;
                        *cached = Some(count);
                    }
                }
                std::cmp::Ordering::Less => {}
            }
            Ok(())
        })?;
        Ok(best.map(|(atom, _, _)| atom))
    }
}
