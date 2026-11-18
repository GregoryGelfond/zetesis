//! Source constants already imported by this compilation, found by content.
//!
//! A program repeats its constants: a fact block names the same symbols and
//! numbers row after row. Each distinct constant is imported into the support
//! vocabulary once; a later occurrence is found here by hashing and comparing
//! its source value (one unit per node and payload word each), instead of
//! hashing and probing its text and node in the vocabulary again. Every
//! occurrence of a root constant names the scalar component its first
//! occurrence admitted, so a repeated constant appends no component.
//!
//! The memo is optional storage under the support workspace. It grows only
//! when its whole replacement fits the remaining allowance; otherwise later
//! constants are simply imported as before, so it never causes a refusal.

use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::hash::DefaultHasher;

use zetesis_core::Value;
use zetesis_core::catalog::{TermAssignment, TermKey};

use crate::ProgramSite;
use crate::formula_support::{
    Counters, StorageLease,
    components::{Admission, Scalar},
};
use crate::{FormulaFailure, FormulaLimits};

pub(crate) struct Constants {
    /// Each remembered source value and the frame slot holding its term.
    slots: HashMap<Value, usize, BuildHasherDefault<DefaultHasher>>,
    /// Owned payload bytes of the remembered values.
    payload: usize,
    /// Remembered terms under the vocabulary's one witness.
    terms: TermAssignment,
    /// The root scalar component admitted for each remembered slot, if any.
    scalars: Vec<Option<Scalar>>,
    lease: StorageLease,
}

impl Constants {
    pub(super) fn new(
        source: &Admission<'_>,
        limits: &FormulaLimits,
        counters: &Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let mut lease = source.lease();
        lease.observe(size_of::<Self>(), location)?;
        source.storage_observed(&lease, 0, size_of::<Self>(), limits, counters, location)?;
        Ok(Self {
            slots: HashMap::default(),
            payload: 0,
            terms: source.read().assignment(),
            scalars: Vec::new(),
            lease,
        })
    }

    /// One unit per node and payload word: what hashing or comparing reads.
    fn read_value(
        value: &Value,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let words = super::value_bytes(value).div_ceil(8);
        let nodes = match value {
            Value::Structured(value) => value.nodes().len() as u128,
            _ => 1,
        };
        for _ in 0..nodes + words {
            counters.work(limits, location)?;
        }
        Ok(())
    }

