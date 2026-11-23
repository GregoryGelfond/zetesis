//! Content-order term identities at each semantic commit.
//!
//! A vocabulary tail's terms form one generation of a
//! [`crate::intern::ContentInterner`] whose content is the term's canonical
//! storage identity and whose one comparison is typed term order. Freezing the
//! generation before the tail is sealed gives every term of the sealed segment
//! a dense identity in content order (`ContentOrderInterning.IdOrderEqCompare`),
//! so two terms of one segment compare by those identities alone. A term still
//! in an unsealed tail has only a pending handle, which never leaves this
//! module: readers see no order for it and compare its contents.

use std::cmp::Ordering;

use crate::intern::{ContentInterner, ContentOrder, Frozen, Handle, Storage, Work as InternWork};

use super::control::Work;
use super::vocabulary::Vocabulary;
use super::{Failure, Fault, Read, Store, TermId, budget};
use crate::catalog::view::TermRef;

/// Typed term order over canonical terms of one writer. Content is a term's
/// order key ([`KEY`] words) and its identity word. The key is a prefix of the
/// term's preorder storage comparison: words that differ decide the order, and
/// equal keys fall back to the full typed comparison.
struct Typed<'a>(Read<'a>);

impl ContentOrder for Typed<'_> {
    fn compare(&self, _: Frozen<'_>, left: &[u32], right: &[u32]) -> Ordering {
        let order = left[..KEY].cmp(&right[..KEY]);
        if order.is_ne() || left[0] == NUMBER {
            return order;
        }
        let term = |word: u32| {
            TermRef::new(self.0, TermId::from_word(word)).expect("frozen tail term resolves")
        };
        super::super::compare::term(term(left[KEY]), term(right[KEY]))
    }
}

/// Terms of more expanded nodes than this get no identity order: a fallback
/// comparison between two keyed terms then reads at most this many nodes of
/// each, so ordering a tail stays `O(n log n)` comparisons of bounded cost even
/// for deep terms (a chain of nested tuples shares every key word). Such terms
/// keep comparing by content, as they did before identities existed.
const ORDERED_NODES: usize = 64;

use super::segments::UNORDERED;

/// Storage rank of a number; a number's key decides it completely.
const NUMBER: u32 = 1;
/// Order-key words per term.
const KEY: usize = 7;
/// A text length word that stops the key: the text continues past its prefix.
const LONG: u32 = 9;

/// A tail term's interner content: its order key, then its identity.
///
/// The key walks the term's nodes in preorder, as storage comparison does.
/// Each node gives its rank, then a number's order-preserving value, a tuple's
/// arity, or a text's first eight bytes (zero-padded, big-endian) and its
/// length capped at [`LONG`]; a function whose name fits gives its sign and
/// arity too. A capped length ends the key. Two terms whose keys agree up to a
/// word agree in every node before it, so they emit, stop or end at the same
/// words: the first differing word orders them as storage comparison does.
fn content(read: Read<'_>, identity: u32, pending: &mut Vec<TermId>) -> [u32; KEY + 1] {
    use crate::ValueNodeRef;
    let mut out = [0_u32; KEY + 1];
    out[KEY] = identity;
    let mut len = 0;
    let mut push = |word: u32, len: &mut usize| {
        if *len < KEY {
            out[*len] = word;
            *len += 1;
        }
    };
    pending.clear();
    pending.push(TermId::from_word(identity));
    'walk: while let Some(id) = pending.pop() {
        if len >= KEY {
            break;
        }
        let term = read.term(id).expect("tail term resolves");
        let descriptor = term.descriptor();
        push(
            u32::from(crate::term_order::node_storage_rank(descriptor)),
            &mut len,
        );
        let text = |text: &str, len: &mut usize, push: &mut dyn FnMut(u32, &mut usize)| {
            let mut bytes = [0_u8; 8];
            let prefix = &text.as_bytes()[..text.len().min(8)];
            bytes[..prefix.len()].copy_from_slice(prefix);
            push(
                u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
                len,
            );
            push(
                u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
                len,
            );
            let capped = u32::try_from(text.len()).map_or(LONG, |length| length.min(LONG));
            push(capped, len);
            capped < LONG
        };
        let arity = match descriptor {
            ValueNodeRef::Number(value) => {
                push(value.cast_unsigned() ^ (1 << 31), &mut len);
                0
            }
            ValueNodeRef::Symbol(name) | ValueNodeRef::String(name) => {
                if !text(name, &mut len, &mut push) {
                    break 'walk;
                }
                0
            }
            ValueNodeRef::Function { name, sign, arity } => {
                if !text(name, &mut len, &mut push) {
                    break 'walk;
                }
                push(u32::from(sign == crate::Sign::Negative), &mut len);
                push(u32::try_from(arity).unwrap_or(u32::MAX), &mut len);
                arity
            }
            ValueNodeRef::Tuple { arity } => {
                push(u32::try_from(arity).unwrap_or(u32::MAX), &mut len);
                arity
            }
            ValueNodeRef::Infimum | ValueNodeRef::Supremum => 0,
        };
        for index in (0..arity.min(KEY)).rev() {
            pending.push(term.child(index).expect("compound child resolves"));
        }
    }
    out
}

