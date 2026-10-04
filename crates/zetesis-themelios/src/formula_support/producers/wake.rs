//! Borrowed signed dependency postings and a packed original-rule wake set.

use std::mem::size_of;
use std::ops::Range;

use crate::ProgramSite;
use themelios_program::symbol::Signature;
use zetesis_core::{
    catalog::{AtomRef, PredicateRef},
    relation::Failure,
};

use super::{Node, find, invalid};
use crate::formula_ir::{HeadIr, RuleIr};
use crate::formula_support::Counters;
use crate::formula_support::relations::Memory;
use crate::{FormulaFailure, FormulaLimits};

struct PredicateInputs<'source> {
    signature: &'source Signature,
    rules: Range<usize>,
}

/// All payloads stay in the source preparation. Postings contain original rule
/// IDs, once per positive occurrence; repeated predicates may mark one bit more
/// than once. Every selected rule is nevertheless visited once in original order.
pub(super) struct Wake<'source> {
    predicates: Vec<PredicateInputs<'source>>,
    postings: Vec<usize>,
    active: Vec<u64>,
}

impl<'source> Wake<'source> {
    pub(super) fn prepare(
        nodes: &[Node<'source>],
        rules: &[Option<Range<usize>>],
        inputs: &[usize],
        memory: &mut Memory<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let mut counts = Vec::<usize>::new();
        memory.add(size_of::<Vec<usize>>())?;
        memory.reserve(&mut counts, nodes.len())?;
        for _ in nodes {
            counters.work(limits, location)?;
            counts.push(0);
        }
        for &predicate in inputs {
            counters.work(limits, location)?;
            counts[predicate] = counts[predicate]
                .checked_add(1)
                .ok_or_else(|| invalid(Failure::Overflow, location))?;
        }
        let mut predicates = Vec::new();
        memory.reserve(&mut predicates, nodes.len())?;
        let mut start = 0_usize;
        for (node, count) in nodes.iter().zip(&mut counts) {
            counters.work(limits, location)?;
            let end = start
                .checked_add(*count)
                .ok_or_else(|| invalid(Failure::Overflow, location))?;
            predicates.push(PredicateInputs {
                signature: node.signature,
                rules: start..end,
            });
            *count = start;
            start = end;
        }
        let postings = postings(
            rules,
            inputs,
            &mut counts,
            memory,
            limits,
            counters,
            location,
        )?;
        let scratch_bytes = size_of::<Vec<usize>>() + counts.capacity() * size_of::<usize>();
        drop(counts);
        memory.release(scratch_bytes);
        let mut wake = Self {
            predicates,
            postings,
            active: Vec::new(),
        };
        wake.bootstrap(rules, memory, limits, counters, location)?;
        Ok(wake)
    }

    fn bootstrap(
        &mut self,
        rules: &[Option<Range<usize>>],
        memory: &mut Memory<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let words = rules.len().div_ceil(u64::BITS as usize);
        memory.reserve(&mut self.active, words)?;
        for _ in 0..words {
            counters.work(limits, location)?;
            self.active.push(0);
        }
        for (rule, inputs) in rules.iter().enumerate() {
            counters.work(limits, location)?;
            if inputs.as_ref().is_some_and(Range::is_empty) {
                self.mark(rule, limits, counters, location)?;
            }
        }
        Ok(())
    }

    fn mark(
        &mut self,
        rule: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        counters.work(limits, location)?;
        self.active[rule / u64::BITS as usize] |= 1 << (rule % u64::BITS as usize);
        Ok(())
    }

    pub(super) fn schedule(&self) -> Schedule<'_> {
        Schedule::Affected {
            words: &self.active,
            next: 0,
            remaining: 0,
        }
    }

    pub(super) fn advance<'atoms>(
        &mut self,
        mut delta: impl Iterator<Item = AtomRef<'atoms>>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        for word in &mut self.active {
            counters.work(limits, location)?;
            *word = 0;
        }
        // Atom order groups each exact signed predicate. Resolve a changed
        // predicate once, without cloning a key or allocating another set.
        let mut previous: Option<PredicateRef<'_>> = None;
        loop {
            counters.work(limits, location)?;
            let Some(atom) = delta.next() else {
                break;
            };
            let predicate = atom.predicate();
            if let Some(previous) = previous
                && previous.equals_ref_with(predicate, || counters.work(limits, location))?
            {
                continue;
            }
            previous = Some(predicate);
            let index = find(
                self.predicates.len(),
                |index| self.predicates[index].signature,
                predicate,
                limits,
                counters,
                location,
            )?;
            let range = self.predicates[index].rules.clone();
            for posting in range {
                counters.work(limits, location)?;
                self.mark(self.postings[posting], limits, counters, location)?;
            }
        }
        Ok(())
    }
}

fn postings(
    rules: &[Option<Range<usize>>],
    inputs: &[usize],
    counts: &mut [usize],
    memory: &mut Memory<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Vec<usize>, FormulaFailure> {
    let mut postings = Vec::new();
    memory.reserve(&mut postings, inputs.len())?;
    for _ in inputs {
        counters.work(limits, location)?;
        postings.push(0);
    }
    for (rule, occurrences) in rules.iter().enumerate() {
        counters.work(limits, location)?;
        if let Some(occurrences) = occurrences {
            for &predicate in &inputs[occurrences.clone()] {
                counters.work(limits, location)?;
                postings[counts[predicate]] = rule;
                counts[predicate] += 1;
            }
        }
    }
    Ok(postings)
}

/// Original rule order is independent of dependency/SCC or publication order.
/// The full variant is the live rich-source fallback and the reference control.
pub(in crate::formula_support) enum Schedule<'a> {
    All {
        rules: &'a [RuleIr],
        next: usize,
    },
    Affected {
        words: &'a [u64],
        next: usize,
        remaining: u64,
    },
}

impl<'a> Schedule<'a> {
    pub(in crate::formula_support) fn all(rules: &'a [RuleIr]) -> Self {
        Self::All { rules, next: 0 }
    }

    pub(in crate::formula_support) fn next(
        &mut self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<usize>, FormulaFailure> {
        match self {
            Self::All { rules, next } => {
                while let Some(rule) = rules.get(*next) {
                    counters.work(limits, rule.location)?;
                    let index = *next;
                    *next += 1;
                    if !matches!(rule.head, HeadIr::Normal(None)) {
                        return Ok(Some(index));
                    }
                }
                Ok(None)
            }
            Self::Affected {
                words,
                next,
                remaining,
            } => loop {
                if *remaining != 0 {
                    counters.work(limits, location)?;
                    let bit = remaining.trailing_zeros() as usize;
                    *remaining &= *remaining - 1;
                    return Ok(Some((*next - 1) * u64::BITS as usize + bit));
                }
                let Some(&word) = words.get(*next) else {
                    return Ok(None);
                };
                counters.work(limits, location)?;
                *remaining = word;
                *next += 1;
            },
        }
    }
}
