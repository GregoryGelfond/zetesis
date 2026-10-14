//! Numeric analysis reads checked canonical identities; it creates no values.

use std::cmp::Ordering;

use zetesis_core::ValueNodeRef;
use zetesis_core::catalog::{CatalogRead, ReadError, TermKey, TermRef};
use zetesis_domain::Limits;
use zetesis_domain::numeric::{Budget, Classification, Error, Values};

pub(super) struct Reader<'a>(pub(super) CatalogRead<'a>);

impl Values for Reader<'_> {
    type Key = TermKey;
    type Error = ReadError;

    fn classify(
        &self,
        key: &TermKey,
        limits: &Limits,
        budget: &Budget,
    ) -> Result<Classification, Error<ReadError>> {
        budget.work(1)?;
        let term = self.0.term(key).map_err(Error::Read)?;
        let number = match term.descriptor() {
            ValueNodeRef::Number(number) => Some(number),
            _ => None,
        };
        if term.expanded_nodes() > limits.max_symbol_nodes
            || term.depth_with(|| budget.work(1))? > limits.max_symbol_depth
        {
            return Ok(Classification {
                number,
                retain: false,
            });
        }
        let bytes = inspect_text(term, budget)?;
        Ok(Classification {
            number,
            retain: bytes <= limits.max_symbol_bytes,
        })
    }

    fn compare(
        &self,
        left: &TermKey,
        right: &TermKey,
        budget: &Budget,
    ) -> Result<Ordering, Error<ReadError>> {
        budget.work(2)?;
        let left = self.0.term(left).map_err(Error::Read)?;
        let right = self.0.term(right).map_err(Error::Read)?;
        if left.equals_ref_with(right, || budget.work(1))? {
            return Ok(Ordering::Equal);
        }
        // Admit both bounded payloads before an ordering may inspect text.
        // This is a conservative byte charge, not a count of compared bytes.
        inspect_text(left, budget)?;
        inspect_text(right, budget)?;
        left.compare_ref_with(right, || budget.work(1))
            .map_err(Error::Limit)
    }
}

fn inspect_text(term: TermRef<'_>, budget: &Budget) -> Result<u64, Error<ReadError>> {
    let mut bytes = 0_u64;
    let mut nodes = term.nodes();
    while let Some(node) = nodes.next_with(|| budget.work(1))? {
        let text = match node {
            ValueNodeRef::String(text)
            | ValueNodeRef::Symbol(text)
            | ValueNodeRef::Function { name: text, .. } => text.len(),
            ValueNodeRef::Number(_)
            | ValueNodeRef::Tuple { .. }
            | ValueNodeRef::Infimum
            | ValueNodeRef::Supremum => 0,
        };
        let text = u64::try_from(text).map_err(|_| Error::InvalidInput)?;
        budget.bytes(text)?;
        bytes = bytes.checked_add(text).ok_or(Error::InvalidInput)?;
    }
    Ok(bytes)
}