impl Store {
    /// [`Self::order_terms_with`] under a caller's work boundary.
    pub(crate) fn order_terms<E>(
        &mut self,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        self.order_terms_with(&mut Work::new(&mut before))
    }

    /// Give the unsealed vocabulary tail's terms their content-order
    /// identities. A no-op for a frozen vocabulary, an empty tail or a tail
    /// already ordered. Source buffers and the native interner receive disjoint
    /// portions of the existing byte allowance. Native work is tallied and
    /// charged through the caller's work boundary after construction; there is
    /// no cancellation poll before every native insertion or freeze step. If
    /// that settlement refuses, its charged prefix need not include every
    /// already-performed native operation.
    pub(super) fn order_terms_with<E>(&mut self, work: &mut Work<'_, E>) -> Result<(), Failure<E>> {
        let Vocabulary::Growing(growing) = &self.vocabulary else {
            return Ok(());
        };
        let tail = &growing.tail;
        let count = tail.terms.kinds.len();
        if count == 0 || tail.order.len() == count {
            return Ok(());
        }
        let start = tail.start.terms;
        let mut operation = OrderingPeak {
            budget: self.budget.clone(),
            store: self,
        };
        let order = build_order(
            Read::Writer(&*operation.store),
            start,
            count,
            &mut operation.budget,
            work,
        )?;
        // The old order, if any, stayed live throughout the operation.
        let before = budget::capacity(&operation.store.vocabulary.admitted_growing().tail.order);
        operation
            .store
            .budget
            .observed(before, budget::capacity(&order))?;
        operation.store.vocabulary.admitted_growing().tail.order = order;
        Ok(())
    }
}

/// Preserve every completed reservation's peak, including on callback unwind.
struct OrderingPeak<'a> {
    store: &'a mut Store,
    budget: budget::Budget,
}

impl Drop for OrderingPeak<'_> {
    fn drop(&mut self) {
        self.store.budget.peak = self.store.budget.peak.max(self.budget.peak);
    }
}

/// The interner has one disjoint remainder of the operation's allowance.
/// The account survives every constructor/freeze refusal and settles on unwind.
struct InternPeak<'a> {
    budget: &'a mut budget::Budget,
    storage: Storage,
    outside: u128,
}

impl Drop for InternPeak<'_> {
    fn drop(&mut self) {
        self.budget.peak = self
            .budget
            .peak
            .max(self.outside + self.storage.peak_bytes());
    }
}

