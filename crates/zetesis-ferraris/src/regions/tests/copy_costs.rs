//! Payload receipts for the Knowledge clones an actual Traversal requests.
//!
//! A receipt counts initialized array representation bytes and the returned
//! clone's nonempty backing allocations. It does not intercept an allocator or
//! measure memory-bus traffic. Inline headers, Region masks, shared indexes,
//! measurement storage and allocator metadata are outside this receipt.

use std::{cell::RefCell, collections::BTreeSet, mem::size_of, rc::Rc};

use zetesis_cpu::{
    Cancellation,
    regions::{Counting, RegionStatistics, Traversal, Visit},
};

use crate::{AdmissionLimits, Narrower, Node, Region, RegionLimits, Theory};

use super::super::{Width, counters::Count};
use super::{Knowledge, Known, counters::native};

const FREE_ATOMS: usize = 8;

#[derive(Clone, Copy, Debug, Default)]
struct Arrays {
    initialized_bytes: u128,
    retained_bytes: u128,
    nonempty_allocations: usize,
}

impl Arrays {
    fn add<T>(&mut self, length: usize, capacity: usize) {
        self.initialized_bytes += length as u128 * size_of::<T>() as u128;
        self.retained_bytes += capacity as u128 * size_of::<T>() as u128;
        self.nonempty_allocations += usize::from(capacity != 0);
    }

    fn knowledge(knowledge: &Knowledge) -> Self {
        match &knowledge.width {
            Width::Compact16(known) => Self::known(knowledge, known),
            Width::Compact32(known) => Self::known(knowledge, known),
            Width::Native(known) => Self::known(knowledge, known),
        }
    }