    /// The term this compilation imported for `value`, if it is remembered.
    pub(super) fn find(
        &self,
        value: &Value,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<(TermKey, usize)>, FormulaFailure> {
        Self::read_value(value, limits, counters, location)?;
        counters.work(limits, location)?;
        let Some(&slot) = self.slots.get(value) else {
            return Ok(None);
        };
        // The equal entry the probe found was compared in full.
        Self::read_value(value, limits, counters, location)?;
        Ok(self.terms.key(slot).ok().flatten().map(|key| (key, slot)))
    }

    /// The root scalar component remembered for `slot`, one unit to read.
    pub(super) fn scalar(
        &self,
        slot: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<Scalar>, FormulaFailure> {
        counters.work(limits, location)?;
        Ok(self.scalars.get(slot).copied().flatten())
    }

    /// Remember `slot`'s root scalar component when the growth fits the
    /// allowance; otherwise leave it unremembered. Growth moves every cell
    /// once and the write is one unit.
    pub(super) fn remember_scalar(
        &mut self,
        slot: usize,
        scalar: Scalar,
        source: &Admission<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let previous = self.lease.bytes();
        if slot >= self.scalars.len() {
            let needed = slot + 1;
            if needed > self.scalars.capacity() {
                let allowance = source.allowance(&self.lease, limits, location)?;
                let target = needed.max(self.scalars.capacity() * 2);
                // Old and new buffers coexist while the cells move.
                let replacement = target * size_of::<Option<Scalar>>();
                if previous.saturating_add(replacement) > allowance
                    || self
                        .scalars
                        .try_reserve_exact(target - self.scalars.len())
                        .is_err()
                {
                    return Ok(());
                }
                for _ in 0..self.scalars.len() {
                    counters.work(limits, location)?;
                }
            }
            self.scalars.resize(needed, None);
        }
        counters.work(limits, location)?;
        self.scalars[slot] = Some(scalar);
        self.observe(previous, source, limits, counters, location)
    }

    /// Remember an imported constant when the memo's growth fits the
    /// allowance; otherwise leave it unremembered.
    pub(super) fn remember(
        &mut self,
        value: &Value,
        key: &TermKey,
        source: &Admission<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<usize>, FormulaFailure> {
        let slot = self.terms.len();
        let allowance = source.allowance(&self.lease, limits, location)?;
        let previous = self.lease.bytes();
        let payload = owned_bytes(value);
        let index = if self.slots.len() == self.slots.capacity() {
            self.slots.capacity().max(2) * 2
        } else {
            self.slots.capacity()
        };
        let others = size_of::<Self>() - size_of::<TermAssignment>()
            + index * size_of::<(Value, usize)>()
            + self.payload.saturating_add(payload);
        if others > allowance
            || self
                .terms
                .resize_with(slot + 1, allowance - others, || {
                    counters.work(limits, location)
                })
                .is_err()
        {
            return self
                .observe(previous, source, limits, counters, location)
                .map(|()| None);
        }
        if index > self.slots.capacity() {
            if self.slots.try_reserve(index - self.slots.len()).is_err() {
                self.terms
                    .truncate_with(slot, || counters.work(limits, location))
                    .map_err(|error| crate::formula_binding::failure(error, location))?;
                return self
                    .observe(previous, source, limits, counters, location)
                    .map(|()| None);
            }
            // Rehashing moves every remembered entry once.
            for _ in 0..self.slots.len() {
                counters.work(limits, location)?;
            }
        }
        self.terms
            .set_with(slot, key, || counters.work(limits, location))
            .map_err(|error| crate::formula_binding::failure(error, location))?;
        // Copying the value reads it once.
        Self::read_value(value, limits, counters, location)?;
        self.slots.insert(value.clone(), slot);
        self.payload += payload;
        self.observe(previous, source, limits, counters, location)
            .map(|()| Some(slot))
    }

    fn observe(
        &mut self,
        previous: usize,
        source: &Admission<'_>,
        limits: &FormulaLimits,
        counters: &Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let bytes = size_of::<Self>() - size_of::<TermAssignment>()
            + self.slots.capacity() * size_of::<(Value, usize)>()
            + self.payload
            + self.scalars.capacity() * size_of::<Option<Scalar>>()
            + self.terms.retained_bytes();
        self.lease.observe(bytes, location)?;
        source.storage_observed(
            &self.lease,
            previous,
            size_of::<Self>(),
            limits,
            counters,
            location,
        )
    }
}

/// Heap bytes a remembered copy of `value` owns beside its inline header.
fn owned_bytes(value: &Value) -> usize {
    let payload = usize::try_from(super::value_bytes(value)).unwrap_or(usize::MAX);
    match value {
        Value::Structured(value) => payload.saturating_add(
            value
                .nodes()
                .len()
                .saturating_mul(size_of::<zetesis_core::ValueNode>()),
        ),
        _ => payload,
    }
}

#[cfg(test)]
mod tests {
    use zetesis_core::{Value, ValueLimits, ValueNode};

    use crate::ExpansionLimits;
    use crate::formula_ir::testing::with_compiler;

    #[test]
    fn a_repeated_constant_names_its_first_term_for_less_work() {
        let compound = Value::from_nodes(
            vec![
                ValueNode::Tuple { arity: 2 },
                ValueNode::Number(1),
                ValueNode::Symbol("a".into()),
            ],
            ValueLimits::default(),
        )
        .unwrap();
        let values = [
            Value::Symbol("n12345".into()),
            Value::Number(7),
            Value::String("n12345".into()),
            compound,
        ];
        with_compiler(ExpansionLimits::default(), |compiler| {
            for value in &values {
                let before = compiler.counters.accounting.work;
                let first = compiler.value(value).unwrap();
                let imported = compiler.counters.accounting.work - before;
                let before = compiler.counters.accounting.work;
                let again = compiler.value(value).unwrap();
                let remembered = compiler.counters.accounting.work - before;
                let read = compiler.source.read();
                assert_eq!(
                    read.term(&first).unwrap(),
                    read.term(&again).unwrap(),
                    "{value:?}"
                );
                assert_eq!(
                    read.term(&again).unwrap(),
                    zetesis_core::catalog::TermRef::from(value)
                );
                assert!(
                    remembered < imported,
                    "{value:?}: {remembered} < {imported}"
                );
            }
            // Equal spellings of different kinds stay different terms.
            let read = compiler.source.read();
            let symbol = compiler
                .imported
                .find(
                    &values[0],
                    compiler.limits,
                    compiler.counters,
                    compiler.location,
                )
                .unwrap()
                .unwrap();
            let string = compiler
                .imported
                .find(
                    &values[2],
                    compiler.limits,
                    compiler.counters,
                    compiler.location,
                )
                .unwrap()
                .unwrap();
            assert_ne!(read.term(&symbol.0).unwrap(), read.term(&string.0).unwrap());
        });
    }
}