fn build_order<E>(
    read: Read<'_>,
    start: usize,
    count: usize,
    budget: &mut budget::Budget,
    work: &mut Work<'_, E>,
) -> Result<Vec<u32>, Failure<E>> {
    work.step()?;
    let mut handles = Vec::new();
    let mut pending = Vec::new();
    let mut order = Vec::new();
    // At most KEY descriptors are read, each adding at most KEY children.
    // This capacity bound prevents hidden growth while constructing a key.
    let prefix_stack = 1 + KEY * KEY;
    budget::reserve(&mut handles, count, budget)?;
    budget::reserve(&mut pending, prefix_stack, budget)?;
    budget::reserve(&mut order, count, budget)?;
    let outside = budget.used;
    let remaining = budget.limit as u128 - outside;
    let remaining = usize::try_from(remaining).map_err(|_| Fault::Overflow)?;
    let storage = Storage::new(remaining).map_err(|error| refusal(error, outside, budget.limit))?;
    let receipt = InternPeak {
        budget,
        storage,
        outside,
    };
    let mut tally = InternWork::default();
    let result = (|| {
        let mut interner = ContentInterner::in_storage(Typed(read), receipt.storage.clone())
            .map_err(|error| refusal(error, outside, receipt.budget.limit))?;
        for local in 0..count {
            let identity = u32::try_from(start + local).map_err(|_| Fault::IdExhausted)?;
            let term = read
                .term(TermId::from_word(identity))
                .expect("tail term resolves");
            handles.push(if term.expanded_nodes() > ORDERED_NODES {
                None
            } else {
                Some(
                    interner
                        .intern(&content(read, identity, &mut pending), &mut tally)
                        .map_err(|error| refusal(error, outside, receipt.budget.limit))?,
                )
            });
        }
        let generation = interner
            .freeze(None, &mut tally)
            .map_err(|error| refusal(error, outside, receipt.budget.limit))?;
        let base = generation.base();
        for handle in handles {
            order.push(match handle {
                Some(handle) => {
                    debug_assert!(matches!(handle, Handle::Pending(_)));
                    let id = generation.finalize(handle).index() - base;
                    u32::try_from(id)
                        .ok()
                        .filter(|&id| id != UNORDERED)
                        .ok_or(Fault::IdExhausted)?
                }
                None => UNORDERED,
            });
        }
        Ok::<_, Fault>(order)
    })();
    // A failed reservation does not discard the work preceding it.
    work.steps(usize::try_from(tally.total()).map_err(|_| Fault::Overflow)?)?;
    result.map_err(Failure::Storage)
}