    fn known<C: Count>(knowledge: &Knowledge, known: &Known<C>) -> Self {
        let mut arrays = Self::default();
        arrays.add::<u64>(known.masks.words.len(), known.masks.words.len());
        for counters in [&known.sure_operands, &known.never_operands, &known.unknown] {
            arrays.add::<C>(counters.len(), counters.len());
        }
        assert_eq!(
            knowledge.retained_bytes(),
            size_of::<Knowledge>() as u128 + arrays.retained_bytes
        );
        arrays
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct CopyReceipt {
    calls: usize,
    initialized_bytes: u128,
    retained_bytes: u128,
    nonempty_allocations: usize,
    max_source_spare_bytes: u128,
}

struct MeasuredKnowledge {
    knowledge: Knowledge,
    receipt: Rc<RefCell<CopyReceipt>>,
}

impl Clone for MeasuredKnowledge {
    fn clone(&self) -> Self {
        let knowledge = self.knowledge.clone();
        let source = Arrays::knowledge(&self.knowledge);
        let copied = Arrays::knowledge(&knowledge);
        assert_eq!(source.initialized_bytes, copied.initialized_bytes);
        let mut receipt = self.receipt.borrow_mut();
        receipt.calls += 1;
        receipt.initialized_bytes += source.initialized_bytes;
        receipt.retained_bytes += copied.retained_bytes;
        receipt.nonempty_allocations += copied.nonempty_allocations;
        receipt.max_source_spare_bytes = receipt
            .max_source_spare_bytes
            .max(source.retained_bytes - source.initialized_bytes);
        Self {
            knowledge,
            receipt: Rc::clone(&self.receipt),
        }
    }
}

fn theory(atoms: usize) -> Theory {
    let mut nodes: Vec<_> = (0..atoms).map(Node::atom).collect();
    // Four-operand chains populate both chain-counter arrays. All but eight
    // atoms are facts, so each size has the same bounded, complete traversal.
    // The unasserted chains affect propagation and ranking, not satisfaction.
    for first in (0..atoms).step_by(4) {
        nodes.push(Node::or_pair([first, first + 1]));
        nodes.push(Node::or_pair([nodes.len() - 1, first + 2]));
        nodes.push(Node::or_pair([nodes.len() - 1, first + 3]));
    }
    Theory::new(
        atoms,
        crate::FormulaParts::new(nodes, vec![]).unwrap(),
        (0..atoms - FREE_ATOMS).collect(),
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn traverse(
    theory: &Theory,
    narrower: &Narrower,
    knowledge: Knowledge,
) -> (RegionStatistics, CopyReceipt, BTreeSet<usize>) {
    let receipt = Rc::new(RefCell::new(CopyReceipt::default()));
    let measured = MeasuredKnowledge {
        knowledge,
        receipt: Rc::clone(&receipt),
    };
    let mut traversal = Traversal::with_state(
        Region::all_open(theory.atom_count()),
        Counting::Never,
        measured,
    );
    let cancellation = Cancellation::default();
    let mut scratch = crate::NarrowingScratch::default();
    let mut leaves = BTreeSet::new();
    while let Some(visit) = traversal
        .next(|region, measured| {
            narrower
                .narrow_known(
                    crate::OriginalSubject::new(theory, None),
                    region,
                    &mut measured.knowledge,
                    &mut scratch,
                    RegionLimits::default(),
                    &cancellation,
                )
                .map(|(narrowing, _)| narrowing)
        })
        .unwrap()
    {
        let Visit::Leaf(region, _) = visit else {
            panic!("Counting::Never returns only complete leaves");
        };
        let first_free = theory.atom_count() - FREE_ATOMS;
        assert!((0..first_free).all(|atom| region.is_held(atom)));
        let code = (0..FREE_ATOMS).fold(0, |code, bit| {
            code | (usize::from(region.decision(first_free + bit).unwrap()) << bit)
        });
        assert!(leaves.insert(code), "every complete leaf occurs once");
    }
    let statistics = traversal.statistics();
    let receipt = *receipt.borrow();
    assert_eq!(
        receipt.calls,
        statistics.splits_since(RegionStatistics::default())
    );
    assert_eq!(leaves, (0..1 << FREE_ATOMS).collect());
    assert_eq!(statistics.regions, (1 << (FREE_ATOMS + 1)) - 1);
    assert_eq!(statistics.leaves, 1 << FREE_ATOMS);
    assert_eq!((statistics.refuted, statistics.counted), (0, 0));
    // A knowledge holds no worklists, so no copy's source retains capacity
    // beyond its live arrays; the walker's scratch keeps the worklists.
    assert_eq!(receipt.max_source_spare_bytes, 0);
    assert_eq!(receipt.retained_bytes, receipt.initialized_bytes);
    (statistics, receipt, leaves)
}

#[test]
fn traversal_copies_only_live_knowledge_arrays() {
    for atoms in [64, 512, 4096] {
        let theory = theory(atoms);
        let narrower = Narrower::new(&theory);
        let selected = narrower.knowledge();
        // Fixture setup is outside the measured Traversal clone calls.
        let native = native(&selected);
        let (selected_stats, selected, selected_leaves) = traverse(&theory, &narrower, selected);
        let (native_stats, native, native_leaves) = traverse(&theory, &narrower, native);
        assert_eq!(selected_stats, native_stats);
        assert_eq!(selected_leaves, native_leaves);
        assert_eq!(selected.calls, (1 << FREE_ATOMS) - 1);
        assert_eq!(selected.nonempty_allocations, 4 * selected.calls);
        assert_eq!(selected.nonempty_allocations, native.nonempty_allocations);
        let counter_values = atoms + 2 * narrower.chains.len();
        let saved_width = size_of::<usize>() - size_of::<u16>().min(size_of::<usize>());
        assert_eq!(
            native.initialized_bytes - selected.initialized_bytes,
            selected.calls as u128 * counter_values as u128 * saved_width as u128
        );
        for (width, receipt) in [("selected", selected), ("native", native)] {
            println!(
                "traversal atoms={atoms} chains={} free_atoms={FREE_ATOMS} counters={width} \
                 clone_calls={} initialized_array_bytes={} initialized_bytes_per_clone={} \
                 cloned_array_capacity_bytes={} \
                 nonempty_backing_allocations={} max_source_spare_bytes={}",
                narrower.chains.len(),
                receipt.calls,
                receipt.initialized_bytes,
                receipt.initialized_bytes / receipt.calls as u128,
                receipt.retained_bytes,
                receipt.nonempty_allocations,
                receipt.max_source_spare_bytes
            );
        }
    }
}