fn refusal(refusal: crate::intern::Refusal, outside: u128, limit: usize) -> Fault {
    match refusal {
        crate::intern::Refusal::Exhausted => Fault::IdExhausted,
        crate::intern::Refusal::Allocation => Fault::Allocation,
        crate::intern::Refusal::Storage { required, .. } => Fault::Storage {
            required: outside + required,
            limit,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use super::*;
    use crate::catalog::Limits;
    use crate::{Sign, Value, ValueLimits, ValueNode};

    fn values() -> Vec<Value> {
        let mut values = vec![
            Value::Symbol("b".into()),
            Value::Number(3),
            Value::String("a".into()),
            Value::Symbol("a".into()),
            Value::Number(-7),
            Value::Infimum,
            Value::Supremum,
            Value::String(String::new()),
            Value::Symbol("abcdefghx".into()),
            Value::Symbol("abcdefgha".into()),
            Value::Symbol("abcdefgh".into()),
            Value::String("a\0".into()),
            Value::String("a".into()),
            Value::Number(i32::MIN),
            Value::Number(i32::MAX),
        ];
        let nodes = |nodes: Vec<ValueNode>| {
            Value::from_nodes(
                nodes,
                ValueLimits {
                    max_nodes: 64,
                    max_depth: 64,
                    max_bytes: 1 << 16,
                },
            )
            .unwrap()
        };
        let f = |name: &str, arity| ValueNode::Function {
            name: name.into(),
            sign: Sign::Positive,
            arity,
        };
        values.push(nodes(vec![
            f("f", 2),
            ValueNode::Number(2),
            ValueNode::Symbol("a".into()),
        ]));
        values.push(nodes(vec![f("f", 1), ValueNode::Number(1)]));
        values.push(nodes(vec![f("g", 1), f("f", 1), ValueNode::Number(1)]));
        values.push(nodes(vec![
            ValueNode::Tuple { arity: 2 },
            ValueNode::Number(1),
            ValueNode::Number(2),
        ]));
        let t = |arity| ValueNode::Tuple { arity };
        let n = ValueNode::Number;
        let y = |text: &str| ValueNode::Symbol(text.into());
        values.push(nodes(vec![f("abcdefgh", 1), n(1)]));
        values.push(nodes(vec![f("abcdefghi", 1), n(1)]));
        values.push(nodes(vec![f("abcdefghi", 1), n(0)]));
        values.push(nodes(vec![f("ab\0", 1), n(5)]));
        values.push(nodes(vec![f("ab", 1), n(9)]));
        values.push(nodes(vec![
            ValueNode::Function {
                name: "f".into(),
                sign: Sign::Negative,
                arity: 1,
            },
            n(1),
        ]));
        values.push(nodes(vec![t(3), n(1), y("abcdefghijk"), n(3)]));
        values.push(nodes(vec![t(3), n(1), y("abcdefghijk"), n(2)]));
        values.push(nodes(vec![t(3), n(1), y("abcdefghijj"), n(9)]));
        values.push(nodes(vec![t(1), t(1), t(1), t(1), t(1), t(1), t(1), n(2)]));
        values.push(nodes(vec![t(1), t(1), t(1), t(1), t(1), t(1), t(1), n(1)]));
        values.push(nodes(vec![t(2), t(2), n(1), n(2), n(3)]));
        values.push(nodes(vec![t(2), t(2), n(1), n(2), n(-3)]));
        values.push(nodes(vec![t(2), n(1), t(0)]));
        values.push(nodes(vec![t(2), n(1), ValueNode::Supremum]));
        values
    }

    fn ordered(values: &[Value]) -> (Store, Vec<TermId>) {
        ordered_within(values, Limits::default())
    }

    fn ordered_within(values: &[Value], limits: Limits) -> (Store, Vec<TermId>) {
        let mut store = Store::new(16_000_000);
        let ids = values
            .iter()
            .map(|value| store.import_value(value, limits).unwrap())
            .collect();
        store.order_terms(|| Ok::<_, Infallible>(())).unwrap();
        (store, ids)
    }

    #[test]
    fn identities_follow_typed_content_order() {
        let (mut store, ids) = ordered(&values());
        let snapshot = store.snapshot(0).unwrap();
        let count = snapshot.data.vocabulary.counts.terms;
        assert!(count >= ids.len());
        for left in 0..count {
            for right in 0..count {
                let term =
                    |position| TermRef::new(&snapshot, TermId::from_position(position)).unwrap();
                let (left, right) = (term(left), term(right));
                let by_identity = left.content_order(right).expect("one frozen generation");
                let by_content =
                    super::super::super::compare::term_contents_with(left, right, &mut || {
                        Ok::<_, Infallible>(())
                    })
                    .unwrap();
                assert_eq!(by_identity, by_content);
            }
        }
    }

    #[test]
    fn identities_do_not_depend_on_arrival_order() {
        // Pairwise identity order of the same values, read through each run's
        // own term identities in the forward value order.
        let pairwise = |values: &[Value], reversed: bool| {
            let (mut store, ids) = ordered(values);
            let snapshot = store.snapshot(0).unwrap();
            let n = ids.len();
            let id = |index: usize| {
                if reversed {
                    ids[n - 1 - index]
                } else {
                    ids[index]
                }
            };
            let mut orders = Vec::new();
            for left in 0..n {
                for right in 0..n {
                    let term = |index| TermRef::new(&snapshot, id(index)).unwrap();
                    orders.push(term(left).content_order(term(right)).unwrap());
                }
            }
            orders
        };
        let forward = values();
        let mut backward = forward.clone();
        backward.reverse();
        assert_eq!(pairwise(&forward, false), pairwise(&backward, true));
    }

    #[test]
    fn an_unfrozen_tail_has_no_identity_order() {
        let mut store = Store::new(16_000_000);
        let a = store
            .import_value(&Value::Number(1), Limits::default())
            .unwrap();
        let b = store
            .import_value(&Value::Number(2), Limits::default())
            .unwrap();
        let snapshot = store.snapshot(0).unwrap();
        let (a, b) = (
            TermRef::new(&snapshot, a).unwrap(),
            TermRef::new(&snapshot, b).unwrap(),
        );
        assert_eq!(a.content_order(b), None);
        assert_eq!(a.cmp(&b), std::cmp::Ordering::Less);
    }

    #[test]
    fn deep_terms_compare_by_content_without_identities() {
        let chain = |depth: usize, leaf: i32| {
            let mut nodes = vec![ValueNode::Tuple { arity: 1 }; depth];
            nodes.push(ValueNode::Number(leaf));
            Value::from_nodes(
                nodes,
                ValueLimits {
                    max_nodes: usize::MAX,
                    max_depth: usize::MAX,
                    max_bytes: usize::MAX,
                },
            )
            .unwrap()
        };
        let unbounded = Limits {
            max_nodes: usize::MAX,
            max_depth: usize::MAX,
            max_bytes: usize::MAX,
        };
        let (mut store, ids) = ordered_within(
            &[chain(10_000, 2), chain(10_000, 1), chain(3, 1)],
            unbounded,
        );
        let snapshot = store.snapshot(0).unwrap();
        let term = |id| TermRef::new(&snapshot, id).unwrap();
        let (deep_two, deep_one, shallow) = (term(ids[0]), term(ids[1]), term(ids[2]));
        assert_eq!(deep_two.content_order(deep_one), None);
        assert_eq!(deep_one.content_order(shallow), None);
        assert_eq!(deep_one.cmp(&deep_two), Ordering::Less);
        assert_eq!(shallow.cmp(&deep_one), Ordering::Less);
        let inner = shallow.child(0).unwrap();
        assert!(shallow.content_order(inner).is_some());
    }
    fn scalar_tail_for_capacity_review() -> Store {
        let mut store = Store::new(usize::MAX);
        for value in 0..4096 {
            store
                .import_value(&Value::Number(value), Limits::default())
                .unwrap();
        }
        store.restart_peak();
        store
    }

    #[test]
    fn freeze_refuses_an_allowance_below_its_live_named_capacity() {
        let mut store = scalar_tail_for_capacity_review();
        let n = 4096_u128;
        let admitted = store.current_bytes() + 53 * n;
        store.ceiling(usize::try_from(admitted).unwrap()).unwrap();
        // Current code checks exactly 53N, then adds only its retained 4N order.
        // But freeze must keep >=84N named capacities simultaneously:
        // open payload/starts/hashes 44N + frozen payload/starts 36N + remap 4N.
        // Handles, entries, hash tables, headers, and sort scratch are extra.
        assert!(matches!(
            store.order_terms(|| Ok::<_, Infallible>(())),
            Err(Failure::Storage(Fault::Storage { .. }))
        ));
    }

    #[test]
    fn freeze_peak_includes_simultaneous_open_and_frozen_buffers() {
        let mut store = scalar_tail_for_capacity_review();
        let initial = store.current_bytes();
        store.order_terms(|| Ok::<_, Infallible>(())).unwrap();
        assert!(store.peak_bytes() >= initial + 84 * 4096);
    }
}
